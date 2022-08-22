//! Module defining a list of units which the student should review.
#[cfg(test)]
mod test;

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use ustr::Ustr;

/// An interface to store and read a list of units that need review.
pub trait ReviewList {
    /// Adds the given unit to the review list.
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<()>;

    /// Removes the given unit from the review list. Do nothing if the unit is not already in the
    /// list.
    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<()>;

    /// Returns all the entries in the review list.
    fn all_review_list_entries(&self) -> Result<Vec<Ustr>>;
}

/// An implementation of ReviewList backed by SQLite.
pub(crate) struct ReviewListDB {
    pool: Pool<SqliteConnectionManager>,
}

impl ReviewListDB {
    /// Returns all the migrations needed to setup the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            M::up("CREATE TABLE review_list(unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE review_list"),
            M::up("CREATE INDEX unit_id_index ON review_list (unit_id);")
                .down("DROP INDEX unit_id_index"),
        ])
    }

    /// Initializes the database by running the migrations. If the migrations have been applied
    /// already, they will have no effect on the database.
    fn init(&mut self) -> Result<()> {
        let mut connection = self.pool.get()?;
        let migrations = Self::migrations();
        migrations
            .to_latest(&mut connection)
            .with_context(|| "failed to initialize review list DB")
    }

    /// A constructor taking a connection manager.
    fn new(connection_manager: SqliteConnectionManager) -> Result<ReviewListDB> {
        let pool = Pool::new(connection_manager)?;
        let mut review_list = ReviewListDB { pool };
        review_list.init()?;
        Ok(review_list)
    }

    /// A constructor taking the path to the database file.
    pub fn new_from_disk(db_path: &str) -> Result<ReviewListDB> {
        let connection_manager = SqliteConnectionManager::file(db_path).with_init(
            |connection: &mut Connection| -> Result<(), rusqlite::Error> {
                connection.pragma_update(None, "journal_mode", &"WAL")?;
                connection.pragma_update(None, "synchronous", &"NORMAL")
            },
        );
        Self::new(connection_manager)
    }

    /// Returns whether there's an entry for the given unit in the review list.
    fn has_entry(&self, unit_id: &Ustr) -> Result<bool> {
        let connection = self.pool.get()?;
        let mut statement = connection
            .prepare_cached("SELECT unit_id FROM review_list WHERE unit_id = ?")
            .with_context(|| "cannot prepare statment to query review list DB")?;
        let mut rows = statement
            .query(params![unit_id.as_str()])
            .with_context(|| format!("cannot query review list DB for unit {}", unit_id))?;
        Ok(rows.next()?.is_some())
    }
}

impl ReviewList for ReviewListDB {
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<()> {
        let has_entry = self.has_entry(unit_id)?;
        if has_entry {
            return Ok(());
        }

        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("INSERT INTO review_list (unit_id) VALUES (?1)")
            .with_context(|| "cannot prepare statement to insert into review list DB")?;
        stmt.execute(params![unit_id.as_str()])
            .with_context(|| format!("cannot insert unit {} into review list DB", unit_id))?;
        Ok(())
    }

    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<()> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("DELETE FROM review_list WHERE unit_id = $1")
            .with_context(|| "cannot prepare statement to delete from review list DB")?;
        stmt.execute(params![unit_id.as_str()])
            .with_context(|| format!("cannot remove unit {} from review list DB", unit_id))?;
        Ok(())
    }

    fn all_review_list_entries(&self) -> Result<Vec<Ustr>> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("SELECT unit_id from review_list;")
            .with_context(|| "cannot prepare statement to get all entries in review list DB")?;
        let mut rows = stmt.query(params![])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let unit_id: String = row.get(0)?;
            entries.push(Ustr::from(&unit_id));
        }
        Ok(entries)
    }
}
