use super::{Segment, SegmentData};
use crate::config::{InputData, Message, ModelConfig, SegmentId, TranscriptEntry};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Default)]
pub struct ContextWindowSegment;

impl ContextWindowSegment {
    pub fn new() -> Self {
        Self
    }

    fn get_context_limit_for_model(model_id: &str) -> u32 {
        ModelConfig::load().get_context_limit(model_id)
    }
}

impl Segment for ContextWindowSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let context_limit = Self::get_context_limit_for_model(&input.model.id);
        let tokens = parse_transcript_usage(&input.transcript_path);

        let (percentage_display, tokens_display) = match tokens {
            Some(t) => {
                let rate = (t as f64 / context_limit as f64) * 100.0;
                let pct = if rate.fract() == 0.0 {
                    format!("{:.0}%", rate)
                } else {
                    format!("{:.1}%", rate)
                };
                let tokens_str = if t >= 1000 {
                    let k = t as f64 / 1000.0;
                    if k.fract() == 0.0 {
                        format!("{}k", k as u32)
                    } else {
                        format!("{:.1}k", k)
                    }
                } else {
                    t.to_string()
                };
                (pct, tokens_str)
            }
            None => ("0%".to_string(), "0".to_string()),
        };

        let mut metadata = HashMap::new();
        metadata.insert(
            "tokens".to_string(),
            tokens.map_or("0".to_string(), |t| t.to_string()),
        );
        metadata.insert(
            "percentage".to_string(),
            tokens.map_or("0".to_string(), |t| {
                ((t as f64 / context_limit as f64) * 100.0).to_string()
            }),
        );
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
    if path.exists() {
        try_parse_transcript_file(path)
    } else {
        None
    }
}

fn try_parse_transcript_file(path: &Path) -> Option<u32> {
    let lines = read_lines(path)?;

    // Check if last line is a summary
    if let Some(entry) = parse_entry(lines.last()?) {
        if entry.r#type.as_deref() == Some("summary") {
            if let Some(leaf_uuid) = &entry.leaf_uuid {
                return find_usage_by_leaf_uuid(leaf_uuid, path.parent()?);
            }
        }
    }

    // Find last assistant message with stop_reason (complete response)
    for line in lines.iter().rev() {
        if let Some(entry) = parse_entry(line) {
            if entry.r#type.as_deref() == Some("assistant") {
                if let Some(message) = &entry.message {
                    if let Some(tokens) = extract_usage(message) {
                        return Some(tokens);
                    }
                }
            }
        }
    }

    None
}

fn find_usage_by_leaf_uuid(leaf_uuid: &str, project_dir: &Path) -> Option<u32> {
    for entry in fs::read_dir(project_dir).ok()? {
        let path = entry.ok()?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            if let Some(usage) = search_uuid_in_file(&path, leaf_uuid) {
                return Some(usage);
            }
        }
    }
    None
}

fn search_uuid_in_file(path: &Path, target_uuid: &str) -> Option<u32> {
    let lines = read_lines(path)?;

    for line in &lines {
        if let Some(entry) = parse_entry(line) {
            if entry.uuid.as_deref() == Some(target_uuid) {
                if let Some(message) = &entry.message {
                    if let Some(tokens) = extract_usage(message) {
                        return Some(tokens);
                    }
                }
                // If user message, find parent assistant
                if entry.r#type.as_deref() == Some("user") {
                    if let Some(parent_uuid) = &entry.parent_uuid {
                        return find_assistant_by_uuid(&lines, parent_uuid);
                    }
                }
                break;
            }
        }
    }
    None
}

fn find_assistant_by_uuid(lines: &[String], target_uuid: &str) -> Option<u32> {
    for line in lines {
        if let Some(entry) = parse_entry(line) {
            if entry.uuid.as_deref() == Some(target_uuid)
                && entry.r#type.as_deref() == Some("assistant")
            {
                if let Some(message) = &entry.message {
                    if let Some(tokens) = extract_usage(message) {
                        return Some(tokens);
                    }
                }
            }
        }
    }
    None
}

// Helper functions

fn read_lines(path: &Path) -> Option<Vec<String>> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    Some(
        reader
            .lines()
            .filter_map(|l| l.ok())
            .collect(),
    )
}

fn parse_entry(line: &str) -> Option<TranscriptEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    serde_json::from_str(line).ok()
}

fn extract_usage(message: &Message) -> Option<u32> {
    // Only messages with stop_reason have complete usage data
    if message.stop_reason.is_some() {
        if let Some(usage) = &message.usage {
            return Some(usage.clone().normalize().display_tokens());
        }
    }
    None
}