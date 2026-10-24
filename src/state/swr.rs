//! Stale-while-revalidate for slow work (git status, user commands): a
//! render uses the stored value, even an outdated one, and leaves the
//! refresh to a detached ccline process. Claude Code cancels a render that
//! runs past its next refresh, so renders must not wait on slow work.

use super::{read_json, spawn_detached, try_lock, write_json};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A refresh lock older than this was left by a refresh that died.
const LOCK_STALE: Duration = Duration::from_secs(30);

#[derive(Serialize, Deserialize)]
struct Stamped<T> {
    at_ms: u64,
    value: T,
}

pub enum Lookup<T> {
    /// Younger than the freshness window.
    Fresh(T),
    /// Usable, but due for a refresh.
    Stale(T),
    /// Nothing stored, or too old to show.
    Missing,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The value stored at `path`, judged against `ttl_ms` (fresh) and
/// `max_ms` (still worth showing).
pub fn lookup<T: DeserializeOwned>(path: &Path, ttl_ms: u64, max_ms: u64) -> Lookup<T> {
    match read_json::<Stamped<T>>(path) {
        Some(s) => match now_ms().saturating_sub(s.at_ms) {
            age if age < ttl_ms => Lookup::Fresh(s.value),
            age if age < max_ms => Lookup::Stale(s.value),
            _ => Lookup::Missing,
        },
        None => Lookup::Missing,
    }
}

pub fn store<T: Serialize>(path: &Path, value: &T) {
    let _ = write_json(
        path,
        &Stamped {
            at_ms: now_ms(),
            value,
        },
    );
}

/// Claim the refresh at `lock`; false when one is already running.
pub fn claim(lock: &Path) -> bool {
    try_lock(lock, LOCK_STALE)
}

/// Start the refresh `ccline <args>` claimed with `lock`, which must
/// release it when done. Releases it here if the process didn't start.
pub fn spawn(lock: &Path, args: &[&OsStr]) {
    if !spawn_detached(args) {
        release(lock);
    }
}

pub fn release(lock: &Path) {
    let _ = std::fs::remove_file(lock);
}
