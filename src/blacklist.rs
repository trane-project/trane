//! Defines the list of units to ignore during scheduling.
//!
//! Users can add units to this list to prevent them from being scheduled, either because they
//! already mastered the material, or because they simply do not want to practice certain skills.
//!
//! The blacklist exists for this purpose. A unit that is on it will never be scheduled. In
//! addition, the scheduler will continue the search past its dependents as if the unit was already
//! mastered. Courses, lessons, and exercises can be added to the blacklist.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use ustr::{Ustr, UstrMap};

/// An interface to store and read the list of units which should be skipped during scheduling.
pub trait Blacklist {
    /// Adds the given unit to the blacklist.
    fn add_to_blacklist(&mut self, unit_id: &Ustr) -> Result<()>;

    /// Removes the given unit from the blacklist. Do nothing if the unit is not already in the
    /// list.
    fn remove_from_blacklist(&mut self, unit_id: &Ustr) -> Result<()>;

    /// Returns whether the given unit is in the blacklist and should be skipped during scheduling.
    fn blacklisted(&self, unit_id: &Ustr) -> Result<bool>;

    /// Returns all the entries in the blacklist.
    fn all_blacklist_entries(&self) -> Result<Vec<Ustr>>;
}

/// An implementation of [Blacklist] backed by SQLite.
pub(crate) struct BlacklistDB {
    /// A cache of the blacklist entries used to avoid unnecessary queries to the database.
    cache: RwLock<UstrMap<bool>>,

    /// A pool of connections to the database.
    pool: Pool<SqliteConnectionManager>,
}

impl BlacklistDB {
    /// Returns all the migrations needed to set up the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            // Create a table with the list of blacklisted units.
            M::up("CREATE TABLE blacklist(unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE blacklist"),
            // Create an index of the blacklisted unit IDs.
            M::up("CREATE INDEX unit_id_index ON blacklist (unit_id);")
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
            .with_context(|| "failed to initialize blacklist DB") // grcov-excl-line
    }

    /// A constructor taking a connection manager.
    fn new(connection_manager: SqliteConnectionManager) -> Result<BlacklistDB> {
        // Initialize the pool and the blacklist database.
        let pool = Pool::new(connection_manager)?;
        let mut blacklist = BlacklistDB {
            cache: RwLock::new(UstrMap::default()),
            pool,
        };
        blacklist.init()?;

        // Initialize the cache with the existing blacklist entries.
        for unit_id in blacklist.all_blacklist_entries()? {
            blacklist.cache.write().insert(unit_id, true);
        }

        Ok(blacklist)
    }

    /// A constructor taking the path to the database file.
    pub fn new_from_disk(db_path: &str) -> Result<BlacklistDB> {
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

    /// Returns whether there's an entry for the given unit in the blacklist.
    fn has_entry(&self, unit_id: &Ustr) -> Result<bool> {
        let mut cache = self.cache.write();
        if let Some(has_entry) = cache.get(unit_id) {
            Ok(*has_entry)
        } else {
            // Because the cache was initialized with all the entries in the blacklist, and it's
            // kept updated, it's safe to assume that the entry is not in the blacklist and update
            // the cache accordingly.
            cache.insert(*unit_id, false);
            Ok(false)
        }
    }
}

impl Blacklist for BlacklistDB {
    fn add_to_blacklist(&mut self, unit_id: &Ustr) -> Result<()> {
        // Check the cache first to avoid unnecessary queries.
        let has_entry = self.has_entry(unit_id)?;
        if has_entry {
            return Ok(());
        }

        // Add the entry to the database.
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("INSERT INTO blacklist (unit_id) VALUES (?1)")
            .with_context(|| "cannot prepare statement to insert into blacklist DB")?; // grcov-excl-line
        stmt.execute(params![unit_id.as_str()])
            .with_context(|| format!("cannot insert unit {unit_id} into blacklist DB"))?;

        // Update the cache.
        self.cache.write().insert(*unit_id, true);
        Ok(())
    }

    fn remove_from_blacklist(&mut self, unit_id: &Ustr) -> Result<()> {
        // Remove the entry from the database.
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("DELETE FROM blacklist WHERE unit_id = $1")
            .with_context(|| "cannot prepare statement to delete from blacklist DB")?; // grcov-excl-line
        stmt.execute(params![unit_id.as_str()])
            .with_context(|| format!("cannot remove unit {unit_id} from blacklist DB"))?;

        // Update the cache.
        self.cache.write().insert(*unit_id, false);
        Ok(())
    }

    fn blacklisted(&self, unit_id: &Ustr) -> Result<bool> {
        self.has_entry(unit_id)
    }

    fn all_blacklist_entries(&self) -> Result<Vec<Ustr>> {
        // Get all the entries from the database.
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("SELECT unit_id from blacklist;")
            .with_context(|| "cannot prepare statement to get all entries in blacklist DB")?; // grcov-excl-line
        let mut rows = stmt.query(params![])?;

        // Convert the rows into a vector of `Ustr` values.
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let unit_id: String = row.get(0)?;
            entries.push(Ustr::from(&unit_id));
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use r2d2_sqlite::SqliteConnectionManager;
    use tempfile::tempdir;
    use ustr::Ustr;

    use crate::blacklist::{Blacklist, BlacklistDB};

    fn new_test_blacklist() -> Result<Box<dyn Blacklist>> {
        let connection_manager = SqliteConnectionManager::memory();
        let blacklist = BlacklistDB::new(connection_manager)?;
        Ok(Box::new(blacklist))
    }

    /// Verifies checking for an element not in the blacklist.
    #[test]
    fn not_in_blacklist() -> Result<()> {
        let blacklist = new_test_blacklist()?;
        assert!(!blacklist.blacklisted(&Ustr::from("unit_id"))?);
        Ok(())
    }

    /// Verifies adding and removing an element from the blacklist.
    #[test]
    fn add_and_remove_from_blacklist() -> Result<()> {
        let mut blacklist = new_test_blacklist()?;

        let unit_id = Ustr::from("unit_id");
        blacklist.add_to_blacklist(&unit_id)?;
        assert!(blacklist.blacklisted(&unit_id)?);
        blacklist.remove_from_blacklist(&unit_id)?;
        assert!(!blacklist.blacklisted(&unit_id)?);
        Ok(())
    }

    /// Verifies the blacklist cache stores the correct values.
    #[test]
    fn blacklist_cache() -> Result<()> {
        let mut blacklist = new_test_blacklist()?;
        let unit_id = Ustr::from("unit_id");
        blacklist.add_to_blacklist(&unit_id)?;
        assert!(blacklist.blacklisted(&unit_id)?);
        // The value in the second call is retrieved from the cache.
        assert!(blacklist.blacklisted(&unit_id)?);
        // The function should return early because it's already in the cache.
        blacklist.add_to_blacklist(&unit_id)?;
        Ok(())
    }

    /// Verifies re-adding an existing entry to the blacklist.
    #[test]
    fn readd_to_blacklist() -> Result<()> {
        let mut blacklist = new_test_blacklist()?;
        let unit_id = Ustr::from("unit_id");
        blacklist.add_to_blacklist(&unit_id)?;
        assert!(blacklist.blacklisted(&unit_id)?);
        blacklist.remove_from_blacklist(&unit_id)?;
        assert!(!blacklist.blacklisted(&unit_id)?);
        blacklist.add_to_blacklist(&unit_id)?;
        assert!(blacklist.blacklisted(&unit_id)?);
        Ok(())
    }

    /// Verifies retrieving all the entries in the blacklist.
    #[test]
    fn all_entries() -> Result<()> {
        let mut blacklist = new_test_blacklist()?;
        let unit_id = Ustr::from("unit_id");
        let unit_id2 = Ustr::from("unit_id2");
        blacklist.add_to_blacklist(&unit_id)?;
        assert!(blacklist.blacklisted(&unit_id)?);
        blacklist.add_to_blacklist(&unit_id2)?;
        assert!(blacklist.blacklisted(&unit_id2)?);
        assert_eq!(blacklist.all_blacklist_entries()?, vec![unit_id, unit_id2]);
        Ok(())
    }

    /// Verifies that closing and re-opening the blacklist database preserves the blacklist.
    #[test]
    fn reopen_blacklist() -> Result<()> {
        let dir = tempdir()?;
        let mut blacklist =
            BlacklistDB::new_from_disk(dir.path().join("blacklist.db").to_str().unwrap())?;
        let unit_id = Ustr::from("unit_id");
        blacklist.add_to_blacklist(&unit_id)?;
        assert!(blacklist.blacklisted(&unit_id)?);

        let new_blacklist =
            BlacklistDB::new_from_disk(dir.path().join("blacklist.db").to_str().unwrap())?;
        assert!(new_blacklist.blacklisted(&unit_id)?);
        Ok(())
    }
}
