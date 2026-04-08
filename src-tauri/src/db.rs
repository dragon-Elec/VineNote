// Database initialization and schema management
use rusqlite::{Connection, Result};
use std::path::Path;

pub const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS sources (
        id          TEXT PRIMARY KEY,
        source_type TEXT NOT NULL,
        name        TEXT NOT NULL,
        url         TEXT,
        config      TEXT,
        active      INTEGER NOT NULL DEFAULT 1,
        last_fetch  TEXT,
        created_at  TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS inbox_items (
        id          TEXT PRIMARY KEY,
        source_id   TEXT REFERENCES sources(id) ON DELETE SET NULL,
        source_name TEXT,
        file_path   TEXT NOT NULL,
        title       TEXT NOT NULL DEFAULT '',
        url         TEXT,
        status      TEXT NOT NULL DEFAULT 'unread',
        word_count  INTEGER,
        ingested_at TEXT NOT NULL
    );
";

pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}
