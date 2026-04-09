// Cards: CRUD Tauri commands
use crate::db_state::DbState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    pub key_insights: Vec<String>,
    pub quotes: Vec<String>,
    pub tags: Vec<String>,
    pub action_items: Vec<String>,
    pub topic_keys: Vec<String>,
    pub topic_fingerprint: Option<String>,
    pub source_items: Vec<String>,   // inbox_item ids
    pub file_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CardConnection {
    pub card_id: String,
    pub related_card_id: String,
    pub shared_tags: Vec<String>,
    // Populated by join when fetching
    pub related_title: Option<String>,
    pub related_summary: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn json_to_vec(s: Option<String>) -> Vec<String> {
    s.and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
        .unwrap_or_default()
}

fn row_to_card(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get(0)?,
        title: row.get(1)?,
        summary: row.get(2)?,
        key_insights: json_to_vec(row.get(3)?),
        quotes: json_to_vec(row.get(4)?),
        tags: json_to_vec(row.get(5)?),
        action_items: json_to_vec(row.get(6)?),
        topic_keys: json_to_vec(row.get(7)?),
        topic_fingerprint: row.get(8)?,
        source_items: json_to_vec(row.get(9)?),
        file_path: row.get(10)?,
        created_at: row.get(11)?,
    })
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_cards(state: State<'_, DbState>) -> Result<Vec<Card>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, summary, key_insights, quotes, tags, action_items,
                    topic_keys, topic_fingerprint, source_items, file_path, created_at
             FROM cards ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let mut cards = Vec::new();
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if let Ok(card) = row_to_card(row) {
            cards.push(card);
        }
    }
    Ok(cards)
}

#[tauri::command]
pub fn get_card(state: State<'_, DbState>, id: String) -> Result<Card, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, title, summary, key_insights, quotes, tags, action_items,
                topic_keys, topic_fingerprint, source_items, file_path, created_at
         FROM cards WHERE id = ?1",
        params![id],
        row_to_card,
    )
    .map_err(|e| format!("Card not found: {e}"))
}

#[tauri::command]
pub fn delete_card(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM cards WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_card_connections(
    state: State<'_, DbState>,
    card_id: String,
) -> Result<Vec<CardConnection>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT cc.card_id, cc.related_card_id, cc.shared_tags, c.title, c.summary
             FROM card_connections cc
             JOIN cards c ON c.id = cc.related_card_id
             WHERE cc.card_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let mut connections = Vec::new();
    let mut rows = stmt.query(params![card_id]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        connections.push(CardConnection {
            card_id: row.get(0).map_err(|e| e.to_string())?,
            related_card_id: row.get(1).map_err(|e| e.to_string())?,
            shared_tags: json_to_vec(row.get(2).map_err(|e| e.to_string())?),
            related_title: row.get(3).map_err(|e| e.to_string())?,
            related_summary: row.get(4).map_err(|e| e.to_string())?,
        });
    }
    Ok(connections)
}

/// Returns all inbox items that are associated with a card
#[tauri::command]
pub fn get_card_sources(
    state: State<'_, DbState>,
    card_id: String,
) -> Result<Vec<crate::inbox::InboxItem>, String> {
    let source_item_ids = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let source_items_json: Option<String> = conn
            .query_row(
                "SELECT source_items FROM cards WHERE id = ?1",
                params![card_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Card not found: {e}"))?;
        json_to_vec(source_items_json)
    };

    if source_item_ids.is_empty() {
        return Ok(vec![]);
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    for item_id in &source_item_ids {
        if let Ok(item) = conn.query_row(
            "SELECT id, source_id, source_name, file_path, title, url, status, word_count, ingested_at
             FROM inbox_items WHERE id = ?1",
            params![item_id],
            |row| {
                Ok(crate::inbox::InboxItem {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    source_name: row.get(2)?,
                    file_path: row.get(3)?,
                    title: row.get(4)?,
                    url: row.get(5)?,
                    status: row.get(6)?,
                    word_count: row.get(7)?,
                    ingested_at: row.get(8)?,
                })
            },
        ) {
            items.push(item);
        }
    }
    Ok(items)
}
