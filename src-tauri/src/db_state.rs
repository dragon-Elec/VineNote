// Shared Tauri managed state
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct DbState {
    pub conn: Mutex<Connection>,
    pub inbox_dir: PathBuf,
}
