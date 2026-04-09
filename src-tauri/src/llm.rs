// LLM proxy: called from the frontend, forwards requests through Rust's reqwest
// so they are not blocked by the webview CSP.
use crate::ai_state::AiState;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

// ── Shared message type ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// ── llm_stream ────────────────────────────────────────────────────────────────
// Streams an OpenAI-compatible SSE response back to the frontend via Tauri events.
// Each chunk emits "llm-chunk" with the partial text.
// On finish emits "llm-done".  On error emits "llm-error" with the message.

#[tauri::command]
pub async fn llm_stream(
    app: AppHandle,
    state: State<'_, Arc<AiState>>,
    messages: Vec<ChatMessage>,
    stream_id: String, // unique ID so the caller can correlate events
) -> Result<(), String> {
    let cfg = state.get();
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        let _ = app.emit(&format!("llm-error:{stream_id}"), "AI not configured");
        return Ok(());
    }

    let model = if !cfg.writer_model.is_empty() {
        cfg.writer_model.clone()
    } else if !cfg.reader_model.is_empty() {
        cfg.reader_model.clone()
    } else {
        "gpt-4o".to_string()
    };

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let request_body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "temperature": 0.7,
        "max_tokens": 2000,
    });

    let response = client
        .post(format!("{}/chat/completions", cfg.endpoint.trim_end_matches('/')))
        .bearer_auth(&cfg.api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            let msg = format!("Request failed: {e}");
            let _ = app.emit(&format!("llm-error:{stream_id}"), &msg);
            msg
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let msg = format!("LLM error {status}: {body}");
        let _ = app.emit(&format!("llm-error:{stream_id}"), &msg);
        return Err(msg);
    }

    // Read the SSE stream and emit each token as an event
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit(&format!("llm-error:{stream_id}"), &e.to_string());
                break;
            }
        };

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if !line.starts_with("data: ") {
                continue;
            }
            let data = &line[6..];
            if data == "[DONE]" {
                let _ = app.emit(&format!("llm-done:{stream_id}"), ());
                return Ok(());
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                    if !content.is_empty() {
                        let _ = app.emit(&format!("llm-chunk:{stream_id}"), content.to_string());
                    }
                }
            }
        }
    }

    let _ = app.emit(&format!("llm-done:{stream_id}"), ());
    Ok(())
}

// ── copilot_complete ──────────────────────────────────────────────────────────
// Non-streaming single completion for the ghost-text copilot feature.

#[tauri::command]
pub async fn copilot_complete(
    state: State<'_, Arc<AiState>>,
    prompt: String,
) -> Result<String, String> {
    let cfg = state.get();
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        return Ok("0".to_string());
    }

    let model = if !cfg.writer_model.is_empty() {
        cfg.writer_model.clone()
    } else if !cfg.reader_model.is_empty() {
        cfg.reader_model.clone()
    } else {
        "gpt-4o-mini".to_string()
    };

    let system = "You are an advanced AI writing assistant. Your task is to predict and generate the next part of the text based on the given context.\n\nRules:\n- Continue the text naturally up to the next punctuation mark (., ,, ;, :, ?, or !).\n- Maintain style and tone. Don't repeat given text.\n- CRITICAL: Always end with a punctuation mark.\n- CRITICAL: Avoid starting a new block. Do not use block formatting like >, #, 1., 2., -, etc.\n- If no context is provided or you can't generate a continuation, return \"0\" without explanation.";

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": format!("Continue the text up to the next punctuation mark:\n\"\"\"\n{prompt}\n\"\"\"") },
        ],
        "stream": false,
        "max_tokens": 60,
        "temperature": 0.2,
    });

    let response = client
        .post(format!("{}/chat/completions", cfg.endpoint.trim_end_matches('/')))
        .bearer_auth(&cfg.api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Ok("0".to_string());
    }

    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("0")
        .trim()
        .to_string();

    Ok(text)
}
