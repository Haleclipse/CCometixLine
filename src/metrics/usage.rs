//! Context, rate limits, cost, prompt cache and timing.

use super::projection::{self, Limit};
use super::{Ctx, LEVELS, Metric, MetricMeta, Options, Reading, fmt};
use crate::input::{ContextWindow, SpendLimit, Window};
use crate::style::{Category, GlyphTier, IconKey, Role, Severity};
use crate::version::gate;
use crate::widgets::WidgetKind;

const FIVE_HOURS: i64 = 5 * 3600;
const SEVEN_DAYS: i64 = 7 * 86_400;

fn meta(
    id: &'static str,
    category: Category,
    name: &'static str,
    summary: &'static str,
    icon: IconKey,
) -> MetricMeta {
    MetricMeta {
        id,
        category,
        name,
        summary,
        icon,
        widget: WidgetKind::Text,
        templates: ["", "", ""],
        fields: &[],
        levels: LEVELS,
        min_cc: None,
        needs_mod: false,
        stateful: false,
        priority: 3,
    }
}

const WINDOW_FIELDS: &[&str] = &["pct", "reset", "left", "pace", "arrow", "eta"];
const WINDOW_TEMPLATES: [&str; 3] = ["{pct}%[ · {reset}]", "{pct}%", "{pct}%"];

pub(super) fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Bar,
                templates: ["{pct}%[ · {tokens}]", "{pct}%", "{pct}%"],
                fields: &["pct", "tokens", "window", "left", "cc_pct"],
                priority: 5,
                ..meta(
                    "context",
                    Category::Context,
                    "Context",
                    "Context window use, corrected for models Claude Code can't size",
                    IconKey::Context,
                )
            },
            compute: context,
        },
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Pace,
                templates: WINDOW_TEMPLATES,
                fields: WINDOW_FIELDS,
                priority: 4,
                ..meta(
                    "quota.5h",
                    Category::Quota,
                    "5-hour limit",
                    "Subscription 5-hour window",
                    IconKey::Quota,
                )
            },
            compute: five_hour,
        },
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Pace,
                templates: WINDOW_TEMPLATES,
                fields: WINDOW_FIELDS,
                priority: 3,
                ..meta(
                    "quota.7d",
                    Category::Quota,
                    "7-day limit",
                    "Subscription weekly window",
                    IconKey::Quota,
                )
            },
            compute: seven_day,
        },
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Bar,
                templates: ["{pct}%[ · {used}/{limit}]", "{pct}%", "{pct}%"],
                fields: &[
                    "pct", "used", "limit", "period", "reset", "left", "pace", "arrow", "eta",
                ],
                min_cc: Some(gate::SPEND_LIMIT),
                ..meta(
                    "quota.spend",
                    Category::Quota,
                    "Spend limit",
                    "Claude apps gateway spend limit",
                    IconKey::Spend,
                )
            },
            compute: spend,
        },
        Metric {
            meta: MetricMeta {
                templates: ["{which} limit in {left}", "{which} {left}", "{left}"],
                fields: &["which", "at", "left"],
                ..meta(
                    "quota.eta",
                    Category::Quota,
                    "Limit run-out",
                    "When the first rate or spend limit runs out at the current pace",
                    IconKey::Eta,
                )
            },
            compute: quota_eta,
        },
        Metric {
            meta: MetricMeta {
                templates: ["{usd}", "{usd}", "{usd}"],
                fields: &["usd"],
                priority: 4,
                ..meta(
                    "cost",
                    Category::Cost,
                    "Cost",
                    "Session cost so far",
                    IconKey::Cost,
                )
            },
            compute: cost,
        },
        Metric {
            meta: MetricMeta {
                templates: [
                    "[{left}][{cold}[ ~{rewarm}]][ · {hit}% hit]",
                    "[{left}][{cold}]",
                    "[{left}][{cold}]",
                ],
                fields: &[
                    "left", "cold", "rewarm", "state", "ttl", "hit", "misses", "cause",
                ],
                min_cc: Some(gate::PROMPT_CACHE),
                priority: 2,
                ..meta(
                    "cache",
                    Category::Cache,
                    "Prompt cache",
                    "Time until the prompt cache goes cold, and hit ratio",
                    IconKey::Cache,
                )
            },
            compute: cache,
        },
        Metric {
            meta: MetricMeta {
                templates: ["{dur}", "{dur}", "{dur}"],
                fields: &["dur"],
                priority: 2,
                ..meta(
                    "duration",
                    Category::Session,
                    "Duration",
                    "Session wall-clock time",
                    IconKey::Duration,
                )
            },
            compute: duration,
        },
        Metric {
            meta: MetricMeta {
                templates: ["{pct}% on model", "{pct}%", "{pct}%"],
                fields: &["pct", "api", "total"],
                levels: &[],
                priority: 1,
                ..meta(
                    "api_share",
                    Category::Timing,
                    "Time on model",
                    "Share of session time spent waiting for the model",
                    IconKey::ApiShare,
                )
            },
            compute: api_share,
        },
    ]
}

/// Tokens in the context: the reported total, else the last request's input.
pub(super) fn context_tokens(cw: &ContextWindow) -> Option<u64> {
    cw.total_input_tokens.or_else(|| {
        let u = cw.current_usage.as_ref()?;
        Some(
            u.input_tokens.unwrap_or(0)
                + u.cache_creation_input_tokens.unwrap_or(0)
                + u.cache_read_input_tokens.unwrap_or(0),
        )
    })
}

fn context(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let cw = ctx.input.context_window.as_ref()?;
    let tokens = context_tokens(cw)?;
    let resolved = ctx.model();
    let window = resolved.and_then(|r| r.window).or(cw.context_window_size)?;
    let corrected = resolved.is_some_and(|r| r.window_corrected);
    let ratio = match cw.used_percentage {
        Some(p) if !corrected => p / 100.0,
        _ if window > 0 => tokens as f64 / window as f64,
        _ => return None,
    };
    let mut r = Reading::default()
        .set("pct", fmt::percent(ratio))
        .set("window", fmt::tokens(window))
        .set("left", fmt::tokens(window.saturating_sub(tokens)))
        .ratio(ratio);
    // Before the first response there is nothing to count; leave the field
    // unset so templates drop it instead of showing "0 tokens".
    if tokens > 0 {
        r = r.set("tokens", fmt::tokens(tokens));
    }
    if corrected && let Some(p) = cw.used_percentage {
        r = r.set("cc_pct", fmt::percent(p / 100.0));
    }
    Some(r)
}

/// A rate-limit window: usage, time to reset, pace against elapsed time and
/// when it runs out at the current rate.
fn window(w: &Window, length: i64, limit: Limit, ctx: &Ctx) -> Option<Reading> {
    let used = w.used_percentage? / 100.0;
    let resets = w.resets_at.map(|t| t as i64);
    if resets.is_some_and(|t| t <= ctx.now) {
        // The window already reset; this reading is stale.
        return None;
    }
    let mut r = Reading::default()
        .set("pct", fmt::percent(used))
        .ratio(used);
    if let Some(at) = resets {
        let left = at - ctx.now;
        let elapsed = (1.0 - left as f64 / length as f64).clamp(0.0, 1.0);
        let delta = used - elapsed;
        let (up, down) = match ctx.glyphs {
            GlyphTier::Ascii => ("+", "-"),
            _ => ("⇡", "⇣"),
        };
        let role = if delta > 0.0 { Role::Bad } else { Role::Good };
        r = r
            .set("reset", fmt::clock(at, ctx.now, ctx.tz))
            .set("left", fmt::countdown(left))
            .set_role("pace", fmt::signed_percent(delta), role)
            .set_role(
                "arrow",
                format!(
                    "{}{}%",
                    if delta > 0.0 { up } else { down },
                    (delta.abs() * 100.0).round()
                ),
                role,
            );
        r.elapsed = Some(elapsed);
        if let Some(out) = projection::run_out(ctx, limit, used, at, length) {
            r = r.set("eta", fmt::clock(out, ctx.now, ctx.tz));
        }
    }
    Some(r)
}

fn five_hour(ctx: &Ctx, _: &Options) -> Option<Reading> {
    window(
        ctx.input.rate_limits.as_ref()?.five_hour.as_ref()?,
        FIVE_HOURS,
        Limit::FiveHour,
        ctx,
    )
}

fn seven_day(ctx: &Ctx, _: &Options) -> Option<Reading> {
    window(
        ctx.input.rate_limits.as_ref()?.seven_day.as_ref()?,
        SEVEN_DAYS,
        Limit::SevenDay,
        ctx,
    )
}

/// A spend limit as a window, and the window's length.
fn spend_window(s: &SpendLimit) -> (Window, i64) {
    let length = match s.period.as_deref() {
        Some("daily") => 86_400,
        Some("weekly") => SEVEN_DAYS,
        _ => 30 * 86_400,
    };
    let w = Window {
        used_percentage: s.used_percentage,
        resets_at: s.resets_at,
    };
    (w, length)
}

fn spend(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let s = ctx.input.rate_limits.as_ref()?.spend_limit.as_ref()?;
    let (w, length) = spend_window(s);
    let mut r = window(&w, length, Limit::Spend, ctx)?;
    if let Some(used) = s.used_usd {
        r = r.set("used", fmt::money(used));
    }
    if let Some(limit) = s.limit_usd {
        r = r.set("limit", fmt::money(limit));
    }
    if let Some(period) = &s.period {
        r = r.set("period", period);
    }
    if r.ratio.is_some_and(|x| x >= 1.0) {
        r = r.severity(Severity::Crit);
    }
    Some(r)
}

/// The limit that runs out first at the current pace, if any runs out
/// before it resets.
fn quota_eta(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let rl = ctx.input.rate_limits.as_ref()?;
    let spend = rl.spend_limit.as_ref().map(spend_window);
    let windows = [
        ("5h", Limit::FiveHour, rl.five_hour.as_ref(), FIVE_HOURS),
        ("7d", Limit::SevenDay, rl.seven_day.as_ref(), SEVEN_DAYS),
        (
            "spend",
            Limit::Spend,
            spend.as_ref().map(|(w, _)| w),
            spend.as_ref().map_or(0, |(_, l)| *l),
        ),
    ];
    let (which, at) = windows
        .into_iter()
        .filter_map(|(which, limit, w, length)| {
            let w = w?;
            let used = w.used_percentage? / 100.0;
            let resets = w.resets_at? as i64;
            Some((
                which,
                projection::run_out(ctx, limit, used, resets, length)?,
            ))
        })
        .min_by_key(|(_, at)| *at)?;
    let left = at - ctx.now;
    Some(
        Reading::default()
            .set("which", which)
            .set("at", fmt::clock(at, ctx.now, ctx.tz))
            .set("left", fmt::countdown(left))
            .severity(if left < 1_800 {
                Severity::Crit
            } else {
                Severity::Warn
            }),
    )
}

fn cost(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let usd = ctx.input.cost.as_ref()?.total_cost_usd?;
    Some(Reading::default().set("usd", fmt::money(usd)))
}

fn cache(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let pc = ctx.input.prompt_cache.as_ref()?;
    if pc.caching_observed == Some(false) {
        return None;
    }
    let ttl = match pc.ttl.as_deref() {
        Some("1h") => 3_600,
        _ => 300,
    };
    let left = pc
        .expires_at
        .map(|t| t as i64 - ctx.now)
        .filter(|l| *l > 0 && pc.warm == Some(true));
    let mut r = Reading::default();
    match left {
        Some(left) => {
            let warn_below = (ttl / 5).clamp(60, 300);
            r = r
                .set("left", fmt::countdown(left))
                .set("state", "warm")
                .ratio(left as f64 / ttl as f64)
                .severity(if left < warn_below {
                    Severity::Warn
                } else {
                    Severity::Normal
                });
        }
        None => {
            r = r
                .set("cold", "cold")
                .set("state", "cold")
                .ratio(0.0)
                .severity(Severity::Warn);
        }
    }
    if let Some((usd, _)) = projection::rewarm_cost(ctx) {
        r = r.set("rewarm", fmt::money(usd));
    }
    if let Some(hit) = pc.hit_ratio {
        r = r.set("hit", fmt::percent(hit));
    }
    if let Some(m) = pc.misses.filter(|m| *m > 0) {
        r = r.set("misses", m.to_string());
    }
    if let Some(cause) = pc.last_miss_cause.as_ref().and_then(|c| c.causes.first()) {
        // `tools_changed` → `tools changed`, `ttl_expired_5m` → `ttl expired 5m`.
        r = r.set("cause", cause.replace('_', " "));
    }
    Some(r.set("ttl", pc.ttl.clone().unwrap_or_default()))
}

fn duration(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let ms = ctx.input.cost.as_ref()?.total_duration_ms?;
    Some(Reading::default().set("dur", fmt::duration_ms(ms as u64)))
}

fn api_share(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let c = ctx.input.cost.as_ref()?;
    let (api, total) = (c.total_api_duration_ms?, c.total_duration_ms?);
    if total <= 0.0 {
        return None;
    }
    let ratio = (api / total).clamp(0.0, 1.0);
    Some(
        Reading::default()
            .set("pct", fmt::percent(ratio))
            .set("api", fmt::duration_ms(api as u64))
            .set("total", fmt::duration_ms(total as u64))
            .ratio(ratio),
    )
}
