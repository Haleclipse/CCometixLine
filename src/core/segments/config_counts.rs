//! Config Counts Segment
//!
//! Displays counts of Claude Code configuration items:
//! - CLAUDE.md files
//! - Rule files (.md in rules directories)
//! - MCP servers configured
//! - Hooks configured
//!
//! Output format: "2 CLAUDE.md | 3 rules | 5 MCPs | 2 hooks"
//! Only shows non-zero counts. Returns None if no configs found.

use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use crate::core::activity::cache::get_config_counts_cached;
use crate::core::activity::types::ConfigCounts;
use std::collections::HashMap;

/// Segment that displays configuration counts from Claude Code environment
pub struct ConfigCountsSegment {
    /// Optional cache TTL in seconds. If None, uses default (60 seconds)
    cache_ttl: Option<u64>,
}

impl Default for ConfigCountsSegment {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigCountsSegment {
    /// Create a new ConfigCountsSegment with default cache TTL
    pub fn new() -> Self {
        Self { cache_ttl: None }
    }

    /// Create a new ConfigCountsSegment with a custom cache TTL
    ///
    /// # Arguments
    ///
    /// * `ttl_secs` - Cache TTL in seconds
    pub fn with_cache_ttl(ttl_secs: u64) -> Self {
        Self {
            cache_ttl: Some(ttl_secs),
        }
    }
}

impl Segment for ConfigCountsSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let counts =
            get_config_counts_cached(Some(&input.workspace.current_dir), self.cache_ttl);

        if !counts.has_any() {
            return None;
        }

        // Build display string: "2 CLAUDE.md | 3 rules | 5 MCPs | 2 hooks"
        let parts = build_display_parts(&counts);

        let mut metadata = HashMap::new();
        metadata.insert(
            "claude_md_count".to_string(),
            counts.claude_md_count.to_string(),
        );
        metadata.insert("rules_count".to_string(), counts.rules_count.to_string());
        metadata.insert("mcp_count".to_string(), counts.mcp_count.to_string());
        metadata.insert("hooks_count".to_string(), counts.hooks_count.to_string());
        metadata.insert("total".to_string(), counts.total().to_string());

        Some(SegmentData {
            primary: parts.join(" | "),
            secondary: String::new(),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::ConfigCounts
    }
}

/// Build display parts from config counts
///
/// Only includes non-zero counts in the output.
///
/// # Arguments
///
/// * `counts` - The configuration counts to format
///
/// # Returns
///
/// A vector of formatted strings for each non-zero count
///
/// # Examples
///
/// ```ignore
/// let counts = ConfigCounts {
///     claude_md_count: 2,
///     rules_count: 3,
///     mcp_count: 0,
///     hooks_count: 1,
/// };
/// let parts = build_display_parts(&counts);
/// // parts = ["2 CLAUDE.md", "3 rules", "1 hook"]
/// ```
fn build_display_parts(counts: &ConfigCounts) -> Vec<String> {
    let mut parts = Vec::new();

    if counts.claude_md_count > 0 {
        parts.push(format!("{} CLAUDE.md", counts.claude_md_count));
    }

    if counts.rules_count > 0 {
        // Use singular/plural form
        let label = if counts.rules_count == 1 {
            "rule"
        } else {
            "rules"
        };
        parts.push(format!("{} {}", counts.rules_count, label));
    }

    if counts.mcp_count > 0 {
        // Use singular/plural form
        let label = if counts.mcp_count == 1 { "MCP" } else { "MCPs" };
        parts.push(format!("{} {}", counts.mcp_count, label));
    }

    if counts.hooks_count > 0 {
        // Use singular/plural form
        let label = if counts.hooks_count == 1 {
            "hook"
        } else {
            "hooks"
        };
        parts.push(format!("{} {}", counts.hooks_count, label));
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_display_parts_all_counts() {
        let counts = ConfigCounts {
            claude_md_count: 2,
            rules_count: 3,
            mcp_count: 5,
            hooks_count: 2,
        };

        let parts = build_display_parts(&counts);

        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "2 CLAUDE.md");
        assert_eq!(parts[1], "3 rules");
        assert_eq!(parts[2], "5 MCPs");
        assert_eq!(parts[3], "2 hooks");
    }

    #[test]
    fn test_build_display_parts_single_counts() {
        let counts = ConfigCounts {
            claude_md_count: 1,
            rules_count: 1,
            mcp_count: 1,
            hooks_count: 1,
        };

        let parts = build_display_parts(&counts);

        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "1 CLAUDE.md");
        assert_eq!(parts[1], "1 rule");
        assert_eq!(parts[2], "1 MCP");
        assert_eq!(parts[3], "1 hook");
    }

    #[test]
    fn test_build_display_parts_partial_counts() {
        let counts = ConfigCounts {
            claude_md_count: 2,
            rules_count: 0,
            mcp_count: 3,
            hooks_count: 0,
        };

        let parts = build_display_parts(&counts);

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "2 CLAUDE.md");
        assert_eq!(parts[1], "3 MCPs");
    }

    #[test]
    fn test_build_display_parts_only_claude_md() {
        let counts = ConfigCounts {
            claude_md_count: 1,
            rules_count: 0,
            mcp_count: 0,
            hooks_count: 0,
        };

        let parts = build_display_parts(&counts);

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], "1 CLAUDE.md");
    }

    #[test]
    fn test_build_display_parts_empty() {
        let counts = ConfigCounts::default();

        let parts = build_display_parts(&counts);

        assert!(parts.is_empty());
    }

    #[test]
    fn test_config_counts_segment_new() {
        let segment = ConfigCountsSegment::new();
        assert!(segment.cache_ttl.is_none());
    }

    #[test]
    fn test_config_counts_segment_with_cache_ttl() {
        let segment = ConfigCountsSegment::with_cache_ttl(30);
        assert_eq!(segment.cache_ttl, Some(30));
    }

    #[test]
    fn test_config_counts_segment_default() {
        let segment = ConfigCountsSegment::default();
        assert!(segment.cache_ttl.is_none());
    }

    #[test]
    fn test_config_counts_segment_id() {
        let segment = ConfigCountsSegment::new();
        assert_eq!(segment.id(), SegmentId::ConfigCounts);
    }

    #[test]
    fn test_display_format_joined() {
        let counts = ConfigCounts {
            claude_md_count: 2,
            rules_count: 3,
            mcp_count: 5,
            hooks_count: 2,
        };

        let parts = build_display_parts(&counts);
        let display = parts.join(" | ");

        assert_eq!(display, "2 CLAUDE.md | 3 rules | 5 MCPs | 2 hooks");
    }

    #[test]
    fn test_display_format_single_item() {
        let counts = ConfigCounts {
            claude_md_count: 0,
            rules_count: 0,
            mcp_count: 1,
            hooks_count: 0,
        };

        let parts = build_display_parts(&counts);
        let display = parts.join(" | ");

        assert_eq!(display, "1 MCP");
    }
}
