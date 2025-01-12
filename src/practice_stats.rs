//! Defines how the results of exercise trials are stored for used during scheduling.
//!
//! Currently, only the score and the timestamp are stored. From the results and timestamps of
//! previous trials, a score for the exercise (in the range 0.0 to 5.0) is calculated. See the
//! documentation in [scorer](crate::scorer) for more details.

use anyhow::{Context, Ok, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rusqlite_migration::{Migrations, M};
use ustr::Ustr;

use crate::{
    data::{ExerciseTrial, MasteryScore},
    db_utils,
    error::PracticeStatsError,
};

/// Contains functions to retrieve and record the scores from each exercise trial.
pub trait PracticeStats {
    /// Retrieves the last `num_scores` scores of a particular exercise. The scores are returned in
    /// descending order according to the timestamp.
    fn get_scores(
        &self,
        exercise_id: Ustr,
        num_scores: usize,
    ) -> Result<Vec<ExerciseTrial>, PracticeStatsError>;

    /// Records the score assigned to the exercise in a particular trial. Therefore, the score is a
    /// value of the `MasteryScore` enum instead of a float. Only units of type `UnitType::Exercise`
    /// should have scores recorded. However, the enforcement of this requirement is left to the
    /// caller.
    fn record_exercise_score(
        &mut self,
        exercise_id: Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), PracticeStatsError>;

    /// Deletes all the exercise trials except for the last `num_scores` with the aim of keeping the
    /// storage size under check.
    fn trim_scores(&mut self, num_scores: usize) -> Result<(), PracticeStatsError>;

    /// Removes all the scores from the units that match the given prefix.
    fn remove_scores_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeStatsError>;
}

/// An implementation of [`PracticeStats`] backed by `SQLite`.
pub struct LocalPracticeStats {
    /// A pool of connections to the database.
    pool: Pool<SqliteConnectionManager>,
}

impl LocalPracticeStats {
    /// Returns all the migrations needed to set up the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            // Create a table with a mapping of unit IDs to a unique integer ID. The purpose of this
            // table is to save space when storing the exercise trials by not having to store the
            // entire ID of the unit.
            M::up("CREATE TABLE uids(unit_uid INTEGER PRIMARY KEY, unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE uids;"),
            // Create a table storing all the exercise trials.
            M::up(
                "CREATE TABLE practice_stats(
                id INTEGER PRIMARY KEY,
                unit_uid INTEGER NOT NULL REFERENCES uids(unit_uid),
                score REAL, timestamp INTEGER);",
            )
            .down("DROP TABLE practice_stats"),
            // Create an index of `unit_ids`.
            M::up("CREATE INDEX unit_ids ON uids (unit_id);").down("DROP INDEX unit_ids"),
            //@<lp-example-6
            // Originally the trials were indexed solely by `unit_uid`. This index was replaced so
            // this migration is immediately canceled by the one right below. They cannot be removed
            // from the migration list without breaking databases created in an earlier version than
            // the one which removes them, so they are kept here for now.
            M::up("CREATE INDEX unit_scores ON practice_stats (unit_uid);")
                .down("DROP INDEX unit_scores"),
            M::up("DROP INDEX unit_scores")
                .down("CREATE INDEX unit_scores ON practice_stats (unit_uid);"),
            //>@lp-example-6
            // Create a combined index of `unit_uid` and `timestamp` for fast trial retrieval.
            M::up("CREATE INDEX trials ON practice_stats (unit_uid, timestamp);")
                .down("DROP INDEX trials"),
        ])
    }

    /// Initializes the database by running the migrations. If the migrations have been applied
    /// already, they will have no effect on the database.
    fn init(&mut self) -> Result<()> {
        let mut connection = self.pool.get()?;
        let migrations = Self::migrations();
        migrations
            .to_latest(&mut connection)
            .context("failed to initialize practice stats DB")
    }

    /// A constructor taking a `SQLite` connection manager.
    fn new(connection_manager: SqliteConnectionManager) -> Result<LocalPracticeStats> {
        // Create a connection pool and initialize the database.
        let pool = Pool::new(connection_manager)?;
        let mut stats = LocalPracticeStats { pool };
        stats.init()?;
        Ok(stats)
    }

    /// A constructor taking the path to a database file.
    pub fn new_from_disk(db_path: &str) -> Result<LocalPracticeStats> {
        Self::new(db_utils::new_connection_manager(db_path))
    }

    /// Helper function to retrieve scores from the database.
    fn get_scores_helper(
        &self,
        exercise_id: Ustr,
        num_scores: usize,
    ) -> Result<Vec<ExerciseTrial>> {
        // Retrieve the exercise trials from the database.
        let connection = self.pool.get()?;
        let mut stmt = connection.prepare_cached(
            "SELECT score, timestamp from practice_stats WHERE unit_uid = (
                SELECT unit_uid FROM uids WHERE unit_id = $1)
                ORDER BY timestamp DESC LIMIT ?2;",
        )?;

        // Convert the results into a vector of `ExerciseTrial` objects.
        #[allow(clippy::let_and_return)]
        let rows = stmt
            .query_map(params![exercise_id.as_str(), num_scores], |row| {
                let score = row.get(0)?;
                let timestamp = row.get(1)?;
                rusqlite::Result::Ok(ExerciseTrial { score, timestamp })
            })?
            .map(|r| r.context("failed to retrieve scores from practice stats DB"))
            .collect::<Result<Vec<ExerciseTrial>, _>>()?;
        Ok(rows)
    }

    /// Helper function to record a score to the database.
    fn record_exercise_score_helper(
        &mut self,
        exercise_id: Ustr,
        score: &MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        // Update the mapping of unit ID to unique integer ID.
        let connection = self.pool.get()?;
        let mut uid_stmt =
            connection.prepare_cached("INSERT OR IGNORE INTO uids(unit_id) VALUES ($1);")?;
        uid_stmt.execute(params![exercise_id.as_str()])?;

        // Insert the exercise trial into the database.
        let mut stmt = connection.prepare_cached(
            "INSERT INTO practice_stats (unit_uid, score, timestamp) VALUES (
                (SELECT unit_uid FROM uids WHERE unit_id = $1), $2, $3);",
        )?;
        stmt.execute(params![
            exercise_id.as_str(),
            score.float_score(),
            timestamp
        ])?;
        Ok(())
    }

    /// Helper function to trim the number of scores for each exercise.
    fn trim_scores_helper(&mut self, num_scores: usize) -> Result<()> {
        // Get all the UIDs from the database.
        let connection = self.pool.get()?;
        let mut uid_stmt = connection.prepare_cached("SELECT unit_uid from uids")?;
        let uids = uid_stmt
            .query_map([], |row| row.get(0))?
            .map(|r| r.context("failed to retrieve UIDs from practice stats DB"))
            .collect::<Result<Vec<i64>, _>>()?;

        // Delete the oldest trials for each UID but keep the most recent `num_scores` trials.
        for uid in uids {
            let mut stmt = connection.prepare_cached(
                "DELETE FROM practice_stats WHERE unit_uid = $1 AND timestamp NOT IN (
                    SELECT timestamp FROM practice_stats WHERE unit_uid = $1
                    ORDER BY timestamp DESC LIMIT ?2);",
            )?;
            let _ = stmt.execute(params![uid, num_scores])?;
        }

        // Call the `VACUUM` command to reclaim the space freed by the deleted trials.
        connection.execute_batch("VACUUM;")?;
        Ok(())
    }

    /// Helper function to remove all the scores from units that match the given prefix.
    fn remove_scores_with_prefix_helper(&mut self, prefix: &str) -> Result<()> {
        // Get all the UIDs for the units that match the prefix.
        let connection = self.pool.get()?;
        let mut uid_stmt =
            connection.prepare_cached("SELECT unit_uid FROM uids WHERE unit_id LIKE $1;")?;
        let uids = uid_stmt
            .query_map(params![format!("{}%", prefix)], |row| row.get(0))?
            .map(|r| r.context("failed to retrieve UIDs from practice stats DB"))
            .collect::<Result<Vec<i64>, _>>()?;

        // Delete all the trials for those units.
        for uid in uids {
            let mut stmt =
                connection.prepare_cached("DELETE FROM practice_stats WHERE unit_uid = $1;")?;
            let _ = stmt.execute(params![uid])?;
        }

        // Call the `VACUUM` command to reclaim the space freed by the deleted trials.
        connection.execute_batch("VACUUM;")?;
        Ok(())
    }
}

impl PracticeStats for LocalPracticeStats {
    fn get_scores(
        &self,
        exercise_id: Ustr,
        num_scores: usize,
    ) -> Result<Vec<ExerciseTrial>, PracticeStatsError> {
        self.get_scores_helper(exercise_id, num_scores)
            .map_err(|e| PracticeStatsError::GetScores(exercise_id, e))
    }

    fn record_exercise_score(
        &mut self,
        exercise_id: Ustr,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<(), PracticeStatsError> {
        self.record_exercise_score_helper(exercise_id, &score, timestamp)
            .map_err(|e| PracticeStatsError::RecordScore(exercise_id, e))
    }

    fn trim_scores(&mut self, num_scores: usize) -> Result<(), PracticeStatsError> {
        self.trim_scores_helper(num_scores)
            .map_err(PracticeStatsError::TrimScores)
    }

    fn remove_scores_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeStatsError> {
        self.remove_scores_with_prefix_helper(prefix)
            .map_err(|e| PracticeStatsError::RemovePrefix(prefix.to_string(), e))
    }
}

#[cfg(test)]
mod test {
    use anyhow::{Ok, Result};
    use r2d2_sqlite::SqliteConnectionManager;
    use ustr::Ustr;

    use crate::{
        data::{ExerciseTrial, MasteryScore},
        practice_stats::{LocalPracticeStats, PracticeStats},
    };

    fn new_tests_stats() -> Result<Box<dyn PracticeStats>> {
        let connection_manager = SqliteConnectionManager::memory();
        let practice_stats = LocalPracticeStats::new(connection_manager)?;
        Ok(Box::new(practice_stats))
    }

    fn assert_scores(expected: &[f32], actual: &[ExerciseTrial]) {
        let only_scores: Vec<f32> = actual.iter().map(|t| t.score).collect();
        assert_eq!(expected, only_scores);
        let all_sorted = actual
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i == 0 {
                    return true;
                }
                actual[i - 1].score >= actual[i].score
            })
            .all(|b| b);
        assert!(all_sorted);
    }

    /// Verifies setting and retrieving a single score for an exercise.
    #[test]
    fn basic() -> Result<()> {
        let mut stats = new_tests_stats()?;
        let exercise_id = Ustr::from("ex_123");
        stats.record_exercise_score(exercise_id, MasteryScore::Five, 1)?;
        let scores = stats.get_scores(exercise_id, 1)?;
        assert_scores(&[5.0], &scores);
        Ok(())
    }

    /// Verifies setting and retrieving multiple scores for an exercise.
    #[test]
    fn multiple_records() -> Result<()> {
        let mut stats = new_tests_stats()?;
        let exercise_id = Ustr::from("ex_123");
        stats.record_exercise_score(exercise_id, MasteryScore::Three, 1)?;
        stats.record_exercise_score(exercise_id, MasteryScore::Four, 2)?;
        stats.record_exercise_score(exercise_id, MasteryScore::Five, 3)?;

        let one_score = stats.get_scores(exercise_id, 1)?;
        assert_scores(&[5.0], &one_score);

        let three_scores = stats.get_scores(exercise_id, 3)?;
        assert_scores(&[5.0, 4.0, 3.0], &three_scores);

        let more_scores = stats.get_scores(exercise_id, 10)?;
        assert_scores(&[5.0, 4.0, 3.0], &more_scores);
        Ok(())
    }

    /// Verifies retrieving an empty list of scores for an exercise with no previous scores.
    #[test]
    fn no_records() -> Result<()> {
        let stats = new_tests_stats()?;
        let scores = stats.get_scores(Ustr::from("ex_123"), 10)?;
        assert_scores(&[], &scores);
        Ok(())
    }

    /// Verifies trimming all but the most recent scores.
    #[test]
    fn trim_scores_some_scores_removed() -> Result<()> {
        let mut stats = new_tests_stats()?;
        let exercise1_id = Ustr::from("exercise1");
        stats.record_exercise_score(exercise1_id, MasteryScore::Three, 1)?;
        stats.record_exercise_score(exercise1_id, MasteryScore::Four, 2)?;
        stats.record_exercise_score(exercise1_id, MasteryScore::Five, 3)?;

        let exercise2_id = Ustr::from("exercise2");
        stats.record_exercise_score(exercise2_id, MasteryScore::One, 1)?;
        stats.record_exercise_score(exercise2_id, MasteryScore::One, 2)?;
        stats.record_exercise_score(exercise2_id, MasteryScore::Three, 3)?;

        stats.trim_scores(2)?;

        let scores = stats.get_scores(exercise1_id, 10)?;
        assert_scores(&[5.0, 4.0], &scores);
        let scores = stats.get_scores(exercise2_id, 10)?;
        assert_scores(&[3.0, 1.0], &scores);
        Ok(())
    }

    /// Verifies trimming no scores when the number of scores is less than the limit.
    #[test]
    fn trim_scores_no_scores_removed() -> Result<()> {
        let mut stats = new_tests_stats()?;
        let exercise1_id = Ustr::from("exercise1");
        stats.record_exercise_score(exercise1_id, MasteryScore::Three, 1)?;
        stats.record_exercise_score(exercise1_id, MasteryScore::Four, 2)?;
        stats.record_exercise_score(exercise1_id, MasteryScore::Five, 3)?;

        let exercise2_id = Ustr::from("exercise2");
        stats.record_exercise_score(exercise2_id, MasteryScore::One, 1)?;
        stats.record_exercise_score(exercise2_id, MasteryScore::One, 2)?;
        stats.record_exercise_score(exercise2_id, MasteryScore::Three, 3)?;

        stats.trim_scores(10)?;

        let scores = stats.get_scores(exercise1_id, 10)?;
        assert_scores(&[5.0, 4.0, 3.0], &scores);
        let scores = stats.get_scores(exercise2_id, 10)?;
        assert_scores(&[3.0, 1.0, 1.0], &scores);
        Ok(())
    }

    /// Verifies removing the trials for units that match the given prefix.
    #[test]
    fn remove_scores_with_prefix() -> Result<()> {
        let mut stats = new_tests_stats()?;
        let exercise1_id = Ustr::from("exercise1");
        stats.record_exercise_score(exercise1_id, MasteryScore::Three, 1)?;
        stats.record_exercise_score(exercise1_id, MasteryScore::Four, 2)?;
        stats.record_exercise_score(exercise1_id, MasteryScore::Five, 3)?;

        let exercise2_id = Ustr::from("exercise2");
        stats.record_exercise_score(exercise2_id, MasteryScore::One, 1)?;
        stats.record_exercise_score(exercise2_id, MasteryScore::One, 2)?;
        stats.record_exercise_score(exercise2_id, MasteryScore::Three, 3)?;

        let exercise3_id = Ustr::from("exercise3");
        stats.record_exercise_score(exercise3_id, MasteryScore::One, 1)?;
        stats.record_exercise_score(exercise3_id, MasteryScore::One, 2)?;
        stats.record_exercise_score(exercise3_id, MasteryScore::Three, 3)?;

        // Remove the prefix "exercise1".
        stats.remove_scores_with_prefix("exercise1")?;
        let scores = stats.get_scores(exercise1_id, 10)?;
        assert_scores(&[], &scores);
        let scores = stats.get_scores(exercise2_id, 10)?;
        assert_scores(&[3.0, 1.0, 1.0], &scores);
        let scores = stats.get_scores(exercise3_id, 10)?;
        assert_scores(&[3.0, 1.0, 1.0], &scores);

        // Remove the prefix "exercise". All the scores should be removed.
        stats.remove_scores_with_prefix("exercise")?;
        let scores = stats.get_scores(exercise1_id, 10)?;
        assert_scores(&[], &scores);
        let scores = stats.get_scores(exercise2_id, 10)?;
        assert_scores(&[], &scores);
        let scores = stats.get_scores(exercise3_id, 10)?;
        assert_scores(&[], &scores);

        Ok(())
    }
}
