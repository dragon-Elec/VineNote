// Scheduler: background daemon for periodic collect + reader agent
use crate::ai_state::AiState;
use crate::db_state::DbState;
use crate::reader_agent;
use crate::sources;
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// Spawn the background daemon. Called once from lib.rs setup.
pub fn start(app: tauri::AppHandle, db: Arc<DbState>, ai: Arc<AiState>) {
    tauri::async_runtime::spawn(async move {
        run_daemon(app, db, ai).await;
    });
}

async fn run_daemon(app: tauri::AppHandle, db: Arc<DbState>, ai: Arc<AiState>) {
    // Check every minute; honor the configured interval
    let mut tick = interval(Duration::from_secs(60));

    // Track when we last collected so we respect the interval
    let mut last_collect: Option<std::time::Instant> = None;

    loop {
        tick.tick().await;

        let cfg = ai.get();
        // Skip everything if AI not configured
        if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
            continue;
        }

        let interval_secs = cfg.collect_interval_mins * 60;
        let should_collect = interval_secs > 0
            && last_collect
                .map(|t| t.elapsed().as_secs() >= interval_secs)
                .unwrap_or(true); // first run: collect immediately

        if should_collect {
            let count = sources::collect_all_sources_raw(&app, &db).await;
            if count > 0 {
                eprintln!("[scheduler] collected {count} new items");
            }
            last_collect = Some(std::time::Instant::now());
        }

        // Always try to process pending items (they accumulate between collects)
        reader_agent::process_pending_items(
            Arc::new(app.clone()),
            db.clone(),
            ai.clone(),
        )
        .await;
    }
}
