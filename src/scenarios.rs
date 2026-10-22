//! Built-in sample inputs for `ccline preview`, the TUI and snapshot tests.
//!
//! Timestamps are written against `BASE`; `input` shifts them so countdowns
//! stay meaningful when previewing at the current time, and `state` builds
//! the history the state layer would hold.

use crate::git::GitInfo;
use crate::input::{Input, SubagentInput};
use crate::state::{Sample, StateView, prompt_key};

/// 2026-10-01 00:00:00 UTC.
pub const BASE: i64 = 1_790_812_800;

pub struct Scenario {
    pub name: &'static str,
    pub summary: &'static str,
    json: &'static str,
    git: Option<GitSpec>,
    /// Earlier renders of the session, oldest first.
    history: &'static [Hist],
    /// Spend across sessions today.
    today: Option<f64>,
    /// A newer ccline known to be published.
    latest: Option<&'static str>,
    /// What the companion mod would report; times are milliseconds before
    /// the current render.
    bridge: Option<&'static str>,
}

const NORMAL_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 124000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "Memory files", "kind": "used", "tokens": 2400},
                   {"name": "Messages", "kind": "used", "tokens": 100600},
                   {"name": "Free space", "kind": "free", "tokens": 43000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "claude-opus-4-8", "ttft_ms": 820, "tps": 74.2, "duration_ms": 41000, "output_tokens": 1400},
  "tools": [{"name": "Bash", "since": -14000}],
  "agents": [{"id": "a1", "type": "Explore", "status": "running", "description": "Explore test fixtures"}]}"#;

const NEAR_FULL_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 182000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "MCP tools", "kind": "used", "tokens": 9200},
                   {"name": "Messages", "kind": "used", "tokens": 151800},
                   {"name": "Free space", "kind": "free", "tokens": 0}]},
  "turn": {"model": "claude-opus-4-8", "ttft_ms": 2400, "tps": 41.0, "duration_ms": 96000, "output_tokens": 3900},
  "tools": [{"name": "Edit", "since": -3000}, {"name": "Bash", "since": -62000}],
  "agents": [{"id": "a1", "type": "Explore", "status": "running"},
             {"id": "a2", "type": "general-purpose", "status": "waiting"}]}"#;

const FRESH_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 24000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "Memory files", "kind": "used", "tokens": 1200},
                   {"name": "Messages", "kind": "used", "tokens": 1800},
                   {"name": "Free space", "kind": "free", "tokens": 143000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "claude-sonnet-5", "ttft_ms": 1400, "tps": 88.0, "duration_ms": 9000, "output_tokens": 600},
  "tools": [{"name": "Read", "since": -2000}],
  "agents": [{"id": "a1", "type": "Explore", "status": "running"}]}"#;

const BIG_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 967000, "auto": true, "window": 1000000, "used": 270000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "MCP tools", "kind": "used", "tokens": 12000},
                   {"name": "Memory files", "kind": "used", "tokens": 6000},
                   {"name": "Messages", "kind": "used", "tokens": 231000},
                   {"name": "Free space", "kind": "free", "tokens": 697000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "claude-opus-5-5", "ttft_ms": 3100, "tps": 96.0, "duration_ms": 52000, "output_tokens": 900},
  "tools": [{"name": "Grep", "since": -4000}],
  "agents": [{"id": "a1", "type": "general-purpose", "status": "running"}]}"#;

const THIRD_PARTY_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 104000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "Messages", "kind": "used", "tokens": 83000},
                   {"name": "Free space", "kind": "free", "tokens": 63000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "kimi-k2-turbo-preview", "ttft_ms": 1100, "tps": 120.0, "duration_ms": 30000, "output_tokens": 800},
  "tools": [{"name": "Bash", "since": -8000}],
  "agents": [{"id": "a1", "type": "Explore", "status": "running"}]}"#;

const WORKTREE_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 38000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "Custom agents", "kind": "used", "tokens": 1500},
                   {"name": "Messages", "kind": "used", "tokens": 15500},
                   {"name": "Free space", "kind": "free", "tokens": 129000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "claude-sonnet-5", "ttft_ms": 1600, "tps": 70.0, "duration_ms": 14000, "output_tokens": 500},
  "tools": [{"name": "Edit", "since": -1500}],
  "agents": [{"id": "a1", "type": "code-reviewer", "status": "running"}]}"#;

const NON_GIT_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 9000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 4000},
                   {"name": "Messages", "kind": "used", "tokens": 1900},
                   {"name": "Free space", "kind": "free", "tokens": 158000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "claude-haiku-4-5", "ttft_ms": 500, "tps": 160.0, "duration_ms": 4000, "output_tokens": 200},
  "tools": [{"name": "Read", "since": -1000}],
  "agents": [{"id": "a1", "type": "Explore", "status": "running"}]}"#;

const GATEWAY_BRIDGE: &str = r#"{"v": 1, "updated_at": 0,
  "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 80000,
    "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                   {"name": "System tools", "kind": "used", "tokens": 17900},
                   {"name": "Messages", "kind": "used", "tokens": 59000},
                   {"name": "Free space", "kind": "free", "tokens": 87000},
                   {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
  "turn": {"model": "claude-sonnet-5", "ttft_ms": 1300, "tps": 65.0, "duration_ms": 22000, "output_tokens": 700},
  "tools": [{"name": "WebFetch", "since": -6000}],
  "agents": [{"id": "a1", "type": "general-purpose", "status": "running"}]}"#;

/// An earlier render, `ago` seconds before the current one.
struct Hist {
    ago: i64,
    prompt: &'static str,
    ctx: u64,
    cost: f64,
    /// 5-hour window use in percent.
    h5: Option<f64>,
}

const fn h(ago: i64, prompt: &'static str, ctx: u64, cost: f64, h5: Option<f64>) -> Hist {
    Hist {
        ago,
        prompt,
        ctx,
        cost,
        h5,
    }
}

struct GitSpec {
    branch: &'static str,
    ahead: u32,
    behind: u32,
    modified: u32,
    untracked: u32,
    conflicts: u32,
}

impl GitSpec {
    fn info(&self) -> GitInfo {
        GitInfo {
            branch: Some(self.branch.to_string()),
            oid: Some("9224f01a3c5e".into()),
            upstream: Some(format!("origin/{}", self.branch)),
            ahead: self.ahead,
            behind: self.behind,
            staged: 0,
            modified: self.modified,
            untracked: self.untracked,
            conflicts: self.conflicts,
        }
    }
}

pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "normal",
        summary: "A typical session midway through",
        git: Some(GitSpec {
            branch: "main",
            ahead: 2,
            behind: 0,
            modified: 2,
            untracked: 0,
            conflicts: 0,
        }),
        history: &[
            h(2400, "p-1", 31_000, 0.10, Some(14.0)),
            h(2100, "p-2", 48_000, 0.28, Some(16.0)),
            h(1800, "p-3", 62_000, 0.41, Some(18.0)),
            h(1500, "p-4", 75_000, 0.55, Some(19.0)),
            h(1200, "p-5", 88_000, 0.70, Some(21.0)),
            h(900, "p-6", 97_000, 0.82, Some(22.0)),
            h(600, "p-7", 104_000, 0.93, Some(24.0)),
            h(300, "p-8", 112_000, 1.05, Some(25.0)),
            h(60, "p-9", 118_000, 1.12, Some(26.0)),
        ],
        today: Some(4.85),
        latest: None,
        bridge: Some(NORMAL_BRIDGE),
        json: r#"{
  "session_id": "s-normal", "prompt_id": "p-9", "transcript_path": "/home/u/.claude/projects/app/s.jsonl", "cwd": "/home/u/app",
  "model": {"id": "claude-opus-4-8", "display_name": "Opus 4.8"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": [],
    "repo": {"host": "github.com", "owner": "acme", "name": "app"}},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 1.2, "total_duration_ms": 1830000, "total_api_duration_ms": 1100000,
    "total_lines_added": 128, "total_lines_removed": 34},
  "context_window": {"total_input_tokens": 124000, "total_output_tokens": 1400, "context_window_size": 200000,
    "used_percentage": 62, "remaining_percentage": 38,
    "current_usage": {"input_tokens": 12, "output_tokens": 1400, "cache_creation_input_tokens": 3000, "cache_read_input_tokens": 120988}},
  "exceeds_200k_tokens": false, "effort": {"level": "high"}, "thinking": {"enabled": true}, "fast_mode": false,
  "rate_limits": {"five_hour": {"used_percentage": 26, "resets_at": 1790820420},
    "seven_day": {"used_percentage": 7, "resets_at": 1790989200}},
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "5m", "expires_at": 1790813022,
    "requests": 41, "misses": 1, "expected_rebuilds": 0, "hit_ratio": 0.94, "cache_write_tokens": 40000,
    "miss_recache_tokens": 9000, "last_miss_at": 1790811000, "last_miss_cause": {"causes": ["tools_changed"], "tools_added": 2, "tools_removed": 0},
    "miss_causes": {"tools_changed": 1}, "recache_tokens_if_cold": 124000},
  "pr": {"number": 1287, "url": "https://github.com/acme/app/pull/1287", "review_state": "pending"}
}"#,
    },
    Scenario {
        name: "fresh",
        summary: "A new session just after its first answer",
        git: Some(GitSpec {
            branch: "main",
            ahead: 0,
            behind: 0,
            modified: 0,
            untracked: 0,
            conflicts: 0,
        }),
        history: &[],
        today: None,
        latest: Some("2.1.0"),
        bridge: Some(FRESH_BRIDGE),
        json: r#"{
  "session_id": "s-fresh", "cwd": "/home/u/app",
  "model": {"id": "claude-sonnet-5", "display_name": "Sonnet 5"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": []},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 0.06, "total_duration_ms": 40000, "total_api_duration_ms": 9000,
    "total_lines_added": 0, "total_lines_removed": 0},
  "context_window": {"total_input_tokens": 24000, "total_output_tokens": 600, "context_window_size": 200000,
    "used_percentage": 12, "remaining_percentage": 88,
    "current_usage": {"input_tokens": 8, "output_tokens": 600, "cache_creation_input_tokens": 23992, "cache_read_input_tokens": 0}},
  "exceeds_200k_tokens": false, "effort": {"level": "medium"}, "thinking": {"enabled": true}, "fast_mode": false,
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "5m", "expires_at": 1790813060,
    "requests": 1, "misses": 0, "expected_rebuilds": 0, "hit_ratio": 0.0, "cache_write_tokens": 24000,
    "miss_recache_tokens": 0, "last_miss_at": null, "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 24000}
}"#,
    },
    Scenario {
        name: "near-full",
        summary: "Context and the 5-hour limit nearly used up, cache cold",
        git: Some(GitSpec {
            branch: "feature/auth",
            ahead: 0,
            behind: 3,
            modified: 4,
            untracked: 1,
            conflicts: 1,
        }),
        history: &[
            h(3000, "q-1", 120_000, 14.10, Some(62.0)),
            h(2400, "q-2", 138_000, 15.20, Some(66.0)),
            h(1800, "q-3", 151_000, 16.30, Some(74.0)),
            h(1200, "q-4", 163_000, 17.10, Some(79.0)),
            h(600, "q-5", 171_000, 17.80, Some(84.0)),
        ],
        today: Some(31.6),
        latest: None,
        bridge: Some(NEAR_FULL_BRIDGE),
        json: r#"{
  "session_id": "s-full", "prompt_id": "q-6", "cwd": "/home/u/app",
  "model": {"id": "claude-opus-4-8", "display_name": "Opus 4.8"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": [],
    "repo": {"host": "github.com", "owner": "acme", "name": "app"}},
  "version": "2.1.305", "output_style": {"name": "Explanatory"},
  "cost": {"total_cost_usd": 18.42, "total_duration_ms": 14400000, "total_api_duration_ms": 9900000,
    "total_lines_added": 1450, "total_lines_removed": 610},
  "context_window": {"total_input_tokens": 182000, "total_output_tokens": 3100, "context_window_size": 200000,
    "used_percentage": 91, "remaining_percentage": 9,
    "current_usage": {"input_tokens": 40, "output_tokens": 3100, "cache_creation_input_tokens": 182000, "cache_read_input_tokens": 0}},
  "exceeds_200k_tokens": false, "effort": {"level": "max"}, "thinking": {"enabled": true}, "fast_mode": false,
  "rate_limits": {"five_hour": {"used_percentage": 88, "resets_at": 1790814600},
    "seven_day": {"used_percentage": 64, "resets_at": 1791158400}},
  "prompt_cache": {"warm": false, "caching_observed": true, "ttl": "5m", "expires_at": null,
    "requests": 210, "misses": 6, "expected_rebuilds": 2, "hit_ratio": 0.71, "cache_write_tokens": 900000,
    "miss_recache_tokens": 520000, "last_miss_at": 1790812500, "last_miss_cause": {"causes": ["ttl_expired_5m"]},
    "miss_causes": {"ttl_expired_5m": 5, "tools_changed": 1}, "recache_tokens_if_cold": 182000},
  "vim": {"mode": "NORMAL"},
  "pr": {"number": 1288, "url": "https://github.com/acme/app/pull/1288", "review_state": "changes_requested"}
}"#,
    },
    Scenario {
        name: "big-context",
        summary: "A 1M-context model with fast mode on",
        git: Some(GitSpec {
            branch: "main",
            ahead: 0,
            behind: 0,
            modified: 0,
            untracked: 0,
            conflicts: 0,
        }),
        history: &[
            h(1800, "b-1", 180_000, 5.90, Some(2.0)),
            h(1200, "b-2", 215_000, 6.60, Some(3.0)),
            h(600, "b-3", 248_000, 7.20, Some(3.0)),
        ],
        today: Some(12.4),
        latest: None,
        bridge: Some(BIG_BRIDGE),
        json: r#"{
  "session_id": "s-big", "prompt_id": "b-4", "cwd": "/home/u/monorepo",
  "model": {"id": "claude-opus-5-5[1m]", "display_name": "Opus 5.5 (1M context)"},
  "workspace": {"current_dir": "/home/u/monorepo/services/api", "project_dir": "/home/u/monorepo", "added_dirs": [],
    "repo": {"host": "github.com", "owner": "acme", "name": "monorepo"}},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 7.74, "total_duration_ms": 5400000, "total_api_duration_ms": 2100000,
    "total_lines_added": 40, "total_lines_removed": 2},
  "context_window": {"total_input_tokens": 270000, "total_output_tokens": 900, "context_window_size": 1000000,
    "used_percentage": 27, "remaining_percentage": 73,
    "current_usage": {"input_tokens": 3, "output_tokens": 900, "cache_creation_input_tokens": 1000, "cache_read_input_tokens": 268997}},
  "exceeds_200k_tokens": true, "effort": {"level": "xhigh"}, "thinking": {"enabled": true}, "fast_mode": true,
  "rate_limits": {"five_hour": {"used_percentage": 4, "resets_at": 1790829000},
    "seven_day": {"used_percentage": 80, "resets_at": 1790899200}},
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "1h", "expires_at": 1790815500,
    "requests": 40, "misses": 0, "expected_rebuilds": 1, "hit_ratio": 0.97, "cache_write_tokens": 90000,
    "miss_recache_tokens": 0, "last_miss_at": null, "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 270000},
  "pr": {"number": 77, "url": "https://github.com/acme/monorepo/pull/77", "review_state": "approved"}
}"#,
    },
    Scenario {
        name: "third-party",
        summary: "A model Claude Code can't size; the registry corrects it",
        git: Some(GitSpec {
            branch: "main",
            ahead: 0,
            behind: 0,
            modified: 1,
            untracked: 0,
            conflicts: 0,
        }),
        history: &[
            h(900, "k-1", 60_000, 0.40, None),
            h(600, "k-2", 82_000, 0.60, None),
            h(300, "k-3", 95_000, 0.75, None),
        ],
        today: Some(0.84),
        latest: None,
        bridge: Some(THIRD_PARTY_BRIDGE),
        json: r#"{
  "session_id": "s-kimi", "prompt_id": "k-4", "cwd": "/home/u/app",
  "model": {"id": "kimi-k2-turbo-preview", "display_name": "kimi-k2-turbo-preview"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": []},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 0.84, "total_duration_ms": 900000, "total_api_duration_ms": 300000},
  "context_window": {"total_input_tokens": 104000, "total_output_tokens": 800, "context_window_size": 200000,
    "used_percentage": 52, "remaining_percentage": 48,
    "current_usage": {"input_tokens": 4000, "output_tokens": 800, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 100000}},
  "exceeds_200k_tokens": false, "thinking": {"enabled": false}, "fast_mode": false,
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "5m", "expires_at": 1790812980,
    "requests": 12, "misses": 1, "expected_rebuilds": 0, "hit_ratio": 0.88, "cache_write_tokens": 104000,
    "miss_recache_tokens": 0, "last_miss_at": null, "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 104000}
}"#,
    },
    Scenario {
        name: "worktree",
        summary: "A --worktree session with a name and an agent",
        git: Some(GitSpec {
            branch: "worktree-feature-x",
            ahead: 1,
            behind: 0,
            modified: 0,
            untracked: 2,
            conflicts: 0,
        }),
        history: &[
            h(420, "w-1", 20_000, 0.10, Some(40.0)),
            h(200, "w-2", 30_000, 0.22, Some(41.0)),
        ],
        today: Some(2.15),
        latest: None,
        bridge: Some(WORKTREE_BRIDGE),
        json: r#"{
  "session_id": "s-wt", "prompt_id": "w-3", "session_name": "Refactor the auth middleware", "cwd": "/home/u/app/.claude/worktrees/feature-x",
  "model": {"id": "claude-sonnet-5", "display_name": "Sonnet 5"},
  "workspace": {"current_dir": "/home/u/app/.claude/worktrees/feature-x", "project_dir": "/home/u/app", "added_dirs": [],
    "git_worktree": "feature-x", "repo": {"host": "gitlab.com", "owner": "group/team", "name": "app"}},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 0.31, "total_duration_ms": 420000, "total_api_duration_ms": 150000,
    "total_lines_added": 12, "total_lines_removed": 0},
  "context_window": {"total_input_tokens": 38000, "total_output_tokens": 500, "context_window_size": 200000,
    "used_percentage": 19, "remaining_percentage": 81,
    "current_usage": {"input_tokens": 5, "output_tokens": 500, "cache_creation_input_tokens": 2000, "cache_read_input_tokens": 35995}},
  "exceeds_200k_tokens": false, "effort": {"level": "high"}, "thinking": {"enabled": true}, "fast_mode": false,
  "rate_limits": {"five_hour": {"used_percentage": 41, "resets_at": 1790823600}},
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "5m", "expires_at": 1790813010,
    "requests": 9, "misses": 0, "expected_rebuilds": 0, "hit_ratio": 0.91, "cache_write_tokens": 38000,
    "miss_recache_tokens": 0, "last_miss_at": null, "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 38000},
  "vim": {"mode": "INSERT"}, "agent": {"name": "code-reviewer"},
  "pr": {"number": 12, "url": "https://gitlab.com/group/team/app/-/merge_requests/12", "review_state": "draft", "kind": "mr"},
  "worktree": {"name": "feature-x", "path": "/home/u/app/.claude/worktrees/feature-x", "branch": "worktree-feature-x",
    "original_cwd": "/home/u/app", "original_branch": "main"}
}"#,
    },
    Scenario {
        name: "non-git",
        summary: "Outside any repository",
        git: None,
        history: &[],
        today: None,
        latest: None,
        bridge: Some(NON_GIT_BRIDGE),
        json: r#"{
  "session_id": "s-tmp", "cwd": "/tmp/scratch",
  "model": {"id": "claude-haiku-4-5", "display_name": "Haiku 4.5"},
  "workspace": {"current_dir": "/tmp/scratch", "project_dir": "/tmp/scratch", "added_dirs": ["/tmp/data"]},
  "version": "2.1.305", "output_style": {"name": "Learning"},
  "cost": {"total_cost_usd": 0.02, "total_duration_ms": 61000, "total_api_duration_ms": 8000},
  "context_window": {"total_input_tokens": 9000, "total_output_tokens": 200, "context_window_size": 200000,
    "used_percentage": 5, "remaining_percentage": 95,
    "current_usage": {"input_tokens": 900, "output_tokens": 200, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 8100}},
  "exceeds_200k_tokens": false, "thinking": {"enabled": false}, "fast_mode": false,
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "5m", "expires_at": 1790812895,
    "requests": 3, "misses": 0, "expected_rebuilds": 0, "hit_ratio": 0.4, "cache_write_tokens": 9000,
    "miss_recache_tokens": 0, "last_miss_at": null, "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 9000}
}"#,
    },
    Scenario {
        name: "gateway",
        summary: "Behind a Claude apps gateway with a monthly spend limit",
        git: Some(GitSpec {
            branch: "main",
            ahead: 0,
            behind: 0,
            modified: 0,
            untracked: 0,
            conflicts: 0,
        }),
        history: &[],
        today: None,
        latest: None,
        bridge: Some(GATEWAY_BRIDGE),
        json: r#"{
  "session_id": "s-gw", "cwd": "/home/u/app",
  "model": {"id": "claude-sonnet-5", "display_name": "Sonnet 5"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": []},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 3.1, "total_duration_ms": 2400000, "total_api_duration_ms": 800000},
  "context_window": {"total_input_tokens": 80000, "total_output_tokens": 700, "context_window_size": 200000,
    "used_percentage": 40, "remaining_percentage": 60,
    "current_usage": {"input_tokens": 10, "output_tokens": 700, "cache_creation_input_tokens": 1000, "cache_read_input_tokens": 78990}},
  "exceeds_200k_tokens": false, "effort": {"level": "medium"}, "thinking": {"enabled": true}, "fast_mode": false,
  "rate_limits": {"spend_limit": {"used_percentage": 64, "resets_at": 1791849600, "used_usd": 320, "limit_usd": 500, "period": "monthly"}},
  "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "1h", "expires_at": 1790815200,
    "requests": 30, "misses": 1, "expected_rebuilds": 0, "hit_ratio": 0.93, "cache_write_tokens": 80000,
    "miss_recache_tokens": 0, "last_miss_at": null, "last_miss_cause": null, "miss_causes": {}, "recache_tokens_if_cold": 80000}
}"#,
    },
    Scenario {
        name: "old-cc",
        summary: "A Claude Code older than the minimum ccline supports",
        git: None,
        history: &[],
        today: None,
        latest: None,
        bridge: None,
        json: r#"{
  "session_id": "s-old", "cwd": "/home/u/app", "version": "2.1.140",
  "model": {"id": "claude-sonnet-4-5", "display_name": "Sonnet 4.5"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app"}
}"#,
    },
];

/// A `subagentStatusLine` payload: two running subagents, a finished one
/// and a shell command. Start times are milliseconds against `BASE`.
const SUBAGENT: &str = r#"{
  "session_id": "s-normal", "cwd": "/home/u/app", "prompt_id": "p-9", "columns": 100,
  "tasks": [
    {"id": "a1", "type": "local_agent", "status": "running", "description": "Review the layout code",
     "label": "Reading src/statusline.rs", "startTime": 1790812608000, "model": "claude-opus-5-5[1m]",
     "effort": "high", "contextWindowSize": 1000000, "tokenCount": 211265,
     "tokenSamples": [150000, 168000, 182000, 190000, 206927, 211265], "cwd": "/home/u/app"},
    {"id": "a2", "type": "local_agent", "agentType": "Explore", "status": "running", "description": "Explore test fixtures",
     "label": "Searching for *.snap files under tests/ and fixtures/", "startTime": 1790812755000,
     "model": "claude-sonnet-5", "contextWindowSize": 200000, "tokenCount": 48000,
     "tokenSamples": [12000, 30000, 48000], "cwd": "/home/u/app"},
    {"id": "a3", "type": "local_agent", "status": "completed", "description": "Find callers of budget()",
     "label": "Find callers of budget()", "startTime": 1790812200000, "model": "claude-haiku-4-5",
     "contextWindowSize": 200000, "tokenCount": 31000, "tokenSamples": [31000], "cwd": "/home/u/app"},
    {"id": "b1", "type": "local_bash", "status": "running", "description": "cargo test",
     "label": "cargo test", "startTime": 1790812780000, "tokenCount": 0, "tokenSamples": [],
     "cwd": "/home/u/app"}
  ]
}"#;

/// The sample subagent payload with start times moved from `BASE` to `now`.
pub fn subagent(now: i64) -> SubagentInput {
    let mut sub = SubagentInput::detect(SUBAGENT).expect("built-in subagent sample parses");
    for task in &mut sub.tasks {
        if let Some(t) = task.start_time.as_mut() {
            *t += ((now - BASE) * 1000) as f64;
        }
    }
    sub
}

pub fn find(name: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|s| s.name == name)
}

impl Scenario {
    pub fn git(&self) -> Option<GitInfo> {
        self.git.as_ref().map(GitSpec::info)
    }

    /// The mod's bridge as it would read at `now`.
    pub fn bridge(&self, now: i64) -> Option<crate::state::bridge::Bridge> {
        let mut b: crate::state::bridge::Bridge = serde_json::from_str(self.bridge?).ok()?;
        let at = now * 1000;
        b.updated_at = at;
        for t in &mut b.tools {
            t.since += at;
        }
        Some(b)
    }

    /// The input with timestamps moved from `BASE` to `now`.
    pub fn input(&self, now: i64) -> Input {
        let mut input = Input::parse(self.json).expect("built-in scenarios parse");
        shift(&mut input, now - BASE);
        input
    }

    /// The session's history up to `now`, ending with the current render.
    pub fn state(&self, now: i64) -> StateView {
        let current = Sample::from_input(&self.input(now), now);
        let samples = self
            .history
            .iter()
            .map(|h| Sample {
                t: now - h.ago,
                p: Some(prompt_key(h.prompt)),
                ctx: Some(h.ctx),
                cost: Some(h.cost),
                h5: h.h5,
                h5_reset: current.h5_reset,
                d7: current.d7,
                d7_reset: current.d7_reset,
            })
            .chain(std::iter::once(current))
            .collect();
        StateView {
            samples,
            today_cost: self.today,
            latest_version: self.latest.map(str::to_string),
            bridge: self.bridge(now).map(std::rc::Rc::new),
        }
    }
}

/// Move every absolute timestamp in `input` by `delta` seconds.
pub fn shift(input: &mut Input, delta: i64) {
    let d = delta as f64;
    let bump = |t: &mut Option<f64>| {
        if let Some(v) = t {
            *v += d;
        }
    };
    if let Some(rl) = input.rate_limits.as_mut() {
        if let Some(w) = rl.five_hour.as_mut() {
            bump(&mut w.resets_at);
        }
        if let Some(w) = rl.seven_day.as_mut() {
            bump(&mut w.resets_at);
        }
        if let Some(s) = rl.spend_limit.as_mut() {
            bump(&mut s.resets_at);
        }
    }
    if let Some(pc) = input.prompt_cache.as_mut() {
        bump(&mut pc.expires_at);
        bump(&mut pc.last_miss_at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_parses_with_core_fields() {
        for s in SCENARIOS {
            let input = s.input(BASE);
            assert!(input.model_id().is_some(), "{}", s.name);
            assert!(input.cc_version().is_some(), "{}", s.name);
        }
    }

    #[test]
    fn shifts_timestamps() {
        let input = find("normal").unwrap().input(BASE + 100);
        let five = input.rate_limits.unwrap().five_hour.unwrap();
        assert_eq!(five.resets_at, Some(1_790_820_520.0));
    }
}
