//! Tool Activity line rendering
//!
//! This module provides rendering logic for displaying tool call activity
//! in the statusline. It shows running tools with their targets and
//! completed tools with call counts.
//!
//! Output format examples:
//! - Running tool: `[running_icon] Edit: src/main.rs`
//! - Completed tool: `[completed_icon] Read x3`
//! - Error tool: `[error_icon] Bash x1`
//! - Full line: `[running_icon] Edit: src/main.rs | [completed_icon] Read x3 | [completed_icon] Grep x2`

use super::types::{truncate_path, ActivityData, ToolEntry, ToolStatus};
use crate::config::AnsiColor;
use std::collections::HashMap;

/// Unicode icons for tool status
pub mod icons {
    /// Running tool icon (half circle)
    pub const RUNNING: &str = "\u{25D0}";
    /// Completed tool icon (check mark)
    pub const COMPLETED: &str = "\u{2713}";
    /// Error tool icon (cross mark)
    pub const ERROR: &str = "\u{2717}";
}

/// Configuration for tools line rendering
#[derive(Debug, Clone)]
pub struct ToolsLineConfig {
    /// Maximum number of running tools to display (default: 2)
    pub max_running: usize,
    /// Maximum number of completed tool types to display (default: 4)
    pub max_completed: usize,
    /// Maximum length for target paths/strings (default: 20)
    pub max_target_len: usize,
    /// Color for running tool icon (default: Yellow)
    pub running_icon_color: Option<AnsiColor>,
    /// Color for completed tool icon (default: Green)
    pub completed_icon_color: Option<AnsiColor>,
    /// Color for error tool icon (default: Red)
    pub error_icon_color: Option<AnsiColor>,
    /// Color for tool names (default: Cyan)
    pub tool_name_color: Option<AnsiColor>,
    /// Color for dimmed text like counts and targets (default: Gray)
    pub dim_color: Option<AnsiColor>,
    /// Separator between tool entries (default: " | ")
    pub separator: String,
}

impl Default for ToolsLineConfig {
    fn default() -> Self {
        Self {
            max_running: 2,
            max_completed: 4,
            max_target_len: 20,
            running_icon_color: Some(AnsiColor::Color16 { c16: 3 }), // Yellow
            completed_icon_color: Some(AnsiColor::Color16 { c16: 2 }), // Green
            error_icon_color: Some(AnsiColor::Color16 { c16: 1 }), // Red
            tool_name_color: Some(AnsiColor::Color16 { c16: 6 }), // Cyan
            dim_color: Some(AnsiColor::Color16 { c16: 8 }), // Bright Black (Gray)
            separator: " | ".to_string(),
        }
    }
}

/// Statistics for a completed tool type
#[derive(Debug, Clone)]
struct ToolStats {
    /// Tool name
    name: String,
    /// Number of successful completions
    completed_count: usize,
    /// Number of errors
    error_count: usize,
}

impl ToolStats {
    fn new(name: String) -> Self {
        Self {
            name,
            completed_count: 0,
            error_count: 0,
        }
    }

    /// Total invocations (completed + errors)
    fn total(&self) -> usize {
        self.completed_count + self.error_count
    }

    /// Whether this tool has any errors
    fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

/// Apply ANSI color to text
fn apply_color(text: &str, color: Option<&AnsiColor>) -> String {
    match color {
        Some(AnsiColor::Color16 { c16 }) => {
            let code = if *c16 < 8 { 30 + c16 } else { 90 + (c16 - 8) };
            format!("\x1b[{}m{}\x1b[0m", code, text)
        }
        Some(AnsiColor::Color256 { c256 }) => {
            format!("\x1b[38;5;{}m{}\x1b[0m", c256, text)
        }
        Some(AnsiColor::Rgb { r, g, b }) => {
            format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text)
        }
        None => text.to_string(),
    }
}

/// Render a single running tool entry
fn render_running_tool(tool: &ToolEntry, config: &ToolsLineConfig) -> String {
    let icon = apply_color(icons::RUNNING, config.running_icon_color.as_ref());
    let name = apply_color(&tool.name, config.tool_name_color.as_ref());

    match &tool.target {
        Some(target) => {
            let truncated = truncate_path(target, config.max_target_len);
            let target_colored = apply_color(&truncated, config.dim_color.as_ref());
            format!("{} {}: {}", icon, name, target_colored)
        }
        None => format!("{} {}", icon, name),
    }
}

/// Render a single completed tool stats entry
fn render_completed_tool(stats: &ToolStats, config: &ToolsLineConfig) -> String {
    // Determine icon and color based on error status
    let (icon, icon_color) = if stats.has_errors() && stats.completed_count == 0 {
        // All errors
        (icons::ERROR, config.error_icon_color.as_ref())
    } else if stats.has_errors() {
        // Mixed: some errors, some completed - show completed icon but could indicate mixed
        (icons::COMPLETED, config.completed_icon_color.as_ref())
    } else {
        // All completed successfully
        (icons::COMPLETED, config.completed_icon_color.as_ref())
    };

    let icon_colored = apply_color(icon, icon_color);
    let name_colored = apply_color(&stats.name, config.tool_name_color.as_ref());

    // Format count
    let count = stats.total();
    if count > 1 {
        let count_str = format!("x{}", count);
        let count_colored = apply_color(&count_str, config.dim_color.as_ref());
        format!("{} {} {}", icon_colored, name_colored, count_colored)
    } else {
        format!("{} {}", icon_colored, name_colored)
    }
}

/// Collect statistics for completed tools
fn collect_completed_stats(tools: &[ToolEntry]) -> Vec<ToolStats> {
    let mut stats_map: HashMap<String, ToolStats> = HashMap::new();

    for tool in tools {
        match tool.status {
            ToolStatus::Completed => {
                let entry = stats_map
                    .entry(tool.name.clone())
                    .or_insert_with(|| ToolStats::new(tool.name.clone()));
                entry.completed_count += 1;
            }
            ToolStatus::Error => {
                let entry = stats_map
                    .entry(tool.name.clone())
                    .or_insert_with(|| ToolStats::new(tool.name.clone()));
                entry.error_count += 1;
            }
            ToolStatus::Running => {
                // Skip running tools
            }
        }
    }

    // Convert to vector and sort by total count (descending)
    let mut stats: Vec<ToolStats> = stats_map.into_values().collect();
    stats.sort_by(|a, b| b.total().cmp(&a.total()));

    stats
}

/// Render the tools activity line
///
/// # Arguments
/// * `activity` - Activity data containing tool entries
/// * `config` - Configuration for rendering
///
/// # Returns
/// `Some(String)` with the rendered line, or `None` if there's no activity to display.
///
/// # Output Format
/// Running tools are shown first with their targets, followed by completed tools
/// with their call counts, sorted by frequency.
///
/// Example: `[running_icon] Edit: src/main.rs | [completed_icon] Read x3 | [completed_icon] Grep x2`
pub fn render_tools_line(activity: &ActivityData, config: &ToolsLineConfig) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    // 1. Get running tools (most recent first, limited to max_running)
    let running_tools: Vec<&ToolEntry> = activity
        .tools
        .iter()
        .filter(|t| t.status == ToolStatus::Running)
        .rev() // Most recent first
        .take(config.max_running)
        .collect();

    // Render running tools
    for tool in running_tools {
        parts.push(render_running_tool(tool, config));
    }

    // 2. Collect and render completed tool statistics
    let completed_stats = collect_completed_stats(&activity.tools);

    // Take top N completed tools by count
    for stats in completed_stats.into_iter().take(config.max_completed) {
        parts.push(render_completed_tool(&stats, config));
    }

    // Return None if no parts to display
    if parts.is_empty() {
        return None;
    }

    Some(parts.join(&config.separator))
}

/// Render tools line with default configuration
pub fn render_tools_line_default(activity: &ActivityData) -> Option<String> {
    render_tools_line(activity, &ToolsLineConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn create_tool(id: &str, name: &str, target: Option<&str>, status: ToolStatus) -> ToolEntry {
        let mut tool = ToolEntry::new(
            id.to_string(),
            name.to_string(),
            target.map(String::from),
        );
        tool.status = status;
        if status != ToolStatus::Running {
            tool.end_time = Some(SystemTime::now());
        }
        tool
    }

    #[test]
    fn test_default_config() {
        let config = ToolsLineConfig::default();
        assert_eq!(config.max_running, 2);
        assert_eq!(config.max_completed, 4);
        assert_eq!(config.max_target_len, 20);
        assert_eq!(config.separator, " | ");
        assert!(config.running_icon_color.is_some());
        assert!(config.completed_icon_color.is_some());
        assert!(config.error_icon_color.is_some());
        assert!(config.tool_name_color.is_some());
        assert!(config.dim_color.is_some());
    }

    #[test]
    fn test_render_empty_activity() {
        let activity = ActivityData::default();
        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_render_single_running_tool() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool(
            "1",
            "Edit",
            Some("src/main.rs"),
            ToolStatus::Running,
        ));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains(icons::RUNNING));
        assert!(line.contains("Edit"));
        assert!(line.contains("src/main.rs"));
    }

    #[test]
    fn test_render_running_tool_without_target() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Read", None, ToolStatus::Running));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains(icons::RUNNING));
        assert!(line.contains("Read"));
    }

    #[test]
    fn test_render_completed_tools() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("2", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("3", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("4", "Grep", None, ToolStatus::Completed));
        activity.tools.push(create_tool("5", "Grep", None, ToolStatus::Completed));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains(icons::COMPLETED));
        assert!(line.contains("Read"));
        assert!(line.contains("x3")); // Read count
        assert!(line.contains("Grep"));
        assert!(line.contains("x2")); // Grep count
    }

    #[test]
    fn test_render_error_tools() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Bash", None, ToolStatus::Error));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains(icons::ERROR));
        assert!(line.contains("Bash"));
    }

    #[test]
    fn test_render_mixed_status() {
        let mut activity = ActivityData::default();
        // Running tool
        activity.tools.push(create_tool(
            "1",
            "Edit",
            Some("auth.ts"),
            ToolStatus::Running,
        ));
        // Completed tools
        activity.tools.push(create_tool("2", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("3", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("4", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("5", "Grep", None, ToolStatus::Completed));
        activity.tools.push(create_tool("6", "Grep", None, ToolStatus::Completed));
        // Error tool
        activity.tools.push(create_tool("7", "Bash", None, ToolStatus::Error));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should contain running icon for Edit
        assert!(line.contains(icons::RUNNING));
        assert!(line.contains("Edit"));
        assert!(line.contains("auth.ts"));

        // Should contain completed icons
        assert!(line.contains(icons::COMPLETED));
        assert!(line.contains("Read"));
        assert!(line.contains("x3"));

        // Should contain separator
        assert!(line.contains(" | "));
    }

    #[test]
    fn test_max_running_limit() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Edit", Some("file1.rs"), ToolStatus::Running));
        activity.tools.push(create_tool("2", "Read", Some("file2.rs"), ToolStatus::Running));
        activity.tools.push(create_tool("3", "Grep", Some("pattern"), ToolStatus::Running));

        let config = ToolsLineConfig {
            max_running: 2,
            ..Default::default()
        };
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should only show 2 running tools (most recent: Grep and Read)
        // Count occurrences of running icon
        let running_count = line.matches(icons::RUNNING).count();
        assert_eq!(running_count, 2);
    }

    #[test]
    fn test_max_completed_limit() {
        let mut activity = ActivityData::default();
        // Add 6 different completed tools
        for (i, name) in ["Read", "Write", "Edit", "Grep", "Glob", "Bash"].iter().enumerate() {
            activity.tools.push(create_tool(
                &format!("{}", i),
                name,
                None,
                ToolStatus::Completed,
            ));
        }

        let config = ToolsLineConfig {
            max_completed: 3,
            ..Default::default()
        };
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should only show 3 completed tools
        let completed_count = line.matches(icons::COMPLETED).count();
        assert_eq!(completed_count, 3);
    }

    #[test]
    fn test_completed_tools_sorted_by_count() {
        let mut activity = ActivityData::default();
        // Add tools with different counts
        activity.tools.push(create_tool("1", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("2", "Grep", None, ToolStatus::Completed));
        activity.tools.push(create_tool("3", "Grep", None, ToolStatus::Completed));
        activity.tools.push(create_tool("4", "Grep", None, ToolStatus::Completed));
        activity.tools.push(create_tool("5", "Edit", None, ToolStatus::Completed));
        activity.tools.push(create_tool("6", "Edit", None, ToolStatus::Completed));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Grep (3) should appear before Edit (2) which should appear before Read (1)
        let grep_pos = line.find("Grep").unwrap();
        let edit_pos = line.find("Edit").unwrap();
        let read_pos = line.find("Read").unwrap();

        assert!(grep_pos < edit_pos);
        assert!(edit_pos < read_pos);
    }

    #[test]
    fn test_target_truncation() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool(
            "1",
            "Edit",
            Some("very/long/path/to/some/deeply/nested/file.rs"),
            ToolStatus::Running,
        ));

        let config = ToolsLineConfig {
            max_target_len: 15,
            ..Default::default()
        };
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should contain truncated path
        assert!(line.contains(".../file.rs") || line.contains("..."));
    }

    #[test]
    fn test_custom_separator() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Read", None, ToolStatus::Completed));
        activity.tools.push(create_tool("2", "Write", None, ToolStatus::Completed));

        let config = ToolsLineConfig {
            separator: " :: ".to_string(),
            ..Default::default()
        };
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains(" :: "));
    }

    #[test]
    fn test_single_completed_no_count() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Read", None, ToolStatus::Completed));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Single tool should not show "x1"
        assert!(!line.contains("x1"));
        assert!(line.contains("Read"));
    }

    #[test]
    fn test_render_tools_line_default() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Read", None, ToolStatus::Completed));

        let result = render_tools_line_default(&activity);
        assert!(result.is_some());
    }

    #[test]
    fn test_apply_color_color16() {
        let text = "test";
        let color = AnsiColor::Color16 { c16: 2 }; // Green
        let result = apply_color(text, Some(&color));
        assert!(result.contains("\x1b[32m")); // Green foreground
        assert!(result.contains("test"));
        assert!(result.contains("\x1b[0m")); // Reset
    }

    #[test]
    fn test_apply_color_color256() {
        let text = "test";
        let color = AnsiColor::Color256 { c256: 208 }; // Orange
        let result = apply_color(text, Some(&color));
        assert!(result.contains("\x1b[38;5;208m"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_apply_color_rgb() {
        let text = "test";
        let color = AnsiColor::Rgb { r: 255, g: 128, b: 0 };
        let result = apply_color(text, Some(&color));
        assert!(result.contains("\x1b[38;2;255;128;0m"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_apply_color_none() {
        let text = "test";
        let result = apply_color(text, None);
        assert_eq!(result, "test");
    }

    #[test]
    fn test_tool_stats() {
        let mut stats = ToolStats::new("Read".to_string());
        assert_eq!(stats.total(), 0);
        assert!(!stats.has_errors());

        stats.completed_count = 3;
        assert_eq!(stats.total(), 3);
        assert!(!stats.has_errors());

        stats.error_count = 1;
        assert_eq!(stats.total(), 4);
        assert!(stats.has_errors());
    }

    #[test]
    fn test_collect_completed_stats() {
        let tools = vec![
            create_tool("1", "Read", None, ToolStatus::Completed),
            create_tool("2", "Read", None, ToolStatus::Completed),
            create_tool("3", "Read", None, ToolStatus::Error),
            create_tool("4", "Write", None, ToolStatus::Completed),
            create_tool("5", "Edit", None, ToolStatus::Running), // Should be skipped
        ];

        let stats = collect_completed_stats(&tools);

        // Should have 2 tool types (Read and Write), Edit is running
        assert_eq!(stats.len(), 2);

        // Read should be first (3 total: 2 completed + 1 error)
        assert_eq!(stats[0].name, "Read");
        assert_eq!(stats[0].completed_count, 2);
        assert_eq!(stats[0].error_count, 1);
        assert_eq!(stats[0].total(), 3);

        // Write should be second (1 completed)
        assert_eq!(stats[1].name, "Write");
        assert_eq!(stats[1].completed_count, 1);
        assert_eq!(stats[1].error_count, 0);
    }

    #[test]
    fn test_bright_color16() {
        let text = "test";
        let color = AnsiColor::Color16 { c16: 9 }; // Bright Red
        let result = apply_color(text, Some(&color));
        assert!(result.contains("\x1b[91m")); // Bright red (90 + 1)
    }

    #[test]
    fn test_mixed_completed_and_error_same_tool() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Bash", None, ToolStatus::Completed));
        activity.tools.push(create_tool("2", "Bash", None, ToolStatus::Error));
        activity.tools.push(create_tool("3", "Bash", None, ToolStatus::Completed));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should show completed icon (mixed status shows completed)
        assert!(line.contains(icons::COMPLETED));
        assert!(line.contains("Bash"));
        assert!(line.contains("x3")); // Total count
    }

    #[test]
    fn test_only_errors_shows_error_icon() {
        let mut activity = ActivityData::default();
        activity.tools.push(create_tool("1", "Bash", None, ToolStatus::Error));
        activity.tools.push(create_tool("2", "Bash", None, ToolStatus::Error));

        let config = ToolsLineConfig::default();
        let result = render_tools_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should show error icon when all are errors
        assert!(line.contains(icons::ERROR));
        assert!(line.contains("Bash"));
        assert!(line.contains("x2"));
    }
}
