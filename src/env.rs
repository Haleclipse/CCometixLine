//! The terminal and Claude Code environment the status line runs in.

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

/// `statusLine.padding` from Claude Code's settings: project local, then
/// project, then user settings (which honor `CLAUDE_CONFIG_DIR`).
pub fn statusline_padding(project_dir: Option<&str>) -> usize {
    let mut files = Vec::new();
    if let Some(dir) = project_dir.map(Path::new) {
        files.push(dir.join(".claude").join("settings.local.json"));
        files.push(dir.join(".claude").join("settings.json"));
    }
    files.push(crate::paths::claude_dir().join("settings.json"));
    files
        .iter()
        .find_map(|f| {
            let text = std::fs::read_to_string(f).ok()?;
            let json: serde_json::Value = serde_json::from_str(&text).ok()?;
            json.get("statusLine")?.get("padding")?.as_u64()
        })
        .unwrap_or(0) as usize
}
