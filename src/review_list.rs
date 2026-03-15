//! Defines a list of units which the student wants to review.
//!
//! Students might identify exercises, lessons, or courses which need additional review. They can
//! add them to the review list. The scheduler implements a special mode that will only schedule
//! exercises from the units in the review list.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use rusqlite_migration::{M, Migrations};
use ustr::Ustr;

use crate::{error::ReviewListError, utils};

/// An interface to store and read a list of units that need review.
pub trait ReviewList {
    /// Adds the given unit to the review list.
    fn add_to_review_list(&mut self, unit_id: Ustr) -> Result<(), ReviewListError>;

    /// Removes the given unit from the review list. Do nothing if the unit is not already in the
    /// list.
    fn remove_from_review_list(&mut self, unit_id: Ustr) -> Result<(), ReviewListError>;

    /// Returns all the entries in the review list.
    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError>;
}

/// An implementation of [`ReviewList`] backed by `SQLite`.
pub struct LocalReviewList {
    /// A connection used for read-only queries.
    read_connection: Mutex<Connection>,

    /// A connection used for write operations.
    write_connection: Mutex<Connection>,
}

impl LocalReviewList {
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
        let migrations = Self::migrations();
        let mut connection = self.write_connection.lock();
        migrations
            .to_latest(&mut connection)
            .context("failed to initialize review list DB")
    }

    /// Opens two connections to the given database path and initializes the schema. One connection
    /// is used for reads and one for writes to avoid prepared statement invalidation.
    pub fn new(db_path: &str) -> Result<LocalReviewList> {
        let mut review_list = LocalReviewList {
            read_connection: Mutex::new(utils::new_connection(db_path)?),
            write_connection: Mutex::new(utils::new_connection(db_path)?),
        };
        review_list.init()?;
        Ok(review_list)
    }

    /// Helper to add a unit to the review list.
    fn add_to_review_list_helper(&mut self, unit_id: Ustr) -> Result<()> {
        // Add the unit to the database.
        let connection = self.write_connection.lock();
        let mut stmt =
            connection.prepare_cached("INSERT OR IGNORE INTO review_list (unit_id) VALUES (?1)")?;
        stmt.execute(params![unit_id.as_str()])?;
        Ok(())
    }

    /// Helper to remove a unit from the review list.
    fn remove_from_review_list_helper(&mut self, unit_id: Ustr) -> Result<()> {
        // Remove the unit from the database.
        let connection = self.write_connection.lock();
        let mut stmt = connection.prepare_cached("DELETE FROM review_list WHERE unit_id = $1")?;
        stmt.execute(params![unit_id.as_str()])?;
        Ok(())
    }

    /// Helper to get all the entries in the review list.
    fn get_review_list_entries_helper(&self) -> Result<Vec<Ustr>> {
        // Retrieve all the units from the database.
        let connection = self.read_connection.lock();
        let mut stmt = connection.prepare_cached("SELECT unit_id from review_list;")?;
        let mut rows = stmt.query(params![])?;

        // Convert the rows into a vector of unit IDs.
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let unit_id: String = row.get(0)?;
            entries.push(Ustr::from(&unit_id));
        }
        Ok(entries)
    }
}

impl ReviewList for LocalReviewList {
    fn add_to_review_list(&mut self, unit_id: Ustr) -> Result<(), ReviewListError> {
        self.add_to_review_list_helper(unit_id)
            .map_err(|e| ReviewListError::AddUnit(unit_id, e))
    }

    fn remove_from_review_list(&mut self, unit_id: Ustr) -> Result<(), ReviewListError> {
        self.remove_from_review_list_helper(unit_id)
            .map_err(|e| ReviewListError::RemoveUnit(unit_id, e))
    }

    fn get_review_list_entries(&self) -> Result<Vec<Ustr>, ReviewListError> {
        self.get_review_list_entries_helper()
            .map_err(ReviewListError::GetEntries)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use std::sync::atomic::{AtomicU64, Ordering};

    use anyhow::Result;
    use ustr::Ustr;

    use crate::review_list::{LocalReviewList, ReviewList};

    fn new_test_review_list() -> Result<Box<dyn ReviewList>> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let review_list = LocalReviewList::new(&format!(
            "file:review_list_test_{id}?mode=memory&cache=shared"
        ))?;
        Ok(Box::new(review_list))
    }

    /// Verifies adding and removing units from the review list.
    #[test]
    fn add_and_remove_from_review_list() -> Result<()> {
        let mut review_list = new_test_review_list()?;

        let unit_id = Ustr::from("unit_id");
        let unit_id2 = Ustr::from("unit_id2");
        review_list.add_to_review_list(unit_id)?;
        review_list.add_to_review_list(unit_id)?;
        review_list.add_to_review_list(unit_id2)?;

        let entries = review_list.get_review_list_entries()?;
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&unit_id));
        assert!(entries.contains(&unit_id2));

        review_list.remove_from_review_list(unit_id)?;
        let entries = review_list.get_review_list_entries()?;
        assert_eq!(entries.len(), 1);
        assert!(!entries.contains(&unit_id));
        assert!(entries.contains(&unit_id2));
        Ok(())
    }
}
