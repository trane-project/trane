//! Defines how the rewards for lessons and courses are stored in the database.
//!
//! A reward is a positive or negative number that is used to adjust the score of a unit. While
//! scores are based on the performance of individual exercises, rewards are assigned based on the
//! results of other exercises and propagated to connected lessons and courses.
//!
//! The purpose is to model how good or bad performance in one exercise reflects the performance in
//! related exercises. Good scores in one exercise positively reward the scores in its dependencies
//! (that is, they flow down the unit graph). Bad scores in one exercise negatively reward the
//! scores in its dependents (that is, they flow up the unit graph).
//!
//! As a result, rewarded exercises are not shown to the student as aften as they would otherwise be
//! and penalized exercises are shown more often, allowing for faster review of already mastered
//! material and more practice of material whose dependencies are not fully mastered.

use anyhow::{Context, Ok, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rusqlite_migration::{M, Migrations};
use std::collections::VecDeque;
use ustr::{Ustr, UstrMap};

use crate::{data::UnitReward, error::PracticeRewardsError, utils};

/// Contains functions to retrieve and record rewards for lessons and courses.
pub trait PracticeRewards {
    /// Retrieves the last given number of rewards of a particular lesson or course. The rewards are
    /// in descending order according to the timestamp.
    fn get_rewards(
        &self,
        unit_id: Ustr,
        num_rewards: usize,
    ) -> Result<Vec<UnitReward>, PracticeRewardsError>;

    /// Records the reward assigned to the unit. Only lessons and courses should have rewards.
    /// However, the caller must enforce this requirement. Because similar exercises can write
    /// similar rewards in quick succession, the implementation can choose to skip the reward if it
    /// is deemed too similar to another recent one. If that's the case, the function returns
    /// `false`.
    fn record_unit_reward(
        &mut self,
        unit_id: Ustr,
        reward: &UnitReward,
    ) -> Result<bool, PracticeRewardsError>;

    /// Deletes all rewards of the given unit except for the last given number with the aim of
    /// keeping the storage size under check.
    fn trim_rewards(&mut self, num_rewards: usize) -> Result<(), PracticeRewardsError>;

    /// Removes all the rewards from the units that match the given prefix.
    fn remove_rewards_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeRewardsError>;
}

/// Number of seconds in a day.
const SECONDS_IN_DAY: i64 = 86_400;

/// The maximum difference in weights between two rewards to consider them similar.
const WEIGHT_EPSILON: f32 = 0.1;

/// The maximum number of rewards per unit to keep in the cache.
const MAX_CACHE_SIZE: usize = 10;

/// A cached list of the most recent rewards. Rewards propagate when scoring every exercise, which
/// means a lot of them will be repeated. This cache checks if there's a similar reward and skips
/// the current reward otherwise.
struct RewardCache {
    /// A map of unit IDs to a list of rewards.
    cache: UstrMap<VecDeque<UnitReward>>,
}

impl RewardCache {
    /// Checks if the cache contains a similar reward. Two rewards are considered similar if their
    /// reward value is the same, their timestamp is within a day of each other, and their weight
    /// differs by less than the epsilon.
    fn has_similar_reward(&self, unit_id: Ustr, reward: &UnitReward) -> bool {
        self.cache
            .get(&unit_id)
            .and_then(|rewards| {
                rewards.iter().find(|r| {
                    r.value == reward.value
                        && (r.timestamp - reward.timestamp).abs() < SECONDS_IN_DAY
                        && (r.weight - reward.weight).abs() < WEIGHT_EPSILON
                })
            })
            .is_some()
    }

    /// Stores the new reward. Replaces the oldest reward in the cache with the given reward if the
    /// cache is full. Assumes that the cache is already sorted by ascending timestamp.
    fn add_new_reward(&mut self, unit_id: Ustr, reward: UnitReward) {
        let rewards = self.cache.get(&unit_id).cloned().unwrap_or_default();
        let mut new_rewards = rewards;
        if new_rewards.len() >= MAX_CACHE_SIZE {
            new_rewards.pop_front();
        }
        new_rewards.push_back(reward);
        self.cache.insert(unit_id, new_rewards);
    }
}

/// An implementation of [`PracticeRewards`] backed by `SQLite`.
pub struct LocalPracticeRewards {
    /// A pool of connections to the database.
    pool: Pool<SqliteConnectionManager>,

    /// A cache of previous rewards to avoid storing the same reward multiple times.
    cache: RewardCache,
}

impl LocalPracticeRewards {
    /// Returns all the migrations needed to set up the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            // Create a table with a mapping of unit IDs to a unique integer ID. The purpose of this
            // table is to save space when storing the unit rewards by not having to store the
            // entire ID of the unit.
            M::up("CREATE TABLE uids(unit_uid INTEGER PRIMARY KEY, unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE uids;"),
            // Create a table storing all the unit rewards.
            M::up(
                "CREATE TABLE practice_rewards(
                id INTEGER PRIMARY KEY,
                unit_uid INTEGER NOT NULL REFERENCES uids(unit_uid),
                reward REAL,
                weight REAL,
                timestamp INTEGER);",
            )
            .down("DROP TABLE practice_rewards"),
            // Create an index of `unit_ids`.
            M::up("CREATE INDEX unit_ids ON uids (unit_id);").down("DROP INDEX unit_ids"),
            // Create a combined index of `unit_uid` and `timestamp` for fast reward retrieval.
            M::up("CREATE INDEX rewards ON practice_rewards (unit_uid, timestamp);")
                .down("DROP INDEX rewards"),
        ])
    }

    /// Initializes the database by running the migrations. If the migrations have been applied
    /// already, they will have no effect on the database.
    fn init(&mut self) -> Result<()> {
        let mut connection = self.pool.get()?;
        let migrations = Self::migrations();
        migrations
            .to_latest(&mut connection)
            .context("failed to initialize practice rewards DB")
    }

    /// Creates a connection pool and initializes the database.
    fn new(connection_manager: SqliteConnectionManager) -> Result<LocalPracticeRewards> {
        let pool = utils::new_connection_pool(connection_manager)?;
        let mut rewards = LocalPracticeRewards {
            pool,
            cache: RewardCache {
                cache: UstrMap::default(),
            },
        };
        rewards.init()?;
        Ok(rewards)
    }

    /// A constructor taking the path to a database file.
    pub fn new_from_disk(db_path: &str) -> Result<LocalPracticeRewards> {
        Self::new(utils::new_connection_manager(db_path))
    }

    /// Helper function to retrieve rewards from the database.
    fn get_rewards_helper(&self, unit_id: Ustr, num_rewards: usize) -> Result<Vec<UnitReward>> {
        // Retrieve the rewards from the database.
        let connection = self.pool.get()?;
        let mut stmt = connection.prepare_cached(
            "SELECT reward, weight, timestamp from practice_rewards WHERE unit_uid = (
                SELECT unit_uid FROM uids WHERE unit_id = $1)
                ORDER BY timestamp DESC LIMIT ?2;",
        )?;

        // Convert the results into a vector of `UnitRewards` objects.
        #[allow(clippy::let_and_return)]
        let rows = stmt
            .query_map(params![unit_id.as_str(), num_rewards], |row| {
                let value = row.get(0)?;
                let weight = row.get(1)?;
                let timestamp = row.get(2)?;
                rusqlite::Result::Ok(UnitReward {
                    value,
                    weight,
                    timestamp,
                })
            })?
            .map(|r| r.context("failed to retrieve rewards from practice rewards DB"))
            .collect::<Result<Vec<UnitReward>, _>>()?;
        Ok(rows)
    }

    /// Helper function to record a reward to the database.
    fn record_unit_reward_helper(&mut self, unit_id: Ustr, reward: &UnitReward) -> Result<bool> {
        // Check the cache and exit early if there is a similar reward.
        if self.cache.has_similar_reward(unit_id, reward) {
            return Ok(false);
        }

        // Update the mapping of unit ID to unique integer ID.
        let connection = self.pool.get()?;
        let mut uid_stmt =
            connection.prepare_cached("INSERT OR IGNORE INTO uids(unit_id) VALUES ($1);")?;
        uid_stmt.execute(params![unit_id.as_str()])?;

        // Insert the unit reward into the database.
        let mut stmt = connection.prepare_cached(
            "INSERT INTO practice_rewards (unit_uid, reward, weight, timestamp) VALUES (
                (SELECT unit_uid FROM uids WHERE unit_id = $1), $2, $3, $4);",
        )?;
        stmt.execute(params![
            unit_id.as_str(),
            reward.value,
            reward.weight,
            reward.timestamp
        ])?;

        // Update the cache with the new reward.
        self.cache.add_new_reward(unit_id, reward.clone());

        // Delete the oldest trials and keep the most recent 20 rewards. Otherwise, the database can
        // grow indefinitely.
        let mut stmt = connection.prepare_cached(
            "DELETE FROM practice_rewards WHERE id IN (
                    SELECT id FROM practice_rewards WHERE unit_uid = (
                        SELECT unit_uid FROM uids WHERE unit_id = $1)
                    ORDER BY timestamp DESC LIMIT -1 OFFSET 20
                );",
        )?;
        let _ = stmt.execute(params![unit_id.as_str()])?;

        Ok(true)
    }

    /// Helper function to trim the number of rewards for each unit to the given number. If the
    /// number of rewards is less than the given number, the method deletes no rewards.
    fn trim_rewards_helper(&mut self, num_rewards: usize) -> Result<()> {
        let connection = self.pool.get()?;
        for row in connection
            .prepare("SELECT unit_uid FROM uids")?
            .query_map([], |row| row.get(0))?
        {
            let unit_uid: i64 = row?;
            connection.execute(
                "DELETE FROM practice_rewards WHERE id IN (
                    SELECT id FROM practice_rewards WHERE unit_uid = ?1
                    ORDER BY timestamp DESC LIMIT -1 OFFSET ?2
                )",
                params![unit_uid, num_rewards],
            )?;
        }
        Ok(())
    }

    /// Helper function to remove all the rewards from units that match the given prefix.
    fn remove_rewards_with_prefix_helper(&mut self, prefix: &str) -> Result<()> {
        // Get all the UIDs for the units that match the prefix.
        let connection = self.pool.get()?;
        for row in connection
            .prepare("SELECT unit_uid FROM uids WHERE unit_id LIKE ?1")?
            .query_map(params![format!("{}%", prefix)], |row| row.get(0))?
        {
            let unit_uid: i64 = row?;
            connection.execute(
                "DELETE FROM practice_rewards WHERE unit_uid = ?1;",
                params![unit_uid],
            )?;
        }

        // Call the `VACUUM` command to reclaim the space freed by the deleted trials.
        connection.execute_batch("VACUUM;")?;
        Ok(())
    }
}

impl PracticeRewards for LocalPracticeRewards {
    fn get_rewards(
        &self,
        unit_id: Ustr,
        num_rewards: usize,
    ) -> Result<Vec<UnitReward>, PracticeRewardsError> {
        self.get_rewards_helper(unit_id, num_rewards)
            .map_err(|e| PracticeRewardsError::GetRewards(unit_id, e))
    }

    fn record_unit_reward(
        &mut self,
        unit_id: Ustr,
        reward: &UnitReward,
    ) -> Result<bool, PracticeRewardsError> {
        self.record_unit_reward_helper(unit_id, reward)
            .map_err(|e| PracticeRewardsError::RecordReward(unit_id, e))
    }

    fn trim_rewards(&mut self, num_rewards: usize) -> Result<(), PracticeRewardsError> {
        self.trim_rewards_helper(num_rewards)
            .map_err(PracticeRewardsError::TrimReward)
    }

    fn remove_rewards_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeRewardsError> {
        self.remove_rewards_with_prefix_helper(prefix)
            .map_err(|e| PracticeRewardsError::RemovePrefix(prefix.to_string(), e))
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::{Ok, Result};
    use r2d2_sqlite::SqliteConnectionManager;
    use ustr::Ustr;

    use crate::{
        data::UnitReward,
        practice_rewards::{LocalPracticeRewards, PracticeRewards},
    };

    fn new_tests_rewards() -> Result<Box<dyn PracticeRewards>> {
        let connection_manager = SqliteConnectionManager::memory();
        let practice_rewards = LocalPracticeRewards::new(connection_manager)?;
        Ok(Box::new(practice_rewards))
    }

    fn assert_rewards(expected_rewards: &[f32], expected_weights: &[f32], actual: &[UnitReward]) {
        let only_rewards: Vec<f32> = actual.iter().map(|t| t.value).collect();
        assert_eq!(expected_rewards, only_rewards);
        let only_weights: Vec<f32> = actual.iter().map(|t| t.weight).collect();
        assert_eq!(expected_weights, only_weights);
        let timestamps_sorted = actual
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i == 0 {
                    return true;
                }
                actual[i - 1].timestamp >= actual[i].timestamp
            })
            .all(|b| b);
        assert!(timestamps_sorted);
    }

    /// Verifies setting and retrieving a single reward for a unit.
    #[test]
    fn basic() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit_id = Ustr::from("unit_123");
        practice_rewards.record_unit_reward(
            unit_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        let rewards = practice_rewards.get_rewards(unit_id, 1)?;
        assert_rewards(&[3.0], &[1.0], &rewards);
        Ok(())
    }

    /// Verifies setting and retrieving multiple rewards for a unit.
    #[test]
    fn multiple_rewards() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit_id = Ustr::from("unit_123");
        practice_rewards.record_unit_reward(
            unit_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit_id,
            &UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit_id,
            &UnitReward {
                value: -1.0,
                weight: 0.05,
                timestamp: 3,
            },
        )?;

        let one_reward = practice_rewards.get_rewards(unit_id, 1)?;
        assert_rewards(&[-1.0], &[0.05], &one_reward);

        let three_rewards = practice_rewards.get_rewards(unit_id, 3)?;
        assert_rewards(&[-1.0, 2.0, 3.0], &[0.05, 1.0, 1.0], &three_rewards);

        let more_rewards = practice_rewards.get_rewards(unit_id, 10)?;
        assert_rewards(&[-1.0, 2.0, 3.0], &[0.05, 1.0, 1.0], &more_rewards);
        Ok(())
    }

    /// Verifies older rewards are trimmed when the number of rewards exceeds the limit.
    #[test]
    fn many_rewards() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit_id = Ustr::from("unit_123");
        for i in 0..20 {
            practice_rewards.record_unit_reward(
                unit_id,
                &UnitReward {
                    value: i as f32,
                    weight: 1.0,
                    timestamp: i as i64,
                },
            )?;
        }

        let rewards = practice_rewards.get_rewards(unit_id, 10)?;
        let expected_rewards: Vec<f32> = (10..20).rev().map(|i| i as f32).collect();
        let expected_weights: Vec<f32> = vec![1.0; 10];
        assert_rewards(&expected_rewards, &expected_weights, &rewards);
        Ok(())
    }

    /// Verifies retrieving an empty list of rewards for a unit with no previous rewards.
    #[test]
    fn no_records() -> Result<()> {
        let practice_rewards = new_tests_rewards()?;
        let rewards = practice_rewards.get_rewards(Ustr::from("unit_123"), 10)?;
        assert_rewards(&[], &[], &rewards);
        Ok(())
    }

    /// Verifies trimming all but the most recent reward.
    #[test]
    fn trim_rewards_some_rewards_removed() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit1_id = Ustr::from("unit1");
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 4.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 5.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;
        assert_eq!(3, practice_rewards.get_rewards(unit1_id, 10)?.len());

        let unit2_id = Ustr::from("unit2");
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;
        assert_eq!(3, practice_rewards.get_rewards(unit2_id, 10)?.len());

        practice_rewards.trim_rewards(2)?;
        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[5.0, 4.0], &[1.0, 1.0], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[3.0, 2.0], &[1.0, 1.0], &rewards);
        Ok(())
    }

    /// Verifies trimming no rewards when the number of rewards is less than the limit.
    #[test]
    fn trim_rewards_no_rewards_removed() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit1_id = Ustr::from("unit1");
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 4.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 5.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;

        let unit2_id = Ustr::from("unit2");
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;

        practice_rewards.trim_rewards(10)?;

        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[5.0, 4.0, 3.0], &[1.0, 1.0, 1.0], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[3.0, 2.0, 1.0], &[1.0, 1.0, 1.0], &rewards);
        Ok(())
    }

    /// Verifies removing the trials for units that match the given prefix.
    #[test]
    fn remove_rewards_with_prefix() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit1_id = Ustr::from("unit1");
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 4.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit1_id,
            &UnitReward {
                value: 5.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;

        let unit2_id = Ustr::from("unit2");
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit2_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;

        let unit3_id = Ustr::from("unit3");
        practice_rewards.record_unit_reward(
            unit3_id,
            &UnitReward {
                value: 1.0,
                weight: 1.0,
                timestamp: 1,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit3_id,
            &UnitReward {
                value: 2.0,
                weight: 1.0,
                timestamp: 2,
            },
        )?;
        practice_rewards.record_unit_reward(
            unit3_id,
            &UnitReward {
                value: 3.0,
                weight: 1.0,
                timestamp: 3,
            },
        )?;

        // Remove the prefix "unit1".
        practice_rewards.remove_rewards_with_prefix("unit1")?;
        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[], &[], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[3.0, 2.0, 1.0], &[1.0, 1.0, 1.0], &rewards);
        let rewards = practice_rewards.get_rewards(unit3_id, 10)?;
        assert_rewards(&[3.0, 2.0, 1.0], &[1.0, 1.0, 1.0], &rewards);

        // Remove the prefix "unit". All the rewards should be removed.
        practice_rewards.remove_rewards_with_prefix("unit")?;
        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[], &[], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[], &[], &rewards);
        let rewards = practice_rewards.get_rewards(unit3_id, 10)?;
        assert_rewards(&[], &[], &rewards);

        Ok(())
    }
}
