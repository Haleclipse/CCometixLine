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
pub const PLUGIN: &str = "cometix@ccline";
/// Each `claude plugin` command may take this long.
const STEP_TIMEOUT: Duration = Duration::from_secs(90);

/// The marketplace's files, relative to its root.
const FILES: [(&str, &str); 4] = [
    (
        ".claude-plugin/marketplace.json",
        include_str!("../mod/.claude-plugin/marketplace.json"),
    ),
    (
        "cometix/.claude-plugin/plugin.json",
        include_str!("../mod/cometix/.claude-plugin/plugin.json"),
    ),
    (
        "cometix/hooks/hooks.json",
        include_str!("../mod/cometix/hooks/hooks.json"),
    ),
    (
        "cometix/hooks/ccline.ts",
        include_str!("../mod/cometix/hooks/ccline.ts"),
    ),
];

/// The plugin's version: ccline's own, plus a digest of the mod's files.
/// Claude Code keeps a plugin whose version it already has as it is, so a
/// changed mod needs a new version even within one ccline release.
pub fn version() -> String {
    format!("{}+{:08x}", env!("CARGO_PKG_VERSION"), digest())
}

/// FNV-1a over the files' paths and texts.
fn digest() -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for (rel, text) in FILES {
        for b in rel.bytes().chain(text.bytes()) {
            h ^= u32::from(b);
            h = h.wrapping_mul(0x0100_0193);
        }
    }
    h
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
    crate::paths::write_atomic(&root.join("cometix/bridge.json"), &bridge.to_string())
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
        Some(i) if i.version.as_deref() == Some(version().as_str()) => Status::Current(i),
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

/// Where Claude Code has the `ccline` marketplace from, as it records it in
/// `plugins/known_marketplaces.json`.
pub fn marketplace() -> Option<PathBuf> {
    marketplace_in(&crate::paths::claude_dir())
}

fn marketplace_in(claude_dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(claude_dir.join("plugins/known_marketplaces.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let entry = json.get(MARKETPLACE)?;
    entry
        .get("source")
        .and_then(|s| s.get("path"))
        .or_else(|| entry.get("installLocation"))
        .and_then(|p| p.as_str())
        .map(PathBuf::from)
}

/// The commands that install the mod from `root`, or bring an install up to
/// date: the marketplace is added, read again, or (when Claude Code has it
/// from elsewhere) replaced, then the plugin installed or updated.
pub fn install_steps(status: &Status, known: Option<&Path>, root: &Path) -> Vec<Vec<String>> {
    let shown = root.to_string_lossy();
    let add = step(&[
        "plugin",
        "marketplace",
        "add",
        &shown,
        "--scope",
        "user",
        "--json",
    ]);
    let mut steps = match known {
        None => vec![add],
        Some(path) if path == root => vec![step(&[
            "plugin",
            "marketplace",
            "update",
            MARKETPLACE,
            "--json",
        ])],
        Some(_) => vec![
            step(&[
                "plugin",
                "marketplace",
                "remove",
                MARKETPLACE,
                "--scope",
                "user",
                "--json",
            ]),
            add,
        ],
    };
    steps.push(match status {
        Status::NotInstalled => step(&["plugin", "install", PLUGIN, "--scope", "user", "--json"]),
        Status::Outdated(_) | Status::Current(_) => step(&[
            "plugin", "update", PLUGIN, "--scope", "user", "-y", "--json",
        ]),
    });
    steps
}

/// The commands `install` runs now.
pub fn install_plan() -> Vec<Vec<String>> {
    install_steps(&status(), marketplace().as_deref(), &dir())
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
    // Tests never reach the person's own Claude Code.
    if cfg!(test) && std::env::var_os("CCLINE_CLAUDE").is_none() {
        return Err("tests run no `claude` unless CCLINE_CLAUDE says which".into());
    }
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
    // Planned before writing: the files decide nothing, the records do.
    let steps = install_plan();
    let root = dir();
    write_files(&root).map_err(|e| format!("couldn't write {}: {e}", root.display()))?;
    run(&steps)
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
            &std::fs::read_to_string(root.join("cometix/.claude-plugin/plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(plugin["version"], version());
        assert_eq!(plugin["name"], "cometix");
        let market: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude-plugin/marketplace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(market["name"], MARKETPLACE);
        assert_eq!(market["plugins"][0]["name"], "cometix");
        assert_eq!(market["plugins"][0]["source"], "./cometix");
        assert_eq!(
            format!(
                "{}@{}",
                market["plugins"][0]["name"].as_str().unwrap(),
                MARKETPLACE
            ),
            PLUGIN
        );
        let bridge = std::fs::read_to_string(root.join("cometix/bridge.json")).unwrap();
        assert!(bridge.contains("bridge"));
        let hooks = std::fs::read_to_string(root.join("cometix/hooks/hooks.json")).unwrap();
        assert!(hooks.contains("./ccline.ts"));
        assert!(root.join("cometix/hooks/ccline.ts").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn versions_by_release_and_content() {
        let v = version();
        let (release, hash) = v.split_once('+').unwrap();
        assert_eq!(release, env!("CARGO_PKG_VERSION"));
        assert_eq!(hash.len(), 8);
        assert_eq!(version(), v, "stable for the same files");
    }

    #[test]
    fn reads_where_claude_code_has_the_marketplace() {
        let claude = temp("known");
        std::fs::create_dir_all(claude.join("plugins")).unwrap();
        assert_eq!(marketplace_in(&claude), None);
        std::fs::write(
            claude.join("plugins/known_marketplaces.json"),
            r#"{"other": {"source": {"source": "github", "repo": "a/b"}},
                "ccline": {"source": {"source": "directory", "path": "/old/mod"},
                           "installLocation": "/old/mod"}}"#,
        )
        .unwrap();
        assert_eq!(marketplace_in(&claude), Some(PathBuf::from("/old/mod")));
        std::fs::remove_dir_all(&claude).unwrap();
    }

    #[test]
    fn reads_claude_codes_install_record() {
        let claude = temp("record");
        std::fs::create_dir_all(claude.join("plugins")).unwrap();
        assert_eq!(installed_in(&claude), None);
        std::fs::write(
            claude.join("plugins/installed_plugins.json"),
            r#"{"version": 2, "plugins": {"cometix@ccline": [
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
    fn plans_steps_by_status_and_marketplace() {
        let root = Path::new("/home/u/.claude/ccline/mod");
        let shown_all = |steps: Vec<Vec<String>>| -> Vec<String> {
            steps
                .iter()
                .map(|s| shown(s).split_once(' ').unwrap().1.to_string())
                .collect()
        };
        assert_eq!(
            shown_all(install_steps(&Status::NotInstalled, None, root)),
            [
                "plugin marketplace add /home/u/.claude/ccline/mod --scope user --json",
                "plugin install cometix@ccline --scope user --json",
            ]
        );
        let old = Installed {
            version: Some("1.0.0".into()),
            scope: "user".into(),
        };
        assert_eq!(
            shown_all(install_steps(&Status::Outdated(old), Some(root), root)),
            [
                "plugin marketplace update ccline --json",
                "plugin update cometix@ccline --scope user -y --json",
            ]
        );
        // Claude Code has the marketplace from another folder: replace it.
        assert_eq!(
            shown_all(install_steps(
                &Status::NotInstalled,
                Some(Path::new("/home/u/.claude/ccline-dev/mod")),
                root
            )),
            [
                "plugin marketplace remove ccline --scope user --json",
                "plugin marketplace add /home/u/.claude/ccline/mod --scope user --json",
                "plugin install cometix@ccline --scope user --json",
            ]
        );
        assert!(shown(&uninstall_steps()[0]).contains("uninstall cometix@ccline"));
    }
}
