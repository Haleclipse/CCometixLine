//! Output of user commands (`cmd` items), served stale-while-revalidate
//! (see `swr`). A command gets the status line payload on stdin, like a
//! status line script would, and contributes its first line of output.

use super::swr::{self, Lookup};
use super::{dir, file_key};
use crate::process::{self, Outcome};
use regex::Regex;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How long a render waits, in total, for commands with no stored output,
/// and at most for any one of them, so a slow command early on the line
/// doesn't hold back quick ones after it.
const RENDER_WAIT: Duration = Duration::from_millis(500);
const COMMAND_WAIT: Duration = Duration::from_millis(150);
/// Older output is not shown.
const MAX_STALE_MS: u64 = 60 * 60 * 1000;
/// Longest output kept, in characters.
const MAX_CHARS: usize = 200;

/// One command to run for an item.
pub struct Job<'a> {
    /// Shell command line.
    pub run: &'a str,
    pub cwd: Option<&'a Path>,
    /// The status line payload, passed on stdin.
    pub stdin: &'a str,
    /// Limit for a background run.
    pub timeout: Duration,
    /// How long output stays fresh.
    pub ttl_ms: u64,
}

struct Paths {
    output: PathBuf,
    lock: PathBuf,
    /// Payload handed to the background run.
    stdin: PathBuf,
}

fn paths(run: &str, cwd: Option<&Path>) -> Paths {
    let cwd = cwd.map(|c| c.to_string_lossy()).unwrap_or_default();
    let key = file_key(&format!("{run}\0{cwd}"));
    let dir = dir().join("cmd");
    Paths {
        output: dir.join(format!("{key}.json")),
        lock: dir.join(format!("{key}.lock")),
        stdin: dir.join(format!("{key}.stdin")),
    }
}

/// The first non-empty line of `stdout`, without terminal escapes.
pub fn first_line(stdout: &str) -> Option<String> {
    static ESCAPES: OnceLock<Regex> = OnceLock::new();
    let re = ESCAPES.get_or_init(|| {
        // CSI sequences (colors, cursor) and OSC sequences (links, titles).
        Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)")
            .expect("escape regex compiles")
    });
    let clean = re.replace_all(stdout, "");
    let line = clean.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.chars().take(MAX_CHARS).collect())
}

fn shell(run: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", run]);
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", run]);
        cmd
    }
}

/// Run the command: `Some(output)` once it finished (a failing command has
/// no output), `None` if it timed out.
fn execute(
    run: &str,
    cwd: Option<&Path>,
    stdin: &str,
    timeout: Duration,
) -> Option<Option<String>> {
    let mut cmd = shell(run);
    if let Some(dir) = cwd.filter(|d| d.is_dir()) {
        cmd.current_dir(dir);
    }
    match process::run(cmd, Some(stdin.as_bytes()), timeout) {
        Outcome::Exited {
            success: true,
            stdout,
        } => Some(first_line(&stdout)),
        Outcome::Exited { success: false, .. } | Outcome::Failed => Some(None),
        Outcome::TimedOut => None,
    }
}

/// What is left of this render's wait for commands. A process renders once,
/// so the budget starts with the first command that needs it.
fn wait_left() -> Duration {
    static DEADLINE: OnceLock<Instant> = OnceLock::new();
    DEADLINE
        .get_or_init(|| Instant::now() + RENDER_WAIT)
        .saturating_duration_since(Instant::now())
}

fn refresh_in_background(job: &Job, p: &Paths) {
    if !swr::claim(&p.lock) {
        return;
    }
    if crate::paths::write_atomic(&p.stdin, job.stdin).is_err() {
        swr::release(&p.lock);
        return;
    }
    let timeout = job.timeout.as_millis().to_string();
    let mut args = vec![
        "refresh-cmd".as_ref(),
        "--timeout-ms".as_ref(),
        timeout.as_ref(),
    ];
    if let Some(cwd) = job.cwd {
        args.extend(["--cwd".as_ref(), cwd.as_os_str()]);
    }
    args.extend(["--".as_ref(), OsStr::new(job.run)]);
    swr::spawn(&p.lock, &args);
}

/// The command's output for this render, from the store when there is a
/// usable one; a stale one triggers a background run.
pub fn output(job: &Job) -> Option<String> {
    let p = paths(job.run, job.cwd);
    match swr::lookup::<Option<String>>(&p.output, job.ttl_ms, MAX_STALE_MS) {
        Lookup::Fresh(out) => out,
        Lookup::Stale(out) => {
            refresh_in_background(job, &p);
            out
        }
        Lookup::Missing => {
            let wait = wait_left().min(COMMAND_WAIT).min(job.timeout);
            match (!wait.is_zero())
                .then(|| execute(job.run, job.cwd, job.stdin, wait))
                .flatten()
            {
                Some(out) => {
                    swr::store(&p.output, &out);
                    out
                }
                None => {
                    // Out of time for this render; finish in the background.
                    refresh_in_background(job, &p);
                    None
                }
            }
        }
    }
}

/// `ccline refresh-cmd`: run the command without a render's deadline, store
/// its output and release the lock.
pub fn refresh(run: &str, cwd: Option<&Path>, timeout: Duration) {
    let p = paths(run, cwd);
    let stdin = std::fs::read_to_string(&p.stdin).unwrap_or_default();
    // A command that is still too slow shows nothing rather than retrying
    // on every render.
    let out = execute(run, cwd, &stdin, timeout).flatten();
    swr::store(&p.output, &out);
    let _ = std::fs::remove_file(&p.stdin);
    swr::release(&p.lock);
    super::sometimes(100, || {
        super::sweep(&dir().join("cmd"), Duration::from_secs(86_400), 200);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_first_line_without_escapes() {
        assert_eq!(
            first_line("\n  \x1b[31mprod\x1b[0m  \nsecond\n").as_deref(),
            Some("prod")
        );
        assert_eq!(
            first_line("\x1b]8;;https://x.y\x1b\\link\x1b]8;;\x1b\\").as_deref(),
            Some("link")
        );
        assert_eq!(first_line(" \n\n"), None);
        assert_eq!(first_line(&"x".repeat(500)).unwrap().len(), MAX_CHARS);
    }

    #[cfg(unix)]
    #[test]
    fn runs_with_the_payload_on_stdin() {
        let out = execute("head -c 7", None, "payload-json", Duration::from_secs(5));
        assert_eq!(out, Some(Some("payload".into())));
        assert_eq!(
            execute("exit 1", None, "", Duration::from_secs(5)),
            Some(None)
        );
        assert_eq!(
            execute("exec sleep 5", None, "", Duration::from_millis(50)),
            None
        );
    }
}
