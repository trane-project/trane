#[cfg(test)]
mod test;

use anyhow::{Context, Result};
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
    connection: Connection,
}

impl PracticeStatsDB {
    /// Returns all the migrations needed to setup the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            M::up(
                "CREATE TABLE practice_stats(
                unit_id TEXT NOT NULL, score REAL, timestamp INTEGER);",
            )
            .down("DROP TABLE practice_stats"),
            M::up("CREATE INDEX unit_scores ON practice_stats (unit_id);")
                .down("DROP INDEX unit_scores"),
        ])
    }

    /// Initializes the database by running the migrations. If the migrations have been applied
    /// already, they will have no effect on the database.
    fn init(&mut self) -> Result<()> {
        let migrations = Self::migrations();
        migrations
            .to_latest(&mut self.connection)
            .with_context(|| "failed to initialize practice stats DB")?;
        self.connection
            .pragma_update(None, "temp_store", &"2")
            .with_context(|| "failed to set temp store mode to memory")?;
        self.connection
            .pragma_update(None, "journal_mode", &"WAL")
            .with_context(|| "failed to set journal mode to WAL")?;
        self.connection
            .pragma_update(None, "synchronous", &"NORMAL")
            .with_context(|| "failed to set synchronous mode to NORMAL")
    }

    /// A constructor taking a SQLite connection.
    fn new(connection: Connection) -> Result<PracticeStatsDB> {
        let mut stats = PracticeStatsDB { connection };
        stats.init()?;
        Ok(stats)
    }

    /// A constructor taking the path to the database file.
    pub fn new_from_disk(db_path: &str) -> Result<PracticeStatsDB> {
        let connection = Connection::open(db_path)
            .with_context(|| format!("cannot open practice stats DB at path {}", db_path))?;
        Self::new(connection)
    }
}

impl PracticeStats for PracticeStatsDB {
    fn get_scores(&self, exercise_id: &str, num_scores: usize) -> Result<Vec<ExerciseTrial>> {
        let mut stmt = self
            .connection
            .prepare_cached(
                "SELECT score, timestamp from practice_stats WHERE unit_id = ?1 ORDER BY
                    timestamp DESC LIMIT ?2;",
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
        let mut stmt = self.connection.prepare_cached(
            "INSERT INTO practice_stats (unit_id, score, timestamp) VALUES (?1, ?2, ?3)",
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
