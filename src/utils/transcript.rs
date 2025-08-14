use crate::billing::UsageEntry;
use crate::config::{NormalizedUsage, TranscriptEntry};
use chrono::{DateTime, Utc};
use std::collections::HashSet;

/// Extract session ID from file path (the UUID part)
pub fn extract_session_id(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Parse a JSONL line and extract usage entry if valid
pub fn parse_line_to_usage(
    line: &str,
    session_id: &str,
    seen: &mut HashSet<String>,
) -> Option<UsageEntry> {
    // Parse the JSON line
    let entry: TranscriptEntry = serde_json::from_str(line).ok()?;

    // Only process assistant messages with usage data
    if entry.r#type.as_deref() != Some("assistant") {
        return None;
    }

    let message = entry.message.as_ref()?;
    let raw_usage = message.usage.as_ref()?;

    // Deduplication check - match ccusage behavior exactly
    if let (Some(msg_id), Some(req_id)) = (message.id.as_ref(), entry.request_id.as_ref()) {
        // Use message_id:request_id when both are available
        let hash = format!("{}:{}", msg_id, req_id);
        if seen.contains(&hash) {
            return None; // Skip duplicate
        }
        seen.insert(hash);
    }
    // For null ID entries: don't deduplicate (matching ccusage behavior)

    // Normalize the usage data
    let normalized = raw_usage.clone().normalize();

    // Get model name from message
    let model = message.model.as_deref();

    // Convert to UsageEntry
    extract_usage_entry(&normalized, session_id, entry.timestamp.as_deref(), model)
}

/// Convert NormalizedUsage to UsageEntry
pub fn extract_usage_entry(
    normalized: &NormalizedUsage,
    session_id: &str,
    timestamp_str: Option<&str>,
    model: Option<&str>,
) -> Option<UsageEntry> {
    // Parse timestamp or use current time
    let timestamp = if let Some(ts_str) = timestamp_str {
        DateTime::parse_from_rfc3339(ts_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    } else {
        Utc::now()
    };

    Some(UsageEntry {
        timestamp,
        input_tokens: normalized.input_tokens,
        output_tokens: normalized.output_tokens,
        cache_creation_tokens: normalized.cache_creation_input_tokens,
        cache_read_tokens: normalized.cache_read_input_tokens,
        model: model.unwrap_or("").to_string(),
        cost: None, // Will be calculated later with pricing data
        session_id: session_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_session_id() {
        let path = std::path::Path::new(
            "/home/user/.claude/projects/test/c040b0ba-658d-4188-befa-0d2dad1f0ea5.jsonl",
        );
        assert_eq!(
            extract_session_id(path),
            "c040b0ba-658d-4188-befa-0d2dad1f0ea5"
        );
    }

    #[test]
    fn test_normalized_to_usage_entry() {
        let normalized = NormalizedUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 5,
            calculation_source: "test".to_string(),
            raw_data_available: vec![],
        };

        let entry =
            extract_usage_entry(&normalized, "test-session", None, Some("claude-3-5-sonnet"))
                .unwrap();
        assert_eq!(entry.input_tokens, 100);
        assert_eq!(entry.output_tokens, 50);
        assert_eq!(entry.cache_creation_tokens, 10);
        assert_eq!(entry.cache_read_tokens, 5);
        assert_eq!(entry.session_id, "test-session");
        assert_eq!(entry.model, "claude-3-5-sonnet");
        assert!(entry.cost.is_none());
    }
}
