//! Defines a list of units which the student wants to review.
//!
//! Students might identify exercises, lessons, or courses which need additional review. They can
//! add them to the review list. The scheduler implements a special mode that will only schedule
//! exercises from the units in the review list.

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use ustr::Ustr;

use crate::error::ReviewListError;

/// An interface to store and read a list of units that need review.
pub trait ReviewList {
    /// Adds the given unit to the review list.
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError>;

    /// Removes the given unit from the review list. Do nothing if the unit is not already in the
    /// list.
    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError>;

    /// Returns all the entries in the review list.
    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError>;
}

/// An implementation of [ReviewList] backed by SQLite.
pub(crate) struct ReviewListDB {
    /// A pool of connections to the database.
    pool: Pool<SqliteConnectionManager>,
}

impl ReviewListDB {
    /// Returns all the migrations needed to set up the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            // Create a table with the IDs of the units in the review list.
            M::up("CREATE TABLE review_list(unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE review_list"),
            // Create an index of the unit IDs in the review list.
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
            .with_context(|| "failed to initialize review list DB") // grcov-excl-line
    }

    /// A constructor taking a connection manager.
    fn new(connection_manager: SqliteConnectionManager) -> Result<ReviewListDB> {
        // Initialize the pool and the review list database.
        let pool = Pool::new(connection_manager)?;
        let mut review_list = ReviewListDB { pool };
        review_list.init()?;
        Ok(review_list)
    }

    /// A constructor taking the path to the database file.
    pub fn new_from_disk(db_path: &str) -> Result<ReviewListDB> {
        let connection_manager = SqliteConnectionManager::file(db_path).with_init(
            |connection: &mut Connection| -> Result<(), rusqlite::Error> {
                // The following pragma statements are set to improve the read and write performance
                // of SQLite. See the SQLite [docs](https://www.sqlite.org/pragma.html) for more
                // information.
                connection.pragma_update(None, "journal_mode", "WAL")?;
                connection.pragma_update(None, "synchronous", "NORMAL")
            },
        );
        Self::new(connection_manager)
    }
}

impl ReviewList for ReviewListDB {
    fn add_to_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError> {
        // Add the unit to the database.
        let connection = self.pool.get().map_err(ReviewListError::Connection)?;
        let mut stmt = connection
            .prepare_cached("INSERT OR IGNORE INTO review_list (unit_id) VALUES (?1)")
            .map_err(|e| ReviewListError::AddUnit(*unit_id, e))?; // grcov-excl-line
        let _ = stmt
            .execute(params![unit_id.as_str()])
            .map_err(|e| ReviewListError::AddUnit(*unit_id, e))?;
        Ok(())
    }

    fn remove_from_review_list(&mut self, unit_id: &Ustr) -> Result<(), ReviewListError> {
        // Remove the unit from the database.
        let connection = self.pool.get().map_err(ReviewListError::Connection)?;
        let mut stmt = connection
            .prepare_cached("DELETE FROM review_list WHERE unit_id = $1")
            .map_err(|e| ReviewListError::RemoveUnit(*unit_id, e))?; // grcov-excl-line
        let _ = stmt
            .execute(params![unit_id.as_str()])
            .map_err(|e| ReviewListError::RemoveUnit(*unit_id, e))?; // grcov-excl-line
        Ok(())
    }

    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError> {
        // Retrieve all the units from the database.
        let connection = self.pool.get().map_err(ReviewListError::Connection)?;
        let mut stmt = connection
            .prepare_cached("SELECT unit_id from review_list;")
            .map_err(ReviewListError::GetEntries)?; // grcov-excl-line
        let mut rows = stmt.query(params![]).map_err(ReviewListError::GetEntries)?;

        // Convert the rows into a vector of unit IDs.
        let mut entries = Vec::new();
        let mut row = rows.next().map_err(ReviewListError::GetEntries)?;
        while row.is_some() {
            let unit_id: String = row.unwrap().get(0).map_err(ReviewListError::GetEntries)?;
            entries.push(Ustr::from(&unit_id));
            row = rows.next().map_err(ReviewListError::GetEntries)?;
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use r2d2_sqlite::SqliteConnectionManager;
    use ustr::Ustr;

    use crate::review_list::{ReviewList, ReviewListDB};

    fn new_test_review_list() -> Result<Box<dyn ReviewList>> {
        let connection_manager = SqliteConnectionManager::memory();
        let review_list = ReviewListDB::new(connection_manager)?;
        Ok(Box::new(review_list))
    }

    /// Verifies adding and removing units from the review list.
    #[test]
    fn add_and_remove_from_review_list() -> Result<()> {
        let mut review_list = new_test_review_list()?;

        let unit_id = Ustr::from("unit_id");
        let unit_id2 = Ustr::from("unit_id2");
        review_list.add_to_review_list(&unit_id)?;
        review_list.add_to_review_list(&unit_id)?;
        review_list.add_to_review_list(&unit_id2)?;

        let entries = review_list.get_review_list_entries()?;
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&unit_id));
        assert!(entries.contains(&unit_id2));

        review_list.remove_from_review_list(&unit_id)?;
        let entries = review_list.get_review_list_entries()?;
        assert_eq!(entries.len(), 1);
        assert!(!entries.contains(&unit_id));
        assert!(entries.contains(&unit_id2));
        Ok(())
    }
}
