// Inbox: query and status management
use crate::db_state::DbState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxItem {
    pub id: String,
    pub source_id: Option<String>,
    pub source_name: Option<String>,
    pub file_path: String,
    pub title: String,
    pub url: Option<String>,
    pub status: String,
    pub word_count: Option<i64>,
    pub ingested_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InboxBadges {
    pub unread: i64,
    pub total: i64,
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_inbox_items(
    state: State<'_, DbState>,
    status: Option<String>,
) -> Result<Vec<InboxItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let sql = if status.is_some() {
        "SELECT id, source_id, source_name, file_path, title, url, status, word_count, ingested_at
         FROM inbox_items WHERE status = ?1 ORDER BY ingested_at DESC"
    } else {
        "SELECT id, source_id, source_name, file_path, title, url, status, word_count, ingested_at
         FROM inbox_items ORDER BY ingested_at DESC"
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut items = Vec::new();

    let mut rows = if let Some(ref s) = status {
        stmt.query(rusqlite::params![s]).map_err(|e| e.to_string())?
    } else {
        stmt.query([]).map_err(|e| e.to_string())?
    };

    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        items.push(InboxItem {
            id: row.get(0).map_err(|e| e.to_string())?,
            source_id: row.get(1).map_err(|e| e.to_string())?,
            source_name: row.get(2).map_err(|e| e.to_string())?,
            file_path: row.get(3).map_err(|e| e.to_string())?,
            title: row.get(4).map_err(|e| e.to_string())?,
            url: row.get(5).map_err(|e| e.to_string())?,
            status: row.get(6).map_err(|e| e.to_string())?,
            word_count: row.get(7).map_err(|e| e.to_string())?,
            ingested_at: row.get(8).map_err(|e| e.to_string())?,
        });
    }
    Ok(items)
}

#[tauri::command]
pub fn get_inbox_item_content(id: String, state: State<'_, DbState>) -> Result<String, String> {
    let file_path = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT file_path FROM inbox_items WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| format!("Item not found: {e}"))?
    };
    std::fs::read_to_string(&file_path).map_err(|e| format!("Cannot read file: {e}"))
}

#[tauri::command]
pub fn update_inbox_item_status(
    state: State<'_, DbState>,
    id: String,
    status: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE inbox_items SET status = ?1 WHERE id = ?2",
        params![status, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_inbox_badges(state: State<'_, DbState>) -> Result<InboxBadges, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let unread: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM inbox_items WHERE status = 'unread'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM inbox_items", [], |row| row.get(0))
        .unwrap_or(0);
    Ok(InboxBadges { unread, total })
}

/// Add a manual inbox item (paste text or URL)
#[tauri::command]
pub fn add_inbox_item_manual(
    state: State<'_, DbState>,
    title: String,
    content: String,
    url: Option<String>,
    source_name: Option<String>,
) -> Result<InboxItem, String> {
    use chrono::Utc;
    use uuid::Uuid;

    let id = Uuid::new_v4().to_string();
    let ingested_at = Utc::now().to_rfc3339();
    let inbox_dir = state.inbox_dir.clone();

    std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;

    let safe_title = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '_' })
        .collect::<String>()
        .trim()
        .chars()
        .take(60)
        .collect::<String>();
    let file_name = format!("{}-{}.md", &id[..8], safe_title.replace(' ', "-"));
    let file_path = inbox_dir.join(&file_name);
    let file_path_str = file_path.to_string_lossy().to_string();

    let md_content = format!(
        "---\ntitle: {}\nsource: {}\nsource_type: manual\nurl: {}\ningested: {}\nstatus: unread\n---\n\n{}\n",
        serde_json::to_string(&title).unwrap_or_default(),
        serde_json::to_string(&source_name.clone().unwrap_or_default()).unwrap_or_default(),
        url.clone().unwrap_or_default(),
        ingested_at,
        content,
    );

    std::fs::write(&file_path, md_content).map_err(|e| e.to_string())?;

    let word_count = content.split_whitespace().count() as i64;

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at)
         VALUES (?1, NULL, ?2, ?3, ?4, ?5, 'unread', ?6, ?7)",
        params![id, source_name, file_path_str, title, url, word_count, ingested_at],
    )
    .map_err(|e| e.to_string())?;

    Ok(InboxItem {
        id,
        source_id: None,
        source_name,
        file_path: file_path_str,
        title,
        url,
        status: "unread".to_string(),
        word_count: Some(word_count),
        ingested_at,
    })
}


