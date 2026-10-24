//! The JSON Claude Code writes to the status line command's stdin.
//!
//! Every field is optional and parsed leniently: a top-level field with an
//! unexpected shape is dropped on its own instead of failing the whole input,
//! so a schema change upstream degrades one widget rather than the line.

use crate::version::Version;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).ok())
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Input {
    #[serde(deserialize_with = "lenient")]
    pub session_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub session_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub transcript_path: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub cwd: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub prompt_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub version: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub model: Option<Model>,
    #[serde(deserialize_with = "lenient")]
    pub workspace: Option<Workspace>,
    #[serde(deserialize_with = "lenient")]
    pub output_style: Option<OutputStyle>,
    #[serde(deserialize_with = "lenient")]
    pub cost: Option<Cost>,
    #[serde(deserialize_with = "lenient")]
    pub context_window: Option<ContextWindow>,
    #[serde(deserialize_with = "lenient")]
    pub exceeds_200k_tokens: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub effort: Option<Effort>,
    #[serde(deserialize_with = "lenient")]
    pub thinking: Option<Thinking>,
    #[serde(deserialize_with = "lenient")]
    pub fast_mode: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub rate_limits: Option<RateLimits>,
    #[serde(deserialize_with = "lenient")]
    pub prompt_cache: Option<PromptCache>,
    #[serde(deserialize_with = "lenient")]
    pub vim: Option<Vim>,
    #[serde(deserialize_with = "lenient")]
    pub agent: Option<Agent>,
    #[serde(deserialize_with = "lenient")]
    pub remote: Option<Remote>,
    #[serde(deserialize_with = "lenient")]
    pub pr: Option<PullRequest>,
    #[serde(deserialize_with = "lenient")]
    pub worktree: Option<Worktree>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Model {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub current_dir: Option<String>,
    pub project_dir: Option<String>,
    pub added_dirs: Vec<String>,
    pub git_worktree: Option<String>,
    pub repo: Option<Repo>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Repo {
    pub host: Option<String>,
    pub owner: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OutputStyle {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Cost {
    pub total_cost_usd: Option<f64>,
    pub total_duration_ms: Option<f64>,
    pub total_api_duration_ms: Option<f64>,
    pub total_lines_added: Option<u64>,
    pub total_lines_removed: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ContextWindow {
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
    pub context_window_size: Option<u64>,
    pub current_usage: Option<Usage>,
    pub used_percentage: Option<f64>,
    pub remaining_percentage: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Effort {
    pub level: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Thinking {
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RateLimits {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    pub spend_limit: Option<SpendLimit>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Window {
    pub used_percentage: Option<f64>,
    /// Unix epoch seconds.
    pub resets_at: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SpendLimit {
    pub used_percentage: Option<f64>,
    pub resets_at: Option<f64>,
    pub used_usd: Option<f64>,
    pub limit_usd: Option<f64>,
    pub period: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PromptCache {
    pub warm: Option<bool>,
    pub caching_observed: Option<bool>,
    pub ttl: Option<String>,
    /// Unix epoch seconds; null when the last response reported no cache tokens.
    pub expires_at: Option<f64>,
    pub requests: Option<u64>,
    pub misses: Option<u64>,
    pub expected_rebuilds: Option<u64>,
    pub hit_ratio: Option<f64>,
    pub cache_write_tokens: Option<u64>,
    pub miss_recache_tokens: Option<u64>,
    pub last_miss_at: Option<f64>,
    pub last_miss_cause: Option<MissCause>,
    pub miss_causes: Option<BTreeMap<String, u64>>,
    pub recache_tokens_if_cold: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MissCause {
    pub causes: Vec<String>,
    pub tools_added: Option<u64>,
    pub tools_removed: Option<u64>,
    pub system_char_delta: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Vim {
    pub mode: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Agent {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Remote {
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PullRequest {
    pub number: Option<u64>,
    pub url: Option<String>,
    pub review_state: Option<String>,
    /// `mr` for a GitLab merge request; absent for GitHub.
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Worktree {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub original_cwd: Option<String>,
    pub original_branch: Option<String>,
}

/// A list whose malformed entries are dropped one by one.
fn lenient_list<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(d)?;
    Ok(match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        _ => Vec::new(),
    })
}

/// The JSON Claude Code writes to the `subagentStatusLine` command: the
/// session and the background tasks (subagents, shells, workflows) listed
/// under the prompt, one line each.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SubagentInput {
    #[serde(deserialize_with = "lenient")]
    pub session_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub cwd: Option<String>,
    /// Columns left for each task's line after Claude Code's own prefix.
    #[serde(deserialize_with = "lenient")]
    pub columns: Option<usize>,
    #[serde(deserialize_with = "lenient_list")]
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    /// Name the agent was given, if any.
    #[serde(deserialize_with = "lenient")]
    pub name: Option<String>,
    /// `local_agent`, `local_bash`, `local_workflow`, `remote_agent`,
    /// `in_process_teammate`.
    #[serde(rename = "type", deserialize_with = "lenient")]
    pub kind: Option<String>,
    /// The subagent type, e.g. `Explore` (Claude Code 2.1.293+).
    #[serde(deserialize_with = "lenient")]
    pub agent_type: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub status: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub description: Option<String>,
    /// What it is doing now.
    #[serde(deserialize_with = "lenient")]
    pub label: Option<String>,
    /// Unix epoch milliseconds.
    #[serde(deserialize_with = "lenient")]
    pub start_time: Option<f64>,
    #[serde(deserialize_with = "lenient")]
    pub model: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub effort: Option<serde_json::Value>,
    #[serde(deserialize_with = "lenient")]
    pub context_window_size: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    pub token_count: Option<u64>,
    /// Recent token counts, oldest first (up to 16).
    #[serde(deserialize_with = "lenient_list")]
    pub token_samples: Vec<f64>,
    #[serde(deserialize_with = "lenient")]
    pub cwd: Option<String>,
}

impl SubagentInput {
    /// Parse `json` if it is a subagent payload (it has a `tasks` list).
    pub fn detect(json: &str) -> Option<SubagentInput> {
        if !json.contains("\"tasks\"") {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        value.get("tasks")?.as_array()?;
        serde_json::from_value(value).ok()
    }
}

impl Task {
    /// The task as a status line input, so session metrics (model,
    /// context, effort, dir, git) can describe it.
    pub fn input(&self, parent: &SubagentInput) -> Input {
        let cwd = self.cwd.clone().or_else(|| parent.cwd.clone());
        let effort = match &self.effort {
            Some(serde_json::Value::String(level)) => Some(level.clone()),
            Some(v) => v.get("level").and_then(|l| l.as_str()).map(str::to_string),
            None => None,
        };
        Input {
            session_id: parent.session_id.clone(),
            cwd: cwd.clone(),
            model: self.model.clone().map(|id| Model {
                id: Some(id),
                display_name: None,
            }),
            workspace: Some(Workspace {
                current_dir: cwd,
                project_dir: parent.cwd.clone(),
                ..Workspace::default()
            }),
            context_window: Some(ContextWindow {
                total_input_tokens: self.token_count,
                context_window_size: self.context_window_size,
                ..ContextWindow::default()
            }),
            effort: effort.map(|level| Effort { level: Some(level) }),
            ..Input::default()
        }
    }
}

/// Largest stdin payload accepted; real payloads are a few KiB.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

impl Input {
    pub fn parse(json: &str) -> serde_json::Result<Input> {
        serde_json::from_str(json)
    }

    pub fn cc_version(&self) -> Option<Version> {
        self.version.as_deref().and_then(Version::parse)
    }

    /// The working directory, preferring `workspace.current_dir`.
    pub fn current_dir(&self) -> Option<&str> {
        let non_empty = |d: &&str| !d.is_empty();
        self.workspace
            .as_ref()
            .and_then(|w| w.current_dir.as_deref())
            .filter(non_empty)
            .or(self.cwd.as_deref().filter(non_empty))
    }

    pub fn model_id(&self) -> Option<&str> {
        self.model.as_ref().and_then(|m| m.id.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURED: &str = r#"{
        "session_id": "s1", "transcript_path": "/t.jsonl", "cwd": "/w/app", "prompt_id": "p1",
        "model": {"id": "claude-opus-5-5[1m]", "display_name": "Opus 5.5 (1M context)"},
        "workspace": {"current_dir": "/w/app", "project_dir": "/w/app", "added_dirs": [],
            "repo": {"host": "github.com", "owner": "acme", "name": "app"}},
        "version": "2.1.285", "output_style": {"name": "default"},
        "cost": {"total_cost_usd": 7.74, "total_duration_ms": 61000, "total_api_duration_ms": 9000,
            "total_lines_added": 12, "total_lines_removed": 3},
        "context_window": {"total_input_tokens": 270000, "total_output_tokens": 900,
            "context_window_size": 1000000, "used_percentage": 27, "remaining_percentage": 73,
            "current_usage": {"input_tokens": 3, "output_tokens": 900,
                "cache_creation_input_tokens": 1000, "cache_read_input_tokens": 268997}},
        "exceeds_200k_tokens": true, "effort": {"level": "xhigh"}, "thinking": {"enabled": true},
        "fast_mode": false,
        "rate_limits": {"five_hour": {"used_percentage": 4, "resets_at": 1790820000},
            "seven_day": {"used_percentage": 21.5, "resets_at": 1791200000}},
        "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "1h", "expires_at": 1790812000,
            "requests": 40, "misses": 0, "expected_rebuilds": 1, "hit_ratio": 0.97,
            "cache_write_tokens": 90000, "miss_recache_tokens": 0, "last_miss_at": null,
            "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 270000}
    }"#;

    #[test]
    fn parses_captured_payload() {
        let input = Input::parse(CAPTURED).unwrap();
        assert_eq!(input.cc_version(), Some(Version::new(2, 1, 285)));
        assert_eq!(input.model_id(), Some("claude-opus-5-5[1m]"));
        assert_eq!(input.current_dir(), Some("/w/app"));
        let cw = input.context_window.unwrap();
        assert_eq!(cw.context_window_size, Some(1_000_000));
        assert_eq!(
            cw.current_usage.unwrap().cache_read_input_tokens,
            Some(268_997)
        );
        let rl = input.rate_limits.unwrap();
        assert_eq!(rl.seven_day.unwrap().used_percentage, Some(21.5));
        let pc = input.prompt_cache.unwrap();
        assert_eq!(pc.expires_at, Some(1_790_812_000.0));
        assert!(pc.last_miss_cause.is_none());
    }

    #[test]
    fn drops_only_malformed_fields() {
        let input = Input::parse(
            r#"{"model": "not-an-object", "cost": {"total_cost_usd": 1.5}, "pr": {"number": "x"}}"#,
        )
        .unwrap();
        assert!(input.model.is_none());
        assert!(input.pr.is_none());
        assert_eq!(input.cost.unwrap().total_cost_usd, Some(1.5));
    }

    #[test]
    fn accepts_empty_and_unknown_fields() {
        let input = Input::parse(r#"{"future_field": {"a": 1}}"#).unwrap();
        assert!(input.model.is_none());
        assert!(input.current_dir().is_none());
    }

    #[test]
    fn reads_subagent_payloads() {
        let json = r#"{"session_id": "s1", "cwd": "/w/app", "columns": 120, "tasks": [
            {"id": "a1", "type": "local_agent", "status": "running", "label": "Reading files",
             "startTime": 1790812000000, "model": "claude-opus-5-5[1m]", "effort": "high",
             "contextWindowSize": 1000000, "tokenCount": 211265, "tokenSamples": [206927, 211265],
             "cwd": "/w/app"},
            {"id": "bad", "tokenCount": "many"},
            {"id": "b1", "type": "local_bash", "status": "completed"}
        ]}"#;
        let sub = SubagentInput::detect(json).unwrap();
        assert_eq!(sub.columns, Some(120));
        let ids: Vec<&str> = sub.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["a1", "bad", "b1"]);
        assert_eq!(sub.tasks[1].token_count, None);
        let input = sub.tasks[0].input(&sub);
        assert_eq!(input.model_id(), Some("claude-opus-5-5[1m]"));
        assert_eq!(input.current_dir(), Some("/w/app"));
        assert_eq!(input.effort.unwrap().level.as_deref(), Some("high"));
        assert!(SubagentInput::detect(CAPTURED).is_none());
    }

    #[test]
    fn falls_back_to_cwd() {
        let input = Input::parse(r#"{"cwd": "/a", "workspace": {"current_dir": ""}}"#).unwrap();
        assert_eq!(input.current_dir(), Some("/a"));
    }
}
