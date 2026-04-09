// Reader Agent: full-text extraction + LLM card generation + connection detection
use crate::ai_state::AiState;
use crate::db_state::DbState;
use crate::inbox::InboxItem;
use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

// ── LLM response schema ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CardDraft {
    title: String,
    summary: String,
    key_insights: Vec<String>,
    quotes: Vec<String>,
    tags: Vec<String>,
    action_items: Vec<String>,
    topic_keys: Vec<String>,
    topic_fingerprint: String,
}

// ── Public entry point (called by scheduler) ─────────────────────────────────

/// Process all pending inbox items: fetch full text, generate cards.
pub async fn process_pending_items(
    app: Arc<tauri::AppHandle>,
    db: Arc<DbState>,
    ai: Arc<AiState>,
) {
    let cfg = ai.get();
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        return; // AI not configured yet
    }

    // 1. Fetch pending items
    let items = match fetch_pending_items(&db) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[reader_agent] fetch_pending_items error: {e}");
            return;
        }
    };

    for item in items {
        if let Err(e) = process_item(&app, &db, &ai, item).await {
            eprintln!("[reader_agent] process_item error: {e}");
        }
    }
}

// ── Per-item pipeline ─────────────────────────────────────────────────────────

async fn process_item(
    app: &tauri::AppHandle,
    db: &DbState,
    ai: &AiState,
    item: InboxItem,
) -> Result<(), String> {
    // Mark as "fetching" so we don't pick it up again in parallel
    set_reader_status(db, &item.id, "fetching")?;

    // Step 1: get full text (fetch URL or fall back to RSS markdown)
    let (full_text, content_source) = match item.url.as_deref() {
        Some(url) if !url.is_empty() => match fetch_full_text(url).await {
            Ok(text) if text.split_whitespace().count() >= 100 => (text, "full"),
            _ => (read_markdown_body(db, &item)?, "summary"),
        },
        _ => (read_markdown_body(db, &item)?, "summary"),
    };

    // Update markdown file with full text if we got the real article
    if content_source == "full" {
        update_markdown_body(db, &item, &full_text)?;
    }

    // Mark content_source and reader_status = ready
    {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE inbox_items SET reader_status = 'ready', content_source = ?1 WHERE id = ?2",
            params![content_source, item.id],
        )
        .map_err(|e| e.to_string())?;
    }

    // Step 2: generate card via LLM
    let cfg = ai.get();
    let draft = call_llm_for_card(&cfg, &item, &full_text).await?;

    // Step 3: write card to disk + DB
    let card_id = save_card(db, &item, draft)?;

    // Step 4: find related cards and write connections
    if let Err(e) = build_connections(db, ai, &card_id).await {
        eprintln!("[reader_agent] build_connections error: {e}");
    }

    // Mark item done
    set_reader_status(db, &item.id, "card_generated")?;

    // Notify frontend
    use tauri::Emitter;
    let _ = app.emit("card-added", card_id);

    Ok(())
}

// ── Full text extraction ──────────────────────────────────────────────────────

async fn fetch_full_text(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let html = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    extract_article_text(&html)
}

fn extract_article_text(html: &str) -> Result<String, String> {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);

    // Remove noise elements first
    // Note: scraper is immutable, so we work with selectors to skip noise during extraction

    // Priority selectors for main content
    let candidate_selectors = [
        "article",
        "[role='main']",
        "main",
        ".post-content",
        ".article-body",
        ".entry-content",
        ".article-content",
        ".post-body",
        ".content-body",
        "#article-body",
        "#main-content",
    ];

    let noise_selector = Selector::parse(
        "script, style, nav, footer, header, aside, noscript, .sidebar, .menu, .comments, .ad, .banner, .navigation"
    ).unwrap();

    for sel_str in &candidate_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(element) = doc.select(&sel).next() {
                let text = extract_text_from_element(&doc, &element, &noise_selector);
                if text.split_whitespace().count() >= 100 {
                    return Ok(text);
                }
            }
        }
    }

    // Fallback: paragraph density algorithm
    // Collect all <p> tags, join if substantial
    if let Ok(p_sel) = Selector::parse("p") {
        let paragraphs: Vec<String> = doc
            .select(&p_sel)
            .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .filter(|t| t.split_whitespace().count() > 10)
            .collect();

        if !paragraphs.is_empty() {
            return Ok(paragraphs.join("\n\n"));
        }
    }

    Err("Could not extract article content".to_string())
}

fn extract_text_from_element(
    _doc: &scraper::Html,
    element: &scraper::ElementRef,
    noise_selector: &scraper::Selector,
) -> String {
    // Collect text, skipping noise children
    let mut result = String::new();
    for node in element.descendants() {
        if let Some(el) = scraper::ElementRef::wrap(node) {
            // Skip noise elements
            if el.select(noise_selector).next().is_some() {
                continue;
            }
            let tag = el.value().name();
            if ["script", "style", "nav", "footer", "header", "aside", "noscript"].contains(&tag) {
                continue;
            }
            if ["p", "h1", "h2", "h3", "h4", "li"].contains(&tag) {
                let text: String = el.text().collect::<Vec<_>>().join(" ");
                let text = text.trim();
                if !text.is_empty() {
                    result.push_str(text);
                    result.push('\n');
                }
            }
        }
    }
    result.trim().to_string()
}

// ── Markdown helpers ──────────────────────────────────────────────────────────

fn read_markdown_body(_db: &DbState, item: &InboxItem) -> Result<String, String> {
    let content = std::fs::read_to_string(&item.file_path)
        .map_err(|e| format!("Cannot read markdown: {e}"))?;
    // Strip frontmatter
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("\n---") {
            return Ok(content[3 + end + 4..].trim().to_string());
        }
    }
    Ok(content)
}

fn update_markdown_body(_db: &DbState, item: &InboxItem, body: &str) -> Result<(), String> {
    let old = std::fs::read_to_string(&item.file_path)
        .map_err(|e| format!("Cannot read markdown: {e}"))?;
    // Preserve frontmatter, replace body
    let new_content = if old.starts_with("---") {
        if let Some(end) = old[3..].find("\n---") {
            let frontmatter = &old[..3 + end + 4];
            // Update content_source in frontmatter
            let fm = frontmatter.replace(
                "status: unread",
                "status: unread\ncontent_source: full",
            );
            format!("{}\n\n{}\n", fm, body)
        } else {
            format!("{}\n\n{}\n", old, body)
        }
    } else {
        format!("{}\n\n{}\n", old, body)
    };
    std::fs::write(&item.file_path, new_content).map_err(|e| e.to_string())?;
    Ok(())
}

// ── LLM call ─────────────────────────────────────────────────────────────────

async fn call_llm_for_card(
    cfg: &crate::ai_state::AiConfig,
    item: &InboxItem,
    body: &str,
) -> Result<CardDraft, String> {
    let system = build_system_prompt(cfg);

    let user_msg = format!(
        r#"Please generate a structured knowledge card for the following article.
Return ONLY valid JSON (no markdown fences) with this exact schema:
{{
  "title": "...",
  "summary": "2-3 sentence overview of the core argument",
  "key_insights": ["insight1", "insight2", ...],
  "quotes": ["exact quote1", ...],
  "tags": ["BroadTag1", "BroadTag2"],
  "action_items": ["actionable item1", ...],
  "topic_keys": ["specific-topic-slug-1", "specific-topic-slug-2"],
  "topic_fingerprint": "One sentence describing what specific problem/topic this article addresses"
}}

Article title: {title}
Source: {source}
URL: {url}

Content:
{body}
"#,
        title = item.title,
        source = item.source_name.as_deref().unwrap_or(""),
        url = item.url.as_deref().unwrap_or(""),
        body = &body[..body.len().min(8000)],
    );

    let request_body = serde_json::json!({
        "model": cfg.reader_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user_msg }
        ],
        "temperature": 0.3,
        "max_tokens": 1500,
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/chat/completions", cfg.endpoint.trim_end_matches('/')))
        .bearer_auth(&cfg.api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM error {status}: {body}"));
    }

    #[derive(Deserialize)]
    struct LlmResponse {
        choices: Vec<LlmChoice>,
    }
    #[derive(Deserialize)]
    struct LlmChoice {
        message: LlmMessage,
    }
    #[derive(Deserialize)]
    struct LlmMessage {
        content: String,
    }

    let resp: LlmResponse = response
        .json()
        .await
        .map_err(|e| format!("LLM parse error: {e}"))?;

    let content = resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default();

    // Strip possible markdown fences
    let json_str = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<CardDraft>(json_str)
        .map_err(|e| format!("Failed to parse card JSON: {e}\nRaw: {json_str}"))
}

fn build_system_prompt(cfg: &crate::ai_state::AiConfig) -> String {
    if cfg.system_prompt.is_empty() {
        "You are a knowledge management assistant. Extract structured insights from articles to build a personal knowledge base.".to_string()
    } else {
        cfg.system_prompt.clone()
    }
}

// ── Save card ─────────────────────────────────────────────────────────────────

fn save_card(db: &DbState, item: &InboxItem, draft: CardDraft) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    let key_insights = serde_json::to_string(&draft.key_insights).unwrap_or_default();
    let quotes = serde_json::to_string(&draft.quotes).unwrap_or_default();
    let tags = serde_json::to_string(&draft.tags).unwrap_or_default();
    let action_items = serde_json::to_string(&draft.action_items).unwrap_or_default();
    let topic_keys = serde_json::to_string(&draft.topic_keys).unwrap_or_default();
    let source_items = serde_json::to_string(&[&item.id]).unwrap_or_default();

    // Write markdown card file
    let card_dir = std::path::Path::new(&item.file_path)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("cards");
    let _ = std::fs::create_dir_all(&card_dir);

    let safe_title: String = draft
        .title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '_' })
        .take(60)
        .collect::<String>()
        .trim()
        .to_string();
    let file_name = format!("{}-{}.md", &id[..8], safe_title.replace(' ', "-"));
    let file_path = card_dir.join(&file_name);
    let file_path_str = file_path.to_string_lossy().to_string();

    let md = format!(
        "---\nid: {id}\ntitle: {title}\nsource_item: {src}\ncreated: {created_at}\ntopic_fingerprint: {tf}\n---\n\n## Summary\n{summary}\n\n## Key Insights\n{insights}\n\n## Quotes\n{quotes_md}\n\n## Tags\n{tags_md}\n\n## Action Items\n{actions}\n",
        id = id,
        title = serde_json::to_string(&draft.title).unwrap_or_default(),
        src = item.id,
        created_at = created_at,
        tf = draft.topic_fingerprint,
        summary = draft.summary,
        insights = draft.key_insights.iter().map(|i| format!("- {i}")).collect::<Vec<_>>().join("\n"),
        quotes_md = draft.quotes.iter().map(|q| format!("> {q}")).collect::<Vec<_>>().join("\n\n"),
        tags_md = draft.tags.join(", "),
        actions = draft.action_items.iter().map(|a| format!("- [ ] {a}")).collect::<Vec<_>>().join("\n"),
    );
    let _ = std::fs::write(&file_path, md);

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO cards (id, title, summary, key_insights, quotes, tags, action_items, topic_keys, topic_fingerprint, source_items, file_path, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id, draft.title, draft.summary,
            key_insights, quotes, tags, action_items,
            topic_keys, draft.topic_fingerprint,
            source_items, file_path_str, created_at
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

// ── Connection detection ──────────────────────────────────────────────────────

async fn build_connections(
    db: &DbState,
    ai: &AiState,
    new_card_id: &str,
) -> Result<(), String> {
    // Load the new card
    let (new_keys, new_fp) = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let (keys_json, fp): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT topic_keys, topic_fingerprint FROM cards WHERE id = ?1",
                params![new_card_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let keys: Vec<String> =
            keys_json.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        (keys, fp.unwrap_or_default())
    };

    if new_keys.is_empty() || new_fp.is_empty() {
        return Ok(());
    }

    // Find candidates that share at least one topic_key
    let candidates = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, topic_keys, topic_fingerprint FROM cards WHERE id != ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![new_card_id]).map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let id: String = row.get(0).map_err(|e| e.to_string())?;
            let keys: Vec<String> = row
                .get::<_, Option<String>>(1)
                .map_err(|e| e.to_string())?
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default();
            let fp: String = row
                .get::<_, Option<String>>(2)
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            let shared: Vec<String> = new_keys.iter().filter(|k| keys.contains(k)).cloned().collect();
            if !shared.is_empty() {
                result.push((id, fp, shared));
            }
        }
        result
    };

    let cfg = ai.get();
    for (candidate_id, candidate_fp, shared_keys) in candidates {
        // LLM second-pass validation
        if is_same_topic(&cfg, &new_fp, &candidate_fp).await {
            let shared_json = serde_json::to_string(&shared_keys).unwrap_or_default();
            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            // Insert both directions
            let _ = conn.execute(
                "INSERT OR IGNORE INTO card_connections (card_id, related_card_id, shared_tags) VALUES (?1, ?2, ?3)",
                params![new_card_id, candidate_id, shared_json],
            );
            let _ = conn.execute(
                "INSERT OR IGNORE INTO card_connections (card_id, related_card_id, shared_tags) VALUES (?1, ?2, ?3)",
                params![candidate_id, new_card_id, shared_json],
            );
        }
    }

    Ok(())
}

async fn is_same_topic(cfg: &crate::ai_state::AiConfig, fp_a: &str, fp_b: &str) -> bool {
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        return false;
    }

    let prompt = format!(
        "Do these two article descriptions discuss the same specific topic? Reply with only 'yes' or 'no'.\n\nA: {fp_a}\n\nB: {fp_b}"
    );

    let body = serde_json::json!({
        "model": cfg.reader_model,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": 5,
        "temperature": 0,
    });

    let client = reqwest::Client::new();
    let Ok(resp) = client
        .post(format!("{}/chat/completions", cfg.endpoint.trim_end_matches('/')))
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()
        .await
    else {
        return false;
    };

    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return false;
    };

    let answer = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();

    answer.contains("yes")
}

// ── DB utilities ──────────────────────────────────────────────────────────────

fn fetch_pending_items(db: &DbState) -> Result<Vec<InboxItem>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, source_id, source_name, file_path, title, url, status, word_count, ingested_at
             FROM inbox_items WHERE reader_status = 'pending' ORDER BY ingested_at ASC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
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

fn set_reader_status(db: &DbState, id: &str, status: &str) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE inbox_items SET reader_status = ?1 WHERE id = ?2",
        params![status, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
