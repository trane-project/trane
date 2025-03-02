//! Contains utilities for working with `SQLite` databases.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::time::Duration;

/// Returns a new connection manager with the appropriate `SQLite` pragmas.
#[must_use]
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

/// Returns a new connection pool with appropriate setting.
pub fn new_connection_pool(
    connection_manager: SqliteConnectionManager,
) -> Result<r2d2::Pool<SqliteConnectionManager>, r2d2::Error> {
    let builder = Pool::builder()
        .max_size(5)
        .min_idle(Some(1))
        .connection_timeout(Duration::from_secs(5));
    builder.build(connection_manager)
}
