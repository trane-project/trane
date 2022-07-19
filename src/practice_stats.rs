//! Module defining the data structures used to store user's answers to exercises.
#[cfg(test)]
mod test;

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};

use crate::data::{ExerciseTrial, MasteryScore};

/// Contains functions to retrieve and record the scores from each exercise trial.
pub trait PracticeStats {
    /// Retrieves the last num_scores scores of a particular exercese.
    fn get_scores(&self, exercise_id: &str, num_scores: usize) -> Result<Vec<ExerciseTrial>>;

    /// Records the score assigned to the exercise in a particular trial. Therefore, the score is a
    /// value of the MasteryScore enum instead of a float. Only units of type UnitType::Exercise
    /// should have scores recorded. However, the enforcement of this requirement is left to the
    /// caller.
    fn record_exercise_score(
        &mut self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()>;
}

/// An implementation of PracticeStats backed by SQLite.
pub(crate) struct PracticeStatsDB {
    /// A SQLite connection to the database storing the records.
    pool: Pool<SqliteConnectionManager>,
}

impl PracticeStatsDB {
    /// Returns all the migrations needed to setup the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            // Create a table with a mapping of unit IDs to a unique integer ID.
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
            // Create an index of unit_ids.
            M::up("CREATE INDEX unit_ids ON uids (unit_id);").down("DROP INDEX unit_ids"),
            // Originally the trials were indexed solely by the unit_uid. This index was replaced so
            // this migration is immediately canceled by the one right below. Remove both of them
            // altogether in a later version.
            M::up("CREATE INDEX unit_scores ON practice_stats (unit_uid);")
                .down("DROP INDEX unit_scores"),
            M::up("DROP INDEX unit_scores")
                .down("CREATE INDEX unit_scores ON practice_stats (unit_uid);"),
            // Create a combined index of unit_uid and timestamp for fast trial retrieval.
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
            .with_context(|| "failed to initialize practice stats DB")
    }

    /// A constructor taking a SQLite connection.
    fn new(connection_manager: SqliteConnectionManager) -> Result<PracticeStatsDB> {
        let pool = Pool::new(connection_manager)?;
        let mut stats = PracticeStatsDB { pool };
        stats.init()?;
        Ok(stats)
    }

    /// A constructor taking the path to the database file.
    pub fn new_from_disk(db_path: &str) -> Result<PracticeStatsDB> {
        let connection_manager = SqliteConnectionManager::file(db_path).with_init(
            |connection: &mut Connection| -> Result<(), rusqlite::Error> {
                connection.pragma_update(None, "journal_mode", &"WAL")?;
                connection.pragma_update(None, "synchronous", &"NORMAL")
            },
        );
        Self::new(connection_manager)
    }
}

impl PracticeStats for PracticeStatsDB {
    fn get_scores(&self, exercise_id: &str, num_scores: usize) -> Result<Vec<ExerciseTrial>> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached(
                "SELECT score, timestamp from practice_stats WHERE unit_uid = (
                    SELECT unit_uid FROM uids WHERE unit_id = ?1)
                    ORDER BY timestamp DESC LIMIT ?2;",
            )
            .with_context(|| "cannot prepare statement to query practice stats DB")?;

        let rows = stmt
            .query_map(params![exercise_id, num_scores], |row| {
                Ok(ExerciseTrial {
                    score: row.get(0)?,
                    timestamp: row.get(1)?,
                })
            })?
            .map(|r| {
                r.with_context(|| {
                    format!("cannot query practice stats for exercise {}", exercise_id)
                })
            })
            .collect();
        rows
    }

    fn record_exercise_score(
        &mut self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        let connection = self.pool.get()?;
        // Add the exercise to the table of uids if not there already.
        let mut uid_stmt =
            connection.prepare_cached("INSERT OR IGNORE INTO uids(unit_id) VALUES (?1);")?;
        uid_stmt.execute(params![exercise_id]).with_context(|| {
            format!(
                "cannot add {} to uids table in practice stats DB",
                exercise_id
            )
        })?;

        let mut stmt = connection.prepare_cached(
            "INSERT INTO practice_stats (unit_uid, score, timestamp) VALUES (
                (SELECT unit_uid FROM uids WHERE unit_id = ?1), ?2, ?3);",
        )?;
        stmt.execute(params![exercise_id, score.float_score(), timestamp])
            .with_context(|| {
                format!(
                    "cannot record score {:?} for exercise {} to practice stats DB",
                    score, exercise_id
                )
            })?;
        Ok(())
    }
}
