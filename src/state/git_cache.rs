//! Git status per working directory, served stale-while-revalidate (see
//! `swr`): one git status costs tens of milliseconds in a real repository,
//! and far more on a slow disk or a busy machine.

use super::swr::{self, Lookup};
use super::{dir, file_key};
use crate::git::GitInfo;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Older results are not shown; the render runs git itself instead.
const MAX_STALE_MS: u64 = 10 * 60 * 1000;
/// A background refresh may take this long.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

fn paths(cwd: &Path) -> (PathBuf, PathBuf) {
    let key = file_key(&cwd.to_string_lossy());
    let git = dir().join("git");
    (
        git.join(format!("{key}.json")),
        git.join(format!("{key}.lock")),
    )
}

fn refresh_in_background(cwd: &Path, lock: &Path) {
    if swr::claim(lock) {
        swr::spawn(lock, &["refresh-git".as_ref(), cwd.as_os_str()]);
    }
}

/// Git status for `cwd`. A result younger than `ttl_ms` is used as is; an
/// older one is still used while a background process refreshes it. Only
/// without a usable result does the render run git itself.
pub fn status(cwd: &Path, ttl_ms: u64) -> Option<GitInfo> {
    // Outside a repository there is nothing worth caching.
    if ttl_ms == 0 || !crate::git::in_repository(cwd) {
        return crate::git::status(cwd);
    }
    let (path, lock) = paths(cwd);
    match swr::lookup(&path, ttl_ms, MAX_STALE_MS) {
        Lookup::Fresh(info) => Some(info),
        Lookup::Stale(info) => {
            refresh_in_background(cwd, &lock);
            Some(info)
        }
        Lookup::Missing => match crate::git::status(cwd) {
            Some(info) => {
                swr::store(&path, &info);
                Some(info)
            }
            None => {
                // Git missed the render's deadline; let it finish unhurried.
                refresh_in_background(cwd, &lock);
                None
            }
        },
    }
}

/// `ccline refresh-git <dir>`: run git without a render's deadline, store
/// the result and release the lock.
pub fn refresh(cwd: &Path) {
    let (path, lock) = paths(cwd);
    if let Some(info) = crate::git::status_within(cwd, REFRESH_TIMEOUT) {
        swr::store(&path, &info);
    }
    swr::release(&lock);
    super::sometimes(100, || {
        super::sweep(&dir().join("git"), Duration::from_secs(86_400), 200);
    });
}
