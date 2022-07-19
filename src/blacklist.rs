//! Module defining a blacklist of units that should not be shown to the user.
#[cfg(test)]
mod test;

use std::collections::{hash_map::Entry, HashMap};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};

/// An interface to store and read the list of units which should be skipped during scheduling.
pub trait Blacklist {
    /// Adds the given unit to the list of blacklisted units.
    fn add_unit(&mut self, unit_id: &str) -> Result<()>;

    /// Removes the given unit from the list of blacklisted units. Do nothing if the unit is not
    /// already in the list.
    fn remove_unit(&mut self, unit_id: &str) -> Result<()>;

    /// Returns whether the given unit should be skipped during scheduling.
    fn blacklisted(&self, unit_id: &str) -> Result<bool>;

    /// Returns the list of blacklisted units.
    fn all_entries(&self) -> Result<Vec<String>>;
}

/// An implementation of BlackList backed by SQLite.
pub(crate) struct BlackListDB {
    cache: Mutex<HashMap<String, bool>>,
    pool: Pool<SqliteConnectionManager>,
}

impl BlackListDB {
    /// Returns all the migrations needed to setup the database.
    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![
            M::up("CREATE TABLE blacklist(unit_id TEXT NOT NULL UNIQUE);")
                .down("DROP TABLE blacklist"),
            M::up("CREATE INDEX unit_id_index ON blacklist (unit_id);")
                .down("DROP INDEX unit_scores"),
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

    /// A constructor taking a SQLite connection.
    fn new(connection_manager: SqliteConnectionManager) -> Result<BlackListDB> {
        let pool = Pool::new(connection_manager)?;
        let mut blacklist = BlackListDB {
            cache: Mutex::new(HashMap::new()),
            pool,
        };
        blacklist.init()?;
        Ok(blacklist)
    }

    /// A constructor taking the path to the database file.
    pub fn new_from_disk(db_path: &str) -> Result<BlackListDB> {
        let connection_manager = SqliteConnectionManager::file(db_path).with_init(
            |connection: &mut Connection| -> Result<(), rusqlite::Error> {
                connection.pragma_update(None, "journal_mode", &"WAL")?;
                connection.pragma_update(None, "synchronous", &"NORMAL")
            },
        );
        Self::new(connection_manager)
    }

    /// Returns whether there's an entry for the given unit in the blacklist.
    fn has_entry(&self, unit_id: &str) -> Result<bool> {
        if let Entry::Occupied(o) = self.cache.lock().entry(unit_id.to_string()) {
            return Ok(*o.get());
        }

        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("SELECT * from blacklist WHERE unit_id = ?1;")
            .with_context(|| "cannot prepare statement to query blacklist DB")?;

        let mut rows = stmt.query(params![unit_id])?;
        let next = rows.next();
        if next.is_err() {
            return Err(anyhow!(
                "error looking for unit {} in the blacklist",
                unit_id
            ));
        }

        match next.unwrap() {
            None => {
                self.cache.lock().insert(unit_id.to_string(), false);
                Ok(false)
            }
            Some(_) => {
                self.cache.lock().insert(unit_id.to_string(), true);
                Ok(true)
            }
        }
    }
}

impl Blacklist for BlackListDB {
    fn add_unit(&mut self, unit_id: &str) -> Result<()> {
        let has_entry = self.has_entry(unit_id)?;
        if has_entry {
            return Ok(());
        }

        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("INSERT INTO blacklist (unit_id) VALUES (?1)")
            .with_context(|| "cannot prepare statement to insert into blacklist DB")?;
        stmt.execute(params![unit_id])
            .with_context(|| format!("cannot insert unit {} into blacklist DB", unit_id))?;
        self.cache.lock().insert(unit_id.to_string(), true);
        Ok(())
    }

    fn remove_unit(&mut self, unit_id: &str) -> Result<()> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("DELETE FROM blacklist WHERE unit_id = $1")
            .with_context(|| "cannot prepare statement to delete from blacklist DB")?;
        stmt.execute(params![unit_id])
            .with_context(|| format!("cannot remove unit {} from blacklist DB", unit_id))?;
        self.cache.lock().insert(unit_id.to_string(), false);
        Ok(())
    }

    fn blacklisted(&self, unit_id: &str) -> Result<bool> {
        self.has_entry(unit_id)
    }

    fn all_entries(&self) -> Result<Vec<String>> {
        let connection = self.pool.get()?;
        let mut stmt = connection
            .prepare_cached("SELECT unit_id from blacklist;")
            .with_context(|| "cannot prepare statement to get all entries in blacklist DB")?;
        let mut rows = stmt.query(params![])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let unit_id = row.get(0)?;
            entries.push(unit_id);
        }
        Ok(entries)
    }
}
