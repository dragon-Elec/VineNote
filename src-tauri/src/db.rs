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
        id            TEXT PRIMARY KEY,
        source_id     TEXT REFERENCES sources(id) ON DELETE SET NULL,
        source_name   TEXT,
        file_path     TEXT NOT NULL,
        title         TEXT NOT NULL DEFAULT '',
        url           TEXT,
        status        TEXT NOT NULL DEFAULT 'unread',
        word_count    INTEGER,
        ingested_at   TEXT NOT NULL,
        reader_status TEXT NOT NULL DEFAULT 'pending',
        content_source TEXT
    );

    CREATE TABLE IF NOT EXISTS cards (
        id            TEXT PRIMARY KEY,
        title         TEXT NOT NULL,
        summary       TEXT,
        key_insights  TEXT,
        quotes        TEXT,
        tags          TEXT,
        action_items  TEXT,
        topic_keys    TEXT,
        topic_fingerprint TEXT,
        source_items  TEXT,
        file_path     TEXT,
        created_at    TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS card_connections (
        card_id         TEXT NOT NULL,
        related_card_id TEXT NOT NULL,
        shared_tags     TEXT,
        PRIMARY KEY (card_id, related_card_id),
        FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE,
        FOREIGN KEY (related_card_id) REFERENCES cards(id) ON DELETE CASCADE
    );
";

pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    // Migrate existing inbox_items table: add new columns if missing
    let _ = conn.execute_batch(
        "ALTER TABLE inbox_items ADD COLUMN reader_status TEXT NOT NULL DEFAULT 'pending';",
    );
    let _ = conn.execute_batch(
        "ALTER TABLE inbox_items ADD COLUMN content_source TEXT;",
    );
    Ok(conn)
}
