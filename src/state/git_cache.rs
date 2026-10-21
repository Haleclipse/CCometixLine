//! Git status per working directory, served from a cache and refreshed in
//! the background. Claude Code refreshes the status line many times during
//! a turn and cancels a render when the next one is due, so a render must
//! not wait on git: one git status costs tens of milliseconds in a real
//! repository, and far more on a slow disk or a busy machine.

use super::{dir, file_key, read_json, spawn_detached, try_lock, write_json};
use crate::git::GitInfo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Older results are not shown; the render runs git itself instead.
const MAX_STALE_MS: u64 = 10 * 60 * 1000;
/// A background refresh may take this long.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);
/// A refresh lock older than this was left by a refresh that died.
const LOCK_STALE: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize)]
struct Entry {
    at_ms: u64,
    info: GitInfo,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn paths(cwd: &Path) -> (PathBuf, PathBuf) {
    let key = file_key(&cwd.to_string_lossy());
    let git = dir().join("git");
    (
        git.join(format!("{key}.json")),
        git.join(format!("{key}.lock")),
    )
}

fn save(path: &Path, info: &GitInfo) {
    let _ = write_json(
        path,
        &Entry {
            at_ms: now_ms(),
            info: info.clone(),
        },
    );
}

/// Git status for `cwd`. A result younger than `ttl_ms` is used as is; an
/// older one is still used while a background process refreshes it. Only
/// without a usable result does the render run git itself.
pub fn status(cwd: &Path, ttl_ms: u64) -> Option<GitInfo> {
    // Outside a repository there is nothing worth caching.
    if ttl_ms == 0 || !crate::git::in_repository(cwd) {
        return crate::git::status(cwd);
    }
    let (path, _) = paths(cwd);
    let age = |e: &Entry| now_ms().saturating_sub(e.at_ms);
    if let Some(entry) = read_json::<Entry>(&path).filter(|e| age(e) < MAX_STALE_MS) {
        if age(&entry) >= ttl_ms {
            refresh_in_background(cwd);
        }
        return Some(entry.info);
    }
    match crate::git::status(cwd) {
        Some(info) => {
            save(&path, &info);
            Some(info)
        }
        None => {
            // Git missed the render's deadline; let it finish unhurried.
            refresh_in_background(cwd);
            None
        }
    }
}

fn refresh_in_background(cwd: &Path) {
    let (_, lock) = paths(cwd);
    if try_lock(&lock, LOCK_STALE) && !spawn_detached(&["refresh-git".as_ref(), cwd.as_os_str()]) {
        let _ = std::fs::remove_file(&lock);
    }
}

/// `ccline refresh-git <dir>`: run git without a render's deadline, store
/// the result and release the lock.
pub fn refresh(cwd: &Path) {
    let (path, lock) = paths(cwd);
    if let Some(info) = crate::git::status_within(cwd, REFRESH_TIMEOUT) {
        save(&path, &info);
    }
    let _ = std::fs::remove_file(&lock);
    super::sometimes(100, || {
        super::sweep(&dir().join("git"), Duration::from_secs(86_400), 200);
    });
}
