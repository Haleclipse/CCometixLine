//! Built-in sample inputs for `ccline preview`, the TUI and snapshot tests.
//!
//! Timestamps are written against `BASE`; `load` shifts them so countdowns
//! stay meaningful when previewing at the current time.

use crate::git::GitInfo;
use crate::input::Input;

/// 2026-10-01 00:00:00 UTC.
pub const BASE: i64 = 1_790_812_800;

pub struct Scenario {
    pub name: &'static str,
    pub summary: &'static str,
    json: &'static str,
    git: Option<GitSpec>,
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
        json: r#"{
  "session_id": "s-normal", "transcript_path": "/home/u/.claude/projects/app/s.jsonl", "cwd": "/home/u/app",
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
        summary: "A new session before the first response",
        git: Some(GitSpec {
            branch: "main",
            ahead: 0,
            behind: 0,
            modified: 0,
            untracked: 0,
            conflicts: 0,
        }),
        json: r#"{
  "session_id": "s-fresh", "cwd": "/home/u/app",
  "model": {"id": "claude-sonnet-5", "display_name": "Sonnet 5"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": []},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 0, "total_duration_ms": 4000, "total_api_duration_ms": 0},
  "context_window": {"total_input_tokens": 0, "total_output_tokens": 0, "context_window_size": 200000,
    "used_percentage": null, "remaining_percentage": null, "current_usage": null},
  "exceeds_200k_tokens": false, "effort": {"level": "medium"}, "thinking": {"enabled": true}, "fast_mode": false
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
        json: r#"{
  "session_id": "s-full", "cwd": "/home/u/app",
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
        json: r#"{
  "session_id": "s-big", "cwd": "/home/u/monorepo",
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
        json: r#"{
  "session_id": "s-kimi", "cwd": "/home/u/app",
  "model": {"id": "kimi-k2-turbo-preview", "display_name": "kimi-k2-turbo-preview"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app", "added_dirs": []},
  "version": "2.1.305", "output_style": {"name": "default"},
  "cost": {"total_cost_usd": 0.84, "total_duration_ms": 900000, "total_api_duration_ms": 300000},
  "context_window": {"total_input_tokens": 104000, "total_output_tokens": 800, "context_window_size": 200000,
    "used_percentage": 52, "remaining_percentage": 48,
    "current_usage": {"input_tokens": 104000, "output_tokens": 800, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}},
  "exceeds_200k_tokens": false, "thinking": {"enabled": false}, "fast_mode": false
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
        json: r#"{
  "session_id": "s-wt", "session_name": "Refactor the auth middleware", "cwd": "/home/u/app/.claude/worktrees/feature-x",
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
        json: r#"{
  "session_id": "s-tmp", "cwd": "/tmp/scratch",
  "model": {"id": "claude-haiku-4-5", "display_name": "Haiku 4.5"},
  "workspace": {"current_dir": "/tmp/scratch", "project_dir": "/tmp/scratch", "added_dirs": ["/tmp/data"]},
  "version": "2.1.305", "output_style": {"name": "Learning"},
  "cost": {"total_cost_usd": 0.02, "total_duration_ms": 61000, "total_api_duration_ms": 8000},
  "context_window": {"total_input_tokens": 9000, "total_output_tokens": 200, "context_window_size": 200000,
    "used_percentage": 5, "remaining_percentage": 95,
    "current_usage": {"input_tokens": 9000, "output_tokens": 200, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}},
  "exceeds_200k_tokens": false, "thinking": {"enabled": false}, "fast_mode": false
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
  "rate_limits": {"spend_limit": {"used_percentage": 64, "resets_at": 1791849600, "used_usd": 320, "limit_usd": 500, "period": "monthly"}}
}"#,
    },
    Scenario {
        name: "old-cc",
        summary: "A Claude Code older than the minimum ccline supports",
        git: None,
        json: r#"{
  "session_id": "s-old", "cwd": "/home/u/app", "version": "2.1.140",
  "model": {"id": "claude-sonnet-4-5", "display_name": "Sonnet 4.5"},
  "workspace": {"current_dir": "/home/u/app", "project_dir": "/home/u/app"}
}"#,
    },
];

pub fn find(name: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|s| s.name == name)
}

impl Scenario {
    pub fn git(&self) -> Option<GitInfo> {
        self.git.as_ref().map(GitSpec::info)
    }

    /// The input with timestamps moved from `BASE` to `now`.
    pub fn input(&self, now: i64) -> Input {
        let mut input = Input::parse(self.json).expect("built-in scenarios parse");
        shift(&mut input, now - BASE);
        input
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
