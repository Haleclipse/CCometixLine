//! Per-model weekly limits that Claude Code's status line input leaves out.
//! The usage endpoint Claude Code itself reads (`/api/oauth/usage`) lists
//! them in `limits[]`; renders only read `oauth_usage.json`, and when it is
//! due they start a detached `ccline usage-check`, which asks the endpoint
//! with Claude Code's own sign-in (through curl, so the binary carries no
//! HTTP stack) and records the answer. Nothing is fetched unless a layout
//! shows one of these limits.

use super::{dir, read_json, swr, write_json};
use crate::input::Window;
use crate::process::{self, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
/// Ask again this long after an answer.
const REFRESH_SECS: i64 = 300;
/// After a check that got no answer, try again this much later.
const RETRY_SECS: i64 = 900;
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageState {
    pub checked_at: i64,
    /// Whether the last check got an answer.
    #[serde(default)]
    pub ok: bool,
    /// The Fable weekly window from the last answer that had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fable_weekly: Option<StoredWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredWindow {
    pub used_percentage: f64,
    /// Unix epoch seconds.
    pub resets_at: i64,
}

impl From<StoredWindow> for Window {
    fn from(w: StoredWindow) -> Self {
        Window {
            used_percentage: Some(w.used_percentage),
            resets_at: Some(w.resets_at as f64),
        }
    }
}

fn paths() -> (PathBuf, PathBuf) {
    (
        dir().join("oauth_usage.json"),
        dir().join("oauth_usage.lock"),
    )
}

/// The recorded Fable weekly window, unless it has already reset. Starts a
/// background check when the last one is due; never waits for the network.
pub fn fable_weekly(now: i64) -> Option<Window> {
    let (path, lock) = paths();
    let state: UsageState = read_json(&path).unwrap_or_default();
    let every = if state.ok { REFRESH_SECS } else { RETRY_SECS };
    if now - state.checked_at >= every && swr::claim(&lock) {
        swr::spawn(&lock, &["usage-check".as_ref()]);
    }
    state
        .fable_weekly
        .filter(|w| w.resets_at > now)
        .map(Window::from)
}

/// Check now, record the result and release the background lock.
pub fn check(now: i64) {
    let (path, lock) = paths();
    let answer = access_token().and_then(|token| fetch(&token));
    let previous: UsageState = read_json(&path).unwrap_or_default();
    let state = match answer {
        Some(body) => UsageState {
            checked_at: now,
            ok: true,
            fable_weekly: fable_window(&body),
        },
        None => UsageState {
            checked_at: now,
            ok: false,
            ..previous
        },
    };
    let _ = write_json(&path, &state);
    swr::release(&lock);
}

/// GET the endpoint. The token goes to curl on stdin (`-H @-`), never on
/// its command line, where other users' `ps` could read it.
fn fetch(token: &str) -> Option<Value> {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsS", "--max-time", "8", "-H", "@-", ENDPOINT]);
    let headers = format!("Authorization: Bearer {token}\nanthropic-beta: oauth-2025-04-20\n");
    match process::run(cmd, Some(headers.as_bytes()), FETCH_TIMEOUT) {
        Outcome::Exited {
            success: true,
            stdout,
        } => serde_json::from_str(&stdout).ok(),
        _ => None,
    }
}

/// Claude Code's OAuth access token: the macOS keychain item it writes,
/// else its `.credentials.json`. An expired token is left alone; Claude
/// Code refreshes it on its next request.
fn access_token() -> Option<String> {
    let raw = keychain_credentials().or_else(|| {
        std::fs::read_to_string(crate::paths::claude_dir().join(".credentials.json")).ok()
    })?;
    token_from_credentials(&raw, chrono::Utc::now().timestamp_millis())
}

#[cfg(target_os = "macos")]
fn keychain_credentials() -> Option<String> {
    // A custom CLAUDE_CONFIG_DIR gets its own keychain item; fall back to
    // the file rather than guess its name.
    if std::env::var_os("CLAUDE_CONFIG_DIR").is_some_and(|d| !d.is_empty()) {
        return None;
    }
    let mut cmd = Command::new("security");
    cmd.args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"]);
    match process::run(cmd, None, Duration::from_secs(5)) {
        Outcome::Exited {
            success: true,
            stdout,
        } => Some(stdout),
        _ => None,
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_credentials() -> Option<String> {
    let _ = KEYCHAIN_SERVICE;
    None
}

fn token_from_credentials(raw: &str, now_ms: i64) -> Option<String> {
    let json: Value = serde_json::from_str(raw.trim()).ok()?;
    let oauth = json.get("claudeAiOauth")?;
    if oauth
        .get("expiresAt")
        .and_then(Value::as_i64)
        .is_some_and(|at| at <= now_ms)
    {
        return None;
    }
    let token = oauth.get("accessToken")?.as_str()?.trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// The Fable row of `limits[]`: a `weekly_scoped` meter whose scope is the
/// model whose label starts with the word "Fable" ("Fable", "Fable 5.2"),
/// the same test Claude Code applies.
fn fable_window(body: &Value) -> Option<StoredWindow> {
    let row = body.get("limits")?.as_array()?.iter().find(|row| {
        row.get("kind").and_then(Value::as_str) == Some("weekly_scoped")
            && row
                .pointer("/scope/model/display_name")
                .and_then(Value::as_str)
                .is_some_and(is_fable)
    })?;
    Some(StoredWindow {
        used_percentage: row.get("percent")?.as_f64()?,
        resets_at: to_minute(epoch_seconds(row.get("resets_at")?)?),
    })
}

fn is_fable(label: &str) -> bool {
    let label = label.trim();
    label
        .get(..5)
        .is_some_and(|head| head.eq_ignore_ascii_case("fable"))
        && !label[5..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
}

/// The endpoint can end a window a second before the minute (22:59:59);
/// round to the nearest minute so it reads 23:00 like the other limits.
fn to_minute(t: i64) -> i64 {
    (t + 30).div_euclid(60) * 60
}

/// An ISO 8601 timestamp or epoch seconds.
fn epoch_seconds(v: &Value) -> Option<i64> {
    match v {
        Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|t| t.timestamp()),
        Value::Number(n) => n.as_f64().map(|t| t as i64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(rows: &str) -> Value {
        serde_json::from_str(&format!(r#"{{"limits": [{rows}]}}"#)).unwrap()
    }

    #[test]
    fn reads_the_fable_weekly_row() {
        let b = body(
            r#"{"kind": "session", "percent": 4, "resets_at": "2026-11-16T01:00:00Z"},
               {"kind": "weekly_all", "percent": 72, "resets_at": "2026-11-16T23:00:00Z"},
               {"kind": "weekly_scoped", "percent": 35.5, "resets_at": "2026-11-20T10:00:00+00:00",
                "scope": {"model": {"display_name": "Fable"}}}"#,
        );
        assert_eq!(
            fable_window(&b),
            Some(StoredWindow {
                used_percentage: 35.5,
                resets_at: 1_795_168_800,
            })
        );
    }

    #[test]
    fn ignores_other_scopes_and_missing_rows() {
        let other = body(
            r#"{"kind": "weekly_scoped", "percent": 10, "resets_at": 1795168800,
                "scope": {"model": {"display_name": "Opus"}}}"#,
        );
        assert_eq!(fable_window(&other), None);
        assert_eq!(fable_window(&serde_json::json!({})), None);
        let no_reset = body(
            r#"{"kind": "weekly_scoped", "percent": 10,
                "scope": {"model": {"display_name": "fable"}}}"#,
        );
        assert_eq!(fable_window(&no_reset), None);
    }

    #[test]
    fn rounds_the_reset_to_the_minute() {
        let b = body(
            r#"{"kind": "weekly_scoped", "percent": 27, "resets_at": "2026-11-20T09:59:59Z",
                "scope": {"model": {"display_name": "Fable"}}}"#,
        );
        assert_eq!(fable_window(&b).map(|w| w.resets_at), Some(1_795_168_800));
    }

    #[test]
    fn matches_fable_as_a_leading_word() {
        assert!(is_fable("Fable") && is_fable(" fable ") && is_fable("Fable 5.2"));
        assert!(!is_fable("Fabled") && !is_fable("Opus") && !is_fable("Fab"));
        assert!(!is_fable("Fable_x") && !is_fable("éFable"));
    }

    #[test]
    fn accepts_epoch_seconds_for_the_reset() {
        let b = body(
            r#"{"kind": "weekly_scoped", "percent": 1, "resets_at": 1795168800,
                "scope": {"model": {"display_name": " FABLE "}}}"#,
        );
        assert_eq!(fable_window(&b).map(|w| w.resets_at), Some(1_795_168_800));
    }

    #[test]
    fn skips_an_expired_or_empty_token() {
        let creds = |expires: i64, token: &str| {
            format!(r#"{{"claudeAiOauth": {{"accessToken": "{token}", "expiresAt": {expires}}}}}"#)
        };
        assert_eq!(
            token_from_credentials(&creds(2_000, "tok"), 1_000).as_deref(),
            Some("tok")
        );
        assert_eq!(token_from_credentials(&creds(1_000, "tok"), 1_000), None);
        assert_eq!(token_from_credentials(&creds(2_000, " "), 1_000), None);
        assert_eq!(token_from_credentials("not json", 1_000), None);
    }
}
