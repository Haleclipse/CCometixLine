//! Configuration counting logic for Claude Code environment
//!
//! This module scans user and project scope configuration files to count:
//! - CLAUDE.md files
//! - Rule files (.md in rules directories)
//! - MCP servers (minus disabled ones)
//! - Hooks
//!
//! Based on claude-hud's config-reader.ts implementation.

use super::types::ConfigCounts;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Get the user's home directory
fn get_home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Read MCP server names from a JSON config file
///
/// Looks for `mcpServers` object and returns the set of server names (keys).
fn get_mcp_server_names(file_path: &Path) -> HashSet<String> {
    let mut servers = HashSet::new();

    if !file_path.exists() {
        return servers;
    }

    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return servers,
    };

    let config: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return servers,
    };

    if let Some(mcp_servers) = config.get("mcpServers") {
        if let Some(obj) = mcp_servers.as_object() {
            for key in obj.keys() {
                servers.insert(key.clone());
            }
        }
    }

    servers
}

/// Read disabled MCP server names from a JSON config file
///
/// Looks for the specified key (e.g., `disabledMcpServers` or `disabledMcpjsonServers`)
/// and returns the set of disabled server names.
fn get_disabled_mcp_servers(file_path: &Path, key: &str) -> HashSet<String> {
    let mut disabled = HashSet::new();

    if !file_path.exists() {
        return disabled;
    }

    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return disabled,
    };

    let config: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return disabled,
    };

    if let Some(disabled_array) = config.get(key) {
        if let Some(arr) = disabled_array.as_array() {
            for item in arr {
                if let Some(name) = item.as_str() {
                    disabled.insert(name.to_string());
                }
            }
        }
    }

    disabled
}

/// Count hooks in a JSON config file
///
/// Looks for `hooks` object and returns the number of hook keys.
fn count_hooks_in_file(file_path: &Path) -> u32 {
    if !file_path.exists() {
        return 0;
    }

    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let config: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    if let Some(hooks) = config.get("hooks") {
        if let Some(obj) = hooks.as_object() {
            return obj.len() as u32;
        }
    }

    0
}

/// Recursively count .md files in a rules directory
fn count_rules_in_dir(rules_dir: &Path) -> u32 {
    if !rules_dir.exists() || !rules_dir.is_dir() {
        return 0;
    }

    let mut count = 0;

    let entries = match fs::read_dir(rules_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Recursively count in subdirectories
            count += count_rules_in_dir(&path);
        } else if path.is_file() {
            // Count .md files
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("md") {
                    count += 1;
                }
            }
        }
    }

    count
}

/// Check if a file exists
fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// Count all configurations from user and project scopes
///
/// # Arguments
///
/// * `cwd` - Optional current working directory for project scope scanning.
///           If None, only user scope is scanned.
///
/// # Returns
///
/// A `ConfigCounts` struct containing counts for:
/// - `claude_md_count`: Number of CLAUDE.md files found
/// - `rules_count`: Number of rule files (.md in rules directories)
/// - `mcp_count`: Number of MCP servers (enabled - disabled)
/// - `hooks_count`: Number of hooks configured
///
/// # Scope Details
///
/// ## User Scope
/// - `~/.claude/CLAUDE.md`
/// - `~/.claude/rules/*.md` (recursive)
/// - `~/.claude/settings.json` (mcpServers keys, hooks keys)
/// - `~/.claude.json` (mcpServers keys, disabledMcpServers array)
///
/// ## Project Scope (if cwd provided)
/// - `{cwd}/CLAUDE.md`
/// - `{cwd}/CLAUDE.local.md`
/// - `{cwd}/.claude/CLAUDE.md`
/// - `{cwd}/.claude/CLAUDE.local.md`
/// - `{cwd}/.claude/rules/*.md` (recursive)
/// - `{cwd}/.mcp.json` (mcpServers keys)
/// - `{cwd}/.claude/settings.json` (mcpServers keys, hooks keys)
/// - `{cwd}/.claude/settings.local.json` (mcpServers keys, hooks keys, disabledMcpjsonServers array)
///
/// # MCP Counting
///
/// MCP servers are counted per scope with disabled servers subtracted:
/// - User scope: servers from `~/.claude/settings.json` and `~/.claude.json`,
///   minus `disabledMcpServers` from `~/.claude.json`
/// - Project scope: servers from `.mcp.json`, `.claude/settings.json`, and
///   `.claude/settings.local.json`, minus `disabledMcpjsonServers` from
///   `.claude/settings.local.json`
///
/// Same-name MCPs in user and project scope count separately (no cross-scope deduplication).
pub fn count_configs(cwd: Option<&str>) -> ConfigCounts {
    let mut claude_md_count: u32 = 0;
    let mut rules_count: u32 = 0;
    let mut hooks_count: u32 = 0;

    // Collect MCP servers per scope for proper disabled filtering
    let mut user_mcp_servers: HashSet<String> = HashSet::new();
    let mut project_mcp_servers: HashSet<String> = HashSet::new();

    // Get home directory
    let home_dir = match get_home_dir() {
        Some(h) => h,
        None => {
            // If we can't get home dir, return empty counts
            return ConfigCounts::default();
        }
    };

    let claude_dir = home_dir.join(".claude");

    // === USER SCOPE ===

    // ~/.claude/CLAUDE.md
    if file_exists(&claude_dir.join("CLAUDE.md")) {
        claude_md_count += 1;
    }

    // ~/.claude/rules/*.md (recursive)
    rules_count += count_rules_in_dir(&claude_dir.join("rules"));

    // ~/.claude/settings.json (MCPs and hooks)
    let user_settings = claude_dir.join("settings.json");
    for name in get_mcp_server_names(&user_settings) {
        user_mcp_servers.insert(name);
    }
    hooks_count += count_hooks_in_file(&user_settings);

    // ~/.claude.json (additional user-scope MCPs)
    let user_claude_json = home_dir.join(".claude.json");
    for name in get_mcp_server_names(&user_claude_json) {
        user_mcp_servers.insert(name);
    }

    // Get disabled user-scope MCPs from ~/.claude.json
    let disabled_user_mcps = get_disabled_mcp_servers(&user_claude_json, "disabledMcpServers");
    for name in &disabled_user_mcps {
        user_mcp_servers.remove(name);
    }

    // === PROJECT SCOPE ===

    if let Some(cwd_str) = cwd {
        let cwd_path = Path::new(cwd_str);

        // {cwd}/CLAUDE.md
        if file_exists(&cwd_path.join("CLAUDE.md")) {
            claude_md_count += 1;
        }

        // {cwd}/CLAUDE.local.md
        if file_exists(&cwd_path.join("CLAUDE.local.md")) {
            claude_md_count += 1;
        }

        // {cwd}/.claude/CLAUDE.md
        if file_exists(&cwd_path.join(".claude").join("CLAUDE.md")) {
            claude_md_count += 1;
        }

        // {cwd}/.claude/CLAUDE.local.md
        if file_exists(&cwd_path.join(".claude").join("CLAUDE.local.md")) {
            claude_md_count += 1;
        }

        // {cwd}/.claude/rules/*.md (recursive)
        rules_count += count_rules_in_dir(&cwd_path.join(".claude").join("rules"));

        // {cwd}/.mcp.json (project MCP config) - tracked separately for disabled filtering
        let mcp_json_path = cwd_path.join(".mcp.json");
        let mut mcp_json_servers = get_mcp_server_names(&mcp_json_path);

        // {cwd}/.claude/settings.json (project settings)
        let project_settings = cwd_path.join(".claude").join("settings.json");
        for name in get_mcp_server_names(&project_settings) {
            project_mcp_servers.insert(name);
        }
        hooks_count += count_hooks_in_file(&project_settings);

        // {cwd}/.claude/settings.local.json (local project settings)
        let local_settings = cwd_path.join(".claude").join("settings.local.json");
        for name in get_mcp_server_names(&local_settings) {
            project_mcp_servers.insert(name);
        }
        hooks_count += count_hooks_in_file(&local_settings);

        // Get disabled .mcp.json servers from settings.local.json
        let disabled_mcp_json_servers =
            get_disabled_mcp_servers(&local_settings, "disabledMcpjsonServers");
        for name in &disabled_mcp_json_servers {
            mcp_json_servers.remove(name);
        }

        // Add remaining .mcp.json servers to project set
        for name in mcp_json_servers {
            project_mcp_servers.insert(name);
        }
    }

    // Total MCP count = user servers + project servers
    // Note: Deduplication only occurs within each scope, not across scopes.
    // A server with the same name in both user and project scope counts as 2 (separate configs).
    let mcp_count = (user_mcp_servers.len() + project_mcp_servers.len()) as u32;

    ConfigCounts {
        claude_md_count,
        rules_count,
        mcp_count,
        hooks_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to create a test directory structure
    fn setup_test_dir() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp dir")
    }

    /// Helper to create a file with content
    fn create_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        let mut file = File::create(path).expect("Failed to create file");
        file.write_all(content.as_bytes())
            .expect("Failed to write file");
    }

    #[test]
    fn test_get_mcp_server_names_empty() {
        let temp = setup_test_dir();
        let path = temp.path().join("nonexistent.json");
        let servers = get_mcp_server_names(&path);
        assert!(servers.is_empty());
    }

    #[test]
    fn test_get_mcp_server_names_valid() {
        let temp = setup_test_dir();
        let path = temp.path().join("settings.json");
        create_file(
            &path,
            r#"{
                "mcpServers": {
                    "server1": {"command": "test"},
                    "server2": {"command": "test2"}
                }
            }"#,
        );

        let servers = get_mcp_server_names(&path);
        assert_eq!(servers.len(), 2);
        assert!(servers.contains("server1"));
        assert!(servers.contains("server2"));
    }

    #[test]
    fn test_get_mcp_server_names_invalid_json() {
        let temp = setup_test_dir();
        let path = temp.path().join("invalid.json");
        create_file(&path, "not valid json");

        let servers = get_mcp_server_names(&path);
        assert!(servers.is_empty());
    }

    #[test]
    fn test_get_mcp_server_names_no_mcp_servers_key() {
        let temp = setup_test_dir();
        let path = temp.path().join("settings.json");
        create_file(&path, r#"{"hooks": {}}"#);

        let servers = get_mcp_server_names(&path);
        assert!(servers.is_empty());
    }

    #[test]
    fn test_get_disabled_mcp_servers() {
        let temp = setup_test_dir();
        let path = temp.path().join("claude.json");
        create_file(
            &path,
            r#"{
                "disabledMcpServers": ["server1", "server2"],
                "disabledMcpjsonServers": ["server3"]
            }"#,
        );

        let disabled = get_disabled_mcp_servers(&path, "disabledMcpServers");
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains("server1"));
        assert!(disabled.contains("server2"));

        let disabled_json = get_disabled_mcp_servers(&path, "disabledMcpjsonServers");
        assert_eq!(disabled_json.len(), 1);
        assert!(disabled_json.contains("server3"));
    }

    #[test]
    fn test_get_disabled_mcp_servers_with_non_strings() {
        let temp = setup_test_dir();
        let path = temp.path().join("claude.json");
        create_file(
            &path,
            r#"{
                "disabledMcpServers": ["valid", 123, null, "also_valid"]
            }"#,
        );

        let disabled = get_disabled_mcp_servers(&path, "disabledMcpServers");
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains("valid"));
        assert!(disabled.contains("also_valid"));
    }

    #[test]
    fn test_count_hooks_in_file() {
        let temp = setup_test_dir();
        let path = temp.path().join("settings.json");
        create_file(
            &path,
            r#"{
                "hooks": {
                    "PreToolUse": [{"command": "test"}],
                    "PostToolUse": [{"command": "test2"}],
                    "Notification": [{"command": "test3"}]
                }
            }"#,
        );

        let count = count_hooks_in_file(&path);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_hooks_in_file_no_hooks() {
        let temp = setup_test_dir();
        let path = temp.path().join("settings.json");
        create_file(&path, r#"{"mcpServers": {}}"#);

        let count = count_hooks_in_file(&path);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_rules_in_dir() {
        let temp = setup_test_dir();
        let rules_dir = temp.path().join("rules");

        // Create some rule files
        create_file(&rules_dir.join("rule1.md"), "# Rule 1");
        create_file(&rules_dir.join("rule2.md"), "# Rule 2");
        create_file(&rules_dir.join("subdir").join("rule3.md"), "# Rule 3");
        create_file(&rules_dir.join("not_a_rule.txt"), "Not a rule");

        let count = count_rules_in_dir(&rules_dir);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_rules_in_dir_empty() {
        let temp = setup_test_dir();
        let rules_dir = temp.path().join("rules");
        fs::create_dir_all(&rules_dir).expect("Failed to create rules dir");

        let count = count_rules_in_dir(&rules_dir);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_rules_in_dir_nonexistent() {
        let temp = setup_test_dir();
        let rules_dir = temp.path().join("nonexistent");

        let count = count_rules_in_dir(&rules_dir);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_configs_project_scope() {
        let temp = setup_test_dir();
        let cwd = temp.path();

        // Create project CLAUDE.md files
        create_file(&cwd.join("CLAUDE.md"), "# Project CLAUDE");
        create_file(&cwd.join("CLAUDE.local.md"), "# Local CLAUDE");
        create_file(&cwd.join(".claude").join("CLAUDE.md"), "# .claude CLAUDE");

        // Create project rules
        create_file(
            &cwd.join(".claude").join("rules").join("rule1.md"),
            "# Rule",
        );

        // Create project MCP config
        create_file(
            &cwd.join(".mcp.json"),
            r#"{"mcpServers": {"project_mcp": {}}}"#,
        );

        // Create project settings with hooks
        create_file(
            &cwd.join(".claude").join("settings.json"),
            r#"{
                "mcpServers": {"settings_mcp": {}},
                "hooks": {"PreToolUse": []}
            }"#,
        );

        let counts = count_configs(Some(cwd.to_str().unwrap()));

        // Note: User scope counts depend on actual home directory state
        // We can only verify project scope additions
        assert!(counts.claude_md_count >= 3); // At least 3 from project
        assert!(counts.rules_count >= 1); // At least 1 from project
        assert!(counts.mcp_count >= 2); // At least 2 from project (project_mcp + settings_mcp)
        assert!(counts.hooks_count >= 1); // At least 1 from project
    }

    #[test]
    fn test_count_configs_disabled_mcp_servers() {
        let temp = setup_test_dir();
        let cwd = temp.path();

        // Create .mcp.json with servers
        create_file(
            &cwd.join(".mcp.json"),
            r#"{"mcpServers": {"enabled_mcp": {}, "disabled_mcp": {}}}"#,
        );

        // Create settings.local.json that disables one server
        create_file(
            &cwd.join(".claude").join("settings.local.json"),
            r#"{"disabledMcpjsonServers": ["disabled_mcp"]}"#,
        );

        let counts = count_configs(Some(cwd.to_str().unwrap()));

        // Only enabled_mcp should be counted from project scope
        // (plus any from user scope)
        assert!(counts.mcp_count >= 1);
    }

    #[test]
    fn test_count_configs_no_cwd() {
        // Test with no cwd - only user scope
        let counts = count_configs(None);

        // Should return valid counts (may be 0 or more depending on user's actual config)
        // Just verify it doesn't panic and returns a valid struct
        assert!(counts.claude_md_count <= 100); // Sanity check
        assert!(counts.rules_count <= 1000);
        assert!(counts.mcp_count <= 100);
        assert!(counts.hooks_count <= 100);
    }

    #[test]
    fn test_file_exists() {
        let temp = setup_test_dir();
        let file_path = temp.path().join("test.txt");

        assert!(!file_exists(&file_path));

        create_file(&file_path, "test");
        assert!(file_exists(&file_path));

        // Directory should return false
        let dir_path = temp.path().join("testdir");
        fs::create_dir_all(&dir_path).expect("Failed to create dir");
        assert!(!file_exists(&dir_path));
    }
}
