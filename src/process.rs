//! Running a child process with a deadline.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Exited {
        success: bool,
        stdout: String,
    },
    /// Still running at the deadline; it was killed.
    TimedOut,
    /// Could not be started.
    Failed,
}

/// Run `cmd`, feeding it `stdin`, and collect stdout until it exits or
/// `timeout` passes. Stderr is discarded.
pub fn run(mut cmd: Command, stdin: Option<&[u8]>, timeout: Duration) -> Outcome {
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else {
        return Outcome::Failed;
    };

    // Write and read on other threads so a full pipe in either direction
    // can't stall the child while we wait for it to exit.
    if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let data = data.to_vec();
        std::thread::spawn(move || {
            // A child that exits without reading closes the pipe; fine.
            let _ = pipe.write_all(&data);
        });
    }
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        return Outcome::Failed;
    };
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        String::from_utf8_lossy(&out).into_owned()
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                // A grandchild may still hold stdout open; don't wait for it.
                return Outcome::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => return Outcome::Failed,
        }
    };
    Outcome::Exited {
        success: status.success(),
        stdout: reader.join().unwrap_or_default(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    #[test]
    fn collects_output_and_status() {
        let ok = run(sh("echo hi"), None, Duration::from_secs(5));
        assert_eq!(
            ok,
            Outcome::Exited {
                success: true,
                stdout: "hi\n".into()
            }
        );
        let bad = run(sh("echo no; exit 3"), None, Duration::from_secs(5));
        assert!(matches!(bad, Outcome::Exited { success: false, .. }));
    }

    #[test]
    fn feeds_stdin() {
        let out = run(sh("cat"), Some(b"payload"), Duration::from_secs(5));
        assert_eq!(
            out,
            Outcome::Exited {
                success: true,
                stdout: "payload".into()
            }
        );
    }

    #[test]
    fn kills_at_the_deadline() {
        let start = Instant::now();
        let out = run(sh("exec sleep 5"), None, Duration::from_millis(100));
        assert_eq!(out, Outcome::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
