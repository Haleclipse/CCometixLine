use std::path::PathBuf;

/// Whether this is a development build (a `-dev` version), which keeps
/// to its own directory so it can't touch an installed release's files.
pub const DEV_BUILD: bool = const_contains(env!("CARGO_PKG_VERSION").as_bytes(), b"-dev");

const fn const_contains(hay: &[u8], needle: &[u8]) -> bool {
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let mut j = 0;
        while j < needle.len() && hay[i + j] == needle[j] {
            j += 1;
        }
        if j == needle.len() {
            return true;
        }
        i += 1;
    }
    false
}

/// ccline's data directory: `$CCLINE_HOME` if set, else `~/.claude/ccline`
/// (`~/.claude/ccline-dev` for development builds).
///
/// The default location is part of the install contract (the npm package
/// copies the binary there), so it does not follow `CLAUDE_CONFIG_DIR`.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("CCLINE_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let name = if DEV_BUILD { "ccline-dev" } else { "ccline" };
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join(name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_builds_keep_their_own_home() {
        assert!(const_contains(b"2.0.0-dev.0", b"-dev"));
        assert!(!const_contains(b"2.0.0", b"-dev"));
        assert_eq!(DEV_BUILD, env!("CARGO_PKG_VERSION").contains("-dev"));
        if std::env::var_os("CCLINE_HOME").is_none() {
            let name = home().file_name().unwrap().to_string_lossy().into_owned();
            assert_eq!(name, if DEV_BUILD { "ccline-dev" } else { "ccline" });
        }
    }
}
