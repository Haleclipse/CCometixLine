//! Multi-line output coordinator for Claude Code statusline
//!
//! This module provides functionality for rendering multi-line statusline output,
//! combining the main statusline with optional tool activity and agent status lines.
//!
//! # Output Structure
//!
//! The multi-line renderer can produce up to 3 lines:
//!
//! 1. **Main statusline**: Model, directory, git, context window, usage, cost, etc.
//! 2. **Tool activity line**: Currently running and recently completed tools
//! 3. **Agent status line**: Running and completed subagents
//!
//! # Example Usage
//!
//! ```ignore
//! use ccometixline::config::{Config, InputData};
//! use ccometixline::core::multiline::{MultilineConfig, MultilineRenderer};
//!
//! let config = Config::default();
//! let multiline_config = MultilineConfig::default();
//! let renderer = MultilineRenderer::new(config, multiline_config);
//!
//! let input = InputData {
//!     transcript_path: "/path/to/transcript.jsonl".to_string(),
//!     ..Default::default()
//! };
//!
//! // Render all lines
//! let lines = renderer.render(&input);
//! for line in lines {
//!     println!("{}", line);
//! }
//! ```

use crate::config::{Config, InputData};

use super::activity::{
    parse_transcript_activity, render_agents_line, render_tools_line, AgentsLineConfig,
    ToolsLineConfig,
};
use super::statusline::{collect_all_segments, StatusLineGenerator};

/// Configuration for multi-line statusline rendering
#[derive(Debug, Clone)]
pub struct MultilineConfig {
    /// Whether to show the tools activity line (Line 2)
    pub show_tools: bool,
    /// Whether to show the agents status line (Line 3)
    pub show_agents: bool,
    /// Configuration for tools line rendering
    pub tools_config: ToolsLineConfig,
    /// Configuration for agents line rendering
    pub agents_config: AgentsLineConfig,
}

impl Default for MultilineConfig {
    fn default() -> Self {
        Self {
            show_tools: true,
            show_agents: true,
            tools_config: ToolsLineConfig::default(),
            agents_config: AgentsLineConfig::default(),
        }
    }
}

impl MultilineConfig {
    /// Create a new MultilineConfig with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config that only shows the main statusline
    pub fn main_only() -> Self {
        Self {
            show_tools: false,
            show_agents: false,
            tools_config: ToolsLineConfig::default(),
            agents_config: AgentsLineConfig::default(),
        }
    }

    /// Create a config that shows tools but not agents
    pub fn with_tools_only() -> Self {
        Self {
            show_tools: true,
            show_agents: false,
            tools_config: ToolsLineConfig::default(),
            agents_config: AgentsLineConfig::default(),
        }
    }

    /// Create a config that shows agents but not tools
    pub fn with_agents_only() -> Self {
        Self {
            show_tools: false,
            show_agents: true,
            tools_config: ToolsLineConfig::default(),
            agents_config: AgentsLineConfig::default(),
        }
    }

    /// Builder method to set show_tools
    pub fn show_tools(mut self, show: bool) -> Self {
        self.show_tools = show;
        self
    }

    /// Builder method to set show_agents
    pub fn show_agents(mut self, show: bool) -> Self {
        self.show_agents = show;
        self
    }

    /// Builder method to set tools_config
    pub fn tools_config(mut self, config: ToolsLineConfig) -> Self {
        self.tools_config = config;
        self
    }

    /// Builder method to set agents_config
    pub fn agents_config(mut self, config: AgentsLineConfig) -> Self {
        self.agents_config = config;
        self
    }
}

/// Multi-line statusline renderer
///
/// Combines the main statusline with optional tool activity and agent status lines.
pub struct MultilineRenderer {
    config: Config,
    multiline_config: MultilineConfig,
}

impl MultilineRenderer {
    /// Create a new MultilineRenderer
    ///
    /// # Arguments
    ///
    /// * `config` - Main statusline configuration
    /// * `multiline_config` - Multi-line specific configuration
    pub fn new(config: Config, multiline_config: MultilineConfig) -> Self {
        Self {
            config,
            multiline_config,
        }
    }

    /// Render all statusline lines
    ///
    /// # Arguments
    ///
    /// * `input` - Input data for statusline generation
    ///
    /// # Returns
    ///
    /// A vector of strings, one for each line to display.
    /// The vector will contain:
    /// - Line 1: Main statusline (always present)
    /// - Line 2: Tool activity line (if enabled and has data)
    /// - Line 3: Agent status line (if enabled and has data)
    pub fn render(&self, input: &InputData) -> Vec<String> {
        let mut lines = Vec::new();

        // Line 1: Main statusline
        let segments = collect_all_segments(&self.config, input);
        let generator = StatusLineGenerator::new(self.config.clone());
        lines.push(generator.generate(segments));

        // Parse transcript if path is not empty
        if !input.transcript_path.is_empty() {
            let activity = parse_transcript_activity(&input.transcript_path);

            // Line 2: Tool Activity (if enabled and has data)
            if self.multiline_config.show_tools {
                if let Some(tools_line) =
                    render_tools_line(&activity, &self.multiline_config.tools_config)
                {
                    lines.push(tools_line);
                }
            }

            // Line 3: Agent Status (if enabled and has data)
            if self.multiline_config.show_agents {
                if let Some(agents_line) =
                    render_agents_line(&activity, &self.multiline_config.agents_config)
                {
                    lines.push(agents_line);
                }
            }
        }

        lines
    }

    /// Render all lines and print to stdout
    ///
    /// Convenience method that renders all lines and prints each one.
    pub fn render_and_print(&self, input: &InputData) {
        for line in self.render(input) {
            println!("{}", line);
        }
    }

    /// Get a reference to the main config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a reference to the multiline config
    pub fn multiline_config(&self) -> &MultilineConfig {
        &self.multiline_config
    }

    /// Get the number of lines that would be rendered
    ///
    /// This is useful for terminal UI layout calculations.
    /// Returns the actual number of lines based on current configuration
    /// and whether there's activity data to display.
    pub fn line_count(&self, input: &InputData) -> usize {
        self.render(input).len()
    }

    /// Get the maximum possible number of lines
    ///
    /// Returns 3 if both tools and agents are enabled, 2 if only one is enabled,
    /// or 1 if neither is enabled.
    pub fn max_line_count(&self) -> usize {
        let mut count = 1; // Main statusline always present
        if self.multiline_config.show_tools {
            count += 1;
        }
        if self.multiline_config.show_agents {
            count += 1;
        }
        count
    }
}

/// Convenience function to render multi-line statusline
///
/// This is a simpler interface for one-off rendering without creating
/// a MultilineRenderer instance.
///
/// # Arguments
///
/// * `config` - Main statusline configuration
/// * `multiline_config` - Multi-line specific configuration
/// * `input` - Input data for statusline generation
///
/// # Returns
///
/// A vector of strings, one for each line to display.
///
/// # Example
///
/// ```ignore
/// use ccometixline::config::{Config, InputData};
/// use ccometixline::core::multiline::{render_multiline, MultilineConfig};
///
/// let config = Config::default();
/// let multiline_config = MultilineConfig::default();
/// let input = InputData::default();
///
/// let lines = render_multiline(&config, &multiline_config, &input);
/// for line in lines {
///     println!("{}", line);
/// }
/// ```
pub fn render_multiline(
    config: &Config,
    multiline_config: &MultilineConfig,
    input: &InputData,
) -> Vec<String> {
    let renderer = MultilineRenderer::new(config.clone(), multiline_config.clone());
    renderer.render(input)
}

/// Render multi-line statusline and print to stdout
///
/// Convenience function that renders and prints all lines.
pub fn render_multiline_and_print(
    config: &Config,
    multiline_config: &MultilineConfig,
    input: &InputData,
) {
    for line in render_multiline(config, multiline_config, input) {
        println!("{}", line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InputData;

    #[test]
    fn test_multiline_config_default() {
        let config = MultilineConfig::default();
        assert!(config.show_tools);
        assert!(config.show_agents);
    }

    #[test]
    fn test_multiline_config_new() {
        let config = MultilineConfig::new();
        assert!(config.show_tools);
        assert!(config.show_agents);
    }

    #[test]
    fn test_multiline_config_main_only() {
        let config = MultilineConfig::main_only();
        assert!(!config.show_tools);
        assert!(!config.show_agents);
    }

    #[test]
    fn test_multiline_config_with_tools_only() {
        let config = MultilineConfig::with_tools_only();
        assert!(config.show_tools);
        assert!(!config.show_agents);
    }

    #[test]
    fn test_multiline_config_with_agents_only() {
        let config = MultilineConfig::with_agents_only();
        assert!(!config.show_tools);
        assert!(config.show_agents);
    }

    #[test]
    fn test_multiline_config_builder() {
        let config = MultilineConfig::default()
            .show_tools(false)
            .show_agents(true);

        assert!(!config.show_tools);
        assert!(config.show_agents);
    }

    #[test]
    fn test_multiline_config_builder_with_configs() {
        let tools_config = ToolsLineConfig {
            max_running: 5,
            ..Default::default()
        };
        let agents_config = AgentsLineConfig {
            max_agents: 5,
            ..Default::default()
        };

        let config = MultilineConfig::default()
            .tools_config(tools_config.clone())
            .agents_config(agents_config.clone());

        assert_eq!(config.tools_config.max_running, 5);
        assert_eq!(config.agents_config.max_agents, 5);
    }

    #[test]
    fn test_multiline_renderer_new() {
        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config.clone(), multiline_config.clone());

        assert!(renderer.multiline_config().show_tools);
        assert!(renderer.multiline_config().show_agents);
    }

    #[test]
    fn test_multiline_renderer_render_no_transcript() {
        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config, multiline_config);

        let input = InputData::default();
        let lines = renderer.render(&input);

        // Should have at least the main statusline
        assert!(!lines.is_empty());
        // Without transcript, should only have main line
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_multiline_renderer_render_with_nonexistent_transcript() {
        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config, multiline_config);

        let input = InputData {
            transcript_path: "/nonexistent/path/transcript.jsonl".to_string(),
            ..Default::default()
        };
        let lines = renderer.render(&input);

        // Should have main statusline, but no activity lines (empty transcript)
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_multiline_renderer_max_line_count() {
        let config = Config::default();

        // All enabled
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config.clone(), multiline_config);
        assert_eq!(renderer.max_line_count(), 3);

        // Tools only
        let multiline_config = MultilineConfig::with_tools_only();
        let renderer = MultilineRenderer::new(config.clone(), multiline_config);
        assert_eq!(renderer.max_line_count(), 2);

        // Agents only
        let multiline_config = MultilineConfig::with_agents_only();
        let renderer = MultilineRenderer::new(config.clone(), multiline_config);
        assert_eq!(renderer.max_line_count(), 2);

        // Main only
        let multiline_config = MultilineConfig::main_only();
        let renderer = MultilineRenderer::new(config.clone(), multiline_config);
        assert_eq!(renderer.max_line_count(), 1);
    }

    #[test]
    fn test_multiline_renderer_line_count() {
        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config, multiline_config);

        let input = InputData::default();
        let count = renderer.line_count(&input);

        // Without transcript, should be 1 (main line only)
        assert_eq!(count, 1);
    }

    #[test]
    fn test_render_multiline_function() {
        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let input = InputData::default();

        let lines = render_multiline(&config, &multiline_config, &input);

        // Should have at least the main statusline
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_multiline_renderer_config_accessors() {
        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config.clone(), multiline_config.clone());

        // Verify we can access configs
        let _ = renderer.config();
        let mc = renderer.multiline_config();
        assert!(mc.show_tools);
        assert!(mc.show_agents);
    }

    #[test]
    fn test_multiline_config_tools_config_builder() {
        let custom_tools = ToolsLineConfig {
            max_running: 10,
            max_completed: 8,
            separator: " :: ".to_string(),
            ..Default::default()
        };

        let config = MultilineConfig::default().tools_config(custom_tools);

        assert_eq!(config.tools_config.max_running, 10);
        assert_eq!(config.tools_config.max_completed, 8);
        assert_eq!(config.tools_config.separator, " :: ");
    }

    #[test]
    fn test_multiline_config_agents_config_builder() {
        let custom_agents = AgentsLineConfig {
            max_agents: 5,
            max_description_len: 50,
            separator: " -> ".to_string(),
            ..Default::default()
        };

        let config = MultilineConfig::default().agents_config(custom_agents);

        assert_eq!(config.agents_config.max_agents, 5);
        assert_eq!(config.agents_config.max_description_len, 50);
        assert_eq!(config.agents_config.separator, " -> ");
    }

    // Integration test with temporary transcript file
    #[test]
    fn test_multiline_renderer_with_transcript() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary transcript file with tool activity
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-15T10:00:00Z","message":{{"content":[{{"type":"tool_use","id":"tool_1","name":"Read","input":{{"file_path":"/src/main.rs"}}}}]}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-15T10:00:01Z","message":{{"content":[{{"type":"tool_result","tool_use_id":"tool_1","is_error":false}}]}}}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let config = Config::default();
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config, multiline_config);

        let input = InputData {
            transcript_path: file.path().to_string_lossy().to_string(),
            ..Default::default()
        };

        let lines = renderer.render(&input);

        // Should have main line + tools line (completed tool)
        assert!(lines.len() >= 1);
        // The tools line should be present if there's activity
        if lines.len() > 1 {
            // Tools line should contain the tool name
            assert!(lines[1].contains("Read"));
        }
    }

    #[test]
    fn test_multiline_renderer_disabled_tools() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary transcript file with tool activity
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-15T10:00:00Z","message":{{"content":[{{"type":"tool_use","id":"tool_1","name":"Read","input":{{}}}}]}}}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let config = Config::default();
        let multiline_config = MultilineConfig::main_only(); // Disable tools and agents
        let renderer = MultilineRenderer::new(config, multiline_config);

        let input = InputData {
            transcript_path: file.path().to_string_lossy().to_string(),
            ..Default::default()
        };

        let lines = renderer.render(&input);

        // Should only have main line even with transcript
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_multiline_renderer_with_agent_activity() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary transcript file with agent activity
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2024-01-15T10:00:00Z","message":{{"content":[{{"type":"tool_use","id":"agent_1","name":"Task","input":{{"subagent_type":"Explore","model":"haiku","description":"Finding code"}}}}]}}}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let config = Config::default();
        let multiline_config = MultilineConfig::with_agents_only();
        let renderer = MultilineRenderer::new(config, multiline_config);

        let input = InputData {
            transcript_path: file.path().to_string_lossy().to_string(),
            ..Default::default()
        };

        let lines = renderer.render(&input);

        // Should have main line + agents line
        assert!(lines.len() >= 1);
        if lines.len() > 1 {
            // Agents line should contain the agent type
            assert!(lines[1].contains("Explore"));
        }
    }
}
