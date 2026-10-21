use std::path::PathBuf;

/// ccline's data directory: `$CCLINE_HOME` if set, else `~/.claude/ccline`.
///
/// The default location is part of the install contract (the npm package
/// copies the binary there), so it does not follow `CLAUDE_CONFIG_DIR`.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("CCLINE_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("ccline")
}

pub fn config_file() -> PathBuf {
    home().join("config.toml")
}

pub fn legacy_config_backup() -> PathBuf {
    home().join("config.v1.toml")
}

pub fn models_file() -> PathBuf {
    home().join("models.toml")
}

pub fn state_dir() -> PathBuf {
    home().join("state")
}

/// Claude Code's own config directory, honoring `CLAUDE_CONFIG_DIR`.
pub fn claude_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::home_dir().unwrap_or_default().join(".claude")
}

/// Write `contents` to `path` atomically: write a sibling temp file, then
/// rename it over the target.
pub fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
