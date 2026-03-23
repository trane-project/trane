//! Defines how the deltas between the student's actual scores and the predicted scores are
//! retrieved and stored.
//!
//! These deltas are used to adjust Trane's predictions to individual performance during the scoring
//! of exercises.

use anyhow::{Context, Ok, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use rusqlite_migration::{M, Migrations};
use ustr::Ustr;

use crate::{data::ExerciseDelta, error::PracticeDeltasError, utils};

/// Contains functions to retrieve and record the deltas between the student's actual scores and the
/// predicted scores.
pub trait PracticeDeltas {
    /// Retrieves the last `num_deltas` deltas of a particular exercise. The deltas are returned in
    /// descending order according to the timestamp.
    fn get_deltas(
        &self,
        exercise_id: Ustr,
        num_deltas: u32,
    ) -> Result<Vec<ExerciseDelta>, PracticeDeltasError>;

    /// Records the delta between the student's actual score and the predicted score for a
    /// particular exercise.
    fn record_exercise_delta(
        &mut self,
        exercise_id: Ustr,
        delta: f32,
        timestamp: i64,
    ) -> Result<(), PracticeDeltasError>;

    /// Deletes all the exercise trials except for the last `num_deltas` with the aim of keeping the
    /// storage size under check.
    fn trim_deltas(&mut self, num_deltas: u32) -> Result<(), PracticeDeltasError>;

    /// Removes all the deltas from the units that match the given prefix.
    fn remove_deltas_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeDeltasError>;
}

/// An implementation of [`PracticeDeltas`] backed by `SQLite`.
pub struct LocalPracticeDeltas {
    /// A connection to the database.
    connection: Mutex<Connection>,
}

impl LocalPracticeDeltas {
    /// Returns all the migrations needed to set up the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            // Create a table with a mapping of unit IDs to a unique integer ID.
            M::up("CREATE TABLE uids(unit_uid INTEGER PRIMARY KEY, unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE uids;"),
            // Create a table storing all the practice deltas.
            M::up(
                "CREATE TABLE practice_deltas(
                id INTEGER PRIMARY KEY,
                unit_uid INTEGER NOT NULL REFERENCES uids(unit_uid),
                delta REAL, timestamp INTEGER);",
            )
            .down("DROP TABLE practice_deltas"),
            // Create a combined index of `unit_uid` and `timestamp` for fast retrieval.
            M::up("CREATE INDEX deltas_trials ON practice_deltas (unit_uid, timestamp);")
                .down("DROP INDEX deltas_trials"),
        ])
    }

    /// Initializes the database by running the migrations.
    fn init(&mut self) -> Result<()> {
        let migrations = Self::migrations();
        let mut connection = self.connection.lock();
        migrations
            .to_latest(&mut connection)
            .context("failed to initialize practice deltas DB")
    }

    /// Creates a new instance with the given connection and initializes the database.
    fn new(connection: Connection) -> Result<LocalPracticeDeltas> {
        let mut deltas = LocalPracticeDeltas {
            connection: Mutex::new(connection),
        };
        deltas.init()?;
        Ok(deltas)
    }

    /// A constructor taking the path to a database file.
    pub fn new_from_disk(db_path: &str) -> Result<LocalPracticeDeltas> {
        Self::new(utils::new_connection(db_path)?)
    }

    /// Helper function to retrieve deltas from the database.
    fn get_deltas_helper(&self, exercise_id: Ustr, num_deltas: u32) -> Result<Vec<ExerciseDelta>> {
        let connection = self.connection.lock();
        let mut stmt = connection.prepare_cached(
            "SELECT delta, timestamp from practice_deltas WHERE unit_uid = (
                SELECT unit_uid FROM uids WHERE unit_id = $1)
                ORDER BY timestamp DESC LIMIT ?2;",
        )?;

        #[allow(clippy::let_and_return)]
        let rows = stmt
            .query_map(params![exercise_id.as_str(), num_deltas], |row| {
                let delta = row.get(0)?;
                let timestamp = row.get(1)?;
                rusqlite::Result::Ok(ExerciseDelta { delta, timestamp })
            })?
            .map(|r| r.context("failed to retrieve deltas from practice deltas DB"))
            .collect::<Result<Vec<ExerciseDelta>, _>>()?;
        Ok(rows)
    }

    /// Helper function to record a delta to the database.
    fn record_exercise_delta_helper(
        &mut self,
        exercise_id: Ustr,
        delta: f32,
        timestamp: i64,
    ) -> Result<()> {
        let mut connection = self.connection.lock();
        let tx = connection.transaction()?;
        {
            let mut uid_stmt =
                tx.prepare_cached("INSERT OR IGNORE INTO uids(unit_id) VALUES ($1);")?;
            uid_stmt.execute(params![exercise_id.as_str()])?;

            let mut stmt = tx.prepare_cached(
                "INSERT INTO practice_deltas (unit_uid, delta, timestamp) VALUES (
                (SELECT unit_uid FROM uids WHERE unit_id = $1), $2, $3);",
            )?;
            stmt.execute(params![exercise_id.as_str(), delta, timestamp])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Helper function to trim the number of deltas for each exercise.
    fn trim_deltas_helper(&mut self, num_deltas: u32) -> Result<()> {
        let connection = self.connection.lock();
        let mut uid_stmt = connection.prepare_cached("SELECT unit_uid from uids")?;
        let uids = uid_stmt
            .query_map([], |row| row.get(0))?
            .map(|r| r.context("failed to retrieve UIDs from practice deltas DB"))
            .collect::<Result<Vec<i64>, _>>()?;

        for uid in uids {
            let mut stmt = connection.prepare_cached(
                "DELETE FROM practice_deltas WHERE unit_uid = $1 AND timestamp NOT IN (
                    SELECT timestamp FROM practice_deltas WHERE unit_uid = $1
                    ORDER BY timestamp DESC LIMIT ?2);",
            )?;
            let _ = stmt.execute(params![uid, num_deltas])?;
        }

        connection.execute_batch("VACUUM;")?;
        Ok(())
    }

    /// Helper function to remove all the deltas from units that match the given prefix.
    fn remove_deltas_with_prefix_helper(&mut self, prefix: &str) -> Result<()> {
        let connection = self.connection.lock();
        let mut uid_stmt =
            connection.prepare_cached("SELECT unit_uid FROM uids WHERE unit_id LIKE $1;")?;
        let uids = uid_stmt
            .query_map(params![format!("{}%", prefix)], |row| row.get(0))?
            .map(|r| r.context("failed to retrieve UIDs from practice deltas DB"))
            .collect::<Result<Vec<i64>, _>>()?;

        for uid in uids {
            let mut stmt =
                connection.prepare_cached("DELETE FROM practice_deltas WHERE unit_uid = $1;")?;
            let _ = stmt.execute(params![uid])?;
        }

        connection.execute_batch("VACUUM;")?;
        Ok(())
    }
}

impl PracticeDeltas for LocalPracticeDeltas {
    fn get_deltas(
        &self,
        exercise_id: Ustr,
        num_deltas: u32,
    ) -> Result<Vec<ExerciseDelta>, PracticeDeltasError> {
        self.get_deltas_helper(exercise_id, num_deltas)
            .map_err(|e| PracticeDeltasError::GetDeltas(exercise_id, e))
    }

    fn record_exercise_delta(
        &mut self,
        exercise_id: Ustr,
        delta: f32,
        timestamp: i64,
    ) -> Result<(), PracticeDeltasError> {
        self.record_exercise_delta_helper(exercise_id, delta, timestamp)
            .map_err(|e| PracticeDeltasError::RecordDelta(exercise_id, e))
    }

    fn trim_deltas(&mut self, num_deltas: u32) -> Result<(), PracticeDeltasError> {
        self.trim_deltas_helper(num_deltas)
            .map_err(PracticeDeltasError::TrimDeltas)
    }

    fn remove_deltas_with_prefix(&mut self, prefix: &str) -> Result<(), PracticeDeltasError> {
        self.remove_deltas_with_prefix_helper(prefix)
            .map_err(|e| PracticeDeltasError::RemovePrefix(prefix.to_string(), e))
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::{Ok, Result};
    use rusqlite::Connection;
    use ustr::Ustr;

    use crate::{
        data::ExerciseDelta,
        practice_deltas::{LocalPracticeDeltas, PracticeDeltas},
    };

    fn new_test_deltas() -> Result<Box<dyn PracticeDeltas>> {
        let practice_deltas = LocalPracticeDeltas::new(Connection::open_in_memory()?)?;
        Ok(Box::new(practice_deltas))
    }

    fn assert_deltas(expected: &[f32], actual: &[ExerciseDelta]) {
        let only_deltas: Vec<f32> = actual.iter().map(|t| t.delta).collect();
        assert_eq!(expected, only_deltas);
        let timestamp_sorted = actual
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i == 0 {
                    return true;
                }
                actual[i - 1].timestamp >= actual[i].timestamp
            })
            .all(|b| b);
        assert!(timestamp_sorted);
    }

    /// Verifies setting and retrieving a single delta for an exercise.
    #[test]
    fn basic() -> Result<()> {
        let mut deltas = new_test_deltas()?;
        let exercise_id = Ustr::from("ex_123");
        deltas.record_exercise_delta(exercise_id, 0.5, 1)?;
        let results = deltas.get_deltas(exercise_id, 1)?;
        assert_deltas(&[0.5], &results);
        Ok(())
    }

    /// Verifies setting and retrieving multiple deltas for an exercise.
    #[test]
    fn multiple_records() -> Result<()> {
        let mut deltas = new_test_deltas()?;
        let exercise_id = Ustr::from("ex_123");
        deltas.record_exercise_delta(exercise_id, 0.1, 1)?;
        deltas.record_exercise_delta(exercise_id, 0.3, 2)?;
        deltas.record_exercise_delta(exercise_id, 0.5, 3)?;

        let one_delta = deltas.get_deltas(exercise_id, 1)?;
        assert_deltas(&[0.5], &one_delta);

        let three_deltas = deltas.get_deltas(exercise_id, 3)?;
        assert_deltas(&[0.5, 0.3, 0.1], &three_deltas);

        let more_deltas = deltas.get_deltas(exercise_id, 10)?;
        assert_deltas(&[0.5, 0.3, 0.1], &more_deltas);
        Ok(())
    }

    /// Verifies retrieving an empty list of deltas for an exercise with no previous deltas.
    #[test]
    fn no_records() -> Result<()> {
        let deltas = new_test_deltas()?;
        let results = deltas.get_deltas(Ustr::from("ex_123"), 10)?;
        assert_deltas(&[], &results);
        Ok(())
    }

    /// Verifies trimming all but the most recent deltas.
    #[test]
    fn trim_deltas_some_removed() -> Result<()> {
        let mut deltas = new_test_deltas()?;
        let exercise1_id = Ustr::from("exercise1");
        deltas.record_exercise_delta(exercise1_id, 0.1, 1)?;
        deltas.record_exercise_delta(exercise1_id, 0.2, 2)?;
        deltas.record_exercise_delta(exercise1_id, 0.3, 3)?;

        let exercise2_id = Ustr::from("exercise2");
        deltas.record_exercise_delta(exercise2_id, -0.1, 1)?;
        deltas.record_exercise_delta(exercise2_id, -0.2, 2)?;
        deltas.record_exercise_delta(exercise2_id, -0.3, 3)?;

        deltas.trim_deltas(2)?;

        let results = deltas.get_deltas(exercise1_id, 10)?;
        assert_deltas(&[0.3, 0.2], &results);
        let results = deltas.get_deltas(exercise2_id, 10)?;
        assert_deltas(&[-0.3, -0.2], &results);
        Ok(())
    }

    /// Verifies trimming no deltas when the number of deltas is less than the limit.
    #[test]
    fn trim_deltas_none_removed() -> Result<()> {
        let mut deltas = new_test_deltas()?;
        let exercise1_id = Ustr::from("exercise1");
        deltas.record_exercise_delta(exercise1_id, 0.1, 1)?;
        deltas.record_exercise_delta(exercise1_id, 0.2, 2)?;
        deltas.record_exercise_delta(exercise1_id, 0.3, 3)?;

        let exercise2_id = Ustr::from("exercise2");
        deltas.record_exercise_delta(exercise2_id, -0.1, 1)?;
        deltas.record_exercise_delta(exercise2_id, -0.2, 2)?;
        deltas.record_exercise_delta(exercise2_id, -0.3, 3)?;

        deltas.trim_deltas(10)?;

        let results = deltas.get_deltas(exercise1_id, 10)?;
        assert_deltas(&[0.3, 0.2, 0.1], &results);
        let results = deltas.get_deltas(exercise2_id, 10)?;
        assert_deltas(&[-0.3, -0.2, -0.1], &results);
        Ok(())
    }

    /// Verifies removing the deltas for units that match the given prefix.
    #[test]
    fn remove_deltas_with_prefix() -> Result<()> {
        let mut deltas = new_test_deltas()?;
        let exercise1_id = Ustr::from("exercise1");
        deltas.record_exercise_delta(exercise1_id, 0.1, 1)?;
        deltas.record_exercise_delta(exercise1_id, 0.2, 2)?;
        deltas.record_exercise_delta(exercise1_id, 0.3, 3)?;

        let exercise2_id = Ustr::from("exercise2");
        deltas.record_exercise_delta(exercise2_id, -0.1, 1)?;
        deltas.record_exercise_delta(exercise2_id, -0.2, 2)?;
        deltas.record_exercise_delta(exercise2_id, -0.3, 3)?;

        let exercise3_id = Ustr::from("exercise3");
        deltas.record_exercise_delta(exercise3_id, 0.4, 1)?;
        deltas.record_exercise_delta(exercise3_id, 0.5, 2)?;
        deltas.record_exercise_delta(exercise3_id, 0.6, 3)?;

        // Remove the prefix "exercise1".
        deltas.remove_deltas_with_prefix("exercise1")?;
        let results = deltas.get_deltas(exercise1_id, 10)?;
        assert_deltas(&[], &results);
        let results = deltas.get_deltas(exercise2_id, 10)?;
        assert_deltas(&[-0.3, -0.2, -0.1], &results);
        let results = deltas.get_deltas(exercise3_id, 10)?;
        assert_deltas(&[0.6, 0.5, 0.4], &results);

        // Remove the prefix "exercise". All the deltas should be removed.
        deltas.remove_deltas_with_prefix("exercise")?;
        let results = deltas.get_deltas(exercise1_id, 10)?;
        assert_deltas(&[], &results);
        let results = deltas.get_deltas(exercise2_id, 10)?;
        assert_deltas(&[], &results);
        let results = deltas.get_deltas(exercise3_id, 10)?;
        assert_deltas(&[], &results);

        Ok(())
    }
}
