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
        "bookmark" => collect_bookmarks(&state, &source).await?,
        "youtube" => collect_youtube(&app, &state, &source).await?,
        "bilibili" => collect_bilibili(&app, &state, &source).await?,
        "reddit" => collect_reddit(&app, &state, &source).await?,
        "twitter" => collect_twitter(&app, &state, &source).await?,
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

/// Scheduler-callable version: takes &AppHandle and &DbState directly.
pub async fn collect_all_sources_raw(app: &tauri::AppHandle, state: &DbState) -> usize {
    let source_ids: Vec<String> = {
        let Ok(conn) = state.conn.lock() else { return 0 };
        let Ok(mut stmt) = conn.prepare("SELECT id FROM sources WHERE active = 1") else { return 0 };
        let Ok(mut rows) = stmt.query([]) else { return 0 };
        let mut ids = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            if let Ok(id) = row.get::<_, String>(0) {
                ids.push(id);
            }
        }
        ids
    };

    let mut total = 0usize;
    for id in source_ids {
        match collect_source_inner(app, state, id).await {
            Ok(n) => total += n,
            Err(e) => eprintln!("[scheduler] collect error: {e}"),
        }
    }
    total
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
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'unread', ?7, ?8, ?9)",
                params![
                    item_id,
                    source.id,
                    source.name,
                    file_path_str,
                    item.title,
                    item.link,
                    word_count,
                    ingested_at,
                    source.source_type,
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

// ── Bookmark collector ────────────────────────────────────────────────────────

struct BookmarkEntry {
    title: String,
    url: String,
}

/// Collect bookmarks: auto-detect Chromium browser, or read from HTML export path stored in source.url.
async fn collect_bookmarks(state: &DbState, source: &Source) -> Result<usize, String> {
    let entries = match source.url.as_deref() {
        Some(path) if !path.is_empty() => parse_html_bookmarks(path)?,
        _ => auto_detect_chromium_bookmarks()?,
    };

    let inbox_dir = state.inbox_dir.clone();
    std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;

    let mut new_count = 0usize;

    for entry in &entries {
        // Only handle http(s) URLs
        if !entry.url.starts_with("http") {
            continue;
        }

        // Dedup by URL
        let already_exists = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                    params![entry.url],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            count > 0
        };

        if already_exists {
            continue;
        }

        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();

        let safe_title: String = entry
            .title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '_' })
            .take(60)
            .collect::<String>()
            .trim()
            .to_string();
        let file_name = format!("{}-{}.md", &item_id[..8], safe_title.replace(' ', "-"));
        let file_path = inbox_dir.join(&file_name);

        // Minimal stub — reader_agent will fetch full text via the URL field
        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: bookmark\nurl: {}\ningested: {}\nstatus: unread\n---\n\n*(Full content will be fetched automatically)*\n",
            serde_json::to_string(&entry.title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            entry.url,
            ingested_at,
        );

        let mut f = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
        f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
        let file_path_str = file_path.to_string_lossy().to_string();

        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'unread', 0, ?7, ?8)",
                params![
                    item_id,
                    source.id,
                    source.name,
                    file_path_str,
                    entry.title,
                    entry.url,
                    ingested_at,
                    source.source_type,
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        new_count += 1;
    }

    Ok(new_count)
}

/// Try common Chromium browser paths in order; return entries from the first found file.
fn auto_detect_chromium_bookmarks() -> Result<Vec<BookmarkEntry>, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        // macOS
        format!("{home}/Library/Application Support/Google/Chrome/Default/Bookmarks"),
        format!("{home}/Library/Application Support/BraveSoftware/Brave-Browser/Default/Bookmarks"),
        format!("{home}/Library/Application Support/Microsoft Edge/Default/Bookmarks"),
        format!("{home}/Library/Application Support/Chromium/Default/Bookmarks"),
        // Linux
        format!("{home}/.config/google-chrome/Default/Bookmarks"),
        format!("{home}/.config/chromium/Default/Bookmarks"),
        format!("{home}/.config/brave-browser/Default/Bookmarks"),
        format!("{home}/.config/microsoft-edge/Default/Bookmarks"),
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return parse_chrome_bookmarks(path);
        }
    }

    Err("No Chromium-based browser bookmark file found. Please export bookmarks to an HTML file and provide the path.".to_string())
}

/// Parse Chrome / Brave / Edge JSON bookmark file.
fn parse_chrome_bookmarks(path: &str) -> Result<Vec<BookmarkEntry>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Cannot read bookmarks: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid bookmark JSON: {e}"))?;

    let mut entries = Vec::new();
    if let Some(roots) = json.get("roots") {
        for key in &["bookmark_bar", "other", "synced"] {
            if let Some(node) = roots.get(key) {
                collect_chrome_nodes(node, &mut entries);
            }
        }
    }
    Ok(entries)
}

fn collect_chrome_nodes(node: &serde_json::Value, entries: &mut Vec<BookmarkEntry>) {
    match node.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "url" => {
            let title = node
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let url = node
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            if !url.is_empty() {
                entries.push(BookmarkEntry { title, url });
            }
        }
        "folder" => {
            if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    collect_chrome_nodes(child, entries);
                }
            }
        }
        _ => {}
    }
}

/// Parse Netscape Bookmark File format (exported by all browsers).
/// Lines look like:  <DT><A HREF="https://..." ADD_DATE="...">Title</A>
fn parse_html_bookmarks(path: &str) -> Result<Vec<BookmarkEntry>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read bookmark HTML: {e}"))?;

    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        // Only process lines that contain an anchor tag
        if !lower.contains("<a ") && !lower.contains("<a\t") {
            continue;
        }

        let url = extract_html_attr(trimmed, "href");
        let title = extract_tag_inner(trimmed, "a");

        if let Some(url) = url {
            if url.starts_with("http") {
                entries.push(BookmarkEntry {
                    title: title.unwrap_or_else(|| url.clone()),
                    url,
                });
            }
        }
    }

    Ok(entries)
}

/// Extract an attribute value like `href="..."` from an HTML tag string.
fn extract_html_attr(line: &str, attr: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let search = format!("{}=\"", attr);
    let pos = lower.find(&search)?;
    let start = pos + search.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// Extract text content between opening and closing HTML tag.
fn extract_tag_inner(line: &str, tag: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let open_end = lower.find('>')? + 1;
    let close = format!("</{}", tag);
    let close_pos = lower[open_end..].find(&close)? + open_end;
    let text = line[open_end..close_pos].trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Sanitise a string to safe filename chars, max 60 chars.
fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '_' })
        .take(60)
        .collect::<String>()
        .trim()
        .replace(' ', "-")
}

// ── YouTube collector ─────────────────────────────────────────────────────────

/// Collect a YouTube channel (by @handle, /channel/ID, /user/...) or a single video URL.
/// Uses YouTube's public Atom feed — no API key required.
async fn collect_youtube(
    _app: &tauri::AppHandle,
    state: &DbState,
    source: &Source,
) -> Result<usize, String> {
    let url = source.url.as_deref().ok_or("YouTube source requires a URL")?;

    // Single video: add directly as one inbox item
    if let Some(video_id) = crate::reader_agent::extract_youtube_video_id(url) {
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let already = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                params![video_url],
                |row| row.get(0),
            ).unwrap_or(0);
            count > 0
        };
        if already { return Ok(0); }
        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();
        let title = format!("YouTube video {}", video_id);
        let file_name = format!("{}-youtube-{}.md", &item_id[..8], video_id);
        let file_path = state.inbox_dir.join(&file_name);
        std::fs::create_dir_all(&state.inbox_dir).map_err(|e| e.to_string())?;
        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: youtube\nurl: {}\ningested: {}\nstatus: unread\n---\n\n*(Transcript will be fetched automatically)*\n",
            serde_json::to_string(&title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            video_url,
            ingested_at,
        );
        std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;
        let file_path_str = file_path.to_string_lossy().to_string();
        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type) VALUES (?1,?2,?3,?4,?5,?6,'unread',0,?7,'youtube')",
                params![item_id, source.id, source.name, file_path_str, title, video_url, ingested_at],
            ).map_err(|e| e.to_string())?;
        }
        return Ok(1);
    }

    // Channel: extract channel_id → fetch Atom feed
    let channel_id = extract_youtube_channel_id(url).await?;
    let feed_url = format!(
        "https://www.youtube.com/feeds/videos.xml?channel_id={}",
        channel_id
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&feed_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("YouTube feed HTTP {}", resp.status()));
    }
    let xml = resp.text().await.map_err(|e| e.to_string())?;
    let entries = parse_youtube_atom_feed(&xml)?;

    let inbox_dir = state.inbox_dir.clone();
    std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;
    let mut new_count = 0usize;

    for entry in &entries {
        let already = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                params![entry.link],
                |row| row.get(0),
            ).unwrap_or(0);
            count > 0
        };
        if already { continue; }

        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();
        let file_name = format!("{}-{}.md", &item_id[..8], safe_filename(&entry.title));
        let file_path = inbox_dir.join(&file_name);

        let desc = if entry.description.is_empty() {
            "*(Transcript will be fetched automatically)*".to_string()
        } else {
            entry.description.chars().take(500).collect()
        };
        let word_count = desc.split_whitespace().count() as i64;
        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: youtube\nurl: {}\npublished: {}\ningested: {}\nstatus: unread\n---\n\n{}\n",
            serde_json::to_string(&entry.title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            entry.link,
            entry.published,
            ingested_at,
            desc,
        );
        std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;
        let file_path_str = file_path.to_string_lossy().to_string();

        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type) VALUES (?1,?2,?3,?4,?5,?6,'unread',?7,?8,'youtube')",
                params![item_id, source.id, source.name, file_path_str, entry.title, entry.link, word_count, ingested_at],
            ).map_err(|e| e.to_string())?;
        }
        new_count += 1;
    }

    Ok(new_count)
}

/// Extract YouTube channel ID (UCxxxxxx) from a channel URL.
/// Fetches the page if necessary to find the externalId.
async fn extract_youtube_channel_id(url: &str) -> Result<String, String> {
    // Direct /channel/UC... format
    if let Some(idx) = url.find("/channel/") {
        let after = &url[idx + "/channel/".len()..];
        let id: String = after.chars().take_while(|&c| c != '/' && c != '?' && c != '#').collect();
        if id.len() > 10 && id.starts_with("UC") {
            return Ok(id);
        }
    }

    // @handle or /user/ or /c/ — need to fetch the page
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|e| format!("YouTube channel fetch: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("YouTube channel HTTP {}", resp.status()));
    }
    let html = resp.text().await.map_err(|e| e.to_string())?;

    // 1. Try og:url or canonical <link> meta tags — most reliable
    //    <meta property="og:url" content="https://www.youtube.com/channel/UCXXXXXX">
    //    <link rel="canonical" href="https://www.youtube.com/channel/UCXXXXXX">
    for pattern in &[
        "og:url\" content=\"https://www.youtube.com/channel/",
        "og:url\" content='https://www.youtube.com/channel/",
        "canonical\" href=\"https://www.youtube.com/channel/",
        "canonical\" href='https://www.youtube.com/channel/",
    ] {
        if let Some(pos) = html.find(pattern) {
            let after = &html[pos + pattern.len()..];
            let id: String = after.chars().take_while(|&c| c != '"' && c != '\'' && c != '/').collect();
            if id.len() > 10 && id.starts_with("UC") {
                return Ok(id);
            }
        }
    }

    // 2. Fallback: search embedded JSON for known channel ID keys
    for marker in &["\"externalId\":\"", "\"channelId\":\"", "\"ucid\":\"", "\"browseId\":\"UC"] {
        let search = if marker.ends_with("UC") { &marker[..marker.len()-2] } else { *marker };
        if let Some(pos) = html.find(search) {
            let after = &html[pos + search.len()..];
            let id: String = after.chars().take_while(|&c| c != '"').collect();
            let id = if id.starts_with("UC") { id } else { format!("UC{}", id) };
            if id.len() > 10 && id.starts_with("UC") {
                return Ok(id);
            }
        }
    }

    Err(format!("Could not find YouTube channel ID in page: {}", url))
}

/// YouTube Atom feed entry.
struct YoutubeEntry {
    title: String,
    link: String,
    published: String,
    description: String,
}

/// Parse YouTube's Atom channel feed (https://www.youtube.com/feeds/videos.xml?channel_id=...).
fn parse_youtube_atom_feed(xml: &str) -> Result<Vec<YoutubeEntry>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut entries: Vec<YoutubeEntry> = Vec::new();
    let mut current: Option<YoutubeEntry> = None;
    let mut current_field: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let raw = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                // Strip XML namespace prefix (e.g., "yt:videoId" → "videoId")
                let local = raw.split(':').last().unwrap_or(&raw).to_string();

                match raw.as_str() {
                    "entry" => {
                        current = Some(YoutubeEntry {
                            title: String::new(),
                            link: String::new(),
                            published: String::new(),
                            description: String::new(),
                        });
                        current_field = None;
                    }
                    "link" => {
                        if let Some(ref mut entry) = current {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"href" {
                                    if let Ok(v) = attr.unescape_value() {
                                        if entry.link.is_empty() {
                                            entry.link = v.into_owned();
                                        }
                                    }
                                }
                            }
                        }
                        current_field = None;
                    }
                    _ if current.is_some() => {
                        current_field = match local.as_str() {
                            "videoId" | "title" | "published" | "description" => Some(local),
                            _ => None,
                        };
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref mut entry) = current {
                    if let Ok(text) = e.unescape() {
                        match current_field.as_deref() {
                            Some("title") => entry.title = text.into_owned(),
                            Some("videoId") => {
                                let id = text.into_owned();
                                if entry.link.is_empty() {
                                    entry.link = format!("https://www.youtube.com/watch?v={}", id);
                                }
                            }
                            Some("published") => entry.published = text.into_owned(),
                            Some("description") => {
                                if entry.description.is_empty() {
                                    entry.description = text.into_owned();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                current_field = None;
                let name_bytes = e.name();
                let raw = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                if raw == "entry" {
                    if let Some(entry) = current.take() {
                        if !entry.title.is_empty() && !entry.link.is_empty() {
                            entries.push(entry);
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(entries)
}

// ── Bilibili collector ───────────────────────────────────────────────────────

/// Collect videos from a Bilibili UP主 (https://space.bilibili.com/{uid}).
/// Uses Bilibili's public space API; falls back gracefully if blocked.
async fn collect_bilibili(
    _app: &tauri::AppHandle,
    state: &DbState,
    source: &Source,
) -> Result<usize, String> {
    let url = source.url.as_deref().ok_or("Bilibili source requires a URL")?;
    let uid = extract_bilibili_uid(url)?;

    let api_url = format!(
        "https://api.bilibili.com/x/space/arc/search?mid={}&pn=1&ps=20&order=pubdate",
        uid
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&api_url)
        .header("Referer", "https://www.bilibili.com")
        .send()
        .await
        .map_err(|e| format!("Bilibili API error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Bilibili API HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp.json().await
        .map_err(|e| format!("Bilibili JSON parse: {e}"))?;

    if data["code"].as_i64() != Some(0) {
        let msg = data["message"].as_str().unwrap_or("unknown");
        return Err(format!("Bilibili API: code={}, message={msg}", data["code"]));
    }

    let vlist = data["data"]["list"]["vlist"]
        .as_array()
        .ok_or("Bilibili API: no vlist in response")?;

    let inbox_dir = state.inbox_dir.clone();
    std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;
    let mut new_count = 0usize;

    for video in vlist {
        let bvid = video["bvid"].as_str().unwrap_or("").to_string();
        let title = video["title"].as_str().unwrap_or("").to_string();
        let description = video["description"].as_str().unwrap_or("").to_string();
        let created = video["created"].as_i64().unwrap_or(0);

        if bvid.is_empty() || title.is_empty() { continue; }

        let video_url = format!("https://www.bilibili.com/video/{}", bvid);

        let already = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                params![video_url],
                |row| row.get(0),
            ).unwrap_or(0);
            count > 0
        };
        if already { continue; }

        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();
        let pub_date = if created > 0 {
            chrono::DateTime::from_timestamp(created, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| ingested_at.clone())
        } else {
            ingested_at.clone()
        };

        let body = if description.is_empty() {
            "*(Full content will be fetched automatically)*".to_string()
        } else {
            description.chars().take(500).collect()
        };
        let word_count = body.split_whitespace().count() as i64;

        let file_name = format!("{}-{}.md", &item_id[..8], safe_filename(&title));
        let file_path = inbox_dir.join(&file_name);
        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: bilibili\nurl: {}\npublished: {}\ningested: {}\nstatus: unread\n---\n\n{}\n",
            serde_json::to_string(&title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            video_url, pub_date, ingested_at, body,
        );
        std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;
        let file_path_str = file_path.to_string_lossy().to_string();

        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type) VALUES (?1,?2,?3,?4,?5,?6,'unread',?7,?8,'bilibili')",
                params![item_id, source.id, source.name, file_path_str, title, video_url, word_count, ingested_at],
            ).map_err(|e| e.to_string())?;
        }
        new_count += 1;
    }

    Ok(new_count)
}

fn extract_bilibili_uid(url: &str) -> Result<String, String> {
    // https://space.bilibili.com/12345  or  https://space.bilibili.com/12345/video
    if let Some(idx) = url.find("space.bilibili.com/") {
        let after = &url[idx + "space.bilibili.com/".len()..];
        let uid: String = after.chars().take_while(|&c| c != '/' && c != '?').collect();
        if !uid.is_empty() && uid.chars().all(|c| c.is_ascii_digit()) {
            return Ok(uid);
        }
    }
    // Bare UID
    let s = url.trim_matches('/');
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        return Ok(s.to_string());
    }
    Err(format!("Cannot extract Bilibili UID from: {}", url))
}

// ── Reddit collector ──────────────────────────────────────────────────────────

/// Collect top posts from a subreddit using Reddit's public JSON API.
async fn collect_reddit(
    _app: &tauri::AppHandle,
    state: &DbState,
    source: &Source,
) -> Result<usize, String> {
    let url = source.url.as_deref().ok_or("Reddit source requires a URL")?;
    let subreddit = extract_subreddit_name(url)?;

    let api_url = format!(
        "https://www.reddit.com/r/{}/top.json?t=week&limit=25",
        subreddit
    );

    let client = reqwest::Client::builder()
        // Reddit requires a proper User-Agent that identifies the app
        .user_agent("VineNote:collector:v1.0 (by /u/vinenote_app)")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client.get(&api_url).send().await
        .map_err(|e| format!("Reddit API error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Reddit API HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp.json().await
        .map_err(|e| format!("Reddit JSON parse: {e}"))?;

    let children = data["data"]["children"]
        .as_array()
        .ok_or("Reddit API: no posts in response")?;

    let inbox_dir = state.inbox_dir.clone();
    std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;
    let mut new_count = 0usize;

    for child in children {
        let post = &child["data"];
        let title = post["title"].as_str().unwrap_or("").to_string();
        let permalink = post["permalink"].as_str().unwrap_or("").to_string();
        let post_url = post["url"].as_str().unwrap_or("").to_string();
        let selftext = post["selftext"].as_str().unwrap_or("").to_string();
        let is_self = post["is_self"].as_bool().unwrap_or(false);
        let score = post["score"].as_i64().unwrap_or(0);
        let num_comments = post["num_comments"].as_i64().unwrap_or(0);
        let created = post["created_utc"].as_f64().unwrap_or(0.0) as i64;
        let flair = post["link_flair_text"].as_str().unwrap_or("").to_string();

        if title.is_empty() || permalink.is_empty() { continue; }

        let canonical_url = format!("https://www.reddit.com{}", permalink.trim_end_matches('/'));

        let already = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                params![canonical_url],
                |row| row.get(0),
            ).unwrap_or(0);
            count > 0
        };
        if already { continue; }

        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();
        let pub_date = if created > 0 {
            chrono::DateTime::from_timestamp(created, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| ingested_at.clone())
        } else {
            ingested_at.clone()
        };

        // Build summary body
        let meta = if flair.is_empty() {
            format!("**↑{}** | **{} comments**", score, num_comments)
        } else {
            format!("**↑{}** | **{} comments** | {}", score, num_comments, flair)
        };

        let body = if is_self && !selftext.is_empty() && selftext != "[removed]" && selftext != "[deleted]" {
            format!("{}\n\n{}", meta, selftext.chars().take(1000).collect::<String>())
        } else if !is_self && !post_url.is_empty() {
            format!("{}\n\n[{}]({})", meta, post_url, post_url)
        } else {
            meta
        };

        // For link posts, the URL to fetch full content is the external link
        // We store the Reddit permalink as canonical but the external URL helps reader_agent
        let fetch_url = if !is_self && post_url.starts_with("http") && !post_url.contains("reddit.com") {
            post_url.clone()
        } else {
            canonical_url.clone()
        };

        let word_count = body.split_whitespace().count() as i64;
        let file_name = format!("{}-{}.md", &item_id[..8], safe_filename(&title));
        let file_path = inbox_dir.join(&file_name);
        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: reddit\nurl: {}\npublished: {}\ningested: {}\nstatus: unread\n---\n\n{}\n",
            serde_json::to_string(&title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            fetch_url, pub_date, ingested_at, body,
        );
        std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;
        let file_path_str = file_path.to_string_lossy().to_string();

        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type) VALUES (?1,?2,?3,?4,?5,?6,'unread',?7,?8,'reddit')",
                params![item_id, source.id, source.name, file_path_str, title, fetch_url, word_count, ingested_at],
            ).map_err(|e| e.to_string())?;
        }
        new_count += 1;
    }

    Ok(new_count)
}

fn extract_subreddit_name(url: &str) -> Result<String, String> {
    // https://www.reddit.com/r/programming/   or   r/programming   or   programming
    if let Some(idx) = url.find("reddit.com/r/") {
        let after = &url[idx + "reddit.com/r/".len()..];
        let sub: String = after.chars().take_while(|&c| c != '/' && c != '?').collect();
        if !sub.is_empty() { return Ok(sub); }
    }
    if let Some(rest) = url.strip_prefix("r/") {
        let sub: String = rest.chars().take_while(|&c| c != '/' && c != '?').collect();
        if !sub.is_empty() { return Ok(sub); }
    }
    // Bare subreddit name
    let s = url.trim_matches('/').to_string();
    if !s.is_empty() && !s.contains('/') { return Ok(s); }
    Err(format!("Cannot parse subreddit from: {}", url))
}

// ── Twitter/X collector ───────────────────────────────────────────────────────

/// Known public Nitter instances (RSS feeds, no auth required).
/// These may go offline; users can configure a custom instance in source config.
const NITTER_INSTANCES: &[&str] = &[
    "https://nitter.privacydev.net",
    "https://nitter.woodland.cafe",
    "https://nitter.rawbit.ninja",
    "https://nitter.1d4.us",
];

/// Collect tweets from a Twitter/X profile using Nitter RSS (no auth) or
/// official API cookies (when auth_token + ct0 are configured).
///
/// Source config JSON:
///   `{"nitter_instance": "https://nitter.example"}` — custom Nitter instance
///   `{"auth_token": "xxx", "ct0": "yyy"}` — official API (reserved for future)
async fn collect_twitter(
    _app: &tauri::AppHandle,
    state: &DbState,
    source: &Source,
) -> Result<usize, String> {
    let url = source.url.as_deref().ok_or("Twitter source requires a URL")?;
    let username = extract_twitter_username(url)?;

    let config: serde_json::Value = source.config.as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    let custom_nitter = config["nitter_instance"].as_str().unwrap_or("").to_string();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    // Build list of Nitter instances to try
    let instances: Vec<String> = if !custom_nitter.is_empty() {
        vec![custom_nitter]
    } else {
        NITTER_INSTANCES.iter().map(|s| s.to_string()).collect()
    };

    for instance in &instances {
        let rss_url = format!("{}/{}/rss", instance.trim_end_matches('/'), username);
        let resp = match client.get(&rss_url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };
        let xml = match resp.text().await {
            Ok(s) if !s.is_empty() => s,
            _ => continue,
        };
        let items = match parse_rss_items(&xml) {
            Ok(v) if !v.is_empty() => v,
            _ => continue,
        };

        return insert_twitter_items(state, source, &username, items).await;
    }

    Err(format!(
        "Twitter: could not reach any Nitter instance for @{}. \
         Add {{\"nitter_instance\": \"https://your-nitter.example\"}} to source config, \
         or use a self-hosted Nitter.",
        username
    ))
}

/// Insert parsed Nitter RSS items as Twitter inbox items.
async fn insert_twitter_items(
    state: &DbState,
    source: &Source,
    username: &str,
    items: Vec<RssItem>,
) -> Result<usize, String> {
    let inbox_dir = state.inbox_dir.clone();
    std::fs::create_dir_all(&inbox_dir).map_err(|e| e.to_string())?;
    let mut new_count = 0usize;

    for item in &items {
        // Convert Nitter URL → canonical x.com URL for dedup
        let tweet_url = nitter_to_twitter_url(&item.link, username);

        let already = {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE url = ?1",
                params![tweet_url],
                |row| row.get(0),
            ).unwrap_or(0);
            count > 0
        };
        if already { continue; }

        let item_id = Uuid::new_v4().to_string();
        let ingested_at = Utc::now().to_rfc3339();
        let body = html2text(&item.description);
        let word_count = body.split_whitespace().count() as i64;
        let file_name = format!("{}-{}.md", &item_id[..8], safe_filename(&item.title));
        let file_path = inbox_dir.join(&file_name);
        let content = format!(
            "---\ntitle: {}\nsource: {}\nsource_type: twitter\nurl: {}\npublished: {}\ningested: {}\nstatus: unread\n---\n\n{}\n",
            serde_json::to_string(&item.title).unwrap_or_default(),
            serde_json::to_string(&source.name).unwrap_or_default(),
            tweet_url, item.pub_date, ingested_at, body,
        );
        std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;
        let file_path_str = file_path.to_string_lossy().to_string();

        {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO inbox_items (id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type) VALUES (?1,?2,?3,?4,?5,?6,'unread',?7,?8,'twitter')",
                params![item_id, source.id, source.name, file_path_str, item.title, tweet_url, word_count, ingested_at],
            ).map_err(|e| e.to_string())?;
        }
        new_count += 1;
    }

    Ok(new_count)
}

/// Convert a Nitter URL back to the canonical x.com URL.
/// e.g. https://nitter.net/user/status/123 → https://x.com/user/status/123
fn nitter_to_twitter_url(url: &str, username: &str) -> String {
    let pattern = format!("/{}/status/", username);
    if let Some(pos) = url.find(&pattern) {
        return format!("https://x.com{}", &url[pos..]);
    }
    url.to_string()
}

fn extract_twitter_username(url: &str) -> Result<String, String> {
    // @username
    if let Some(rest) = url.strip_prefix('@') {
        let u: String = rest.chars().take_while(|&c| c != '/' && c != '?').collect();
        if !u.is_empty() { return Ok(u); }
    }
    // https://x.com/username  or  https://twitter.com/username
    for domain in &["x.com/", "twitter.com/"] {
        if let Some(idx) = url.find(domain) {
            let after = &url[idx + domain.len()..];
            let u: String = after.chars().take_while(|&c| c != '/' && c != '?').collect();
            if !u.is_empty() && u != "i" && u != "search" && u != "home" {
                return Ok(u);
            }
        }
    }
    // Bare username
    let s = url.trim_matches('/').trim_matches('@').to_string();
    if !s.is_empty() && !s.contains('/') && !s.contains('.') {
        return Ok(s);
    }
    Err(format!("Cannot extract Twitter username from: {}", url))
}
