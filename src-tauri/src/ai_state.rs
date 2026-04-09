// AiState: shared AI configuration accessible from background tasks
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiConfig {
    pub endpoint: String,
    pub api_key: String,
    pub reader_model: String,
    pub writer_model: String,
    pub system_prompt: String,   // assembled from Persona
    pub collect_interval_mins: u64,  // 0 = disabled
}

pub struct AiState(pub Mutex<AiConfig>);

impl AiState {
    pub fn new() -> Self {
        AiState(Mutex::new(AiConfig {
            endpoint: String::new(),
            api_key: String::new(),
            reader_model: "gpt-4o-mini".to_string(),
            writer_model: "gpt-4o".to_string(),
            system_prompt: String::new(),
            collect_interval_mins: 30,
        }))
    }

    pub fn get(&self) -> AiConfig {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Called by the frontend when settings are saved
#[tauri::command]
pub fn update_ai_config(
    state: tauri::State<'_, std::sync::Arc<AiState>>,
    endpoint: String,
    api_key: String,
    reader_model: String,
    writer_model: String,
    system_prompt: String,
    collect_interval_mins: u64,
) -> Result<(), String> {
    let mut cfg = state.0.lock().map_err(|e| e.to_string())?;
    cfg.endpoint = endpoint;
    cfg.api_key = api_key;
    cfg.reader_model = reader_model;
    cfg.writer_model = writer_model;
    cfg.system_prompt = system_prompt;
    cfg.collect_interval_mins = collect_interval_mins;
    Ok(())
}
