//! Persistent state under `~/.claude/ccline/state/`: per-session history,
//! cached git status, model ids seen, the daily cost ledger and the update
//! check. Every write is atomic; files shared between sessions are
//! read-modify-write with the last writer winning, which the next render
//! corrects.

pub mod git_cache;
pub mod ledger;
pub mod models_seen;
pub mod session;

use crate::input::Input;
use crate::models::Registry;
use chrono::{DateTime, FixedOffset, Timelike};
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

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A stable file name for `s` (FNV-1a, hex).
pub fn file_key(s: &str) -> String {
    format!("{:016x}", fnv1a(s))
}

/// A short stable key for a prompt id; samples only compare them.
pub fn prompt_key(id: &str) -> u32 {
    fnv1a(id) as u32
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

/// Take the lock file at `path` for work that should run once at a time; a
/// lock older than `stale` is taken to be abandoned. Returns whether the
/// lock is now held.
pub fn try_lock(path: &Path, stale: Duration) -> bool {
    let create = || {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match create() {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let abandoned = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .is_ok_and(|t| t.elapsed().is_ok_and(|age| age > stale));
            abandoned && std::fs::remove_file(path).is_ok() && create().is_ok()
        }
        Err(_) => false,
    }
}

/// Start `ccline <args>` in the background, detached from Claude Code's
/// pipes and process group so it outlives the render and nothing waits for
/// it. Returns whether it started.
pub fn spawn_detached(args: &[&std::ffi::OsStr]) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().is_ok()
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
    /// Samples of this session, oldest first, ending with the current one.
    pub samples: Vec<Sample>,
    /// Spend across sessions today, from the ledger.
    pub today_cost: Option<f64>,
    /// Newest ccline version known to be published.
    pub latest_version: Option<String>,
}

/// Record this render: add a session sample, fold the session's cost into
/// today's ledger and note the model id. `raw` is the stdin payload, kept
/// for replay. Returns what metrics read back.
pub fn observe(
    input: &Input,
    raw: &str,
    registry: &Registry,
    now: i64,
    tz: FixedOffset,
) -> StateView {
    let current = Sample::from_input(input, now);
    let Some(id) = input
        .session_id
        .as_deref()
        .filter(|id| safe_name(id).is_some())
    else {
        return StateView {
            samples: vec![current],
            ..StateView::default()
        };
    };

    let mut session = SessionState::load(id);
    let added = session.record(current);
    session.last_input = serde_json::from_str(raw).ok();
    session.save_if_needed(id, now, added);

    let today_cost = current.cost.and_then(|cost| {
        let local = DateTime::from_timestamp(now, 0)?.with_timezone(&tz);
        let midnight = now - local.num_seconds_from_midnight() as i64;
        let started_today = input
            .cost
            .as_ref()
            .and_then(|c| c.total_duration_ms)
            .is_some_and(|ms| now - (ms / 1000.0) as i64 >= midnight);
        let date = local.format("%Y-%m-%d").to_string();
        Some(ledger::update(id, cost, started_today, &date, now))
    });

    if let Some(model) = &input.model
        && let Some(model_id) = model.id.as_deref()
    {
        let window = input
            .context_window
            .as_ref()
            .and_then(|c| c.context_window_size);
        let resolved = registry.resolve(model_id, model.display_name.as_deref(), window);
        models_seen::record(model_id, &resolved, window, now);
    }

    let mut samples = session.samples;
    if !added {
        // Unchanged values aren't stored, but rates still need "now".
        samples.push(current);
    }
    StateView {
        samples,
        today_cost,
        latest_version: None,
    }
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
    fn locks_once_until_released_or_abandoned() {
        let dir = std::env::temp_dir().join(format!("ccline-lock-{}", std::process::id()));
        let lock = dir.join("x.lock");
        assert!(try_lock(&lock, Duration::from_secs(60)));
        assert!(!try_lock(&lock, Duration::from_secs(60)));
        std::thread::sleep(Duration::from_millis(20));
        assert!(try_lock(&lock, Duration::from_millis(10)));
        std::fs::remove_file(&lock).unwrap();
        assert!(try_lock(&lock, Duration::from_secs(60)));
        std::fs::remove_dir_all(&dir).unwrap();
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
