//! The companion mod's bridge: inside Claude Code (2.1.287+), the mod
//! writes what the status line payload lacks to `bridge/<session>.json`
//! under ccline's home: the context window by category and the exact
//! auto-compact threshold, the last turn's timing, and the tools and agents
//! running now. Renders read it back.
//!
//! The mod's writes aren't atomic (mods can't rename), so a read that
//! catches one half done is tried again once, then skipped for this render.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The format the mod writes.
pub const VERSION: u32 = 1;
/// A bridge older than this belongs to a session long gone.
const MAX_AGE_MS: i64 = 6 * 3_600_000;
/// Files untouched this long are removed.
const PRUNE_AFTER: Duration = Duration::from_secs(7 * 86_400);
/// How often renders look for old files.
const PRUNE_EVERY: Duration = Duration::from_secs(86_400);
/// Wait before reading again after catching a write half done.
const RETRY: Duration = Duration::from_millis(3);

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Bridge {
    pub v: u32,
    pub session: Option<String>,
    /// Unix epoch milliseconds of the write.
    pub updated_at: i64,
    pub context: Option<Context>,
    pub turn: Option<Turn>,
    /// Tool calls running now.
    pub tools: Vec<Tool>,
    pub agents: Vec<Agent>,
}

/// The context window as `/context` breaks it down.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Context {
    /// Tokens at which auto-compaction runs; absent when it is off.
    pub threshold: Option<u64>,
    /// Whether auto-compaction is on.
    pub auto: bool,
    /// The window the breakdown measures against (the compaction window).
    pub window: Option<u64>,
    /// Tokens in use, estimated.
    pub used: Option<u64>,
    pub categories: Vec<Category>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Category {
    /// `/context`'s label: `System prompt`, `Messages`, `Free space`, ...
    pub name: String,
    /// `used`, `free`, `buffer` or `deferred`.
    pub kind: String,
    pub tokens: u64,
}

/// The last main-thread turn.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Turn {
    pub model: Option<String>,
    /// From the request going out to the first piece of the answer.
    pub ttft_ms: Option<u64>,
    /// Output tokens per second while the answer streamed.
    pub tps: Option<f64>,
    pub duration_ms: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Unix epoch milliseconds it ended.
    pub ended_at: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Tool {
    pub name: String,
    /// Unix epoch milliseconds it started.
    pub since: i64,
    /// The agent running it; absent on the main loop.
    pub agent: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Agent {
    pub id: String,
    /// The agent type it was spawned as (`Explore`, `general-purpose`, ...).
    #[serde(rename = "type")]
    pub kind: String,
    /// `pending`, `running`, `waiting`, `idle`, `completed`, `failed`, `killed`.
    pub status: String,
    pub description: Option<String>,
}

impl Agent {
    pub fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "pending" | "running" | "waiting")
    }
}

impl Category {
    /// Rows that hold the conversation's own tokens, not free space.
    pub fn is_used(&self) -> bool {
        self.kind == "used"
    }
}

/// Where the mod writes; the mod reads the same path from the file the
/// installer puts beside it.
pub fn dir() -> PathBuf {
    crate::paths::home().join("bridge")
}

fn path_in(dir: &Path, session: &str) -> Option<PathBuf> {
    Some(dir.join(format!("{}.json", super::safe_name(session)?)))
}

fn parse(text: &str, session: &str, now_ms: i64) -> Result<Option<Bridge>, ()> {
    let bridge: Bridge = serde_json::from_str(text).map_err(|_| ())?;
    let usable = bridge.v == VERSION
        && bridge.session.as_deref().is_none_or(|s| s == session)
        && now_ms - bridge.updated_at <= MAX_AGE_MS;
    Ok(usable.then_some(bridge))
}

/// The bridge for `session`, if the mod has written a usable one.
pub fn read(session: &str, now_ms: i64) -> Option<Bridge> {
    read_in(&dir(), session, now_ms)
}

fn read_in(dir: &Path, session: &str, now_ms: i64) -> Option<Bridge> {
    let path = path_in(dir, session)?;
    let text = std::fs::read_to_string(&path).ok()?;
    match parse(&text, session, now_ms) {
        Ok(bridge) => bridge,
        // Caught mid-write: the next read sees it whole.
        Err(()) => {
            std::thread::sleep(RETRY);
            parse(&std::fs::read_to_string(&path).ok()?, session, now_ms).ok()?
        }
    }
}

/// Remove bridges of sessions not seen for a week, at most once a day.
pub fn prune() {
    prune_in(&dir());
}

fn prune_in(dir: &Path) {
    let marker = dir.join(".pruned");
    let fresh = std::fs::metadata(&marker)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < PRUNE_EVERY);
    if fresh || !dir.is_dir() {
        return;
    }
    let _ = std::fs::write(&marker, "");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > PRUNE_AFTER);
        if old && path.extension().is_some_and(|e| e == "json") {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "v": 1, "session": "s1", "updated_at": 1790808774000,
      "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 118000,
        "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                       {"name": "Messages", "kind": "used", "tokens": 98000},
                       {"name": "Free space", "kind": "free", "tokens": 49000},
                       {"name": "Autocompact buffer", "kind": "buffer", "tokens": 33000}]},
      "turn": {"model": "claude-opus-4-8", "ttft_ms": 820, "tps": 74.2, "duration_ms": 41000},
      "tools": [{"name": "Edit", "since": 1790808770000}],
      "agents": [{"id": "a1", "type": "Explore", "status": "running"},
                 {"id": "a2", "type": "general-purpose", "status": "completed"}],
      "future": {"ignored": true}
    }"#;
    const NOW: i64 = 1790808780000;

    #[test]
    fn parses_and_checks_freshness() {
        let b = parse(SAMPLE, "s1", NOW).unwrap().unwrap();
        let ctx = b.context.as_ref().unwrap();
        assert_eq!((ctx.threshold, ctx.window), (Some(167000), Some(200000)));
        assert_eq!(ctx.categories.iter().filter(|c| c.is_used()).count(), 2);
        assert_eq!(b.turn.as_ref().unwrap().ttft_ms, Some(820));
        assert_eq!(b.agents.iter().filter(|a| a.is_active()).count(), 1);
        // Another session's, or a stale one, isn't used.
        assert_eq!(parse(SAMPLE, "s2", NOW), Ok(None));
        assert_eq!(parse(SAMPLE, "s1", NOW + MAX_AGE_MS), Ok(None));
        // Half a file is an error to retry, not a bridge.
        assert_eq!(parse(&SAMPLE[..80], "s1", NOW), Err(()));
    }

    #[test]
    fn reads_from_the_bridge_dir() {
        let dir = std::env::temp_dir().join(format!("ccline-bridge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s1.json"), SAMPLE).unwrap();
        assert!(read_in(&dir, "s1", NOW).is_some());
        assert!(read_in(&dir, "nope", NOW).is_none());
        assert!(read_in(&dir, "", NOW).is_none());
        prune_in(&dir);
        assert!(dir.join(".pruned").exists() && dir.join("s1.json").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
