//! Activity tracking and configuration counting module
//!
//! This module provides functionality for:
//! - Tracking tool and agent activity from transcripts
//! - Counting Claude Code configuration items (CLAUDE.md, rules, MCPs, hooks)
//! - Caching configuration counts with TTL support
//! - Parsing transcript files for activity data

pub mod agents_line;
pub mod cache;
pub mod config_counter;
pub mod tools_line;
pub mod transcript_parser;
pub mod types;

// Re-export commonly used items
pub use agents_line::{render_agents_line, AgentsLineConfig};
pub use cache::{get_config_counts_cached, invalidate_cache, DEFAULT_TTL_SECS};
pub use config_counter::count_configs;
pub use tools_line::{render_tools_line, ToolsLineConfig};
pub use transcript_parser::parse_transcript_activity;
pub use types::{
    format_duration, truncate_path, truncate_string, ActivityData, AgentEntry, AgentStatus,
    ConfigCounts, ToolEntry, ToolStatus,
};
