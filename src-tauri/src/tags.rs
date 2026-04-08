use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TagEntry {
    pub tag_name: String,
    pub files: Vec<String>,
}

/// Parse the `tags:` line from a YAML frontmatter block.
/// Returns an empty vec if not found or malformed.
fn parse_tags_from_frontmatter(content: &str) -> Vec<String> {
    if !content.starts_with("---") {
        return vec![];
    }

    let after_first = &content[3..];
    let end_idx = match after_first.find("\n---") {
        Some(i) => i,
        None => return vec![],
    };

    let yaml = &after_first[..end_idx];

    for line in yaml.lines() {
        let line = line.trim();
        if line.starts_with("tags:") {
            let value = line["tags:".len()..].trim();
            if value.starts_with('[') && value.ends_with(']') {
                let inner = &value[1..value.len() - 1];
                return inner
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }

    vec![]
}

/// Replace frontmatter tags block only, preserving all other fields and body.
fn replace_tags_in_content(content: &str, new_tags: &[String]) -> String {
    if !content.starts_with("---") {
        return content.to_owned();
    }

    let after_first = &content[3..];
    let end_idx = match after_first.find("\n---") {
        Some(i) => i,
        None => return content.to_owned(),
    };

    let yaml = &after_first[..end_idx];
    let rest = &after_first[end_idx..]; // starts with \n---

    // Rebuild yaml lines, replacing the tags line
    let tags_str = format!("[{}]", new_tags.join(", "));
    let mut has_tags_line = false;
    let new_yaml: String = yaml
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("tags:") {
                has_tags_line = true;
                format!("tags: {}", tags_str)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let final_yaml = if has_tags_line {
        new_yaml
    } else {
        format!("{}\ntags: {}", new_yaml, tags_str)
    };

    // Strip any leading \n so the opening --- is not followed by a blank line
    let final_yaml = final_yaml.trim_start_matches('\n');
    let body_after_close = &rest[4..]; // skip the leading \n--- of the closing delimiter

    format!("---\n{}---{}", final_yaml, body_after_close)
}

/// Scan `dir` recursively for .md files and build a tag → [file_path] map.
pub fn get_tags_from_dir(dir: &Path) -> Vec<TagEntry> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
    {
        let path = entry.path();
        if let Ok(content) = fs::read_to_string(path) {
            let tags = parse_tags_from_frontmatter(&content);
            for tag in tags {
                map.entry(tag)
                    .or_default()
                    .push(path.to_string_lossy().into_owned());
            }
        }
    }

    map.into_iter()
        .map(|(tag_name, files)| TagEntry { tag_name, files })
        .collect()
}

/// Remove `tag_name` from every .md file's frontmatter in `dir`.
pub fn remove_tag_from_all_files(dir: &Path, tag_name: &str) -> Result<(), String> {
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tags = parse_tags_from_frontmatter(&content);
        if tags.contains(&tag_name.to_owned()) {
            let new_tags: Vec<String> = tags.into_iter().filter(|t| t != tag_name).collect();
            let new_content = replace_tags_in_content(&content, &new_tags);
            fs::write(path, new_content).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Rename `old_name` to `new_name` in every .md file's frontmatter in `dir`.
pub fn rename_tag_in_all_files(dir: &Path, old_name: &str, new_name: &str) -> Result<(), String> {
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tags = parse_tags_from_frontmatter(&content);
        if tags.contains(&old_name.to_owned()) {
            let new_tags: Vec<String> = tags
                .into_iter()
                .map(|t| if t == old_name { new_name.to_owned() } else { t })
                .collect();
            let new_content = replace_tags_in_content(&content, &new_tags);
            fs::write(path, new_content).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
