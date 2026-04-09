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
        let item_id = item.id.clone();
        if let Err(e) = process_item(&app, &db, &ai, item).await {
            eprintln!("[reader_agent] process_item error for {item_id}: {e}");
            // Increment retry_count; if >= 3, mark as failed so it stops being retried.
            if let Ok(conn) = db.conn.lock() {
                let count: i64 = conn
                    .query_row(
                        "SELECT retry_count FROM inbox_items WHERE id = ?1",
                        params![item_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                if count >= 2 {
                    let _ = conn.execute(
                        "UPDATE inbox_items SET reader_status = 'failed', retry_count = retry_count + 1 WHERE id = ?1",
                        params![item_id],
                    );
                } else {
                    let _ = conn.execute(
                        "UPDATE inbox_items SET reader_status = 'pending', retry_count = retry_count + 1 WHERE id = ?1",
                        params![item_id],
                    );
                }
            }
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

    // Step 1: get full text
    // Twitter and Bilibili are video/social platforms: the content is already in the .md body
    // (tweet text from Nitter RSS, video description from Bilibili API).
    // Trying Jina/HTTP would waste 30-45s and return unhelpful content.
    let skip_fetch = matches!(
        item.source_type.as_deref(),
        Some("twitter") | Some("bilibili")
    );

    let (full_text, content_source) = if skip_fetch {
        (read_markdown_body(db, &item)?, "summary")
    } else {
        match item.url.as_deref() {
            Some(url) if !url.is_empty() => match fetch_full_text(url).await {
                Ok(text) if text.split_whitespace().count() >= 100 => (text, "full"),
                _ => (read_markdown_body(db, &item)?, "summary"),
            },
            _ => (read_markdown_body(db, &item)?, "summary"),
        }
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

/// Route to the best extractor based on URL.
/// Priority: YouTube transcript → Jina Reader → direct HTML + readability.
async fn fetch_full_text(url: &str) -> Result<String, String> {
    // 1. YouTube: try native transcript API
    if let Some(video_id) = extract_youtube_video_id(url) {
        match fetch_youtube_transcript(&video_id).await {
            Ok(t) if t.split_whitespace().count() >= 50 => return Ok(t),
            Ok(_) => {}
            Err(e) => eprintln!("[youtube transcript] {e}"),
        }
    }

    // 2. Jina Reader: handles JS-rendered pages, returns clean Markdown
    match fetch_via_jina(url).await {
        Ok(t) if t.split_whitespace().count() >= 100 => return Ok(t),
        Ok(_) => {}
        Err(e) => eprintln!("[jina] {e}"),
    }

    // 3. Fallback: direct HTTP + readability
    fetch_via_direct(url).await
}

/// Fetch via Jina Reader (https://r.jina.ai/{url}) — returns clean Markdown.
async fn fetch_via_jina(url: &str) -> Result<String, String> {
    let jina_url = format!("https://r.jina.ai/{}", url);
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&jina_url)
        .header("Accept", "text/plain")
        .header("X-Return-Format", "markdown")
        .send()
        .await
        .map_err(|e| format!("Jina error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Jina HTTP {}", resp.status()));
    }

    resp.text().await.map_err(|e| format!("Jina read: {e}"))
}

/// Direct HTTP fetch + readability extraction (original implementation).
async fn fetch_via_direct(url: &str) -> Result<String, String> {
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

// ── YouTube transcript (no external tools) ───────────────────────────────────

/// Extract the 11-char video ID from a YouTube URL. Returns None for non-video URLs.
pub fn extract_youtube_video_id(url: &str) -> Option<String> {
    // https://www.youtube.com/watch?v=XXXXXXXXXXX
    if url.contains("youtube.com/watch") {
        if let Some(pos) = url.find("v=") {
            let after = &url[pos + 2..];
            let id: String = after.chars().take_while(|&c| c != '&' && c != '#' && c != '/').collect();
            if id.len() == 11 {
                return Some(id);
            }
        }
    }
    // https://youtu.be/XXXXXXXXXXX
    if let Some(pos) = url.find("youtu.be/") {
        let after = &url[pos + 9..];
        let id: String = after.chars().take_while(|&c| c != '?' && c != '#' && c != '/').collect();
        if id.len() == 11 {
            return Some(id);
        }
    }
    None
}

/// Fetch the auto-generated or manual transcript for a YouTube video.
/// Uses YouTube's internal timedtext API — no yt-dlp required.
async fn fetch_youtube_transcript(video_id: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let page_url = format!("https://www.youtube.com/watch?v={}", video_id);
    let resp = client
        .get(&page_url)
        .header("Accept-Language", "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7")
        .send()
        .await
        .map_err(|e| format!("YouTube fetch: {e}"))?;

    let html = resp.text().await.map_err(|e| format!("YouTube read: {e}"))?;

    // Extract captionTracks JSON array from embedded page data
    let tracks = extract_caption_tracks(&html);
    if tracks.is_empty() {
        return Err("No caption tracks found".to_string());
    }

    // Choose: zh (manual) > en (manual) > zh (asr) > en (asr) > first available
    let track_url = choose_best_track_url(&tracks)
        .ok_or("No suitable caption track")?;

    let xml_resp = client
        .get(&track_url)
        .send()
        .await
        .map_err(|e| format!("Caption fetch: {e}"))?;

    let xml = xml_resp.text().await.map_err(|e| format!("Caption read: {e}"))?;
    let transcript = parse_timedtext_xml(&xml);

    if transcript.is_empty() {
        return Err("Empty transcript".to_string());
    }
    Ok(transcript)
}

/// Extract captionTracks from a YouTube watch page.
/// Returns Vec<(languageCode, kind, baseUrl)>.
fn extract_caption_tracks(html: &str) -> Vec<(String, String, String)> {
    let marker = "\"captionTracks\":";
    let Some(start) = html.find(marker) else { return vec![] };
    let rest = &html[start + marker.len()..];
    let Some(bracket_pos) = rest.find('[') else { return vec![] };
    let arr_str = &rest[bracket_pos..];

    // Find balanced ] accounting for strings
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut end_idx = arr_str.len();

    for (i, c) in arr_str.char_indices() {
        if escape { escape = false; continue; }
        if in_string {
            if c == '\\' { escape = true; }
            else if c == '"' { in_string = false; }
        } else {
            match c {
                '"' => in_string = true,
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 { end_idx = i + 1; break; }
                }
                _ => {}
            }
        }
    }

    let json_array = &arr_str[..end_idx];
    let arr: serde_json::Value = match serde_json::from_str(json_array) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[youtube] captionTracks parse error: {e}");
            return vec![];
        }
    };

    let mut tracks = Vec::new();
    if let Some(arr) = arr.as_array() {
        for track in arr {
            let base_url = track["baseUrl"].as_str().unwrap_or("").to_string();
            let lang = track["languageCode"].as_str().unwrap_or("").to_string();
            let kind = track["kind"].as_str().unwrap_or("").to_string();
            if !base_url.is_empty() {
                tracks.push((lang, kind, base_url));
            }
        }
    }
    tracks
}

/// Pick the best caption track: prefer zh, then en; prefer non-asr (manual) over asr.
fn choose_best_track_url(tracks: &[(String, String, String)]) -> Option<String> {
    let find = |lang_prefix: &str, require_manual: bool| -> Option<String> {
        tracks.iter()
            .find(|(lang, kind, _)| {
                lang.starts_with(lang_prefix) && (!require_manual || kind != "asr")
            })
            .map(|(_, _, url)| url.clone())
    };

    find("zh", true)
        .or_else(|| find("en", true))
        .or_else(|| find("zh", false))
        .or_else(|| find("en", false))
        .or_else(|| tracks.first().map(|(_, _, u)| u.clone()))
}

/// Parse YouTube timedtext XML into plain text (strips timing tags + bracketed sounds).
fn parse_timedtext_xml(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"text" { in_text = true; }
            }
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(t) = e.unescape() {
                    let s = t.trim().to_string();
                    // Skip bracketed sound descriptions: [Music], [Applause], etc.
                    if !s.is_empty() && !s.starts_with('[') {
                        texts.push(s);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"text" { in_text = false; }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // Join into continuous text with sentence-aware spacing
    texts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

// Tags that are always noise and should be skipped entirely.
const NOISE_TAGS: &[&str] = &[
    "script", "style", "nav", "footer", "header", "aside",
    "noscript", "form", "iframe", "button", "select", "textarea",
    "figure", "figcaption",
];

// Class / id fragments that indicate non-content regions.
const BAD_PATTERNS: &[&str] = &[
    "sidebar", "widget", "menu", "nav", "comment", "footer", "header",
    "ad-", "advert", "sponsor", "promo", "social", "share", "banner",
    "popup", "modal", "cookie", "toolbar", "related", "recommend",
    "breadcrumb", "pagination", "author-bio", "newsletter", "subscribe",
    "toc", "table-of-contents",
];

// Class / id fragments that strongly indicate content regions.
const GOOD_PATTERNS: &[&str] = &[
    "article", "content", "post-body", "post-content", "entry-content",
    "article-body", "article-content", "story-body", "prose",
    "blog-post", "single-content", "main-content", "page-content",
    "entry-body", "post__content",
];

/// Readability-style extraction:
/// 1. Score every block-level element by text density.
/// 2. Pick the highest-scoring candidate.
/// 3. Convert it to Markdown preserving heading/list structure.
fn extract_article_text(html: &str) -> Result<String, String> {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);

    let block_sel = Selector::parse(
        "article, main, [role='main'], [role='article'], section, div",
    )
    .map_err(|e| e.to_string())?;

    let a_sel = Selector::parse("a").map_err(|e| e.to_string())?;

    let mut best_score: i64 = 0;
    let mut best_element: Option<scraper::ElementRef<'_>> = None;

    for element in doc.select(&block_sel) {
        let tag = element.value().name();
        let class = element.value().attr("class").unwrap_or("").to_lowercase();
        let id = element.value().attr("id").unwrap_or("").to_lowercase();
        let combined = format!("{class} {id}");

        // Skip known noise containers by class / id.
        if BAD_PATTERNS.iter().any(|bad| combined.contains(bad)) {
            continue;
        }

        // Require a minimum word count to be a real content block.
        let all_text: String = element.text().collect();
        let total_words = all_text.split_whitespace().count();
        if total_words < 80 {
            continue;
        }

        // Penalise link-heavy blocks (navigation / related-posts sections).
        let link_text: String = element.select(&a_sel).flat_map(|a| a.text()).collect();
        let link_words = link_text.split_whitespace().count();
        let link_density = link_words as f64 / total_words.max(1) as f64;
        if link_density > 0.5 {
            continue;
        }

        let mut score = (total_words as f64 * (1.0 - link_density)) as i64;

        // Semantic tag bonuses.
        if tag == "article" || tag == "main" {
            score += 200;
        }

        // Good class/id bonuses.
        if GOOD_PATTERNS.iter().any(|good| combined.contains(good)) {
            score += 100;
        }

        if score > best_score {
            best_score = score;
            best_element = Some(element);
        }
    }

    // Convert the winner to Markdown.
    if let Some(el) = best_element {
        let md = element_to_markdown(*el);
        if md.split_whitespace().count() >= 100 {
            return Ok(md);
        }
    }

    // Fallback: join all substantial <p> tags when no clear winner found.
    let p_sel = Selector::parse("p").map_err(|e| e.to_string())?;
    let paras: Vec<String> = doc
        .select(&p_sel)
        .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
        .filter(|t| t.split_whitespace().count() > 15)
        .collect();

    if paras.is_empty() {
        return Err("Could not extract article content".to_string());
    }
    Ok(paras.join("\n\n"))
}

/// Convert an ego_tree node (and all its descendants) to Markdown text,
/// skipping noise elements and preserving heading / list / blockquote structure.
fn element_to_markdown(node: ego_tree::NodeRef<'_, scraper::Node>) -> String {
    let mut buf = String::new();
    node_to_md(node, &mut buf);

    // Normalise: collapse multiple blank lines → single blank line, trim lines.
    let mut result = String::new();
    let mut prev_blank = false;
    for line in buf.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_blank {
                result.push('\n');
            }
            prev_blank = true;
        } else {
            prev_blank = false;
            result.push_str(trimmed);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

fn node_to_md(node: ego_tree::NodeRef<'_, scraper::Node>, buf: &mut String) {
    use scraper::Node;

    match node.value() {
        Node::Text(text) => {
            let t = text.trim();
            if !t.is_empty() {
                buf.push_str(t);
                buf.push(' ');
            }
        }
        Node::Element(el) => {
            let tag = el.name();
            let class = el.attr("class").unwrap_or("").to_lowercase();
            let id = el.attr("id").unwrap_or("").to_lowercase();
            let combined = format!("{class} {id}");

            // Skip noise tags entirely — do not recurse into them.
            if NOISE_TAGS.contains(&tag) {
                return;
            }
            // Skip noise by class / id.
            if BAD_PATTERNS.iter().any(|bad| combined.contains(bad)) {
                return;
            }

            // Emit block prefix before children.
            let prefix: &str = match tag {
                "h1" => "\n# ",
                "h2" => "\n## ",
                "h3" => "\n### ",
                "h4" | "h5" | "h6" => "\n#### ",
                "li" => "\n- ",
                "blockquote" => "\n> ",
                "p" | "div" | "section" | "article" | "main" => "\n",
                "br" => "\n",
                _ => "",
            };
            buf.push_str(prefix);

            // Recurse into children.
            for child in node.children() {
                node_to_md(child, buf);
            }

            // Emit block suffix after children.
            let suffix: &str = match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                | "p" | "li" | "blockquote" | "div" | "section" => "\n",
                _ => "",
            };
            buf.push_str(suffix);
        }
        // Document / Fragment / Comment — just recurse.
        _ => {
            for child in node.children() {
                node_to_md(child, buf);
            }
        }
    }
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
        body = truncate_chars(body, 6000),
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

/// Safely truncate a &str to at most `max_chars` Unicode scalar values.
/// Unlike byte-slicing, this never panics on multi-byte (CJK) content.
fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
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
            "SELECT id, source_id, source_name, file_path, title, url, status, word_count, ingested_at, source_type
             FROM inbox_items WHERE reader_status = 'pending' AND retry_count < 3 ORDER BY ingested_at ASC LIMIT 10",
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
            reader_status: None,
            content_source: None,
            source_type: row.get(9).map_err(|e| e.to_string())?,
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
