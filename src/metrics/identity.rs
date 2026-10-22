use super::{Ctx, LEVELS, Metric, MetricMeta, OptKind, OptSpec, Options, Reading};
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
                options: &[OptSpec {
                    key: "effort",
                    kind: OptKind::Flag,
                    default: "true",
                    summary: "Add the reasoning effort while thinking is on",
                }],
                ..meta(
                    "model",
                    Category::Identity,
                    "Model",
                    "Model name, corrected by the model registry, with its reasoning effort",
                    IconKey::Model,
                    ["{name}[ · {effort}]", "{name}[ · {effort}]", "{family}"],
                    &["name", "family", "id", "effort"],
                )
            },
            compute: model,
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

fn model(ctx: &Ctx, opts: &Options) -> Option<Reading> {
    let resolved = ctx.model()?;
    let family = resolved.name.split_whitespace().next().unwrap_or_default();
    let mut r = Reading::default()
        .set("name", &resolved.name)
        .set("family", family)
        .set("id", ctx.input.model_id().unwrap_or_default());
    let with_effort = opts.get("effort").and_then(|v| v.as_bool()).unwrap_or(true);
    if with_effort && let Some(level) = effort(ctx) {
        r = r.set("effort", level);
    }
    Some(r)
}

/// The reasoning effort as `Low`, `Med`, `High`, `xHigh` or `Max`, for
/// models that have one. Thinking and effort go together: with thinking off
/// there is none to show.
fn effort(ctx: &Ctx) -> Option<String> {
    if ctx.input.thinking.as_ref().and_then(|t| t.enabled) == Some(false) {
        return None;
    }
    let level = ctx.input.effort.as_ref()?.level.as_deref()?.to_lowercase();
    Some(match level.as_str() {
        "low" => "Low".into(),
        "medium" => "Med".into(),
        "high" => "High".into(),
        "xhigh" => "xHigh".into(),
        "max" => "Max".into(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        }
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::metrics::{find, read};
    use crate::models::Registry;
    use crate::style::GlyphTier;
    use chrono::FixedOffset;

    /// The model item at `level` (0 full, 1 compact) for `json`.
    fn model_text(json: &str, opts: &str, level: usize) -> String {
        let input = Input::parse(json).unwrap();
        let reg = Registry::default();
        let ctx = Ctx::new(
            &input,
            &reg,
            0,
            FixedOffset::east_opt(0).unwrap(),
            GlyphTier::Unicode,
        );
        let m = find("model").unwrap();
        let opts: Options = toml::from_str(opts).unwrap();
        let r = read(m, &ctx, &opts).unwrap();
        crate::widgets::template::render(m.meta.templates[level], &r)
            .iter()
            .map(|p| p.text.as_str())
            .collect()
    }

    const OPUS: &str = r#""model": {"id": "claude-opus-4-8", "display_name": "Opus 4.8"}"#;

    #[test]
    fn model_carries_the_effort_while_thinking() {
        let on = format!(
            r#"{{{OPUS}, "effort": {{"level": "high"}}, "thinking": {{"enabled": true}}}}"#
        );
        assert_eq!(model_text(&on, "", 0), "Opus 4.8 · High");
        assert_eq!(model_text(&on, "", 1), "Opus 4.8 · High");
        for (level, shown) in [
            ("low", "Low"),
            ("medium", "Med"),
            ("xhigh", "xHigh"),
            ("max", "Max"),
        ] {
            let json = format!(r#"{{{OPUS}, "effort": {{"level": "{level}"}}}}"#);
            assert_eq!(model_text(&json, "", 0), format!("Opus 4.8 · {shown}"));
        }
        assert_eq!(model_text(&on, "effort = false", 0), "Opus 4.8");
        let off = format!(
            r#"{{{OPUS}, "effort": {{"level": "high"}}, "thinking": {{"enabled": false}}}}"#
        );
        assert_eq!(model_text(&off, "", 0), "Opus 4.8");
        let none = format!(r#"{{{OPUS}}}"#);
        assert_eq!(model_text(&none, "", 0), "Opus 4.8");
    }
}
