//! Metrics for subagent lines, one background task each (see
//! `statusline::subagent_lines`). Session metrics such as `model`,
//! `context` and `effort` work there too, reading the task through a
//! synthetic input.

use super::{Ctx, Metric, MetricMeta, Options, Reading, fmt};
use crate::style::{Category, GlyphTier, IconKey, Severity};
use crate::widgets::WidgetKind;

const DEFAULT_MAX_LEN: usize = 50;

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
        stateful: false,
        priority: 3,
    }
}

pub(super) fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            meta: MetricMeta {
                priority: 2,
                ..meta(
                    "task.label",
                    Category::Activity,
                    "Task activity",
                    "What a background task is doing now (subagent lines)",
                    IconKey::Agent,
                    ["{label}", "{short}", "{short}"],
                    &["label", "short", "description"],
                )
            },
            compute: label,
        },
        Metric {
            meta: meta(
                "task.name",
                Category::Activity,
                "Task name",
                "A background task's name or description, and its kind (subagent lines)",
                IconKey::Agent,
                ["[{agent} · ]{name}", "{name}", "{kind}"],
                &["name", "kind", "agent"],
            ),
            compute: name,
        },
        Metric {
            meta: meta(
                "task.elapsed",
                Category::Timing,
                "Task time",
                "How long a running background task has been going (subagent lines)",
                IconKey::Duration,
                ["{dur}", "{dur}", "{dur}"],
                &["dur"],
            ),
            compute: elapsed,
        },
        Metric {
            meta: MetricMeta {
                priority: 2,
                ..meta(
                    "task.status",
                    Category::Activity,
                    "Task status",
                    "Running, completed or failed (subagent lines)",
                    IconKey::Agent,
                    ["{mark}", "{mark}", "{mark}"],
                    &["mark", "status"],
                )
            },
            compute: status,
        },
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Sparkline,
                priority: 1,
                ..meta(
                    "task.trend",
                    Category::Context,
                    "Task tokens trend",
                    "A background task's recent token counts (subagent lines)",
                    IconKey::Trend,
                    ["{delta}", "", "{delta}"],
                    &["delta"],
                )
            },
            compute: trend,
        },
    ]
}

fn running(status: Option<&str>) -> bool {
    matches!(status, None | Some("running" | "pending"))
}

fn label(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let task = ctx.task?;
    let max = opts
        .get("max_len")
        .and_then(|v| v.as_integer())
        .map_or(DEFAULT_MAX_LEN, |n| n.max(1) as usize);
    let text = task
        .label
        .as_deref()
        .or(task.description.as_deref())?
        .trim();
    let truncate = crate::render::width::truncate;
    let mut r = Reading::default()
        .set("label", truncate(text, max))
        .set("short", truncate(text, (max / 2).max(8)));
    if let Some(d) = &task.description {
        r = r.set("description", truncate(d, max));
    }
    Some(r)
}

fn name(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let task = ctx.task?;
    let kind = match task.kind.as_deref() {
        Some("local_agent" | "remote_agent") => "agent",
        Some("local_bash") => "shell",
        Some("local_workflow") => "workflow",
        Some("in_process_teammate") => "teammate",
        Some(other) => other,
        None => "",
    };
    let name = task.name.as_deref().or(task.description.as_deref())?;
    let mut r = Reading::default().set("name", name).set("kind", kind);
    if let Some(agent) = &task.agent_type {
        r = r.set("agent", agent);
    }
    Some(r)
}

fn elapsed(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let task = ctx.task?;
    // Claude Code doesn't report when a task ended, so only running tasks
    // have a meaningful duration.
    if !running(task.status.as_deref()) {
        return None;
    }
    let ms = (ctx.now as f64 * 1000.0 - task.start_time?).max(0.0);
    Some(Reading::default().set("dur", fmt::duration_ms(ms as u64)))
}

fn status(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let task = ctx.task?;
    let status = task.status.as_deref().unwrap_or("running");
    let ascii = ctx.glyphs == GlyphTier::Ascii;
    let (mark, severity) = match status {
        "completed" => (if ascii { "ok" } else { "✓" }, Severity::Good),
        "failed" | "killed" | "error" => (if ascii { "x" } else { "✗" }, Severity::Crit),
        _ => (if ascii { "*" } else { "●" }, Severity::Normal),
    };
    Some(
        Reading::default()
            .set("mark", mark)
            .set("status", status)
            .severity(severity),
    )
}

fn trend(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let samples = &ctx.task?.token_samples;
    let [first, .., last] = samples[..] else {
        return None;
    };
    let r = Reading {
        series: Some(samples.clone()),
        ..Reading::default()
    };
    Some(r.set("delta", fmt::signed_tokens(last - first)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::metrics::{find, read};
    use crate::models::Registry;
    use crate::scenarios::{self, BASE};
    use chrono::FixedOffset;

    fn field(metric: &str, task: &str, name: &str) -> Option<String> {
        let sub = scenarios::subagent(BASE);
        let task = sub.tasks.iter().find(|t| t.id == task).unwrap();
        let input: Input = task.input(&sub);
        let reg = Registry::default();
        let ctx = Ctx::new(
            &input,
            &reg,
            BASE,
            FixedOffset::east_opt(0).unwrap(),
            GlyphTier::Unicode,
        )
        .with_task(task);
        let r = read(find(metric).unwrap(), &ctx, &Options::new())?;
        r.get(name).map(|f| f.text.clone())
    }

    #[test]
    fn describes_tasks() {
        assert_eq!(field("task.elapsed", "a1", "dur").as_deref(), Some("3m12s"));
        assert_eq!(field("task.elapsed", "a3", "dur"), None);
        assert_eq!(field("task.status", "a3", "mark").as_deref(), Some("✓"));
        assert_eq!(
            field("task.trend", "a1", "delta").as_deref(),
            Some("+61.3k")
        );
        assert_eq!(field("task.trend", "a3", "delta"), None);
        assert_eq!(field("task.name", "b1", "kind").as_deref(), Some("shell"));
        assert_eq!(
            field("task.name", "a2", "agent").as_deref(),
            Some("Explore")
        );
        assert_eq!(field("model", "a1", "name").as_deref(), Some("Opus 5.5 1M"));
        assert_eq!(field("context", "a2", "pct").as_deref(), Some("24"));
    }
}
