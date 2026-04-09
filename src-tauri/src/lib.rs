// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod ai_state;
mod cards;
mod db;
mod db_state;
mod inbox;
mod llm;
mod read_dir_recursive;
mod reader_agent;
mod rss;
mod scheduler;
mod search_keyword;
mod sources;
mod tags;

use ai_state::AiState;
use db_state::DbState;
use read_dir_recursive::read_dir_recursive;
use rss::fetch_and_parse_rss;
use search_keyword::{search_files, SearchResult};
use std::path::Path;
use std::sync::Mutex;
use tags::TagEntry;
use tauri::AppHandle;
use tauri::Manager;
use log::{error, info};

#[tauri::command]
fn get_dir_info(path: &str) -> String {
    println!("input path: {}", path);
    let dir_info = read_dir_recursive(path);

    dir_info
}

#[tauri::command]
async fn get_rss(app: AppHandle, url: String) {
    println!("input rss url: {}", url);
    fetch_and_parse_rss(app, &url).await;
}

#[tauri::command]
async fn search_content(dir_path: String, keyword: String) -> Result<Vec<SearchResult>, String> {
  info!("Starting search in {} for keyword: {}", dir_path, keyword);
  let path = Path::new(&dir_path);
  if !path.exists() {
    return Err("Directory does not exist".to_string());
  }
  match search_files(path, &keyword) {
    Ok(results) => {
        info!("Search completed. Found {} matching files", results.len());
        Ok(results)
    }
    Err(e) => {
        error!("Search failed: {}", e);
        Err(format!("Search failed: {}", e))
    }
  }
}

#[tauri::command]
async fn get_tags_from_dir(path: String) -> Result<Vec<TagEntry>, String> {
    let dir = Path::new(&path);
    if !dir.exists() {
        return Ok(vec![]);
    }
    Ok(tags::get_tags_from_dir(dir))
}

#[tauri::command]
async fn remove_tag_from_all_files(path: String, tag_name: String) -> Result<(), String> {
    let dir = Path::new(&path);
    tags::remove_tag_from_all_files(dir, &tag_name)
}

#[tauri::command]
async fn rename_tag_in_all_files(path: String, old_name: String, new_name: String) -> Result<(), String> {
    let dir = Path::new(&path);
    tags::rename_tag_in_all_files(dir, &old_name, &new_name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize app data directory and SQLite database
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let inbox_dir = data_dir.join("inbox");
            std::fs::create_dir_all(&inbox_dir)?;
            let db_path = data_dir.join("vinenote.db");

            // Primary DB connection for Tauri commands
            let conn = db::init_db(&db_path)
                .map_err(|e| format!("DB init failed: {e}"))?;
            // Reset any items stuck in 'fetching' state from a previous crash
            let _ = conn.execute(
                "UPDATE inbox_items SET reader_status = 'pending' WHERE reader_status = 'fetching'",
                [],
            );
            app.manage(DbState {
                conn: Mutex::new(conn),
                inbox_dir: inbox_dir.clone(),
            });

            // AI config state — shared between Tauri commands and scheduler via Arc
            let ai_arc = std::sync::Arc::new(AiState::new());
            app.manage(std::sync::Arc::clone(&ai_arc));

            // Second connection for the background scheduler (WAL allows concurrent readers)
            let sched_conn = db::init_db(&db_path)
                .map_err(|e| format!("Scheduler DB init failed: {e}"))?;
            let sched_db = std::sync::Arc::new(DbState {
                conn: Mutex::new(sched_conn),
                inbox_dir,
            });

            // Start background scheduler daemon
            scheduler::start(app.handle().clone(), sched_db, ai_arc);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dir_info,
            get_rss,
            search_content,
            get_tags_from_dir,
            remove_tag_from_all_files,
            rename_tag_in_all_files,
            // Sources
            sources::list_sources,
            sources::add_source,
            sources::delete_source,
            sources::toggle_source_active,
            sources::collect_source,
            sources::collect_all_sources,
            // Inbox
            inbox::list_inbox_items,
            inbox::get_inbox_item_content,
            inbox::update_inbox_item_status,
            inbox::get_inbox_badges,
            inbox::add_inbox_item_manual,
            // Cards
            cards::list_cards,
            cards::get_card,
            cards::delete_card,
            cards::get_card_connections,
            cards::get_card_sources,
            // LLM editor proxy
            llm::llm_stream,
            llm::copilot_complete,
            // AI config
            ai_state::update_ai_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
