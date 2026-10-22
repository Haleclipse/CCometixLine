use super::{Ctx, LEVELS, Metric, MetricMeta, Options, Reading};
use crate::style::{Category, IconKey};
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
        levels: LEVELS,
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
                priority: 5,
                ..meta(
                    "model",
                    Category::Identity,
                    "Model",
                    "Model name, corrected by the model registry",
                    IconKey::Model,
                    ["{name}", "{name}", "{family}"],
                    &["name", "family", "id"],
                )
            },
            compute: model,
        },
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Text,
                ..meta(
                    "effort",
                    Category::Identity,
                    "Effort",
                    "Reasoning effort level",
                    IconKey::Effort,
                    ["{level}", "{level}", "{short}"],
                    &["level", "short"],
                )
            },
            compute: effort,
        },
        Metric {
            meta: meta(
                "thinking",
                Category::Identity,
                "Thinking",
                "Shown while extended thinking is on",
                IconKey::Thinking,
                ["thinking", "think", "T"],
                &[],
            ),
            compute: thinking,
        },
        Metric {
            meta: meta(
                "fast",
                Category::Identity,
                "Fast mode",
                "Shown while fast mode is on",
                IconKey::Fast,
                ["fast", "fast", "F"],
                &[],
            ),
            compute: fast,
        },
        Metric {
            meta: MetricMeta {
                priority: 2,
                ..meta(
                    "output_style",
                    Category::Identity,
                    "Output style",
                    "Output style name; hidden for the default style unless show_default = true",
                    IconKey::OutputStyle,
                    ["{name}", "{name}", "{name}"],
                    &["name"],
                )
            },
            compute: output_style,
        },
        Metric {
            meta: MetricMeta {
                priority: 4,
                ..meta(
                    "vim",
                    Category::Identity,
                    "Vim mode",
                    "Vim editor mode while vim mode is on",
                    IconKey::Vim,
                    ["{mode}", "{mode}", "{short}"],
                    &["mode", "short"],
                )
            },
            compute: vim,
        },
        Metric {
            meta: meta(
                "agent",
                Category::Identity,
                "Agent",
                "Agent name when started with --agent",
                IconKey::Agent,
                ["{name}", "{name}", "{name}"],
                &["name"],
            ),
            compute: agent,
        },
        Metric {
            meta: MetricMeta {
                priority: 2,
                ..meta(
                    "session_name",
                    Category::Session,
                    "Session name",
                    "Name from /rename or the generated title; max_len (default 30)",
                    IconKey::SessionName,
                    ["{name}", "{name}", "{name}"],
                    &["name"],
                )
            },
            compute: session_name,
        },
        Metric {
            meta: MetricMeta {
                priority: 1,
                ..meta(
                    "cc_version",
                    Category::System,
                    "Claude Code version",
                    "Version of the running Claude Code",
                    IconKey::Version,
                    ["{version}", "{version}", "{version}"],
                    &["version"],
                )
            },
            compute: cc_version,
        },
        Metric {
            meta: MetricMeta {
                priority: 1,
                stateful: true,
                ..meta(
                    "update",
                    Category::System,
                    "Update",
                    "A newer ccline is published; links to its release notes",
                    IconKey::Update,
                    ["{version} available", "{version}", "{version}"],
                    &["version", "current"],
                )
            },
            compute: update,
        },
    ]
}

fn model(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let resolved = ctx.model()?;
    let family = resolved.name.split_whitespace().next().unwrap_or_default();
    Some(
        Reading::default()
            .set("name", &resolved.name)
            .set("family", family)
            .set("id", ctx.input.model_id().unwrap_or_default()),
    )
}

fn effort(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let level = ctx.input.effort.as_ref()?.level.as_deref()?.to_lowercase();
    let (short, ratio) = match level.as_str() {
        "low" => ("L", 0.2),
        "medium" => ("M", 0.4),
        "high" => ("H", 0.6),
        "xhigh" => ("XH", 0.8),
        "max" => ("MAX", 1.0),
        _ => ("?", 0.5),
    };
    Some(
        Reading::default()
            .set("level", &level)
            .set("short", short)
            .ratio(ratio),
    )
}

fn thinking(ctx: &Ctx, _: &Options) -> Option<Reading> {
    (ctx.input.thinking.as_ref()?.enabled == Some(true)).then(Reading::default)
}

fn fast(ctx: &Ctx, _: &Options) -> Option<Reading> {
    (ctx.input.fast_mode == Some(true)).then(Reading::default)
}

fn output_style(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let name = ctx.input.output_style.as_ref()?.name.as_deref()?;
    let show_default = opts
        .get("show_default")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if name.eq_ignore_ascii_case("default") && !show_default {
        return None;
    }
    Some(Reading::default().set("name", name))
}

fn vim(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let mode = ctx.input.vim.as_ref()?.mode.as_deref().unwrap_or("INSERT");
    let short = match mode {
        "VISUAL LINE" => "VL".to_string(),
        m => m.chars().next().map(String::from).unwrap_or_default(),
    };
    Some(Reading::default().set("mode", mode).set("short", short))
}

fn agent(ctx: &Ctx, _: &Options) -> Option<Reading> {
    Some(Reading::default().set("name", ctx.input.agent.as_ref()?.name.as_deref()?))
}

fn session_name(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let name = ctx.input.session_name.as_deref()?;
    let max = opts
        .get("max_len")
        .and_then(|v| v.as_integer())
        .unwrap_or(30)
        .max(4) as usize;
    Some(Reading::default().set("name", crate::render::width::truncate(name, max)))
}

fn cc_version(ctx: &Ctx, _: &Options) -> Option<Reading> {
    Some(Reading::default().set("version", ctx.input.version.as_deref()?))
}

fn update(ctx: &Ctx, _: &Options) -> Option<Reading> {
    use crate::state::update::{CURRENT, RELEASES};
    let latest = ctx.state.latest_version.as_deref()?;
    Some(
        Reading::default()
            .set("version", format!("v{latest}"))
            .set("current", format!("v{CURRENT}"))
            .link(Some(format!("{RELEASES}/tag/v{latest}"))),
    )
}
