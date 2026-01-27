//! Agent status line rendering
//!
//! This module provides rendering for agent (subagent) status display,
//! showing running and recently completed agents in a formatted line.

use super::types::{ActivityData, AgentEntry, AgentStatus, format_duration, truncate_string};
use crate::config::AnsiColor;

/// Unicode icon for running agent
const RUNNING_ICON: &str = "\u{25D0}"; // Half-filled circle

/// Unicode icon for completed agent
const COMPLETED_ICON: &str = "\u{2713}"; // Check mark

/// Configuration for agent status line rendering
#[derive(Debug, Clone)]
pub struct AgentsLineConfig {
    /// Maximum number of agents to display (default: 3)
    pub max_agents: usize,
    /// Maximum length for description text (default: 40)
    pub max_description_len: usize,
    /// Color for running agent icon
    pub running_icon_color: Option<AnsiColor>,
    /// Color for completed agent icon
    pub completed_icon_color: Option<AnsiColor>,
    /// Color for agent type text (default: Magenta)
    pub agent_type_color: Option<AnsiColor>,
    /// Color for dimmed text (elapsed time, brackets)
    pub dim_color: Option<AnsiColor>,
    /// Separator between agents (default: " | ")
    pub separator: String,
}

impl Default for AgentsLineConfig {
    fn default() -> Self {
        Self {
            max_agents: 3,
            max_description_len: 40,
            running_icon_color: Some(AnsiColor::Color16 { c16: 3 }), // Yellow
            completed_icon_color: Some(AnsiColor::Color16 { c16: 2 }), // Green
            agent_type_color: Some(AnsiColor::Color16 { c16: 5 }), // Magenta
            dim_color: Some(AnsiColor::Color16 { c16: 8 }), // Bright black (gray)
            separator: " | ".to_string(),
        }
    }
}

/// Render the agents status line
///
/// # Arguments
/// * `activity` - Activity data containing agent entries
/// * `config` - Configuration for rendering
///
/// # Returns
/// `Some(String)` with the formatted line, or `None` if no agents to display
///
/// # Output Format Examples
/// - Running agent: `\u{25D0} Explore [haiku]: Finding auth code (2m 15s)`
/// - Completed agent: `\u{2713} fix: authentication bug (30s)`
/// - Full line: `\u{25D0} Explore [haiku]: Finding auth code (2m 15s) | \u{2713} fix: authentication bug (30s)`
pub fn render_agents_line(activity: &ActivityData, config: &AgentsLineConfig) -> Option<String> {
    // 1. Get all running agents
    let running_agents: Vec<&AgentEntry> = activity.running_agents();

    // 2. Get recently completed agents (last 2)
    let completed_agents: Vec<&AgentEntry> = activity
        .completed_agents()
        .into_iter()
        .rev() // Most recent first
        .take(2)
        .collect();

    // 3. Merge and limit total to max_agents
    let mut agents_to_display: Vec<&AgentEntry> = Vec::new();

    // Add running agents first (they have priority)
    for agent in &running_agents {
        if agents_to_display.len() >= config.max_agents {
            break;
        }
        agents_to_display.push(agent);
    }

    // Add completed agents if there's room
    for agent in &completed_agents {
        if agents_to_display.len() >= config.max_agents {
            break;
        }
        agents_to_display.push(agent);
    }

    // Return None if no agents to display
    if agents_to_display.is_empty() {
        return None;
    }

    // 4. Format each agent
    let formatted: Vec<String> = agents_to_display
        .iter()
        .map(|agent| format_agent(agent, config))
        .collect();

    Some(formatted.join(&config.separator))
}

/// Format a single agent entry
///
/// Running format: `\u{25D0} {agent_type} [{model}]: {description} ({elapsed})`
/// Completed format: `\u{2713} {agent_type}: {description} ({elapsed})`
fn format_agent(agent: &AgentEntry, config: &AgentsLineConfig) -> String {
    let is_running = agent.status == AgentStatus::Running;

    // Icon with color
    let icon = if is_running {
        apply_color(RUNNING_ICON, config.running_icon_color.as_ref())
    } else {
        apply_color(COMPLETED_ICON, config.completed_icon_color.as_ref())
    };

    // Agent type with color
    let agent_type = apply_color(&agent.agent_type, config.agent_type_color.as_ref());

    // Model part (only for running agents with model)
    let model_part = if is_running {
        if let Some(model) = &agent.model {
            let bracket_open = apply_color("[", config.dim_color.as_ref());
            let bracket_close = apply_color("]", config.dim_color.as_ref());
            format!(" {}{}{}", bracket_open, model, bracket_close)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Description part
    let description_part = if let Some(desc) = &agent.description {
        let truncated = truncate_string(desc, config.max_description_len);
        format!(": {}", truncated)
    } else {
        String::new()
    };

    // Elapsed time
    let elapsed = format_duration(agent.elapsed());
    let elapsed_formatted = apply_color(&format!("({})", elapsed), config.dim_color.as_ref());

    format!("{} {}{}{} {}", icon, agent_type, model_part, description_part, elapsed_formatted)
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

/// Strip ANSI escape sequences from text (for testing)
#[cfg(test)]
fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            in_escape = true;
            if chars.peek() == Some(&'[') {
                chars.next();
            }
        } else if in_escape {
            if ch.is_alphabetic() {
                in_escape = false;
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn create_running_agent(
        agent_type: &str,
        model: Option<&str>,
        description: Option<&str>,
    ) -> AgentEntry {
        AgentEntry {
            id: "test_id".to_string(),
            agent_type: agent_type.to_string(),
            model: model.map(String::from),
            description: description.map(String::from),
            status: AgentStatus::Running,
            start_time: SystemTime::now() - Duration::from_secs(135), // 2m 15s ago
            end_time: None,
        }
    }

    fn create_completed_agent(
        agent_type: &str,
        description: Option<&str>,
        elapsed_secs: u64,
    ) -> AgentEntry {
        let start = SystemTime::now() - Duration::from_secs(elapsed_secs);
        AgentEntry {
            id: "test_id".to_string(),
            agent_type: agent_type.to_string(),
            model: None,
            description: description.map(String::from),
            status: AgentStatus::Completed,
            start_time: start,
            end_time: Some(SystemTime::now()),
        }
    }

    #[test]
    fn test_default_config() {
        let config = AgentsLineConfig::default();
        assert_eq!(config.max_agents, 3);
        assert_eq!(config.max_description_len, 40);
        assert_eq!(config.separator, " | ");
        assert!(config.running_icon_color.is_some());
        assert!(config.completed_icon_color.is_some());
        assert!(config.agent_type_color.is_some());
        assert!(config.dim_color.is_some());
    }

    #[test]
    fn test_render_empty_activity() {
        let activity = ActivityData::default();
        let config = AgentsLineConfig::default();

        let result = render_agents_line(&activity, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_render_single_running_agent() {
        let mut activity = ActivityData::default();
        activity.agents.push(create_running_agent(
            "Explore",
            Some("haiku"),
            Some("Finding auth code"),
        ));

        let config = AgentsLineConfig::default();
        let result = render_agents_line(&activity, &config);

        assert!(result.is_some());
        let line = strip_ansi(&result.unwrap());

        // Check components are present
        assert!(line.contains(RUNNING_ICON));
        assert!(line.contains("Explore"));
        assert!(line.contains("[haiku]"));
        assert!(line.contains("Finding auth code"));
        assert!(line.contains("2m 15s"));
    }

    #[test]
    fn test_render_running_agent_without_model() {
        let mut activity = ActivityData::default();
        activity.agents.push(create_running_agent(
            "Explore",
            None,
            Some("Finding code"),
        ));

        let config = AgentsLineConfig::default();
        let result = render_agents_line(&activity, &config);

        assert!(result.is_some());
        let line = strip_ansi(&result.unwrap());

        // Should not contain brackets for model
        assert!(!line.contains("["));
        assert!(!line.contains("]"));
        assert!(line.contains("Explore"));
        assert!(line.contains("Finding code"));
    }

    #[test]
    fn test_render_running_agent_without_description() {
        let mut activity = ActivityData::default();
        activity.agents.push(create_running_agent(
            "Explore",
            Some("haiku"),
            None,
        ));

        let config = AgentsLineConfig::default();
        let result = render_agents_line(&activity, &config);

        assert!(result.is_some());
        let line = strip_ansi(&result.unwrap());

        // Should not contain colon before description
        assert!(line.contains("Explore"));
        assert!(line.contains("[haiku]"));
        // The format should be: icon Explore [haiku] (elapsed)
        // Not: icon Explore [haiku]: (elapsed)
    }

    #[test]
    fn test_render_completed_agent() {
        let mut activity = ActivityData::default();
        activity.agents.push(create_completed_agent(
            "fix",
            Some("authentication bug"),
            30,
        ));

        let config = AgentsLineConfig::default();
        let result = render_agents_line(&activity, &config);

        assert!(result.is_some());
        let line = strip_ansi(&result.unwrap());

        assert!(line.contains(COMPLETED_ICON));
        assert!(line.contains("fix"));
        assert!(line.contains("authentication bug"));
        // Completed agents don't show model
        assert!(!line.contains("["));
    }

    #[test]
    fn test_render_multiple_agents() {
        let mut activity = ActivityData::default();

        // Add running agent
        activity.agents.push(create_running_agent(
            "Explore",
            Some("haiku"),
            Some("Finding auth code"),
        ));

        // Add completed agent
        activity.agents.push(create_completed_agent(
            "fix",
            Some("authentication bug"),
            30,
        ));

        let config = AgentsLineConfig::default();
        let result = render_agents_line(&activity, &config);

        assert!(result.is_some());
        let line = result.unwrap();

        // Should contain separator
        assert!(line.contains(" | "));

        let stripped = strip_ansi(&line);
        assert!(stripped.contains("Explore"));
        assert!(stripped.contains("fix"));
    }

    #[test]
    fn test_max_agents_limit() {
        let mut activity = ActivityData::default();

        // Add 4 running agents
        for i in 0..4 {
            let mut agent = create_running_agent(
                &format!("Agent{}", i),
                None,
                None,
            );
            agent.id = format!("agent_{}", i);
            activity.agents.push(agent);
        }

        let config = AgentsLineConfig {
            max_agents: 2,
            ..Default::default()
        };

        let result = render_agents_line(&activity, &config);
        assert!(result.is_some());

        let line = strip_ansi(&result.unwrap());

        // Should only show 2 agents
        assert!(line.contains("Agent0"));
        assert!(line.contains("Agent1"));
        assert!(!line.contains("Agent2"));
        assert!(!line.contains("Agent3"));
    }

    #[test]
    fn test_running_agents_priority() {
        let mut activity = ActivityData::default();

        // Add 2 completed agents first
        for i in 0..2 {
            let mut agent = create_completed_agent(
                &format!("Completed{}", i),
                None,
                30,
            );
            agent.id = format!("completed_{}", i);
            activity.agents.push(agent);
        }

        // Add 2 running agents
        for i in 0..2 {
            let mut agent = create_running_agent(
                &format!("Running{}", i),
                None,
                None,
            );
            agent.id = format!("running_{}", i);
            activity.agents.push(agent);
        }

        let config = AgentsLineConfig {
            max_agents: 2,
            ..Default::default()
        };

        let result = render_agents_line(&activity, &config);
        assert!(result.is_some());

        let line = strip_ansi(&result.unwrap());

        // Running agents should have priority
        assert!(line.contains("Running0"));
        assert!(line.contains("Running1"));
        assert!(!line.contains("Completed"));
    }

    #[test]
    fn test_description_truncation() {
        let mut activity = ActivityData::default();
        activity.agents.push(create_running_agent(
            "Explore",
            None,
            Some("This is a very long description that should be truncated to fit the max length"),
        ));

        let config = AgentsLineConfig {
            max_description_len: 20,
            ..Default::default()
        };

        let result = render_agents_line(&activity, &config);
        assert!(result.is_some());

        let line = strip_ansi(&result.unwrap());

        // Description should be truncated with ellipsis
        assert!(line.contains("This is a very lo..."));
        assert!(!line.contains("truncated to fit"));
    }

    #[test]
    fn test_custom_separator() {
        let mut activity = ActivityData::default();

        activity.agents.push(create_running_agent("Agent1", None, None));
        let mut agent2 = create_running_agent("Agent2", None, None);
        agent2.id = "agent_2".to_string();
        activity.agents.push(agent2);

        let config = AgentsLineConfig {
            separator: " :: ".to_string(),
            ..Default::default()
        };

        let result = render_agents_line(&activity, &config);
        assert!(result.is_some());

        let line = result.unwrap();
        assert!(line.contains(" :: "));
    }

    #[test]
    fn test_no_colors_config() {
        let mut activity = ActivityData::default();
        activity.agents.push(create_running_agent(
            "Explore",
            Some("haiku"),
            Some("Finding code"),
        ));

        let config = AgentsLineConfig {
            running_icon_color: None,
            completed_icon_color: None,
            agent_type_color: None,
            dim_color: None,
            ..Default::default()
        };

        let result = render_agents_line(&activity, &config);
        assert!(result.is_some());

        let line = result.unwrap();

        // Should not contain ANSI escape sequences
        assert!(!line.contains("\x1b["));
    }

    #[test]
    fn test_apply_color_color16() {
        let text = "test";
        let color = AnsiColor::Color16 { c16: 2 }; // Green
        let result = apply_color(text, Some(&color));
        assert_eq!(result, "\x1b[32mtest\x1b[0m");

        // Bright color (c16 >= 8)
        let bright_color = AnsiColor::Color16 { c16: 10 }; // Bright green
        let result = apply_color(text, Some(&bright_color));
        assert_eq!(result, "\x1b[92mtest\x1b[0m");
    }

    #[test]
    fn test_apply_color_color256() {
        let text = "test";
        let color = AnsiColor::Color256 { c256: 208 }; // Orange
        let result = apply_color(text, Some(&color));
        assert_eq!(result, "\x1b[38;5;208mtest\x1b[0m");
    }

    #[test]
    fn test_apply_color_rgb() {
        let text = "test";
        let color = AnsiColor::Rgb { r: 255, g: 128, b: 0 };
        let result = apply_color(text, Some(&color));
        assert_eq!(result, "\x1b[38;2;255;128;0mtest\x1b[0m");
    }

    #[test]
    fn test_apply_color_none() {
        let text = "test";
        let result = apply_color(text, None);
        assert_eq!(result, "test");
    }

    #[test]
    fn test_strip_ansi() {
        let text = "\x1b[32mgreen\x1b[0m normal \x1b[38;5;208morange\x1b[0m";
        let result = strip_ansi(text);
        assert_eq!(result, "green normal orange");
    }

    #[test]
    fn test_completed_agents_most_recent_first() {
        let mut activity = ActivityData::default();

        // Add completed agents with different end times
        let mut agent1 = create_completed_agent("OldAgent", None, 100);
        agent1.id = "old".to_string();
        agent1.end_time = Some(SystemTime::now() - Duration::from_secs(60));

        let mut agent2 = create_completed_agent("NewAgent", None, 30);
        agent2.id = "new".to_string();
        agent2.end_time = Some(SystemTime::now() - Duration::from_secs(10));

        activity.agents.push(agent1);
        activity.agents.push(agent2);

        let config = AgentsLineConfig {
            max_agents: 1,
            ..Default::default()
        };

        let result = render_agents_line(&activity, &config);
        assert!(result.is_some());

        let line = strip_ansi(&result.unwrap());

        // Should show the most recent completed agent (NewAgent)
        assert!(line.contains("NewAgent"));
        assert!(!line.contains("OldAgent"));
    }

    #[test]
    fn test_format_agent_running_full() {
        let agent = AgentEntry {
            id: "test".to_string(),
            agent_type: "Explore".to_string(),
            model: Some("haiku".to_string()),
            description: Some("Finding auth code".to_string()),
            status: AgentStatus::Running,
            start_time: SystemTime::now() - Duration::from_secs(135),
            end_time: None,
        };

        let config = AgentsLineConfig {
            running_icon_color: None,
            completed_icon_color: None,
            agent_type_color: None,
            dim_color: None,
            ..Default::default()
        };

        let result = format_agent(&agent, &config);

        // Format: icon agent_type [model]: description (elapsed)
        assert!(result.starts_with(RUNNING_ICON));
        assert!(result.contains("Explore"));
        assert!(result.contains("[haiku]"));
        assert!(result.contains(": Finding auth code"));
        assert!(result.contains("(2m 15s)"));
    }

    #[test]
    fn test_format_agent_completed() {
        let agent = AgentEntry {
            id: "test".to_string(),
            agent_type: "fix".to_string(),
            model: Some("sonnet".to_string()), // Model should be ignored for completed
            description: Some("authentication bug".to_string()),
            status: AgentStatus::Completed,
            start_time: SystemTime::now() - Duration::from_secs(30),
            end_time: Some(SystemTime::now()),
        };

        let config = AgentsLineConfig {
            running_icon_color: None,
            completed_icon_color: None,
            agent_type_color: None,
            dim_color: None,
            ..Default::default()
        };

        let result = format_agent(&agent, &config);

        // Format: icon agent_type: description (elapsed)
        // Note: completed agents don't show model
        assert!(result.starts_with(COMPLETED_ICON));
        assert!(result.contains("fix"));
        assert!(!result.contains("[sonnet]")); // Model not shown for completed
        assert!(result.contains(": authentication bug"));
    }
}
