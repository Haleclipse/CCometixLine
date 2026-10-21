//! Whether a newer ccline is published. Renders only read `update.json`;
//! when the last check is due they start a detached `ccline update-check`,
//! which asks the npm registry, then its China mirror, then GitHub releases
//! (through curl, so the binary carries no HTTP stack) and records the
//! answer.

use super::{dir, read_json, swr, write_json};
use crate::config::UpdateConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// The running ccline.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

pub const RELEASES: &str = "https://github.com/Haleclipse/CCometixLine/releases";

/// Where to ask, and the JSON key holding the version.
const SOURCES: &[(&str, &str)] = &[
    (
        "https://registry.npmjs.org/@cometix/ccline/latest",
        "version",
    ),
    (
        "https://registry.npmmirror.com/@cometix/ccline/latest",
        "version",
    ),
    (
        "https://api.github.com/repos/Haleclipse/CCometixLine/releases/latest",
        "tag_name",
    ),
];
/// Per-source time limit, in seconds.
const FETCH_TIMEOUT: &str = "8";
/// After a check where no source answered, try again this much later.
const RETRY_SECS: i64 = 3_600;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateState {
    pub checked_at: i64,
    /// Newest version seen published, kept across failed checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// Whether the last check got an answer.
    #[serde(default)]
    pub ok: bool,
}

fn paths() -> (PathBuf, PathBuf) {
    (dir().join("update.json"), dir().join("update.lock"))
}

/// `(major, minor, patch, is_release)`; a pre-release sorts before its
/// release, and build metadata is ignored.
fn semver(v: &str) -> Option<(u64, u64, u64, bool)> {
    let v = v.trim().trim_start_matches('v').split('+').next()?;
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (
        parts.next()??,
        parts.next()??,
        parts.next()??,
        pre.is_none(),
    );
    parts.next().is_none().then_some(version)
}

/// Whether `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((semver(latest), semver(current)), (Some(l), Some(c)) if l > c)
}

/// The published version if it is newer than this one. Starts a background
/// check when the last one is due; never waits for the network.
pub fn available(config: &UpdateConfig, now: i64) -> Option<String> {
    if !config.check {
        return None;
    }
    let (path, lock) = paths();
    let state: UpdateState = read_json(&path).unwrap_or_default();
    let every = if state.ok {
        i64::from(config.interval_hours.max(1)) * 3_600
    } else {
        RETRY_SECS
    };
    if now - state.checked_at >= every && swr::claim(&lock) {
        swr::spawn(&lock, &["update-check".as_ref()]);
    }
    state.latest.filter(|v| is_newer(v, CURRENT))
}

/// Ask each source in turn; `None` when none answered with a version.
pub fn fetch() -> Option<String> {
    SOURCES.iter().find_map(|(url, key)| {
        let out = Command::new("curl")
            .args(["-fsSL", "--max-time", FETCH_TIMEOUT, url])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let version = json.get(key)?.as_str()?.trim().trim_start_matches('v');
        semver(version).map(|_| version.to_string())
    })
}

/// Check now, record the result and release the background lock. Returns
/// what the sources said.
pub fn check(now: i64) -> Option<String> {
    let (path, lock) = paths();
    let latest = fetch();
    let previous: UpdateState = read_json(&path).unwrap_or_default();
    let state = UpdateState {
        checked_at: now,
        ok: latest.is_some(),
        latest: latest.clone().or(previous.latest),
    };
    let _ = write_json(&path, &state);
    swr::release(&lock);
    latest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("1.1.3", "1.1.2"));
        assert!(is_newer("v2.0.0", "1.9.9"));
        assert!(is_newer("2.0.0", "2.0.0-dev.0"));
        assert!(!is_newer("1.1.2", "2.0.0-dev.0"));
        assert!(!is_newer("2.0.0-rc.1", "2.0.0"));
        assert!(!is_newer("2.0.0+build.5", "2.0.0"));
        assert!(!is_newer("1.1", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }
}
