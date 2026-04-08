// Sources: CRUD and collection logic
use crate::db_state::DbState;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::io::Write;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Source {
    pub id: String,
    pub source_type: String,
    pub name: String,
    pub url: Option<String>,
    pub config: Option<String>,
    pub active: bool,
    pub last_fetch: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewSource {
    pub name: String,
    pub source_type: String,
    pub url: Option<String>,
    pub config: Option<String>,
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_sources(state: State<'_, DbState>) -> Result<Vec<Source>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, source_type, name, url, config, active, last_fetch, created_at
             FROM sources ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let mut sources = Vec::new();
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        sources.push(Source {
            id: row.get(0).map_err(|e| e.to_string())?,
            source_type: row.get(1).map_err(|e| e.to_string())?,
            name: row.get(2).map_err(|e| e.to_string())?,
            url: row.get(3).map_err(|e| e.to_string())?,
            config: row.get(4).map_err(|e| e.to_string())?,
            active: row.get::<_, i32>(5).map_err(|e| e.to_string())? != 0,
            last_fetch: row.get(6).map_err(|e| e.to_string())?,
            created_at: row.get(7).map_err(|e| e.to_string())?,
        });
    }
    Ok(sources)
}

#[tauri::command]
pub fn add_source(
    state: State<'_, DbState>,
    name: String,
    source_type: String,
    url: Option<String>,
    config: Option<String>,
) -> Result<Source, String> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    let source = Source {
        id: id.clone(),
        source_type: source_type.clone(),
        name: name.clone(),
        url: url.clone(),
        config: config.clone(),
        active: true,
        last_fetch: None,
        created_at: created_at.clone(),
    };

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO sources (id, source_type, name, url, config, active, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
        params![id, source_type, name, url, config, created_at],
    )
    .map_err(|e| e.to_string())?;

    Ok(source)
}

#[tauri::command]
pub fn delete_source(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM sources WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_source_active(
    state: State<'_, DbState>,
    id: String,
    active: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE sources SET active = ?1 WHERE id = ?2",
        params![active as i32, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Trigger collection for a single RSS source. Returns count of new items.
#[tauri::command]
pub async fn collect_source(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    id: String,
) -> Result<usize, String> {
    collect_source_inner(&app, state.inner(), id).await
}

/// Internal helper — usable from both the command and collect_all_sources
async fn collect_source_inner(
    app: &tauri::AppHandle,
    state: &DbState,
    id: String,
) -> Result<usize, String> {
    // 1. Load source info (sync, fast)
    let source = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, source_type, name, url, config, active, last_fetch, created_at
                 FROM sources WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        stmt.query_row(params![id], |row| {
            Ok(Source {
                id: row.get(0)?,
                source_type: row.get(1)?,
                name: row.get(2)?,
                url: row.get(3)?,
                config: row.get(4)?,
                active: row.get::<_, i32>(5)? != 0,
                last_fetch: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
    };

    if !source.active {
        return Ok(0);
    }

    // 2. Dispatch to appropriate collector
    let count = match source.source_type.as_str() {
        "rss" | "podcast" => {
            let url = source.url.clone().ok_or("Source has no URL")?;
            collect_rss(&app, &state, &source, &url).await?
        }
        "manual" => 0, // Manual sources are added via add_inbox_item directly
        _ => return Err(format!("Unsupported source type: {}", source.source_type)),
    };

    // 3. Update last_fetch timestamp
    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sources SET last_fetch = ?1 WHERE id = ?2",
            params![now, source.id],
        )
        .map_err(|e| e.to_string())?;
    }

    // 4. Emit event to frontend
    use tauri::Emitter;
    let _ = app.emit("inbox-updated", ());

    Ok(count)
}

/// Collect from all active sources. Returns total new items count.
#[tauri::command]
pub async fn collect_all_sources(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
) -> Result<usize, String> {
    let source_ids: Vec<String> = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id FROM sources WHERE active = 1")
            .map_err(|e| e.to_string())?;
        let mut ids = Vec::new();
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            ids.push(row.get::<_, String>(0).map_err(|e| e.to_string())?);
        }
        ids
    };

    let mut total = 0usize;
    for id in source_ids {
        match collect_source_inner(&app, state.inner(), id).await {
            Ok(n) => total += n,
            Err(e) => eprintln!("collect_source error: {}", e),
        }
    }
    Ok(total)
}

// ── Internal RSS collector ────────────────────────────────────────────────────

async fn collect_rss(
    _app: &tauri::AppHandle,
    state: &DbState,
    source: &Source,
    url: &str,
) -> Result<usize, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}: {}", response.status(), url));
    }

    let xml = response
        .text()
        .await
        .map_err(|e| format!("Read error: {e}"))?;

    let items = parse_rss_items(&xml)?;
    let inbox_dir = state.inbox_dir.clone();
    let mut new_count = 0usize;

    for item in &items {
        // Dedup by URL
        let already_exists = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                    params![item.link],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            count > 0
        };

        if already_exists {
            continue;
        }

        // Build markdown content
        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();
        let safe_title = item
            .title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '_' })
            .collect::<String>()
            .trim()
            .chars()
            .take(60)
            .collect::<String>();
        let file_name = format!("{}-{}.md", &item_id[..8], safe_title.replace(' ', "-"));
        let file_path = inbox_dir.join(&file_name);

        let description = html2text(&item.description);
        let word_count = description.split_whitespace().count() as i64;

        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: {}\nurl: {}\npublished: {}\ningested: {}\nstatus: unread\n---\n\n{}\n",
            serde_json::to_string(&item.title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            source.source_type,
            item.link,
            item.pub_date,
            ingested_at,
            description,
        );

        // Write markdown file
        std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;
        let mut f = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
        f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;

        let file_path_str = file_path.to_string_lossy().to_string();

        // Insert into DB
        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'unread', ?7, ?8)",
                params![
                    item_id,
                    source.id,
                    source.name,
                    file_path_str,
                    item.title,
                    item.link,
                    word_count,
                    ingested_at,
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        new_count += 1;
    }

    Ok(new_count)
}

// ── RSS parsing (lightweight) ─────────────────────────────────────────────────

struct RssItem {
    title: String,
    link: String,
    description: String,
    pub_date: String,
}

fn parse_rss_items(xml: &str) -> Result<Vec<RssItem>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items: Vec<RssItem> = Vec::new();
    let mut current: Option<RssItem> = None;
    let mut current_field: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                match name.as_str() {
                    "item" | "entry" => {
                        current = Some(RssItem {
                            title: String::new(),
                            link: String::new(),
                            description: String::new(),
                            pub_date: String::new(),
                        });
                    }
                    "title" | "link" | "description" | "summary" | "pubDate" | "published"
                    | "updated" => {
                        if current.is_some() {
                            current_field = Some(name);
                        }
                    }
                    _ => {
                        current_field = None;
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref mut item) = current {
                    let text = e.unescape().unwrap_or_default().into_owned();
                    match current_field.as_deref() {
                        Some("title") => item.title = text,
                        Some("link") => item.link = text,
                        Some("description") | Some("summary") => item.description = text,
                        Some("pubDate") | Some("published") | Some("updated") => {
                            item.pub_date = text
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::CData(ref e)) => {
                if let Some(ref mut item) = current {
                    let text = String::from_utf8_lossy(e.as_ref()).into_owned();
                    match current_field.as_deref() {
                        Some("description") | Some("summary") => item.description = text,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                if (name == "item" || name == "entry") && current.is_some() {
                    if let Some(item) = current.take() {
                        if !item.title.is_empty() {
                            items.push(item);
                        }
                    }
                } else {
                    current_field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

/// Strip HTML tags from description text
fn html2text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                result.push(' ');
            }
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    // Collapse whitespace
    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
