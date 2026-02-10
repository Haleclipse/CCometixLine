//! Transcript parsing for Claude Code session activity
//!
//! This module parses JSONL transcript files to extract tool calls and agent activity.
//! Based on claude-hud's transcript.ts implementation.

use super::types::{ActivityData, AgentEntry, AgentStatus, ToolEntry, ToolStatus};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::SystemTime;

/// Maximum number of tools to keep in activity data
const MAX_TOOLS: usize = 20;

/// Maximum number of agents to keep in activity data
const MAX_AGENTS: usize = 10;

/// Maximum length for truncated targets (command, url, query)
const MAX_TARGET_LEN: usize = 30;

/// A single line entry from the transcript JSONL file
#[derive(Debug, Deserialize)]
struct TranscriptLine {
    /// ISO 8601 timestamp
    timestamp: Option<String>,
    /// Message containing content blocks
    message: Option<Message>,
}

/// Message structure containing content blocks
#[derive(Debug, Deserialize)]
struct Message {
    /// Array of content blocks (tool_use, tool_result, text, etc.)
    content: Option<Vec<ContentBlock>>,
}

/// A content block within a message
#[derive(Debug, Deserialize)]
struct ContentBlock {
    /// Block type: "tool_use", "tool_result", "text", etc.
    #[serde(rename = "type")]
    block_type: String,
    /// Unique identifier for tool_use blocks
    id: Option<String>,
    /// Tool name for tool_use blocks
    name: Option<String>,
    /// Input parameters for tool_use blocks
    input: Option<Value>,
    /// Reference to tool_use id for tool_result blocks
    tool_use_id: Option<String>,
    /// Whether the tool result is an error
    is_error: Option<bool>,
}

/// Internal state for tracking tools and agents during parsing
struct ParserState {
    /// Map of tool_use_id to ToolEntry
    tool_map: HashMap<String, ToolEntry>,
    /// Map of tool_use_id to AgentEntry (for Task tools)
    agent_map: HashMap<String, AgentEntry>,
    /// Session start time from first entry
    session_start: Option<SystemTime>,
}

impl ParserState {
    fn new() -> Self {
        Self {
            tool_map: HashMap::new(),
            agent_map: HashMap::new(),
            session_start: None,
        }
    }

    /// Convert to final ActivityData, keeping only the last N entries
    fn into_activity_data(self) -> ActivityData {
        // Convert maps to vectors, sorted by start_time
        let mut tools: Vec<ToolEntry> = self.tool_map.into_values().collect();
        tools.sort_by(|a, b| a.start_time.cmp(&b.start_time));

        let mut agents: Vec<AgentEntry> = self.agent_map.into_values().collect();
        agents.sort_by(|a, b| a.start_time.cmp(&b.start_time));

        // Keep only the last N entries
        let tools_len = tools.len();
        let tools = if tools_len > MAX_TOOLS {
            tools.into_iter().skip(tools_len - MAX_TOOLS).collect()
        } else {
            tools
        };

        let agents_len = agents.len();
        let agents = if agents_len > MAX_AGENTS {
            agents.into_iter().skip(agents_len - MAX_AGENTS).collect()
        } else {
            agents
        };

        ActivityData {
            tools,
            agents,
            session_start: self.session_start,
        }
    }
}

/// Parse a transcript JSONL file and extract activity data
///
/// # Arguments
/// * `transcript_path` - Path to the JSONL transcript file
///
/// # Returns
/// `ActivityData` containing parsed tools and agents. Returns empty data on error.
///
/// # Example
/// ```ignore
/// let activity = parse_transcript_activity("/path/to/transcript.jsonl");
/// for tool in activity.running_tools() {
///     println!("Running: {}", tool.name);
/// }
/// ```
pub fn parse_transcript_activity<P: AsRef<Path>>(transcript_path: P) -> ActivityData {
    let path = transcript_path.as_ref();

    // Return empty data if path doesn't exist
    if !path.exists() {
        return ActivityData::default();
    }

    // Open file for reading
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return ActivityData::default(),
    };

    let reader = BufReader::new(file);
    let mut state = ParserState::new();

    // Process each line
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON line
        let entry: TranscriptLine = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        process_entry(&entry, &mut state);
    }

    state.into_activity_data()
}

/// Process a single transcript entry
fn process_entry(entry: &TranscriptLine, state: &mut ParserState) {
    // Parse timestamp
    let timestamp = parse_timestamp(entry.timestamp.as_deref());

    // Set session start from first entry with timestamp
    if state.session_start.is_none() && entry.timestamp.is_some() {
        state.session_start = Some(timestamp);
    }

    // Get content blocks
    let content = match &entry.message {
        Some(msg) => match &msg.content {
            Some(c) => c,
            None => return,
        },
        None => return,
    };

    // Process each content block
    for block in content {
        process_content_block(block, timestamp, state);
    }
}

/// Process a single content block
fn process_content_block(block: &ContentBlock, timestamp: SystemTime, state: &mut ParserState) {
    match block.block_type.as_str() {
        "tool_use" => process_tool_use(block, timestamp, state),
        "tool_result" => process_tool_result(block, timestamp, state),
        _ => {} // Ignore other block types (text, etc.)
    }
}

/// Process a tool_use block
fn process_tool_use(block: &ContentBlock, timestamp: SystemTime, state: &mut ParserState) {
    let id = match &block.id {
        Some(id) => id.clone(),
        None => return,
    };

    let name = match &block.name {
        Some(n) => n.clone(),
        None => return,
    };

    // Check if this is a Task (agent) tool
    if name == "Task" {
        let agent_entry = create_agent_entry(&id, &block.input, timestamp);
        state.agent_map.insert(id, agent_entry);
    } else if name != "TodoWrite" {
        // Skip TodoWrite as it's handled separately in the original
        // Create regular tool entry
        let target = extract_target(&name, &block.input);
        let tool_entry = ToolEntry::with_start_time(id.clone(), name, target, timestamp);
        state.tool_map.insert(id, tool_entry);
    }
}

/// Process a tool_result block
fn process_tool_result(block: &ContentBlock, timestamp: SystemTime, state: &mut ParserState) {
    let tool_use_id = match &block.tool_use_id {
        Some(id) => id,
        None => return,
    };

    let is_error = block.is_error.unwrap_or(false);

    // Update tool if found
    if let Some(tool) = state.tool_map.get_mut(tool_use_id) {
        tool.status = if is_error {
            ToolStatus::Error
        } else {
            ToolStatus::Completed
        };
        tool.end_time = Some(timestamp);
    }

    // Update agent if found
    if let Some(agent) = state.agent_map.get_mut(tool_use_id) {
        agent.status = AgentStatus::Completed;
        agent.end_time = Some(timestamp);
    }
}

/// Create an AgentEntry from Task tool input
fn create_agent_entry(id: &str, input: &Option<Value>, timestamp: SystemTime) -> AgentEntry {
    let (agent_type, model, description) = match input {
        Some(Value::Object(obj)) => {
            let agent_type = obj
                .get("subagent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let model = obj.get("model").and_then(|v| v.as_str()).map(String::from);

            let description = obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);

            (agent_type, model, description)
        }
        _ => ("unknown".to_string(), None, None),
    };

    AgentEntry::with_start_time(id.to_string(), agent_type, model, description, timestamp)
}

/// Extract target from tool input based on tool name
fn extract_target(tool_name: &str, input: &Option<Value>) -> Option<String> {
    let obj = match input {
        Some(Value::Object(o)) => o,
        _ => return None,
    };

    match tool_name {
        // File operations: extract file_path or path
        "Read" | "Write" | "Edit" => obj
            .get("file_path")
            .or_else(|| obj.get("path"))
            .and_then(|v| v.as_str())
            .map(String::from),

        // Pattern-based tools
        "Glob" | "Grep" => obj.get("pattern").and_then(|v| v.as_str()).map(String::from),

        // Bash: first 30 chars of command
        "Bash" => obj.get("command").and_then(|v| v.as_str()).map(|cmd| {
            if cmd.len() > MAX_TARGET_LEN {
                format!("{}...", &cmd[..MAX_TARGET_LEN])
            } else {
                cmd.to_string()
            }
        }),

        // Web tools: url or query, truncated
        "WebFetch" => obj.get("url").and_then(|v| v.as_str()).map(|url| {
            if url.len() > MAX_TARGET_LEN {
                format!("{}...", &url[..MAX_TARGET_LEN])
            } else {
                url.to_string()
            }
        }),

        "WebSearch" => obj.get("query").and_then(|v| v.as_str()).map(|query| {
            if query.len() > MAX_TARGET_LEN {
                format!("{}...", &query[..MAX_TARGET_LEN])
            } else {
                query.to_string()
            }
        }),

        // Unknown tool: no target
        _ => None,
    }
}

/// Parse an ISO 8601 timestamp string to SystemTime
fn parse_timestamp(timestamp_str: Option<&str>) -> SystemTime {
    match timestamp_str {
        Some(ts) => {
            // Try parsing as ISO 8601 with chrono
            match DateTime::parse_from_rfc3339(ts) {
                Ok(dt) => {
                    let utc: DateTime<Utc> = dt.into();
                    SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(utc.timestamp() as u64)
                        + std::time::Duration::from_nanos(utc.timestamp_subsec_nanos() as u64)
                }
                Err(_) => {
                    // Try parsing without timezone (assume UTC)
                    match chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f") {
                        Ok(ndt) => {
                            let utc = ndt.and_utc();
                            SystemTime::UNIX_EPOCH
                                + std::time::Duration::from_secs(utc.timestamp() as u64)
                                + std::time::Duration::from_nanos(
                                    utc.timestamp_subsec_nanos() as u64
                                )
                        }
                        Err(_) => SystemTime::now(),
                    }
                }
            }
        }
        None => SystemTime::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_transcript(lines: &[&str]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_parse_empty_file() {
        let file = create_test_transcript(&[]);
        let activity = parse_transcript_activity(file.path());
        assert!(activity.tools.is_empty());
        assert!(activity.agents.is_empty());
        assert!(activity.session_start.is_none());
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let activity = parse_transcript_activity("/nonexistent/path/transcript.jsonl");
        assert!(activity.tools.is_empty());
        assert!(activity.agents.is_empty());
    }

    #[test]
    fn test_parse_tool_use() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{"file_path":"/src/main.rs"}}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), 1);
        assert_eq!(activity.tools[0].name, "Read");
        assert_eq!(activity.tools[0].target, Some("/src/main.rs".to_string()));
        assert_eq!(activity.tools[0].status, ToolStatus::Running);
    }

    #[test]
    fn test_parse_tool_result() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{"file_path":"/src/main.rs"}}]}}"#,
            r#"{"timestamp":"2024-01-15T10:00:01Z","message":{"content":[{"type":"tool_result","tool_use_id":"tool_1","is_error":false}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), 1);
        assert_eq!(activity.tools[0].status, ToolStatus::Completed);
        assert!(activity.tools[0].end_time.is_some());
    }

    #[test]
    fn test_parse_tool_error() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{"file_path":"/nonexistent"}}]}}"#,
            r#"{"timestamp":"2024-01-15T10:00:01Z","message":{"content":[{"type":"tool_result","tool_use_id":"tool_1","is_error":true}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), 1);
        assert_eq!(activity.tools[0].status, ToolStatus::Error);
    }

    #[test]
    fn test_parse_agent_task() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"agent_1","name":"Task","input":{"subagent_type":"Explore","model":"haiku","description":"Finding auth code"}}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert!(activity.tools.is_empty());
        assert_eq!(activity.agents.len(), 1);
        assert_eq!(activity.agents[0].agent_type, "Explore");
        assert_eq!(activity.agents[0].model, Some("haiku".to_string()));
        assert_eq!(
            activity.agents[0].description,
            Some("Finding auth code".to_string())
        );
        assert_eq!(activity.agents[0].status, AgentStatus::Running);
    }

    #[test]
    fn test_parse_agent_completion() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"agent_1","name":"Task","input":{"subagent_type":"Explore"}}]}}"#,
            r#"{"timestamp":"2024-01-15T10:02:00Z","message":{"content":[{"type":"tool_result","tool_use_id":"agent_1"}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.agents.len(), 1);
        assert_eq!(activity.agents[0].status, AgentStatus::Completed);
        assert!(activity.agents[0].end_time.is_some());
    }

    #[test]
    fn test_extract_target_bash() {
        let input = serde_json::json!({"command": "cargo build --release && cargo test"});
        let target = extract_target("Bash", &Some(input));
        assert_eq!(target, Some("cargo build --release && cargo...".to_string()));
    }

    #[test]
    fn test_extract_target_short_command() {
        let input = serde_json::json!({"command": "ls -la"});
        let target = extract_target("Bash", &Some(input));
        assert_eq!(target, Some("ls -la".to_string()));
    }

    #[test]
    fn test_extract_target_glob() {
        let input = serde_json::json!({"pattern": "**/*.rs"});
        let target = extract_target("Glob", &Some(input));
        assert_eq!(target, Some("**/*.rs".to_string()));
    }

    #[test]
    fn test_extract_target_web_fetch() {
        let input = serde_json::json!({"url": "https://example.com/very/long/path/to/resource"});
        let target = extract_target("WebFetch", &Some(input));
        assert_eq!(target, Some("https://example.com/very/long/...".to_string()));
    }

    #[test]
    fn test_extract_target_web_search() {
        let input = serde_json::json!({"query": "rust async programming best practices guide"});
        let target = extract_target("WebSearch", &Some(input));
        assert_eq!(target, Some("rust async programming best pr...".to_string()));
    }

    #[test]
    fn test_max_tools_limit() {
        let mut lines = Vec::new();
        for i in 0..30 {
            lines.push(format!(
                r#"{{"timestamp":"2024-01-15T10:00:{:02}Z","message":{{"content":[{{"type":"tool_use","id":"tool_{}","name":"Read","input":{{"file_path":"/file_{}.rs"}}}}]}}}}"#,
                i, i, i
            ));
        }
        let lines_ref: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let file = create_test_transcript(&lines_ref);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), MAX_TOOLS);
        // Should keep the last 20 (indices 10-29)
        assert_eq!(activity.tools[0].id, "tool_10");
        assert_eq!(activity.tools[19].id, "tool_29");
    }

    #[test]
    fn test_max_agents_limit() {
        let mut lines = Vec::new();
        for i in 0..15 {
            lines.push(format!(
                r#"{{"timestamp":"2024-01-15T10:00:{:02}Z","message":{{"content":[{{"type":"tool_use","id":"agent_{}","name":"Task","input":{{"subagent_type":"Explore"}}}}]}}}}"#,
                i, i
            ));
        }
        let lines_ref: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let file = create_test_transcript(&lines_ref);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.agents.len(), MAX_AGENTS);
        // Should keep the last 10 (indices 5-14)
        assert_eq!(activity.agents[0].id, "agent_5");
        assert_eq!(activity.agents[9].id, "agent_14");
    }

    #[test]
    fn test_session_start() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{}}]}}"#,
            r#"{"timestamp":"2024-01-15T10:05:00Z","message":{"content":[{"type":"tool_use","id":"tool_2","name":"Write","input":{}}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert!(activity.session_start.is_some());
    }

    #[test]
    fn test_skip_malformed_lines() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{}}]}}"#,
            r#"not valid json"#,
            r#"{"timestamp":"2024-01-15T10:00:01Z","message":{"content":[{"type":"tool_use","id":"tool_2","name":"Write","input":{}}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), 2);
    }

    #[test]
    fn test_skip_empty_lines() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{}}]}}"#,
            "",
            "   ",
            r#"{"timestamp":"2024-01-15T10:00:01Z","message":{"content":[{"type":"tool_use","id":"tool_2","name":"Write","input":{}}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), 2);
    }

    #[test]
    fn test_todo_write_ignored() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"tool_use","id":"todo_1","name":"TodoWrite","input":{"todos":[]}}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert!(activity.tools.is_empty());
        assert!(activity.agents.is_empty());
    }

    #[test]
    fn test_mixed_content_blocks() {
        let lines = [
            r#"{"timestamp":"2024-01-15T10:00:00Z","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","id":"tool_1","name":"Read","input":{"file_path":"/test.rs"}},{"type":"text","text":"World"}]}}"#,
        ];
        let file = create_test_transcript(&lines);
        let activity = parse_transcript_activity(file.path());

        assert_eq!(activity.tools.len(), 1);
        assert_eq!(activity.tools[0].name, "Read");
    }

    #[test]
    fn test_parse_timestamp_rfc3339() {
        let ts = parse_timestamp(Some("2024-01-15T10:30:45Z"));
        // Should not be the current time (which would indicate parse failure)
        let now = SystemTime::now();
        assert!(ts < now);
    }

    #[test]
    fn test_parse_timestamp_with_offset() {
        let ts = parse_timestamp(Some("2024-01-15T10:30:45+05:00"));
        let now = SystemTime::now();
        assert!(ts < now);
    }

    #[test]
    fn test_parse_timestamp_none() {
        let before = SystemTime::now();
        let ts = parse_timestamp(None);
        let after = SystemTime::now();
        // Should be approximately now
        assert!(ts >= before);
        assert!(ts <= after);
    }
}
