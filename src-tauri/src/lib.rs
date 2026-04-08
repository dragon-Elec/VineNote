// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod db;
mod db_state;
mod inbox;
mod read_dir_recursive;
mod rss;
mod search_keyword;
mod sources;
mod tags;

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
            let conn = db::init_db(&db_path)
                .map_err(|e| format!("DB init failed: {e}"))?;
            app.manage(DbState {
                conn: Mutex::new(conn),
                inbox_dir,
            });
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
