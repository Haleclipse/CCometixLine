use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct GitInfo {
    pub branch: String,
    pub status: GitStatus,
    pub ahead: u32,
    pub behind: u32,
    pub sha: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum GitStatus {
    Clean,
    Dirty,
    Conflicts,
}

pub struct GitSegment {
    show_sha: bool,
    sub_dirs: Vec<String>,
}

impl Default for GitSegment {
    fn default() -> Self {
        Self::new()
    }
}

impl GitSegment {
    pub fn new() -> Self {
        Self {
            show_sha: false,
            sub_dirs: Vec::new(),
        }
    }

    pub fn with_sha(mut self, show_sha: bool) -> Self {
        self.show_sha = show_sha;
        self
    }

    pub fn with_sub_dirs(mut self, sub_dirs: Vec<String>) -> Self {
        self.sub_dirs = sub_dirs;
        self
    }

    fn get_git_info(&self, working_dir: &str) -> Option<GitInfo> {
        if !self.is_git_repository(working_dir) {
            return None;
        }

        let branch = self
            .get_branch(working_dir)
            .unwrap_or_else(|| "detached".to_string());
        let status = self.get_status(working_dir);
        let (ahead, behind) = self.get_ahead_behind(working_dir);
        let sha = if self.show_sha {
            self.get_sha(working_dir)
        } else {
            None
        };

        Some(GitInfo {
            branch,
            status,
            ahead,
            behind,
            sha,
        })
    }

    fn is_git_repository(&self, working_dir: &str) -> bool {
        Command::new("git")
            .args(["--no-optional-locks", "rev-parse", "--git-dir"])
            .current_dir(working_dir)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn get_branch(&self, working_dir: &str) -> Option<String> {
        if let Ok(output) = Command::new("git")
            .args(["--no-optional-locks", "branch", "--show-current"])
            .current_dir(working_dir)
            .output()
        {
            if output.status.success() {
                let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
                if !branch.is_empty() {
                    return Some(branch);
                }
            }
        }

        if let Ok(output) = Command::new("git")
            .args(["--no-optional-locks", "symbolic-ref", "--short", "HEAD"])
            .current_dir(working_dir)
            .output()
        {
            if output.status.success() {
                let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
                if !branch.is_empty() {
                    return Some(branch);
                }
            }
        }

        None
    }

    fn get_status(&self, working_dir: &str) -> GitStatus {
        let output = Command::new("git")
            .args(["--no-optional-locks", "status", "--porcelain"])
            .current_dir(working_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let status_text = String::from_utf8(output.stdout).unwrap_or_default();

                if status_text.trim().is_empty() {
                    return GitStatus::Clean;
                }

                if status_text.contains("UU")
                    || status_text.contains("AA")
                    || status_text.contains("DD")
                {
                    GitStatus::Conflicts
                } else {
                    GitStatus::Dirty
                }
            }
            _ => GitStatus::Clean,
        }
    }

    fn get_ahead_behind(&self, working_dir: &str) -> (u32, u32) {
        let ahead = self.get_commit_count(working_dir, "@{u}..HEAD");
        let behind = self.get_commit_count(working_dir, "HEAD..@{u}");
        (ahead, behind)
    }

    fn get_commit_count(&self, working_dir: &str, range: &str) -> u32 {
        let output = Command::new("git")
            .args(["--no-optional-locks", "rev-list", "--count", range])
            .current_dir(working_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => String::from_utf8(output.stdout)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn get_sha(&self, working_dir: &str) -> Option<String> {
        let output = Command::new("git")
            .args(["--no-optional-locks", "rev-parse", "--short=7", "HEAD"])
            .current_dir(working_dir)
            .output()
            .ok()?;

        if output.status.success() {
            let sha = String::from_utf8(output.stdout).ok()?.trim().to_string();
            if sha.is_empty() {
                None
            } else {
                Some(sha)
            }
        } else {
            None
        }
    }

    /// 多子目录模式：获取多个子目录的 git 分支信息
    fn collect_multi_dirs(&self, working_dir: &str) -> Option<SegmentData> {
        // 文件夹索引 emoji: 🗂️
        let folder_icon = "🗂️";
        let mut branch_parts = Vec::new();
        let mut metadata = HashMap::new();

        for sub_dir in &self.sub_dirs {
            let full_path = Path::new(working_dir).join(sub_dir);
            let full_path_str = full_path.to_string_lossy();

            if let Some(branch) = self.get_branch(&full_path_str) {
                // 获取 git status
                let status = self.get_status(&full_path_str);
                let status_icon = match status {
                    GitStatus::Clean => "✓",
                    GitStatus::Dirty => "●",
                    GitStatus::Conflicts => "⚠",
                };

                branch_parts.push(format!("{} {}:{} {}", folder_icon, sub_dir, branch, status_icon));
                metadata.insert(format!("{}_branch", sub_dir), branch);
                metadata.insert(format!("{}_status", sub_dir), format!("{:?}", status));
            }
        }

        if branch_parts.is_empty() {
            return None;
        }

        Some(SegmentData {
            primary: branch_parts.join(" | "),
            secondary: String::new(),
            metadata,
        })
    }
}

impl Segment for GitSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let working_dir = &input.workspace.current_dir;

        // 多子目录模式
        if !self.sub_dirs.is_empty() {
            return self.collect_multi_dirs(working_dir);
        }

        // 原有逻辑：单目录模式
        let git_info = self.get_git_info(working_dir)?;

        let mut metadata = HashMap::new();
        metadata.insert("branch".to_string(), git_info.branch.clone());
        metadata.insert("status".to_string(), format!("{:?}", git_info.status));
        metadata.insert("ahead".to_string(), git_info.ahead.to_string());
        metadata.insert("behind".to_string(), git_info.behind.to_string());

        if let Some(ref sha) = git_info.sha {
            metadata.insert("sha".to_string(), sha.clone());
        }

        let primary = git_info.branch;
        let mut status_parts = Vec::new();

        match git_info.status {
            GitStatus::Clean => status_parts.push("✓".to_string()),
            GitStatus::Dirty => status_parts.push("●".to_string()),
            GitStatus::Conflicts => status_parts.push("⚠".to_string()),
        }

        if git_info.ahead > 0 {
            status_parts.push(format!("↑{}", git_info.ahead));
        }
        if git_info.behind > 0 {
            status_parts.push(format!("↓{}", git_info.behind));
        }

        if let Some(ref sha) = git_info.sha {
            status_parts.push(sha.clone());
        }

        Some(SegmentData {
            primary,
            secondary: status_parts.join(" "),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Git
    }
}
