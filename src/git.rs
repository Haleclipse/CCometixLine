//! Repository status from a single `git status --porcelain=v2 --branch`.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitInfo {
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub conflicts: u32,
}

impl GitInfo {
    pub fn dirty(&self) -> bool {
        self.staged + self.modified + self.untracked + self.conflicts > 0
    }
}

/// Whether `dir` or a parent has a `.git` entry (a directory, or a file in
/// linked worktrees and submodules). `GIT_DIR` counts as yes.
pub fn in_repository(dir: &Path) -> bool {
    std::env::var_os("GIT_DIR").is_some() || dir.ancestors().any(|d| d.join(".git").exists())
}

/// Status of the repository containing `dir`, or `None` outside a repository,
/// when git is missing, or when it doesn't answer within the timeout.
pub fn status(dir: &Path) -> Option<GitInfo> {
    // Spawning git costs several milliseconds even when it then finds no
    // repository, so check for one first.
    if !in_repository(dir) {
        return None;
    }
    let mut child = Command::new("git")
        .args([
            "--no-optional-locks",
            "-c",
            "core.quotePath=false",
            "status",
            "--porcelain=v2",
            "--branch",
        ])
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Read on another thread so a large status can't fill the pipe and stall
    // git while we wait for it to exit.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        stdout.read_to_string(&mut out).ok().map(|_| out)
    });

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    reader.join().ok().flatten().map(|out| parse(&out))
}

pub fn parse(porcelain: &str) -> GitInfo {
    let mut info = GitInfo::default();
    for line in porcelain.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.oid" if value != "(initial)" => info.oid = Some(value.to_string()),
                "branch.head" if value != "(detached)" => info.branch = Some(value.to_string()),
                "branch.upstream" => info.upstream = Some(value.to_string()),
                "branch.ab" => {
                    for part in value.split_whitespace() {
                        if let Some(n) = part.strip_prefix('+') {
                            info.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            info.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let mut fields = line.splitn(3, ' ');
        match fields.next() {
            Some("1") | Some("2") => {
                let xy = fields.next().unwrap_or("..").as_bytes();
                if xy.first().is_some_and(|c| *c != b'.') {
                    info.staged += 1;
                }
                if xy.get(1).is_some_and(|c| *c != b'.') {
                    info.modified += 1;
                }
            }
            Some("u") => info.conflicts += 1,
            Some("?") => info.untracked += 1,
            _ => {}
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_headers_and_entries() {
        let out = "\
# branch.oid 9224f01a1b2c3d4e5f
# branch.head v2
# branch.upstream origin/v2
# branch.ab +2 -1
1 .M N... 100644 100644 100644 a b src/main.rs
1 M. N... 100644 100644 100644 a b src/lib.rs
1 MM N... 100644 100644 100644 a b Cargo.toml
2 R. N... 100644 100644 100644 a b R100 new.rs\told.rs
u UU N... 100644 100644 100644 100644 a b c conflict.rs
? notes.txt
? tmp/
";
        let info = parse(out);
        assert_eq!(info.branch.as_deref(), Some("v2"));
        assert_eq!(info.oid.as_deref(), Some("9224f01a1b2c3d4e5f"));
        assert_eq!(info.upstream.as_deref(), Some("origin/v2"));
        assert_eq!((info.ahead, info.behind), (2, 1));
        assert_eq!(
            (info.staged, info.modified, info.untracked, info.conflicts),
            (3, 2, 2, 1)
        );
        assert!(info.dirty());
    }

    #[test]
    fn handles_detached_and_initial() {
        let info = parse("# branch.oid (initial)\n# branch.head (detached)\n");
        assert_eq!(info.branch, None);
        assert_eq!(info.oid, None);
        assert!(!info.dirty());
    }

    #[test]
    fn detects_repositories_without_spawning_git() {
        assert!(in_repository(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path()
        ));
        let outside = std::env::temp_dir().join(format!("ccline-norepo-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        if std::env::var_os("GIT_DIR").is_none() && !in_repository(&std::env::temp_dir()) {
            assert!(!in_repository(&outside));
            assert_eq!(status(&outside), None);
        }
        std::fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn reads_this_repository() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        if let Some(info) = status(here) {
            assert!(info.branch.is_some() || info.oid.is_some());
        }
    }
}
