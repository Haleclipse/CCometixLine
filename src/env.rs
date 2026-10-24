//! The terminal and Claude Code environment the status line runs in.

use crate::input::Input;
use crate::style::ColorDepth;
use std::path::Path;

/// Terminal width from `COLUMNS`, which Claude Code sets since 2.1.153.
pub fn columns() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()?
        .trim()
        .parse()
        .ok()
        .filter(|c| *c > 0)
}

/// Color depth from the environment: `NO_COLOR` turns color off,
/// `COLORTERM=truecolor|24bit` allows RGB, a `*256color` TERM allows 256.
pub fn color_depth() -> ColorDepth {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return ColorDepth::None;
    }
    let colorterm = var("COLORTERM").to_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" {
        return ColorDepth::TrueColor;
    }
    if var("TERM").contains("256color") {
        return ColorDepth::Ansi256;
    }
    ColorDepth::Ansi16
}

/// Claude Code's settings files that exist, highest precedence first:
/// project local, project, then user (which honors `CLAUDE_CONFIG_DIR`).
#[derive(Clone, Debug, Default)]
pub struct ClaudeSettings(Vec<serde_json::Value>);

impl ClaudeSettings {
    pub fn load(project_dir: Option<&str>) -> Self {
        let mut files = Vec::new();
        if let Some(dir) = project_dir.map(Path::new) {
            files.push(dir.join(".claude").join("settings.local.json"));
            files.push(dir.join(".claude").join("settings.json"));
        }
        files.push(crate::paths::claude_dir().join("settings.json"));
        ClaudeSettings(
            files
                .iter()
                .filter_map(|f| serde_json::from_str(&std::fs::read_to_string(f).ok()?).ok())
                .collect(),
        )
    }

    /// Settings for the session's project, falling back to
    /// `CLAUDE_PROJECT_DIR` when the input has none.
    pub fn for_input(input: &Input) -> Self {
        let project = input
            .workspace
            .as_ref()
            .and_then(|w| w.project_dir.clone())
            .or_else(|| std::env::var("CLAUDE_PROJECT_DIR").ok());
        ClaudeSettings::load(project.as_deref())
    }

    /// The first file that sets the value wins.
    fn find<T>(&self, get: impl Fn(&serde_json::Value) -> Option<T>) -> Option<T> {
        self.0.iter().find_map(get)
    }

    pub fn statusline_padding(&self) -> usize {
        self.find(|j| j.get("statusLine")?.get("padding")?.as_u64())
            .unwrap_or(0) as usize
    }

    /// Claude Code's auto-compact settings, with its environment overrides.
    pub fn auto_compact(&self) -> AutoCompact {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let truthy = |k: &str| {
            var(k).is_some_and(|v| {
                matches!(
                    v.trim().to_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
        };
        let enabled = !truthy("DISABLE_COMPACT")
            && !truthy("DISABLE_AUTO_COMPACT")
            && self
                .find(|j| j.get("autoCompactEnabled")?.as_bool())
                .unwrap_or(true);
        // The variable wins over the setting; Claude Code keeps it within
        // 100k..1M and ignores values it can't parse.
        let window = match var("CLAUDE_CODE_AUTO_COMPACT_WINDOW")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| *v > 0.0)
        {
            Some(v) => Some((v as u64).clamp(100_000, 1_000_000)),
            None => self.find(|j| j.get("autoCompactWindow")?.as_u64()),
        };
        let pct = var("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|p| *p > 0.0 && *p <= 100.0);
        AutoCompact {
            enabled,
            window,
            pct,
        }
    }
}

/// How Claude Code decides when to compact automatically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoCompact {
    pub enabled: bool,
    /// Caps the window compaction is measured against
    /// (`CLAUDE_CODE_AUTO_COMPACT_WINDOW` or `autoCompactWindow`).
    pub window: Option<u64>,
    /// `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`: compact at this share of the
    /// usable window, if that comes before the default point.
    pub pct: Option<f64>,
}

impl Default for AutoCompact {
    fn default() -> Self {
        AutoCompact {
            enabled: true,
            window: None,
            pct: None,
        }
    }
}
