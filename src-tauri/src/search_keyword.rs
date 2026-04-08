use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    file_path: String,
    matches: Vec<MatchContent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchContent {
    text: String,
    /// For markdown files the "node" just contains the matched line text.
    /// Kept as a JSON string for frontend compatibility.
    node: serde_json::Value,
}

/// Strip YAML frontmatter (---...---) from the start of a string if present.
fn strip_frontmatter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    let after = &content[3..];
    if let Some(idx) = after.find("\n---") {
        let rest = &after[idx + 4..];
        // skip the single newline that follows closing ---
        if rest.starts_with('\n') {
            return &rest[1..];
        }
        return rest;
    }
    content
}

pub fn search_files(root_dir: &Path, keyword: &str) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let files: Vec<PathBuf> = WalkDir::new(root_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
            ext == "md" || ext == "json"
        })
        .map(|e| e.path().to_owned())
        .collect();

    let results = Arc::new(Mutex::new(Vec::new()));

    files.par_iter().for_each(|file_path| {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let matches = if ext == "md" {
                search_in_markdown(strip_frontmatter(&content), keyword, file_path)
            } else {
                // Legacy JSON notes
                if let Ok(json_content) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                    search_in_json(&json_content, keyword)
                } else {
                    vec![]
                }
            };

            if !matches.is_empty() {
                let mut results = results.lock().unwrap();
                results.push(SearchResult {
                    file_path: file_path.to_string_lossy().into_owned(),
                    matches,
                });
            }
        }
    });

    Ok(Arc::try_unwrap(results).unwrap().into_inner()?)
}

fn search_in_markdown(body: &str, keyword: &str, file_path: &Path) -> Vec<MatchContent> {
    let lower_kw = keyword.to_lowercase();
    let fake_id = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();

    body.lines()
        .filter(|line| line.to_lowercase().contains(&lower_kw))
        .map(|line| MatchContent {
            text: line.trim().to_owned(),
            node: serde_json::json!({
                "type": "p",
                "id": fake_id,
                "children": [{ "text": line.trim() }]
            }),
        })
        .collect()
}

fn search_in_json(json_content: &[serde_json::Value], keyword: &str) -> Vec<MatchContent> {
    let mut matches = Vec::new();

    fn process_node(
        node: &serde_json::Value,
        parent_node: Option<&serde_json::Value>,
        keyword: &str,
        matches: &mut Vec<MatchContent>,
    ) {
        match node {
            serde_json::Value::Object(obj) => {
                if let Some(text) = obj.get("text") {
                    if let Some(text_str) = text.as_str() {
                        if text_str.to_lowercase().contains(&keyword.to_lowercase()) {
                            matches.push(MatchContent {
                                text: text_str.to_string(),
                                node: parent_node.unwrap_or(node).clone(),
                            });
                        }
                    }
                }
                if let Some(children) = obj.get("children") {
                    if let Some(children_array) = children.as_array() {
                        for child in children_array {
                            process_node(child, Some(node), keyword, matches);
                        }
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    process_node(item, None, keyword, matches);
                }
            }
            _ => {}
        }
    }

    process_node(&serde_json::Value::Array(json_content.to_vec()), None, keyword, &mut matches);
    matches
}
