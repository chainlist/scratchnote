//! What `search.db` and `space.db` share: how the file is opened, logged
//! ahead and versioned.

use std::path::Path;

use rusqlite::Connection;

use crate::Result;

/// The database at `path`, its folder made first if it is not there.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Connection::open(path)?)
}

/// Write ahead to a log, synced as `synchronous` says: `NORMAL` or `FULL`.
pub fn write_ahead(conn: &Connection, synchronous: &str) -> Result<()> {
    conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    conn.pragma_update(None, "synchronous", synchronous)?;
    Ok(())
}

/// The version the tables were made at, 0 for a new file.
pub fn version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

pub fn set_version(conn: &Connection, version: i64) -> Result<()> {
    conn.pragma_update(None, "user_version", version)?;
    Ok(())
}
