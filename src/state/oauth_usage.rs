//! Per-model weekly limits that Claude Code's status line input leaves out.
//! The usage endpoint Claude Code itself reads (`/api/oauth/usage`) lists
//! them in `limits[]`; renders only read the recorded answer, and when it
//! is due they start a detached `ccline usage-check`, which asks the
//! endpoint with Claude Code's own sign-in (through curl, so the binary
//! carries no HTTP stack) and records the answer. Nothing is fetched unless
//! a layout shows one of these limits.
//!
//! The answer belongs to an account, so it is recorded per Claude config
//! directory: two profiles sharing one ccline home don't read each other's.

use super::{dir, file_key, read_json, swr, write_json};
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
    /// Why the last check got no answer, to tell a broken setup from an
    /// account without the limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Failure>,
    /// The Fable weekly window from the last answer that had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fable_weekly: Option<StoredWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// No sign-in found, or its token has expired.
    NoToken,
    /// curl is missing, timed out or the endpoint refused the request.
    Request,
    /// The endpoint answered with something that isn't JSON.
    Response,
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
    let key = file_key(&crate::paths::claude_dir().to_string_lossy());
    let usage = dir().join("oauth_usage");
    (
        usage.join(format!("{key}.json")),
        usage.join(format!("{key}.lock")),
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

/// Check now, record the result and release the background lock. `now` is
/// when the check started.
pub fn check(now: i64) {
    let (path, lock) = paths();
    let answer = access_token()
        .ok_or(Failure::NoToken)
        .and_then(|token| fetch(&token));
    let previous: UsageState = read_json(&path).unwrap_or_default();
    if let Some(state) = next_state(previous, now, answer) {
        let _ = write_json(&path, &state);
    }
    swr::release(&lock);
}

/// What to record after a check started at `now`; nothing when a check
/// started later has already recorded its answer.
fn next_state(
    previous: UsageState,
    now: i64,
    answer: Result<Value, Failure>,
) -> Option<UsageState> {
    if previous.checked_at > now {
        return None;
    }
    Some(match answer {
        Ok(body) => UsageState {
            checked_at: now,
            ok: true,
            error: None,
            fable_weekly: fable_window(&body),
        },
        // Keep the last window: it stays right until it resets.
        Err(failure) => UsageState {
            checked_at: now,
            ok: false,
            error: Some(failure),
            ..previous
        },
    })
}

/// GET the endpoint. The token goes to curl on stdin (`-H @-`), never on
/// its command line, where other users' `ps` could read it.
fn fetch(token: &str) -> Result<Value, Failure> {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsS", "--max-time", "8", "-H", "@-", ENDPOINT]);
    let headers = format!("Authorization: Bearer {token}\nanthropic-beta: oauth-2025-04-20\n");
    match process::run(cmd, Some(headers.as_bytes()), FETCH_TIMEOUT) {
        Outcome::Exited {
            success: true,
            stdout,
        } => serde_json::from_str(&stdout).map_err(|_| Failure::Response),
        _ => Err(Failure::Request),
    }
}

/// Claude Code's OAuth access token: the macOS keychain item it writes,
/// else its `.credentials.json`, whichever first holds a usable one. An
/// expired token is left alone; Claude Code refreshes it on its next
/// request.
fn access_token() -> Option<String> {
    let file =
        || std::fs::read_to_string(crate::paths::claude_dir().join(".credentials.json")).ok();
    first_token(
        [keychain_credentials(), file()],
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The token of the first credentials that hold a usable one.
fn first_token(sources: impl IntoIterator<Item = Option<String>>, now_ms: i64) -> Option<String> {
    sources
        .into_iter()
        .flatten()
        .find_map(|raw| token_from_credentials(&raw, now_ms))
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

    fn creds(expires: i64, token: &str) -> String {
        format!(r#"{{"claudeAiOauth": {{"accessToken": "{token}", "expiresAt": {expires}}}}}"#)
    }

    #[test]
    fn falls_back_to_the_next_credentials_when_the_first_are_unusable() {
        let expired = Some(creds(1_000, "keychain"));
        let valid = Some(creds(2_000, "file"));
        assert_eq!(
            first_token([expired.clone(), valid.clone()], 1_000).as_deref(),
            Some("file")
        );
        assert_eq!(
            first_token([valid, expired.clone()], 1_000).as_deref(),
            Some("file")
        );
        assert_eq!(first_token([None, expired], 1_000), None);
    }

    #[test]
    fn a_failed_check_keeps_the_window_and_says_why() {
        let window = StoredWindow {
            used_percentage: 27.0,
            resets_at: 9_000,
        };
        let previous = UsageState {
            checked_at: 100,
            ok: true,
            error: None,
            fable_weekly: Some(window),
        };
        let failed = next_state(previous.clone(), 200, Err(Failure::Request)).unwrap();
        assert_eq!(
            failed,
            UsageState {
                checked_at: 200,
                ok: false,
                error: Some(Failure::Request),
                fable_weekly: Some(window),
            }
        );
        // An answer without the row clears both the error and the window.
        let answered = next_state(failed, 300, Ok(serde_json::json!({}))).unwrap();
        assert_eq!(
            answered,
            UsageState {
                checked_at: 300,
                ok: true,
                error: None,
                fable_weekly: None,
            }
        );
        assert_eq!(
            serde_json::to_value(Failure::NoToken).unwrap(),
            serde_json::json!("no_token")
        );
    }

    #[test]
    fn a_check_that_started_earlier_does_not_overwrite_a_later_one() {
        let later = UsageState {
            checked_at: 500,
            ok: true,
            ..UsageState::default()
        };
        assert_eq!(next_state(later.clone(), 400, Err(Failure::Request)), None);
        assert!(next_state(later, 500, Ok(serde_json::json!({}))).is_some());
    }

    #[test]
    fn skips_an_expired_or_empty_token() {
        assert_eq!(
            token_from_credentials(&creds(2_000, "tok"), 1_000).as_deref(),
            Some("tok")
        );
        assert_eq!(token_from_credentials(&creds(1_000, "tok"), 1_000), None);
        assert_eq!(token_from_credentials(&creds(2_000, " "), 1_000), None);
        assert_eq!(token_from_credentials("not json", 1_000), None);
    }
}
