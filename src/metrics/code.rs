//! Working directory, repository, branch, pull request and line changes.

use super::{Ctx, Metric, MetricMeta, OptKind, OptSpec, Options, Reading};
use crate::style::{Category, GlyphTier, IconKey, Role, Severity};
use crate::widgets::WidgetKind;

fn meta(
    id: &'static str,
    category: Category,
    name: &'static str,
    summary: &'static str,
    icon: IconKey,
    templates: [&'static str; 3],
    fields: &'static [&'static str],
) -> MetricMeta {
    MetricMeta {
        id,
        category,
        name,
        summary,
        icon,
        widget: WidgetKind::Text,
        templates,
        fields,
        levels: &[],
        min_cc: None,
        needs_mod: false,
        options: &[],
        stateful: false,
        priority: 3,
    }
}

pub(super) fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            meta: MetricMeta {
                priority: 4,
                options: &[OptSpec {
                    key: "style",
                    kind: OptKind::Choice(&["name", "parent", "tilde", "short", "full"]),
                    default: "name",
                    summary: "How much of the path to show",
                }],
                ..meta(
                    "dir",
                    Category::Workspace,
                    "Directory",
                    "Working directory",
                    IconKey::Dir,
                    ["{path}", "{path}", "{name}"],
                    &["path", "name"],
                )
            },
            compute: dir,
        },
        Metric {
            meta: MetricMeta {
                priority: 4,
                options: &[OptSpec {
                    key: "sha",
                    kind: OptKind::Flag,
                    default: "false",
                    summary: "Add the short commit id",
                }],
                ..meta(
                    "git",
                    Category::Vcs,
                    "Git",
                    "Branch, status and ahead/behind",
                    IconKey::Git,
                    [
                        "{branch}[ {status}][ {ahead}][ {behind}][ {sha}]",
                        "{branch}[{dirty}]",
                        "{branch}",
                    ],
                    &[
                        "branch",
                        "status",
                        "dirty",
                        "ahead",
                        "behind",
                        "sha",
                        "staged",
                        "modified",
                        "untracked",
                        "conflicts",
                    ],
                )
            },
            compute: git,
        },
        Metric {
            meta: MetricMeta {
                priority: 2,
                ..meta(
                    "repo",
                    Category::Vcs,
                    "Repository",
                    "owner/name from the origin remote",
                    IconKey::Repo,
                    ["{full}", "{name}", "{name}"],
                    &["full", "owner", "name", "host"],
                )
            },
            compute: repo,
        },
        Metric {
            meta: MetricMeta {
                priority: 3,
                ..meta(
                    "pr",
                    Category::Vcs,
                    "Pull request",
                    "Open PR or MR for the branch, with review state",
                    IconKey::Pr,
                    ["{num}[ {state}]", "{num}", "{num}"],
                    &["num", "state"],
                )
            },
            compute: pr,
        },
        Metric {
            meta: meta(
                "worktree",
                Category::Vcs,
                "Worktree",
                "Linked git worktree or --worktree session",
                IconKey::Worktree,
                ["{name}", "{name}", "{name}"],
                &["name", "branch"],
            ),
            compute: worktree,
        },
        Metric {
            meta: MetricMeta {
                priority: 2,
                ..meta(
                    "lines",
                    Category::Vcs,
                    "Lines changed",
                    "Lines added and removed this session",
                    IconKey::Lines,
                    [
                        "[{added}][ {removed}]",
                        "[{added}][ {removed}]",
                        "[{added}][ {removed}]",
                    ],
                    &["added", "removed"],
                )
            },
            compute: lines,
        },
    ]
}

fn dir(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let path = ctx.input.current_dir()?;
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let name = parts.last().copied().unwrap_or("/");
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let tilde = || match &home {
        Some(h) if !h.is_empty() && path.starts_with(h.as_str()) => {
            format!("~{}", &path[h.len()..])
        }
        _ => path.to_string(),
    };
    let shown = match opts.get("style").and_then(|v| v.as_str()).unwrap_or("name") {
        "full" => path.to_string(),
        "tilde" => tilde(),
        "parent" => parts[parts.len().saturating_sub(2)..].join("/"),
        "short" => {
            let t = tilde();
            let segs: Vec<&str> = t.split('/').collect();
            segs.iter()
                .enumerate()
                .map(|(i, s)| {
                    if i + 1 == segs.len() || s.is_empty() || *s == "~" {
                        s.to_string()
                    } else {
                        s.chars()
                            .take(if s.starts_with('.') { 2 } else { 1 })
                            .collect()
                    }
                })
                .collect::<Vec<_>>()
                .join("/")
        }
        _ => name.to_string(),
    };
    Some(Reading::default().set("path", shown).set("name", name))
}

fn git(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let info = ctx.git()?;
    let ascii = ctx.glyphs == GlyphTier::Ascii;
    let branch = match (&info.branch, &info.oid) {
        (Some(b), _) => b.clone(),
        (None, Some(oid)) => format!("@{}", &oid[..oid.len().min(7)]),
        (None, None) => return None,
    };
    let (clean, dirty, conflict) = if ascii {
        ("ok", "*", "!")
    } else {
        ("✓", "●", "⚠")
    };
    let status = if info.conflicts > 0 {
        conflict
    } else if info.dirty() {
        dirty
    } else {
        clean
    };
    let (up, down) = if ascii { ("+", "-") } else { ("↑", "↓") };
    let mut r = Reading::default()
        .set("branch", branch)
        .set("status", status)
        .set("dirty", if info.dirty() { "*" } else { "" });
    if info.ahead > 0 {
        r = r.set("ahead", format!("{up}{}", info.ahead));
    }
    if info.behind > 0 {
        r = r.set("behind", format!("{down}{}", info.behind));
    }
    if opts.get("sha").and_then(|v| v.as_bool()).unwrap_or(false)
        && let Some(oid) = &info.oid
    {
        r = r.set_role("sha", &oid[..oid.len().min(7)], Role::Muted);
    }
    for (name, n) in [
        ("staged", info.staged),
        ("modified", info.modified),
        ("untracked", info.untracked),
        ("conflicts", info.conflicts),
    ] {
        if n > 0 {
            r = r.set(name, n.to_string());
        }
    }
    if info.conflicts > 0 {
        r = r.severity(Severity::Crit);
    }
    Some(r)
}

fn repo(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let repo = ctx.input.workspace.as_ref()?.repo.as_ref()?;
    let (owner, name) = (repo.owner.as_deref()?, repo.name.as_deref()?);
    let host = repo.host.as_deref().unwrap_or("github.com");
    Some(
        Reading::default()
            .set("full", format!("{owner}/{name}"))
            .set("owner", owner)
            .set("name", name)
            .set("host", host)
            .link(Some(format!("https://{host}/{owner}/{name}"))),
    )
}

fn pr(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let pr = ctx.input.pr.as_ref()?;
    let number = pr.number?;
    let num = if pr.kind.as_deref() == Some("mr") {
        format!("!{number}")
    } else {
        format!("#{number}")
    };
    let (state, severity) = match pr.review_state.as_deref() {
        Some("approved") => ("approved", Severity::Good),
        Some("changes_requested") => ("changes", Severity::Crit),
        Some("pending") => ("pending", Severity::Normal),
        Some("draft") => ("draft", Severity::Normal),
        _ => ("", Severity::Normal),
    };
    Some(
        Reading::default()
            .set("num", num)
            .set("state", state)
            .severity(severity)
            .link(pr.url.clone()),
    )
}

fn worktree(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let session = ctx.input.worktree.as_ref();
    let name = session
        .and_then(|w| w.name.as_deref())
        .or_else(|| ctx.input.workspace.as_ref()?.git_worktree.as_deref())?;
    Some(
        Reading::default().set("name", name).set(
            "branch",
            session
                .and_then(|w| w.branch.as_deref())
                .unwrap_or_default(),
        ),
    )
}

fn lines(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let c = ctx.input.cost.as_ref()?;
    let (added, removed) = (
        c.total_lines_added.unwrap_or(0),
        c.total_lines_removed.unwrap_or(0),
    );
    if added == 0 && removed == 0 {
        return None;
    }
    let mut r = Reading::default();
    if added > 0 {
        r = r.set_role("added", format!("+{added}"), Role::Good);
    }
    if removed > 0 {
        r = r.set_role("removed", format!("-{removed}"), Role::Bad);
    }
    Some(r)
}
