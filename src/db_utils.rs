//! Contains utilities for working with SQLite databases.

use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

/// Returns a new connection manager with the appropriate SQLite pragmas.
pub fn new_connection_manager(db_path: &str) -> SqliteConnectionManager {
    SqliteConnectionManager::file(db_path).with_init(
        |connection: &mut Connection| -> Result<(), rusqlite::Error> {
            // The following pragma statements are set to improve the read and write performance
            // of SQLite. See the SQLite [docs](https://www.sqlite.org/pragma.html) for more
            // information.
            connection.pragma_update(None, "journal_mode", "WAL")?;
            connection.pragma_update(None, "synchronous", "NORMAL")
        },
    )
}
