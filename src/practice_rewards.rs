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
use rusqlite_migration::{Migrations, M};
use ustr::Ustr;

use crate::{data::UnitReward, db_utils, error::PracticeRewardsError};

/// Contains functions to retrieve and record rewards for lessons and courses.
pub trait PracticeRewards {
    /// Retrieves the last given number of rewards of a particular lesson or course. The rewards are
    /// returned in descending order according to the timestamp.
    fn get_rewards(
        &self,
        unit_id: Ustr,
        num_rewards: usize,
    ) -> Result<Vec<UnitReward>, PracticeRewardsError>;

    /// Records the reward assigned to the unit. Only lessons and courses should have rewards.
    /// However, the enforcement of this requirement is left to the caller.
    fn record_unit_reward(
        &mut self,
        unit_id: Ustr,
        reward: f32,
        timestamp: i64,
    ) -> Result<(), PracticeRewardsError>;

    /// Deletes all rewards of the given unit except for the last given number with the aim of
    /// keeping the storage size under check.
    fn trim_rewards(&mut self, num_rewards: usize) -> Result<(), PracticeRewardsError>;

    /// Removes all the rewards from the units that match the given prefix.
    fn remove_rewards_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeRewardsError>;
}

/// An implementation of [`PracticeRewards`] backed by `SQLite`.
pub struct LocalPracticeRewards {
    /// A pool of connections to the database.
    pool: Pool<SqliteConnectionManager>,
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

    /// A constructor taking a `SQLite` connection manager.
    fn new(connection_manager: SqliteConnectionManager) -> Result<LocalPracticeRewards> {
        // Create a connection pool and initialize the database.
        let pool = Pool::new(connection_manager)?;
        let mut rewards = LocalPracticeRewards { pool };
        rewards.init()?;
        Ok(rewards)
    }

    /// A constructor taking the path to a database file.
    pub fn new_from_disk(db_path: &str) -> Result<LocalPracticeRewards> {
        Self::new(db_utils::new_connection_manager(db_path))
    }

    /// Helper function to retrieve rewards from the database.
    fn get_rewards_helper(&self, unit_id: Ustr, num_rewards: usize) -> Result<Vec<UnitReward>> {
        // Retrieve the rewards from the database.
        let connection = self.pool.get()?;
        let mut stmt = connection.prepare_cached(
            "SELECT reward, timestamp from practice_rewards WHERE unit_uid = (
                SELECT unit_uid FROM uids WHERE unit_id = $1)
                ORDER BY timestamp DESC LIMIT ?2;",
        )?;

        // Convert the results into a vector of `UnitRewards` objects.
        #[allow(clippy::let_and_return)]
        let rows = stmt
            .query_map(params![unit_id.as_str(), num_rewards], |row| {
                let reward = row.get(0)?;
                let timestamp = row.get(1)?;
                rusqlite::Result::Ok(UnitReward { reward, timestamp })
            })?
            .map(|r| r.context("failed to retrieve rewards from practice rewards DB"))
            .collect::<Result<Vec<UnitReward>, _>>()?;
        Ok(rows)
    }

    /// Helper function to record a reward to the database.
    fn record_unit_reward_helper(
        &mut self,
        unit_id: Ustr,
        reward: f32,
        timestamp: i64,
    ) -> Result<()> {
        // Update the mapping of unit ID to unique integer ID.
        let connection = self.pool.get()?;
        let mut uid_stmt =
            connection.prepare_cached("INSERT OR IGNORE INTO uids(unit_id) VALUES ($1);")?;
        uid_stmt.execute(params![unit_id.as_str()])?;

        // Insert the unit reward into the database.
        let mut stmt = connection.prepare_cached(
            "INSERT INTO practice_rewards (unit_uid, reward, timestamp) VALUES (
                (SELECT unit_uid FROM uids WHERE unit_id = $1), $2, $3);",
        )?;
        stmt.execute(params![unit_id.as_str(), reward, timestamp])?;
        Ok(())
    }

    /// Helper function to trim the number of rewards for each unit.
    fn trim_rewards_helper(&mut self, num_rewards: usize) -> Result<()> {
        // Get all the UIDs from the database.
        let connection = self.pool.get()?;
        let mut uid_stmt = connection.prepare_cached("SELECT unit_uid from uids")?;
        let uids = uid_stmt
            .query_map([], |row| row.get(0))?
            .map(|r| r.context("failed to retrieve UIDs from practice rewards DB"))
            .collect::<Result<Vec<i64>, _>>()?;

        // Delete the oldest trials for each UID but keep the most recent `num_reards` trials.
        for uid in uids {
            let mut stmt = connection.prepare_cached(
                "DELETE FROM practice_rewards WHERE unit_uid = $1 AND timestamp NOT IN (
                    SELECT timestamp FROM practice_rewards WHERE unit_uid = $1
                    ORDER BY timestamp DESC LIMIT ?2);",
            )?;
            let _ = stmt.execute(params![uid, num_rewards])?;
        }

        // Call the `VACUUM` command to reclaim the space freed by the deleted trials.
        connection.execute_batch("VACUUM;")?;
        Ok(())
    }

    /// Helper function to remove all the rewards from units that match the given prefix.
    fn remove_rewards_with_prefix_helper(&mut self, prefix: &str) -> Result<()> {
        // Get all the UIDs for the units that match the prefix.
        let connection = self.pool.get()?;
        let mut uid_stmt =
            connection.prepare_cached("SELECT unit_uid FROM uids WHERE unit_id LIKE $1;")?;
        let uids = uid_stmt
            .query_map(params![format!("{}%", prefix)], |row| row.get(0))?
            .map(|r| r.context("failed to retrieve UIDs from practice rewards DB"))
            .collect::<Result<Vec<i64>, _>>()?;

        // Delete all the trials for those units.
        for uid in uids {
            let mut stmt =
                connection.prepare_cached("DELETE FROM practice_rewards WHERE unit_uid = $1;")?;
            let _ = stmt.execute(params![uid])?;
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
        reward: f32,
        timestamp: i64,
    ) -> Result<(), PracticeRewardsError> {
        self.record_unit_reward_helper(unit_id, reward, timestamp)
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

    fn assert_rewards(expected: &[f32], actual: &[UnitReward]) {
        let only_rewards: Vec<f32> = actual.iter().map(|t| t.reward).collect();
        assert_eq!(expected, only_rewards);
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
        practice_rewards.record_unit_reward(unit_id, 3.0, 1)?;
        let rewards = practice_rewards.get_rewards(unit_id, 1)?;
        assert_rewards(&[3.0], &rewards);
        Ok(())
    }

    /// Verifies setting and retrieving multiple rewards for a unit.
    #[test]
    fn multiple_rewards() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit_id = Ustr::from("unit_123");
        practice_rewards.record_unit_reward(unit_id, 3.0, 1)?;
        practice_rewards.record_unit_reward(unit_id, 2.0, 2)?;
        practice_rewards.record_unit_reward(unit_id, -1.0, 3)?;

        let one_reward = practice_rewards.get_rewards(unit_id, 1)?;
        assert_rewards(&[-1.0], &one_reward);

        let three_rewards = practice_rewards.get_rewards(unit_id, 3)?;
        assert_rewards(&[-1.0, 2.0, 3.0], &three_rewards);

        let more_rewards = practice_rewards.get_rewards(unit_id, 10)?;
        assert_rewards(&[-1.0, 2.0, 3.0], &more_rewards);
        Ok(())
    }

    /// Verifies retrieving an empty list of rewards for a unit with no previous rewards.
    #[test]
    fn no_records() -> Result<()> {
        let practice_rewards = new_tests_rewards()?;
        let rewards = practice_rewards.get_rewards(Ustr::from("unit_123"), 10)?;
        assert_rewards(&[], &rewards);
        Ok(())
    }

    /// Verifies trimming all but the most recent reward.
    #[test]
    fn trim_rewards_some_rewards_removed() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit1_id = Ustr::from("unit1");
        practice_rewards.record_unit_reward(unit1_id, 3.0, 1)?;
        practice_rewards.record_unit_reward(unit1_id, 4.0, 2)?;
        practice_rewards.record_unit_reward(unit1_id, 5.0, 3)?;

        let unit2_id = Ustr::from("unit2");
        practice_rewards.record_unit_reward(unit2_id, 1.0, 1)?;
        practice_rewards.record_unit_reward(unit2_id, 1.0, 2)?;
        practice_rewards.record_unit_reward(unit2_id, 3.0, 3)?;

        practice_rewards.trim_rewards(2)?;

        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[5.0, 4.0], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[3.0, 1.0], &rewards);
        Ok(())
    }

    /// Verifies trimming no rewards when the number of rewards is less than the limit.
    #[test]
    fn trim_rewards_no_rewards_removed() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit1_id = Ustr::from("unit1");
        practice_rewards.record_unit_reward(unit1_id, 3.0, 1)?;
        practice_rewards.record_unit_reward(unit1_id, 4.0, 2)?;
        practice_rewards.record_unit_reward(unit1_id, 5.0, 3)?;

        let unit2_id = Ustr::from("unit2");
        practice_rewards.record_unit_reward(unit2_id, 1.0, 1)?;
        practice_rewards.record_unit_reward(unit2_id, 1.0, 2)?;
        practice_rewards.record_unit_reward(unit2_id, 3.0, 3)?;

        practice_rewards.trim_rewards(10)?;

        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[5.0, 4.0, 3.0], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[3.0, 1.0, 1.0], &rewards);
        Ok(())
    }

    /// Verifies removing the trials for units that match the given prefix.
    #[test]
    fn remove_rewards_with_prefix() -> Result<()> {
        let mut practice_rewards = new_tests_rewards()?;
        let unit1_id = Ustr::from("unit1");
        practice_rewards.record_unit_reward(unit1_id, 3.0, 1)?;
        practice_rewards.record_unit_reward(unit1_id, 4.0, 2)?;
        practice_rewards.record_unit_reward(unit1_id, 5.0, 3)?;

        let unit2_id = Ustr::from("unit2");
        practice_rewards.record_unit_reward(unit2_id, 1.0, 1)?;
        practice_rewards.record_unit_reward(unit2_id, 1.0, 2)?;
        practice_rewards.record_unit_reward(unit2_id, 3.0, 3)?;

        let unit3_id = Ustr::from("unit3");
        practice_rewards.record_unit_reward(unit3_id, 1.0, 1)?;
        practice_rewards.record_unit_reward(unit3_id, 1.0, 2)?;
        practice_rewards.record_unit_reward(unit3_id, 3.0, 3)?;

        // Remove the prefix "unit1".
        practice_rewards.remove_rewards_with_prefix("unit1")?;
        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[3.0, 1.0, 1.0], &rewards);
        let rewards = practice_rewards.get_rewards(unit3_id, 10)?;
        assert_rewards(&[3.0, 1.0, 1.0], &rewards);

        // Remove the prefix "unit". All the rewards should be removed.
        practice_rewards.remove_rewards_with_prefix("unit")?;
        let rewards = practice_rewards.get_rewards(unit1_id, 10)?;
        assert_rewards(&[], &rewards);
        let rewards = practice_rewards.get_rewards(unit2_id, 10)?;
        assert_rewards(&[], &rewards);
        let rewards = practice_rewards.get_rewards(unit3_id, 10)?;
        assert_rewards(&[], &rewards);

        Ok(())
    }
}
