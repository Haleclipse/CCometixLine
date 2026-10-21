//! Persistent state under `~/.claude/ccline/state/`: per-session history,
//! cached git status, model ids seen, the daily cost ledger and the update
//! check. Every write is atomic; files shared between sessions are
//! read-modify-write with the last writer winning, which the next render
//! corrects.

pub mod git_cache;
pub mod ledger;
pub mod models_seen;
pub mod session;

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub use session::{Sample, SessionState};

pub fn dir() -> PathBuf {
    crate::paths::state_dir()
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let text = serde_json::to_string(value).map_err(std::io::Error::other)?;
    crate::paths::write_atomic(path, &text)
}

/// A stable file name for `s` (FNV-1a, hex).
pub fn file_key(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Session ids become file names; keep only safe characters.
pub fn safe_name(id: &str) -> Option<String> {
    let name: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(80)
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Delete files in `dir` older than `max_age`, then the oldest beyond
/// `max_files`. Errors are ignored.
pub fn sweep(dir: &Path, max_age: Duration, max_files: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((modified, e.path()))
        })
        .collect();
    files.retain(|(modified, path)| {
        let old = now.duration_since(*modified).is_ok_and(|age| age > max_age);
        if old {
            let _ = std::fs::remove_file(path);
        }
        !old
    });
    if files.len() > max_files {
        files.sort();
        for (_, path) in &files[..files.len() - max_files] {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Run `f` about once in `every` calls, keyed on the clock so no RNG is needed.
pub fn sometimes(every: u32, f: impl FnOnce()) {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    if nanos.is_multiple_of(every) {
        f();
    }
}

/// What metrics may read from state during a render.
#[derive(Clone, Debug, Default)]
pub struct StateView {
    /// Samples of this session, oldest first, including the current one.
    pub samples: Vec<Sample>,
    /// Spend across sessions today, from the ledger.
    pub today_cost: Option<f64>,
    /// Newest ccline version known to be published.
    pub latest_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_names_safe() {
        assert_eq!(file_key("/home/u/app"), file_key("/home/u/app"));
        assert_ne!(file_key("/a"), file_key("/b"));
        assert_eq!(
            safe_name("2ad4a42c-815f/../x").as_deref(),
            Some("2ad4a42c-815fx")
        );
        assert_eq!(safe_name("../"), None);
    }

    #[test]
    fn sweeps_by_count() {
        let dir = std::env::temp_dir().join(format!("ccline-sweep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            std::fs::write(dir.join(format!("{i}.json")), "{}").unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        sweep(&dir, Duration::from_secs(3600), 3);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["2.json", "3.json", "4.json"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
