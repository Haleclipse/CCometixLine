//! Activity tracking data structures
//!
//! This module defines the core data structures for tracking tool calls,
//! agent status, and configuration counts.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Tool execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Completed,
    Error,
}

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Running,
    Completed,
}

/// Represents a single tool invocation
#[derive(Debug, Clone)]
pub struct ToolEntry {
    /// Unique identifier from tool_use block
    pub id: String,
    /// Tool name (e.g., "Read", "Edit", "Bash")
    pub name: String,
    /// Optional target (file path, pattern, command preview)
    pub target: Option<String>,
    /// Current execution status
    pub status: ToolStatus,
    /// When the tool was invoked
    pub start_time: SystemTime,
    /// When the tool completed (if finished)
    pub end_time: Option<SystemTime>,
}

impl ToolEntry {
    /// Create a new running tool entry
    pub fn new(id: String, name: String, target: Option<String>) -> Self {
        Self {
            id,
            name,
            target,
            status: ToolStatus::Running,
            start_time: SystemTime::now(),
            end_time: None,
        }
    }

    /// Create a new tool entry with a specific start time
    pub fn with_start_time(id: String, name: String, target: Option<String>, start_time: SystemTime) -> Self {
        Self {
            id,
            name,
            target,
            status: ToolStatus::Running,
            start_time,
            end_time: None,
        }
    }

    /// Mark the tool as completed
    pub fn complete(&mut self, is_error: bool) {
        self.status = if is_error {
            ToolStatus::Error
        } else {
            ToolStatus::Completed
        };
        self.end_time = Some(SystemTime::now());
    }

    /// Mark the tool as completed with a specific end time
    pub fn complete_at(&mut self, is_error: bool, end_time: SystemTime) {
        self.status = if is_error {
            ToolStatus::Error
        } else {
            ToolStatus::Completed
        };
        self.end_time = Some(end_time);
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        let end = self.end_time.unwrap_or_else(SystemTime::now);
        end.duration_since(self.start_time).unwrap_or_default()
    }
}

/// Represents an agent (subagent) invocation
#[derive(Debug, Clone)]
pub struct AgentEntry {
    /// Unique identifier from Task tool_use block
    pub id: String,
    /// Agent type (e.g., "Explore", "fix", "Plan")
    pub agent_type: String,
    /// Model used by the agent (e.g., "haiku", "sonnet")
    pub model: Option<String>,
    /// Task description
    pub description: Option<String>,
    /// Current execution status
    pub status: AgentStatus,
    /// When the agent was started
    pub start_time: SystemTime,
    /// When the agent completed (if finished)
    pub end_time: Option<SystemTime>,
}

impl AgentEntry {
    /// Create a new running agent entry
    pub fn new(
        id: String,
        agent_type: String,
        model: Option<String>,
        description: Option<String>,
    ) -> Self {
        Self {
            id,
            agent_type,
            model,
            description,
            status: AgentStatus::Running,
            start_time: SystemTime::now(),
            end_time: None,
        }
    }

    /// Create a new agent entry with a specific start time
    pub fn with_start_time(
        id: String,
        agent_type: String,
        model: Option<String>,
        description: Option<String>,
        start_time: SystemTime,
    ) -> Self {
        Self {
            id,
            agent_type,
            model,
            description,
            status: AgentStatus::Running,
            start_time,
            end_time: None,
        }
    }

    /// Mark the agent as completed
    pub fn complete(&mut self) {
        self.status = AgentStatus::Completed;
        self.end_time = Some(SystemTime::now());
    }

    /// Mark the agent as completed with a specific end time
    pub fn complete_at(&mut self, end_time: SystemTime) {
        self.status = AgentStatus::Completed;
        self.end_time = Some(end_time);
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Duration {
        let end = self.end_time.unwrap_or_else(SystemTime::now);
        end.duration_since(self.start_time).unwrap_or_default()
    }
}

/// Configuration counts from Claude Code environment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigCounts {
    /// Number of CLAUDE.md files found
    pub claude_md_count: u32,
    /// Number of rule files (.md in rules directories)
    pub rules_count: u32,
    /// Number of MCP servers configured
    pub mcp_count: u32,
    /// Number of hooks configured
    pub hooks_count: u32,
}

impl ConfigCounts {
    /// Check if any configs are present
    pub fn has_any(&self) -> bool {
        self.claude_md_count > 0
            || self.rules_count > 0
            || self.mcp_count > 0
            || self.hooks_count > 0
    }

    /// Get total count of all config items
    pub fn total(&self) -> u32 {
        self.claude_md_count + self.rules_count + self.mcp_count + self.hooks_count
    }
}

/// Aggregated activity data from transcript parsing
#[derive(Debug, Clone, Default)]
pub struct ActivityData {
    /// All tracked tools (limited to last N)
    pub tools: Vec<ToolEntry>,
    /// All tracked agents (limited to last N)
    pub agents: Vec<AgentEntry>,
    /// Session start time (from first transcript entry)
    pub session_start: Option<SystemTime>,
}

impl ActivityData {
    /// Get currently running tools
    pub fn running_tools(&self) -> Vec<&ToolEntry> {
        self.tools
            .iter()
            .filter(|t| t.status == ToolStatus::Running)
            .collect()
    }

    /// Get completed tools (including errors)
    pub fn completed_tools(&self) -> Vec<&ToolEntry> {
        self.tools
            .iter()
            .filter(|t| t.status != ToolStatus::Running)
            .collect()
    }

    /// Get currently running agents
    pub fn running_agents(&self) -> Vec<&AgentEntry> {
        self.agents
            .iter()
            .filter(|a| a.status == AgentStatus::Running)
            .collect()
    }

    /// Get completed agents
    pub fn completed_agents(&self) -> Vec<&AgentEntry> {
        self.agents
            .iter()
            .filter(|a| a.status == AgentStatus::Completed)
            .collect()
    }

    /// Check if there's any activity to display
    pub fn has_activity(&self) -> bool {
        !self.tools.is_empty() || !self.agents.is_empty()
    }
}

/// Format a duration for display
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs < 1 {
        return "<1s".to_string();
    }

    if secs < 60 {
        return format!("{}s", secs);
    }

    let mins = secs / 60;
    let remaining_secs = secs % 60;

    if remaining_secs == 0 {
        format!("{}m", mins)
    } else {
        format!("{}m {}s", mins, remaining_secs)
    }
}

/// Truncate a string to max length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Truncate a file path intelligently, showing .../filename format
pub fn truncate_path(path: &str, max_len: usize) -> String {
    // Normalize Windows backslashes to forward slashes
    let normalized = path.replace('\\', "/");

    if normalized.len() <= max_len {
        return normalized;
    }

    // Extract filename
    let parts: Vec<&str> = normalized.split('/').collect();
    let filename = parts.last().unwrap_or(&path);

    if filename.len() >= max_len {
        return truncate_string(filename, max_len);
    }

    // Check if we can fit ".../filename"
    let prefix_len = 4; // ".../"
    if filename.len() + prefix_len <= max_len {
        format!(".../{}", filename)
    } else {
        truncate_string(filename, max_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "<1s");
        assert_eq!(format_duration(Duration::from_secs(0)), "<1s");
        assert_eq!(format_duration(Duration::from_secs(1)), "1s");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(59)), "59s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
        assert_eq!(truncate_string("hello", 3), "...");
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_path() {
        assert_eq!(truncate_path("src/main.rs", 20), "src/main.rs");
        assert_eq!(truncate_path("very/long/path/to/file.rs", 15), ".../file.rs");
        assert_eq!(truncate_path("C:\\Users\\test\\file.rs", 15), ".../file.rs");
        assert_eq!(truncate_path("short.rs", 20), "short.rs");
    }

    #[test]
    fn test_config_counts() {
        let empty = ConfigCounts::default();
        assert!(!empty.has_any());
        assert_eq!(empty.total(), 0);

        let counts = ConfigCounts {
            claude_md_count: 2,
            rules_count: 5,
            mcp_count: 3,
            hooks_count: 1,
        };
        assert!(counts.has_any());
        assert_eq!(counts.total(), 11);
    }

    #[test]
    fn test_tool_entry() {
        let mut tool = ToolEntry::new(
            "123".to_string(),
            "Read".to_string(),
            Some("file.rs".to_string()),
        );
        assert_eq!(tool.status, ToolStatus::Running);
        assert!(tool.end_time.is_none());

        tool.complete(false);
        assert_eq!(tool.status, ToolStatus::Completed);
        assert!(tool.end_time.is_some());
    }

    #[test]
    fn test_agent_entry() {
        let mut agent = AgentEntry::new(
            "456".to_string(),
            "Explore".to_string(),
            Some("haiku".to_string()),
            Some("Finding code".to_string()),
        );
        assert_eq!(agent.status, AgentStatus::Running);
        assert!(agent.end_time.is_none());

        agent.complete();
        assert_eq!(agent.status, AgentStatus::Completed);
        assert!(agent.end_time.is_some());
    }

    #[test]
    fn test_activity_data() {
        let mut activity = ActivityData::default();
        assert!(!activity.has_activity());

        activity.tools.push(ToolEntry::new(
            "1".to_string(),
            "Read".to_string(),
            None,
        ));
        assert!(activity.has_activity());
        assert_eq!(activity.running_tools().len(), 1);
        assert_eq!(activity.completed_tools().len(), 0);

        activity.tools[0].complete(false);
        assert_eq!(activity.running_tools().len(), 0);
        assert_eq!(activity.completed_tools().len(), 1);
    }
}
