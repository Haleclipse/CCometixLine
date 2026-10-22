//! Metrics only the companion mod can supply (Claude Code 2.1.287+), read
//! from its bridge: the context window by category, the tools and agents
//! running now, and the last turn's time to first token and speed.

use super::{Ctx, Metric, MetricMeta, Options, Reading, fmt};
use crate::state::bridge::{Bridge, Category as Row};
use crate::style::{Category, IconKey};
use crate::version::gate;
use crate::widgets::WidgetKind;

/// Running tools named in full; more are counted.
const TOOLS_NAMED: usize = 2;

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
        min_cc: Some(gate::MODS),
        needs_mod: true,
        options: &[],
        stateful: false,
        priority: 2,
    }
}

pub(super) fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            meta: MetricMeta {
                // Its ratio is the window's use, as `context`'s is.
                levels: super::LEVELS,
                ..meta(
                    "context.breakdown",
                    Category::Context,
                    "Context by kind",
                    "What fills the context window, largest first, as /context counts it",
                    IconKey::Context,
                    ["{top}", "{top2}", "{first}"],
                    &[
                        "top", "top2", "first", "messages", "system", "tools", "memory",
                    ],
                )
            },
            compute: breakdown,
        },
        Metric {
            meta: meta(
                "activity.tools",
                Category::Activity,
                "Running tools",
                "The tool calls running now, the longest first",
                IconKey::Tools,
                ["{tools}[ · {elapsed}]", "{tools}", "{count}"],
                &["tools", "count", "elapsed"],
            ),
            compute: tools,
        },
        Metric {
            meta: meta(
                "activity.agents",
                Category::Activity,
                "Running agents",
                "Subagents and teammates working now, and their types",
                IconKey::Agent,
                ["{count} running[ · {types}]", "{count} running", "{count}"],
                &["count", "types"],
            ),
            compute: agents,
        },
        Metric {
            meta: meta(
                "timing.ttft",
                Category::Timing,
                "Time to first token",
                "How long the last turn's answer took to start",
                IconKey::Ttft,
                ["{ttft} to first token", "{ttft}", "{ttft}"],
                &["ttft"],
            ),
            compute: ttft,
        },
        Metric {
            meta: meta(
                "timing.tps",
                Category::Timing,
                "Output speed",
                "Output tokens per second while the last turn's answer streamed",
                IconKey::Speed,
                ["{tps} tok/s", "{tps}/s", "{tps}"],
                &["tps"],
            ),
            compute: tps,
        },
    ]
}

fn bridge<'a>(ctx: &'a Ctx) -> Option<&'a Bridge> {
    ctx.state.bridge.as_deref()
}

/// `/context`'s row labels, shortened.
fn short(name: &str) -> String {
    match name {
        "System prompt" => "sys".into(),
        "System tools" => "tools".into(),
        "MCP tools" => "mcp".into(),
        "Custom agents" => "agents".into(),
        "Memory files" => "memory".into(),
        "Skills" => "skills".into(),
        "Slash commands" => "commands".into(),
        "Messages" => "msgs".into(),
        other => other
            .split_whitespace()
            .next()
            .unwrap_or(other)
            .to_lowercase(),
    }
}

fn breakdown(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let c = bridge(ctx)?.context.as_ref()?;
    let mut used: Vec<&Row> = c
        .categories
        .iter()
        .filter(|r| r.is_used() && r.tokens > 0)
        .collect();
    used.sort_by_key(|r| std::cmp::Reverse(r.tokens));
    let first = used.first()?;
    let label = |r: &&Row| format!("{} {}", short(&r.name), fmt::tokens(r.tokens));
    let list = |n: usize| {
        used.iter()
            .take(n)
            .map(label)
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let of = |name: &str| {
        c.categories
            .iter()
            .find(|r| r.name == name)
            .map(|r| fmt::tokens(r.tokens))
            .unwrap_or_default()
    };
    let mut r = Reading::default()
        .set("top", list(3))
        .set("top2", list(2))
        .set("first", label(first))
        .set("messages", of("Messages"))
        .set("system", of("System prompt"))
        .set("tools", of("System tools"))
        .set("memory", of("Memory files"));
    if let (Some(total), Some(window)) = (c.used, c.window.filter(|w| *w > 0)) {
        r = r.ratio(total as f64 / window as f64);
    }
    Some(r)
}

fn tools(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let b = bridge(ctx)?;
    if b.tools.is_empty() {
        return None;
    }
    let mut running: Vec<_> = b.tools.iter().collect();
    running.sort_by_key(|t| t.since);
    let mut names: Vec<String> = running
        .iter()
        .take(TOOLS_NAMED)
        .map(|t| t.name.clone())
        .collect();
    if running.len() > TOOLS_NAMED {
        names.push(format!("+{}", running.len() - TOOLS_NAMED));
    }
    let since = running[0].since;
    let elapsed = (ctx.now * 1000 - since).max(0) as u64;
    Some(
        Reading::default()
            .set("tools", names.join(" · "))
            .set("count", running.len().to_string())
            .set("elapsed", fmt::duration_ms(elapsed)),
    )
}

fn agents(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let active: Vec<_> = bridge(ctx)?
        .agents
        .iter()
        .filter(|a| a.is_active())
        .collect();
    if active.is_empty() {
        return None;
    }
    let mut types: Vec<&str> = Vec::new();
    for a in &active {
        if !a.kind.is_empty() && !types.contains(&a.kind.as_str()) {
            types.push(&a.kind);
        }
    }
    Some(
        Reading::default()
            .set("count", active.len().to_string())
            .set("types", types.join(" · ")),
    )
}

fn ttft(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let ms = bridge(ctx)?.turn.as_ref()?.ttft_ms?;
    let text = if ms < 10_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        fmt::duration_ms(ms)
    };
    Some(Reading::default().set("ttft", text))
}

fn tps(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let tps = bridge(ctx)?.turn.as_ref()?.tps.filter(|t| *t > 0.0)?;
    Some(Reading::default().set("tps", format!("{tps:.0}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::metrics::{find, read};
    use crate::models::Registry;
    use crate::state::StateView;
    use crate::style::GlyphTier;
    use chrono::FixedOffset;
    use std::rc::Rc;

    const BRIDGE: &str = r#"{"v": 1, "updated_at": 1790808774000,
      "context": {"threshold": 167000, "auto": true, "window": 200000, "used": 118000,
        "categories": [{"name": "System prompt", "kind": "used", "tokens": 3100},
                       {"name": "System tools", "kind": "used", "tokens": 17900},
                       {"name": "Messages", "kind": "used", "tokens": 97000},
                       {"name": "Free space", "kind": "free", "tokens": 49000}]},
      "turn": {"ttft_ms": 820, "tps": 74.2},
      "tools": [{"name": "Bash", "since": 1790808771000}, {"name": "Edit", "since": 1790808760000},
                {"name": "Read", "since": 1790808773000}],
      "agents": [{"id": "a1", "type": "Explore", "status": "running"},
                 {"id": "a2", "type": "Explore", "status": "waiting"},
                 {"id": "a3", "type": "general-purpose", "status": "completed"}]}"#;

    fn text(id: &str, version: &str, bridge: Option<&str>) -> Option<String> {
        let input = Input::parse(&format!(r#"{{"version": "{version}"}}"#)).unwrap();
        let reg = Registry::default();
        let state = StateView {
            bridge: bridge.map(|b| Rc::new(serde_json::from_str(b).unwrap())),
            ..StateView::default()
        };
        let ctx = Ctx::new(
            &input,
            &reg,
            1_790_808_780,
            FixedOffset::east_opt(0).unwrap(),
            GlyphTier::Unicode,
        )
        .with_state(state);
        let m = find(id).unwrap();
        let r = read(m, &ctx, &Options::new())?;
        Some(
            crate::widgets::template::render(m.meta.templates[0], &r)
                .iter()
                .map(|p| p.text.as_str())
                .collect(),
        )
    }

    #[test]
    fn reads_the_bridge() {
        let b = Some(BRIDGE);
        assert_eq!(
            text("context.breakdown", "2.1.300", b).as_deref(),
            Some("msgs 97k · tools 17.9k · sys 3.1k")
        );
        assert_eq!(
            text("activity.tools", "2.1.300", b).as_deref(),
            Some("Edit · Bash · +1 · 20s")
        );
        assert_eq!(
            text("activity.agents", "2.1.300", b).as_deref(),
            Some("2 running · Explore")
        );
        assert_eq!(
            text("timing.ttft", "2.1.300", b).as_deref(),
            Some("0.8s to first token")
        );
        assert_eq!(
            text("timing.tps", "2.1.300", b).as_deref(),
            Some("74 tok/s")
        );
    }

    #[test]
    fn needs_the_mod_and_its_claude_code() {
        assert_eq!(text("timing.tps", "2.1.300", None), None);
        assert_eq!(text("timing.tps", "2.1.285", Some(BRIDGE)), None);
    }
}
