//! Model ids seen in recent sessions, so the TUI can offer to add rules for
//! the ones nothing recognized.

use super::{dir, read_json, write_json};
use crate::models::{NameSource, Resolved};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const KEEP_SECS: i64 = 30 * 86_400;
/// Bump an already known id at most this often.
const REFRESH_SECS: i64 = 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    /// Times seen, counted at most once per refresh interval.
    pub count: u64,
    pub last: i64,
    /// What Claude Code reported as the window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cc_window: Option<u64>,
    /// `user`, `claude_code`, `builtin` or `raw` (nothing recognized it).
    pub source: String,
}

fn source_name(s: NameSource) -> &'static str {
    match s {
        NameSource::UserRule(_) => "user",
        NameSource::ClaudeCode => "claude_code",
        NameSource::Builtin => "builtin",
        NameSource::RawId => "raw",
    }
}

pub fn load() -> BTreeMap<String, Seen> {
    read_json(&dir().join("models_seen.json")).unwrap_or_default()
}

/// Note that `id` was seen and how it resolved.
pub fn record(id: &str, resolved: &Resolved, cc_window: Option<u64>, now: i64) {
    let path = dir().join("models_seen.json");
    let mut seen: BTreeMap<String, Seen> = read_json(&path).unwrap_or_default();
    let source = source_name(resolved.source);
    let stale = match seen.get(id) {
        None => true,
        Some(s) => s.source != source || s.cc_window != cc_window || now - s.last >= REFRESH_SECS,
    };
    if !stale {
        return;
    }
    let entry = seen.entry(id.to_string()).or_insert(Seen {
        count: 0,
        last: now,
        cc_window,
        source: source.to_string(),
    });
    entry.count += 1;
    entry.last = now;
    entry.cc_window = cc_window;
    entry.source = source.to_string();
    seen.retain(|_, s| now - s.last <= KEEP_SECS);
    let _ = write_json(&path, &seen);
}
