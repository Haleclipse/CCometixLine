//! Claude Code's user settings (`settings.json`): reading the status line
//! settings, and changing them with a diff first, a backup and an atomic
//! write. Keys keep their order and settings ccline doesn't know are left
//! as they are.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// `statusLine` as Claude Code reads it (checked against 2.1.285).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusLine {
    pub command: String,
    pub padding: Option<u64>,
    /// Seconds between re-runs besides the event-driven ones; at least 1.
    pub refresh_interval: Option<u64>,
    /// Hide Claude Code's own `-- INSERT --` line.
    pub hide_vim_mode_indicator: Option<bool>,
}

#[derive(Debug)]
pub enum SaveError {
    /// The file changed on disk since it was read.
    Changed,
    Io(std::io::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Changed => write!(f, "settings.json changed on disk; reload and try again"),
            SaveError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// One line of a diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Same,
    Added,
    Removed,
}

/// The settings file and its contents.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub path: PathBuf,
    /// The file as read; `None` when there was none.
    text: Option<String>,
    /// Always an object.
    value: Value,
}

/// Keys of `statusLine` in the order Claude Code documents them.
const STATUS_KEYS: [&str; 5] = [
    "type",
    "command",
    "padding",
    "refreshInterval",
    "hideVimModeIndicator",
];

/// Whether `command` runs ccline.
pub fn is_ccline(command: &str) -> bool {
    command.contains("ccline") || command.contains("ccometixline")
}

/// What to put in `statusLine.command`: the installed binary under
/// `~/.claude/ccline` when present, else this executable.
pub fn suggested_command() -> String {
    let name = if cfg!(windows) {
        "ccline.exe"
    } else {
        "ccline"
    };
    let installed = crate::paths::claude_dir().join("ccline").join(name);
    let path = if installed.exists() {
        installed
    } else {
        std::env::current_exe().unwrap_or(installed)
    };
    let shown = path.to_string_lossy().into_owned();
    match dirs::home_dir() {
        Some(home) if !cfg!(windows) => match path.strip_prefix(&home) {
            Ok(rest) => format!("~/{}", rest.to_string_lossy()),
            Err(_) => shown,
        },
        _ => shown,
    }
}

impl Settings {
    /// `settings.json` in Claude Code's config directory.
    pub fn user_path() -> PathBuf {
        crate::paths::claude_dir().join("settings.json")
    }

    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let value = match text.as_deref().map(str::trim) {
            None | Some("") => Value::Object(Map::new()),
            Some(t) => match serde_json::from_str::<Value>(t) {
                Ok(v @ Value::Object(_)) => v,
                Ok(_) => return Err("settings.json isn't a JSON object".into()),
                Err(e) => return Err(format!("settings.json isn't plain JSON: {e}")),
            },
        };
        Ok(Settings {
            path: path.to_path_buf(),
            text,
            value,
        })
    }

    fn object(&self) -> &Map<String, Value> {
        self.value.as_object().expect("settings are an object")
    }

    fn object_mut(&mut self) -> &mut Map<String, Value> {
        self.value.as_object_mut().expect("settings are an object")
    }

    pub fn status_line(&self) -> Option<StatusLine> {
        let s = self.object().get("statusLine")?;
        Some(StatusLine {
            command: s.get("command")?.as_str()?.to_string(),
            padding: s.get("padding").and_then(Value::as_u64),
            refresh_interval: s.get("refreshInterval").and_then(Value::as_u64),
            hide_vim_mode_indicator: s.get("hideVimModeIndicator").and_then(Value::as_bool),
        })
    }

    pub fn subagent_command(&self) -> Option<String> {
        let s = self.object().get("subagentStatusLine")?;
        Some(s.get("command")?.as_str()?.to_string())
    }

    /// Replace `statusLine`, keeping keys ccline doesn't know.
    pub fn set_status_line(&mut self, line: Option<&StatusLine>) {
        let Some(line) = line else {
            self.object_mut().remove("statusLine");
            return;
        };
        let mut obj = match self.object().get("statusLine") {
            Some(Value::Object(o)) => o.clone(),
            _ => Map::new(),
        };
        let values = [
            Some(Value::from("command")),
            Some(Value::from(line.command.clone())),
            line.padding.map(Value::from),
            line.refresh_interval.map(|n| Value::from(n.max(1))),
            line.hide_vim_mode_indicator.map(Value::from),
        ];
        for (key, value) in STATUS_KEYS.iter().zip(values) {
            match value {
                Some(v) => {
                    obj.insert(key.to_string(), v);
                }
                None => {
                    obj.remove(*key);
                }
            }
        }
        self.object_mut()
            .insert("statusLine".into(), Value::Object(obj));
    }

    pub fn set_subagent(&mut self, command: Option<&str>) {
        match command {
            Some(c) => {
                let mut obj = Map::new();
                obj.insert("type".into(), Value::from("command"));
                obj.insert("command".into(), Value::from(c));
                self.object_mut()
                    .insert("subagentStatusLine".into(), Value::Object(obj));
            }
            None => {
                self.object_mut().remove("subagentStatusLine");
            }
        }
    }

    /// The file as ccline writes it: two-space JSON, like Claude Code.
    pub fn render(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.value).unwrap_or_default();
        s.push('\n');
        s
    }

    /// `"key": value` lines for one top-level key; none when it's absent.
    fn excerpt(&self, key: &str) -> Vec<String> {
        let Some(v) = self.object().get(key) else {
            return Vec::new();
        };
        let pretty = serde_json::to_string_pretty(v).unwrap_or_default();
        pretty
            .lines()
            .enumerate()
            .map(|(i, l)| {
                if i == 0 {
                    format!("{}: {l}", Value::from(key))
                } else {
                    l.to_string()
                }
            })
            .collect()
    }

    /// The top-level keys whose values differ from `before`.
    pub fn changed_keys(&self, before: &Settings) -> Vec<String> {
        let (a, b) = (before.object(), self.object());
        let mut keys: Vec<String> = Vec::new();
        for k in b.keys().chain(a.keys()) {
            if a.get(k) != b.get(k) && !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        keys
    }

    /// The changed keys, before and after, line by line.
    pub fn diff(&self, before: &Settings) -> Vec<(Mark, String)> {
        let mut out = Vec::new();
        for key in self.changed_keys(before) {
            if !out.is_empty() {
                out.push((Mark::Same, String::new()));
            }
            out.extend(diff_lines(&before.excerpt(&key), &self.excerpt(&key)));
        }
        out
    }

    fn backup_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".ccline-bak");
        self.path.with_file_name(name)
    }

    /// Write the settings unless the file changed since `base` was read,
    /// keeping the old file as `settings.json.ccline-bak`. A symlinked file
    /// is written through the link. Returns the backup's path, if any.
    pub fn save(&self, base: &Settings) -> Result<Option<PathBuf>, SaveError> {
        let on_disk = match std::fs::read_to_string(&self.path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(SaveError::Io(e)),
        };
        if on_disk != base.text {
            return Err(SaveError::Changed);
        }
        let backup = match &base.text {
            Some(text) => {
                let path = self.backup_path();
                crate::paths::write_atomic(&path, text).map_err(SaveError::Io)?;
                Some(path)
            }
            None => None,
        };
        let target = std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone());
        crate::paths::write_atomic(&target, &self.render()).map_err(SaveError::Io)?;
        Ok(backup)
    }
}

/// A line diff of `a` into `b` (longest common subsequence).
pub fn diff_lines(a: &[String], b: &[String]) -> Vec<(Mark, String)> {
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((Mark::Same, a[i].clone()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push((Mark::Removed, a[i].clone()));
            i += 1;
        } else {
            out.push((Mark::Added, b[j].clone()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| (Mark::Removed, l.clone())));
    out.extend(b[j..].iter().map(|l| (Mark::Added, l.clone())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ccline-cc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SAMPLE: &str = r#"{
  "model": "opus",
  "statusLine": {
    "type": "command",
    "command": "bash ~/old.sh",
    "padding": 0,
    "futureKey": true
  },
  "env": {"A": "1"}
}
"#;

    #[test]
    fn edits_keep_order_and_unknown_keys() {
        let dir = temp("edit");
        let path = dir.join("settings.json");
        std::fs::write(&path, SAMPLE).unwrap();
        let base = Settings::load(&path).unwrap();
        assert_eq!(base.status_line().unwrap().command, "bash ~/old.sh");

        let mut s = base.clone();
        s.set_status_line(Some(&StatusLine {
            command: "~/.claude/ccline/ccline".into(),
            padding: Some(0),
            refresh_interval: Some(2),
            hide_vim_mode_indicator: None,
        }));
        s.set_subagent(Some("~/.claude/ccline/ccline"));
        let text = s.render();
        let keys: Vec<usize> = [
            "\"model\"",
            "\"statusLine\"",
            "\"env\"",
            "\"subagentStatusLine\"",
        ]
        .iter()
        .map(|k| text.find(k).unwrap())
        .collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "{text}");
        assert!(text.contains("\"futureKey\": true"));
        assert_eq!(
            s.changed_keys(&base),
            vec!["statusLine", "subagentStatusLine"]
        );

        let diff = s.diff(&base);
        assert!(diff.contains(&(Mark::Removed, "  \"command\": \"bash ~/old.sh\",".into())));
        assert!(diff.contains(&(Mark::Added, "  \"refreshInterval\": 2".into())));
        assert!(diff.contains(&(Mark::Added, "\"subagentStatusLine\": {".into())));

        let backup = s.save(&base).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), SAMPLE);
        let again = Settings::load(&path).unwrap();
        assert_eq!(again.status_line().unwrap().refresh_interval, Some(2));
        assert_eq!(
            again.subagent_command().as_deref(),
            Some("~/.claude/ccline/ccline")
        );

        // Saving over a file that changed since is refused.
        assert!(matches!(s.save(&base), Err(SaveError::Changed)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn creates_a_missing_file_and_writes_through_links() {
        let dir = temp("link");
        let path = dir.join("settings.json");
        let base = Settings::load(&path).unwrap();
        assert_eq!(base.status_line(), None);
        let mut s = base.clone();
        s.set_status_line(Some(&StatusLine {
            command: "ccline".into(),
            ..StatusLine::default()
        }));
        assert_eq!(s.save(&base).unwrap(), None);
        assert!(
            s.render()
                .starts_with("{\n  \"statusLine\": {\n    \"type\": \"command\"")
        );

        #[cfg(unix)]
        {
            let real = dir.join("dotfiles.json");
            std::fs::rename(&path, &real).unwrap();
            std::os::unix::fs::symlink(&real, &path).unwrap();
            let base = Settings::load(&path).unwrap();
            let mut s = base.clone();
            s.set_subagent(Some("ccline"));
            s.save(&base).unwrap();
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert!(
                std::fs::read_to_string(&real)
                    .unwrap()
                    .contains("subagentStatusLine")
            );
        }
        assert!(Settings::load(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn spots_ccline_commands() {
        assert!(is_ccline("~/.claude/ccline/ccline --theme nord"));
        assert!(is_ccline("/x/target/debug/ccometixline"));
        assert!(!is_ccline("bash ~/.claude/statusline.sh"));
    }
}
