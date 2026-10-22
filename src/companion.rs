//! The companion mod (see `mod/` and `state::bridge`): its files, built into
//! ccline, and installing it into Claude Code. ccline writes the files as a
//! local plugin marketplace under its home, then has Claude Code's own
//! `claude plugin` commands add the marketplace and install the plugin, so
//! Claude Code records the install itself.

use crate::process::{self, Outcome};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const MARKETPLACE: &str = "ccline";
/// The plugin's id in Claude Code: `<plugin>@<marketplace>`.
pub const PLUGIN: &str = "ccline@ccline";
/// Each `claude plugin` command may take this long.
const STEP_TIMEOUT: Duration = Duration::from_secs(90);

/// The marketplace's files, relative to its root.
const FILES: [(&str, &str); 4] = [
    (
        ".claude-plugin/marketplace.json",
        include_str!("../mod/.claude-plugin/marketplace.json"),
    ),
    (
        "ccline/.claude-plugin/plugin.json",
        include_str!("../mod/ccline/.claude-plugin/plugin.json"),
    ),
    (
        "ccline/hooks/hooks.json",
        include_str!("../mod/ccline/hooks/hooks.json"),
    ),
    (
        "ccline/hooks/ccline.ts",
        include_str!("../mod/ccline/hooks/ccline.ts"),
    ),
];

/// The plugin's version: ccline's own, so an upgrade shows as one.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Where the marketplace is written.
pub fn dir() -> PathBuf {
    crate::paths::home().join("mod")
}

/// Write the marketplace into `root`: the built-in files with the plugin
/// stamped with this version, and where the mod should write the bridge.
pub fn write_files(root: &Path) -> std::io::Result<()> {
    for (rel, text) in FILES {
        let text = if rel.ends_with("plugin.json") {
            let mut json: serde_json::Value =
                serde_json::from_str(text).map_err(std::io::Error::other)?;
            json["version"] = version().into();
            serde_json::to_string_pretty(&json).map_err(std::io::Error::other)? + "\n"
        } else {
            text.to_string()
        };
        crate::paths::write_atomic(&root.join(rel), &text)?;
    }
    let bridge = serde_json::json!({ "dir": crate::state::bridge::dir() });
    crate::paths::write_atomic(&root.join("ccline/bridge.json"), &bridge.to_string())
}

/// What Claude Code has installed of the mod.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    pub version: Option<String>,
    pub scope: String,
}

/// The user-scope install of the mod, as Claude Code records it in
/// `plugins/installed_plugins.json`.
pub fn installed() -> Option<Installed> {
    installed_in(&crate::paths::claude_dir())
}

fn installed_in(claude_dir: &Path) -> Option<Installed> {
    let text = std::fs::read_to_string(claude_dir.join("plugins/installed_plugins.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let entries = json.get("plugins")?.get(PLUGIN)?.as_array()?;
    let entry = entries
        .iter()
        .find(|e| e.get("scope").and_then(|s| s.as_str()) == Some("user"))
        .or_else(|| entries.first())?;
    Some(Installed {
        version: entry
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        scope: entry
            .get("scope")
            .and_then(|s| s.as_str())
            .unwrap_or("user")
            .to_string(),
    })
}

/// Where the mod stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    NotInstalled,
    /// Installed from an older (or newer) ccline.
    Outdated(Installed),
    Current(Installed),
}

pub fn status() -> Status {
    match installed() {
        None => Status::NotInstalled,
        Some(i) if i.version.as_deref() == Some(version()) => Status::Current(i),
        Some(i) => Status::Outdated(i),
    }
}

/// The `claude` command to run: `CCLINE_CLAUDE` when set, else `claude`
/// from the PATH.
fn claude() -> String {
    std::env::var("CCLINE_CLAUDE")
        .ok()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| "claude".into())
}

fn step(args: &[&str]) -> Vec<String> {
    std::iter::once(claude())
        .chain(args.iter().map(|a| a.to_string()))
        .collect()
}

/// The commands that install the mod, or bring an install up to date.
pub fn install_steps(status: &Status, root: &Path) -> Vec<Vec<String>> {
    let root = root.to_string_lossy();
    match status {
        Status::NotInstalled => vec![
            step(&[
                "plugin",
                "marketplace",
                "add",
                &root,
                "--scope",
                "user",
                "--json",
            ]),
            step(&["plugin", "install", PLUGIN, "--scope", "user", "--json"]),
        ],
        Status::Outdated(_) | Status::Current(_) => vec![
            step(&["plugin", "marketplace", "update", MARKETPLACE, "--json"]),
            step(&[
                "plugin", "update", PLUGIN, "--scope", "user", "-y", "--json",
            ]),
        ],
    }
}

/// The commands that remove it.
pub fn uninstall_steps() -> Vec<Vec<String>> {
    vec![
        step(&[
            "plugin",
            "uninstall",
            PLUGIN,
            "--scope",
            "user",
            "-y",
            "--json",
        ]),
        step(&[
            "plugin",
            "marketplace",
            "remove",
            MARKETPLACE,
            "--scope",
            "user",
            "--json",
        ]),
    ]
}

/// A command as the user could type it.
pub fn shown(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run `steps` in order, stopping at the first that fails.
pub fn run(steps: &[Vec<String>]) -> Result<(), String> {
    for argv in steps {
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        match process::run(cmd, None, STEP_TIMEOUT) {
            Outcome::Exited { success: true, .. } => {}
            Outcome::Exited { stdout, .. } => {
                let said = stdout;
                let last = said
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("");
                return Err(format!("`{}` failed: {}", shown(argv), last.trim()));
            }
            Outcome::TimedOut => return Err(format!("`{}` took too long", shown(argv))),
            Outcome::Failed => {
                return Err(format!(
                    "couldn't run `{}`; is Claude Code's `claude` on the PATH?",
                    argv[0]
                ));
            }
        }
    }
    Ok(())
}

/// Write the files and install or update the mod.
pub fn install() -> Result<(), String> {
    let root = dir();
    write_files(&root).map_err(|e| format!("couldn't write {}: {e}", root.display()))?;
    run(&install_steps(&status(), &root))
}

pub fn uninstall() -> Result<(), String> {
    run(&uninstall_steps())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ccline-mod-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn writes_the_marketplace() {
        let root = temp("files");
        write_files(&root).unwrap();
        let plugin: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("ccline/.claude-plugin/plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(plugin["version"], version());
        assert_eq!(plugin["name"], "ccline");
        let market: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude-plugin/marketplace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(market["name"], MARKETPLACE);
        assert_eq!(market["plugins"][0]["source"], "./ccline");
        let bridge = std::fs::read_to_string(root.join("ccline/bridge.json")).unwrap();
        assert!(bridge.contains("bridge"));
        assert!(root.join("ccline/hooks/ccline.ts").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reads_claude_codes_install_record() {
        let claude = temp("record");
        std::fs::create_dir_all(claude.join("plugins")).unwrap();
        assert_eq!(installed_in(&claude), None);
        std::fs::write(
            claude.join("plugins/installed_plugins.json"),
            r#"{"version": 2, "plugins": {"ccline@ccline": [
                {"scope": "local", "version": "1.0.0"},
                {"scope": "user", "version": "2.0.0", "installPath": "/x"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            installed_in(&claude),
            Some(Installed {
                version: Some("2.0.0".into()),
                scope: "user".into()
            })
        );
        std::fs::remove_dir_all(&claude).unwrap();
    }

    #[test]
    fn plans_steps_by_status() {
        let root = Path::new("/home/u/.claude/ccline/mod");
        let fresh = install_steps(&Status::NotInstalled, root);
        assert_eq!(fresh.len(), 2);
        assert!(
            shown(&fresh[0])
                .ends_with("plugin marketplace add /home/u/.claude/ccline/mod --scope user --json")
        );
        assert!(shown(&fresh[1]).ends_with("plugin install ccline@ccline --scope user --json"));
        let old = Installed {
            version: Some("1.0.0".into()),
            scope: "user".into(),
        };
        let update = install_steps(&Status::Outdated(old), root);
        assert!(shown(&update[1]).ends_with("plugin update ccline@ccline --scope user -y --json"));
        assert!(shown(&uninstall_steps()[0]).contains("uninstall ccline@ccline"));
    }
}
