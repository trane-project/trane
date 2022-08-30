/// Defines the list of units to ignore during scheduling.
///
/// Users can add units to this list to prevent them from being scheduled, either because they
/// already mastered the material, or because they simply do not want to practice certain skills.
///
/// The blacklist exists for this purpose. A unit that is on it will never be scheduled. In
/// addition, the scheduler will continue the search past its dependents as if the unit was already
/// mastered. Courses, lessons, and exercises can be added to the blacklist.

#[cfg(test)]
mod test;

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

/// An implementation of `Blacklist` backed by SQLite.
pub(crate) struct BlacklistDB {
    cache: RwLock<UstrMap<bool>>,
    pool: Pool<SqliteConnectionManager>,
}

impl BlacklistDB {
    /// Returns all the migrations needed to set up the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            M::up("CREATE TABLE blacklist(unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE blacklist"),
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
            .with_context(|| "failed to initialize blacklist DB")
    }

    /// A constructor taking a connection manager.
    fn new(connection_manager: SqliteConnectionManager) -> Result<BlacklistDB> {
        let pool = Pool::new(connection_manager)?;
        let mut blacklist = BlacklistDB {
            cache: RwLock::new(UstrMap::default()),
            pool,
        };
        blacklist.init()?;
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
                connection.pragma_update(None, "journal_mode", &"WAL")?;
                connection.pragma_update(None, "synchronous", &"NORMAL")
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
            cache.insert(*unit_id, false);
            Ok(false)
        }
    }
}

impl Blacklist for BlacklistDB {
    fn add_to_blacklist(&mut self, unit_id: &Ustr) -> Result<()> {
        let has_entry = self.has_entry(unit_id)?;
        if has_entry {
            return Ok(());
        }

        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("INSERT INTO blacklist (unit_id) VALUES (?1)")
            .with_context(|| "cannot prepare statement to insert into blacklist DB")?;
        stmt.execute(params![unit_id.as_str()])
            .with_context(|| format!("cannot insert unit {} into blacklist DB", unit_id))?;
        self.cache.write().insert(*unit_id, true);
        Ok(())
    }

    fn remove_from_blacklist(&mut self, unit_id: &Ustr) -> Result<()> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("DELETE FROM blacklist WHERE unit_id = $1")
            .with_context(|| "cannot prepare statement to delete from blacklist DB")?;
        stmt.execute(params![unit_id.as_str()])
            .with_context(|| format!("cannot remove unit {} from blacklist DB", unit_id))?;
        self.cache.write().insert(*unit_id, false);
        Ok(())
    }

    fn blacklisted(&self, unit_id: &Ustr) -> Result<bool> {
        self.has_entry(unit_id)
    }

    fn all_blacklist_entries(&self) -> Result<Vec<Ustr>> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("SELECT unit_id from blacklist;")
            .with_context(|| "cannot prepare statement to get all entries in blacklist DB")?;
        let mut rows = stmt.query(params![])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let unit_id: String = row.get(0)?;
            entries.push(Ustr::from(&unit_id));
        }
        Ok(entries)
    }
}
