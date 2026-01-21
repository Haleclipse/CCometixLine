use super::{Segment, SegmentData};
use crate::config::{InputData, ModelConfig, SegmentId, TranscriptEntry};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct ContextWindowSegment;

impl ContextWindowSegment {
    pub fn new() -> Self {
        Self
    }

    fn get_context_limit_for_model(model_id: &str) -> u32 {
        let model_config = ModelConfig::load();
        model_config.get_context_limit(model_id)
    }

    fn get_context_usage_with_fallback(input: &InputData) -> Option<u32> {
        if input.context_window.is_available() {
            return input.context_window.total_tokens();
        }

        if let Some(usage) = parse_transcript_usage(&input.transcript_path) {
            return Some(usage);
        }

        if let Some(usage) = Self::try_get_from_session_history(&input.transcript_path) {
            return Some(usage);
        }

        None
    }

    fn try_get_from_session_history(transcript_path: &str) -> Option<u32> {
        use std::path::Path;

        let path = Path::new(transcript_path);
        let project_dir = path.parent()?;

        let mut session_files: Vec<PathBuf> = std::fs::read_dir(project_dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .collect();

        if session_files.is_empty() {
            return None;
        }

        session_files.sort_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH)
        });
        session_files.reverse();

        for session_file in session_files.iter().take(5) {
            if let Some(usage) = try_parse_transcript_file(session_file) {
                return Some(usage);
            }
        }

        None
    }
}

impl Segment for ContextWindowSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let context_window = &input.context_window;

        let context_limit = context_window
            .context_window_size
            .unwrap_or_else(|| Self::get_context_limit_for_model(&input.model.id));

        let (percentage_display, tokens_display) = match context_window.get_display_percentage() {
            Some(percentage) => {
                let percentage_str = if percentage.fract() == 0.0 {
                    format!("{:.0}%", percentage)
                } else {
                    format!("{:.1}%", percentage)
                };

                let tokens_str = match context_window.total_tokens() {
                    Some(tokens) => {
                        if tokens >= 1000 {
                            let k_value = tokens as f64 / 1000.0;
                            if k_value.fract() == 0.0 {
                                format!("{}k", k_value as u32)
                            } else {
                                format!("{:.1}k", k_value)
                            }
                        } else {
                            tokens.to_string()
                        }
                    }
                    None => "-".to_string(),
                };

                (percentage_str, tokens_str)
            }
            None => {
                let context_used_token_opt = Self::get_context_usage_with_fallback(input);
                match context_used_token_opt {
                    Some(context_used_token) => {
                        let context_used_rate =
                            (context_used_token as f64 / context_limit as f64) * 100.0;

                        let percentage = if context_used_rate.fract() == 0.0 {
                            format!("{:.0}%", context_used_rate)
                        } else {
                            format!("{:.1}%", context_used_rate)
                        };

                        let tokens = if context_used_token >= 1000 {
                            let k_value = context_used_token as f64 / 1000.0;
                            if k_value.fract() == 0.0 {
                                format!("{}k", k_value as u32)
                            } else {
                                format!("{:.1}k", k_value)
                            }
                        } else {
                            context_used_token.to_string()
                        };

                        (percentage, tokens)
                    }
                    None => ("-".to_string(), "-".to_string()),
                }
            }
        };

        let mut metadata = HashMap::new();

        if let Some(percentage_val) = context_window.get_display_percentage() {
            metadata.insert("percentage".to_string(), percentage_val.to_string());
            metadata.insert(
                "tokens".to_string(),
                context_window.total_tokens().unwrap_or(0).to_string(),
            );
        } else if let Some(context_used_token) = Self::get_context_usage_with_fallback(input) {
            let context_used_rate = (context_used_token as f64 / context_limit as f64) * 100.0;
            metadata.insert("percentage".to_string(), context_used_rate.to_string());
            metadata.insert("tokens".to_string(), context_used_token.to_string());
        } else {
            metadata.insert("percentage".to_string(), "-".to_string());
            metadata.insert("tokens".to_string(), "-".to_string());
        }

        metadata.insert("limit".to_string(), context_limit.to_string());
        metadata.insert("model".to_string(), input.model.id.clone());

        Some(SegmentData {
            primary: format!("{} · {} tokens", percentage_display, tokens_display),
            secondary: String::new(),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::ContextWindow
    }
}

fn parse_transcript_usage<P: AsRef<Path>>(transcript_path: P) -> Option<u32> {
    let path = transcript_path.as_ref();

    if let Some(usage) = try_parse_transcript_file(path) {
        return Some(usage);
    }

    if !path.exists() {
        if let Some(usage) = try_find_usage_from_project_history(path) {
            return Some(usage);
        }
    }

    None
}

fn try_parse_transcript_file(path: &Path) -> Option<u32> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();

    if lines.is_empty() {
        return None;
    }

    let last_line = lines.last()?.trim();
    if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(last_line) {
        if entry.r#type.as_deref() == Some("summary") {
            if let Some(leaf_uuid) = &entry.leaf_uuid {
                let project_dir = path.parent()?;
                return find_usage_by_leaf_uuid(leaf_uuid, project_dir);
            }
        }
    }

    for line in lines.iter().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
            if entry.r#type.as_deref() == Some("assistant") {
                if let Some(message) = &entry.message {
                    if let Some(raw_usage) = &message.usage {
                        let normalized = raw_usage.clone().normalize();
                        return Some(normalized.display_tokens());
                    }
                }
            }
        }
    }

    None
}

fn find_usage_by_leaf_uuid(leaf_uuid: &str, project_dir: &Path) -> Option<u32> {
    let entries = fs::read_dir(project_dir).ok()?;

    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }

        if let Some(usage) = search_uuid_in_file(&path, leaf_uuid) {
            return Some(usage);
        }
    }

    None
}

fn search_uuid_in_file(path: &Path, target_uuid: &str) -> Option<u32> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default();

    for line in &lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
            if let Some(uuid) = &entry.uuid {
                if uuid == target_uuid {
                    if entry.r#type.as_deref() == Some("assistant") {
                        if let Some(message) = &entry.message {
                            if let Some(raw_usage) = &message.usage {
                                let normalized = raw_usage.clone().normalize();
                                return Some(normalized.display_tokens());
                            }
                        }
                    } else if entry.r#type.as_deref() == Some("user") {
                        if let Some(parent_uuid) = &entry.parent_uuid {
                            return find_assistant_message_by_uuid(&lines, parent_uuid);
                        }
                    }
                    break;
                }
            }
        }
    }

    None
}

fn find_assistant_message_by_uuid(lines: &[String], target_uuid: &str) -> Option<u32> {
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
            if let Some(uuid) = &entry.uuid {
                if uuid == target_uuid && entry.r#type.as_deref() == Some("assistant") {
                    if let Some(message) = &entry.message {
                        if let Some(raw_usage) = &message.usage {
                            let normalized = raw_usage.clone().normalize();
                            return Some(normalized.display_tokens());
                        }
                    }
                }
            }
        }
    }

    None
}

fn try_find_usage_from_project_history(transcript_path: &Path) -> Option<u32> {
    let project_dir = transcript_path.parent()?;

    let mut session_files: Vec<PathBuf> = Vec::new();
    let entries = fs::read_dir(project_dir).ok()?;

    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            session_files.push(path);
        }
    }

    if session_files.is_empty() {
        return None;
    }

    session_files.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH)
    });
    session_files.reverse();

    for session_path in &session_files {
        if let Some(usage) = try_parse_transcript_file(session_path) {
            return Some(usage);
        }
    }

    None
}
