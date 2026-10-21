//! A short-lived cache of git status per working directory. Claude Code
//! refreshes the status line many times during a turn, and one git status
//! costs tens of milliseconds in a real repository.

use super::{dir, file_key, read_json, write_json};
use crate::git::GitInfo;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
struct Entry {
    at_ms: u64,
    info: Option<GitInfo>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Git status for `cwd`, reusing a result younger than `ttl_ms`.
pub fn status(cwd: &Path, ttl_ms: u64) -> Option<GitInfo> {
    if ttl_ms == 0 {
        return crate::git::status(cwd);
    }
    let path = dir()
        .join("git")
        .join(format!("{}.json", file_key(&cwd.to_string_lossy())));
    let now = now_ms();
    if let Some(entry) = read_json::<Entry>(&path)
        && now.saturating_sub(entry.at_ms) < ttl_ms
    {
        return entry.info;
    }
    let info = crate::git::status(cwd);
    let _ = write_json(
        &path,
        &Entry {
            at_ms: now,
            info: info.clone(),
        },
    );
    super::sometimes(100, || {
        super::sweep(
            &dir().join("git"),
            std::time::Duration::from_secs(86_400),
            200,
        );
    });
    info
}
