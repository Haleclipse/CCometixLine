//! Metrics that look ahead or back: context growth per turn and the room
//! left before auto-compact, spend per hour and today, what a cold prompt
//! cache costs to rebuild, and when a rate limit runs out. Most of them read
//! the session history kept by the state layer.

use super::{Ctx, Metric, MetricMeta, Options, Reading, fmt, usage};
use crate::env::AutoCompact;
use crate::models::builtin;
use crate::state::Sample;
use crate::state::session::context_turns;
use crate::style::{Category, IconKey, Severity};
use crate::version::gate;
use crate::widgets::WidgetKind;

/// Claude Code keeps room for the reply: the model's output limit, capped
/// at this.
const OUTPUT_RESERVE: u64 = 20_000;
/// It compacts this far below the window that remains.
const COMPACT_BUFFER: u64 = 13_000;
/// It starts warning this far before the compact point.
const WARN_MARGIN: u64 = 20_000;
/// Completed turns averaged to estimate growth per turn.
const GROWTH_TURNS: usize = 5;
/// History `cost.rate` looks back over, and the least it needs.
const RATE_WINDOW: i64 = 3_600;
const RATE_MIN_SPAN: i64 = 120;
/// Least recent history, and least time since a window opened, to project
/// a rate limit from.
const QUOTA_MIN_SPAN: i64 = 300;
const QUOTA_MIN_ELAPSED: i64 = 600;

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
        priority: 2,
    }
}

pub(super) fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Sparkline,
                stateful: true,
                ..meta(
                    "context.trend",
                    Category::Context,
                    "Context trend",
                    "Context size at the end of each turn, and this turn's growth",
                    IconKey::Trend,
                    ["{delta}", "", "{delta}"],
                    &["delta", "turns"],
                )
            },
            compute: trend,
        },
        Metric {
            meta: MetricMeta {
                priority: 3,
                ..meta(
                    "context.to_compact",
                    Category::Context,
                    "Until auto-compact",
                    "Tokens left before Claude Code compacts, and about how many turns",
                    IconKey::Compact,
                    [
                        "[{left} to compact[ · ~{turns} turns][ · {next}]][{due}]",
                        "[{left}][{due}]",
                        "[{left}][{due}]",
                    ],
                    &["left", "at", "pct", "turns", "next", "due"],
                )
            },
            compute: to_compact,
        },
        Metric {
            meta: MetricMeta {
                stateful: true,
                ..meta(
                    "cost.rate",
                    Category::Cost,
                    "Spend rate",
                    "Session spend per hour over the last hour",
                    IconKey::Rate,
                    ["{rate}/h", "{rate}/h", "{rate}/h"],
                    &["rate"],
                )
            },
            compute: cost_rate,
        },
        Metric {
            meta: MetricMeta {
                stateful: true,
                ..meta(
                    "cost.today",
                    Category::Cost,
                    "Spent today",
                    "Spend across all sessions since local midnight",
                    IconKey::Today,
                    ["{usd} today", "{usd}", "{usd}"],
                    &["usd"],
                )
            },
            compute: cost_today,
        },
        Metric {
            meta: MetricMeta {
                min_cc: Some(gate::PROMPT_CACHE),
                ..meta(
                    "cache.cold_cost",
                    Category::Cache,
                    "Rewarm cost",
                    "Estimated cost to rebuild the prompt cache once it goes cold",
                    IconKey::Cache,
                    ["rewarm ~{usd}", "~{usd}", "~{usd}"],
                    &["usd", "tokens"],
                )
            },
            compute: cold_cost,
        },
    ]
}

fn trend(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let turns = context_turns(&ctx.state.samples);
    let [.., before, now] = turns[..] else {
        return None;
    };
    let r = Reading {
        series: Some(turns.clone()),
        ..Reading::default()
    };
    Some(
        r.set("delta", fmt::signed_tokens(now - before))
            .set("turns", turns.len().to_string()),
    )
}

/// The token count at which Claude Code compacts automatically, given the
/// window it reports; `None` when auto-compact is off.
pub(super) fn compact_point(cc_window: u64, ac: &AutoCompact) -> Option<u64> {
    if !ac.enabled {
        return None;
    }
    let window = ac.window.map_or(cc_window, |w| w.min(cc_window));
    let usable = window.checked_sub(OUTPUT_RESERVE)?;
    let default = usable.checked_sub(COMPACT_BUFFER)?;
    Some(match ac.pct {
        Some(pct) => ((usable as f64 * pct / 100.0).floor() as u64).min(default),
        None => default,
    })
}

/// Turns until `left` more tokens are used, at the average growth of recent
/// completed turns since the context last shrank.
fn turns_left(samples: &[Sample], left: u64) -> Option<u64> {
    let turns = context_turns(samples);
    // The last entry is the turn in progress.
    let done = &turns[..turns.len().saturating_sub(1)];
    let start = done
        .windows(2)
        .rposition(|w| w[1] < w[0])
        .map_or(0, |i| i + 1);
    let recent = &done[start..];
    let recent = &recent[recent.len().saturating_sub(GROWTH_TURNS + 1)..];
    if recent.len() < 3 {
        return None;
    }
    let per_turn = (recent[recent.len() - 1] - recent[0]) / (recent.len() - 1) as f64;
    (per_turn > 0.0).then(|| (left as f64 / per_turn).ceil() as u64)
}

fn to_compact(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let cw = ctx.input.context_window.as_ref()?;
    let tokens = usage::context_tokens(cw).filter(|t| *t > 0)?;
    // The mod knows the point exactly; otherwise follow Claude Code's rules.
    let exact = ctx.state.bridge.as_deref().and_then(|b| b.context.as_ref());
    let mut at = match exact {
        Some(c) if !c.auto => return None,
        Some(c) if c.threshold.is_some() => c.threshold?,
        _ => compact_point(cw.context_window_size?, &ctx.compact)?,
    };
    // Claude Code measures against the window it believes in; a smaller
    // real window fails requests before it would compact.
    if let Some(real) = ctx
        .model()
        .filter(|m| m.window_corrected)
        .and_then(|m| m.window)
    {
        at = at.min(real);
    }
    let ratio = tokens as f64 / at as f64;
    let r = Reading::default()
        .set("at", fmt::tokens(at))
        .set("pct", fmt::percent(ratio))
        .ratio(ratio);
    let left = at.saturating_sub(tokens);
    if left == 0 {
        return Some(r.set("due", "compact due").severity(Severity::Crit));
    }
    let mut r = r.set("left", fmt::tokens(left));
    match turns_left(&ctx.state.samples, left) {
        Some(1) => r = r.set("next", "next turn"),
        Some(n) => r = r.set("turns", n.to_string()),
        None => {}
    }
    if left <= WARN_MARGIN {
        r = r.severity(Severity::Warn);
    }
    Some(r)
}

fn cost_rate(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let cost = ctx.input.cost.as_ref()?.total_cost_usd?;
    // Measure from the oldest sample in the window, or from where the cost
    // last went down (the session's cost was reset).
    let mut start: Option<(i64, f64)> = None;
    let mut prev = f64::NEG_INFINITY;
    for s in &ctx.state.samples {
        let Some(c) = s.cost.filter(|_| ctx.now - s.t <= RATE_WINDOW) else {
            continue;
        };
        if start.is_none() || c < prev {
            start = Some((s.t, c));
        }
        prev = c;
    }
    let (t0, c0) = start?;
    let span = ctx.now - t0;
    if span < RATE_MIN_SPAN || cost < c0 {
        return None;
    }
    let per_hour = (cost - c0) / span as f64 * 3_600.0;
    Some(Reading::default().set("rate", fmt::money(per_hour)))
}

fn cost_today(ctx: &Ctx, _: &Options) -> Option<Reading> {
    let usd = ctx.state.today_cost?;
    Some(Reading::default().set("usd", fmt::money(usd)))
}

/// Estimated cost of writing the prompt cache again from scratch, and the
/// tokens involved.
pub(super) fn rewarm_cost(ctx: &Ctx) -> Option<(f64, u64)> {
    let pc = ctx.input.prompt_cache.as_ref()?;
    let tokens = pc.recache_tokens_if_cold.filter(|t| *t > 0)?;
    let price = builtin::input_price(ctx.input.model_id()?)?;
    let write = match pc.ttl.as_deref() {
        Some("1h") => 2.0,
        _ => 1.25,
    };
    Some((tokens as f64 * price * write / 1_000_000.0, tokens))
}

fn cold_cost(ctx: &Ctx, _: &Options) -> Option<Reading> {
    if ctx.input.prompt_cache.as_ref()?.caching_observed == Some(false) {
        return None;
    }
    let (usd, tokens) = rewarm_cost(ctx)?;
    Some(
        Reading::default()
            .set("usd", fmt::money(usd))
            .set("tokens", fmt::tokens(tokens)),
    )
}

/// A rate-limit window, for finding its history in session samples.
#[derive(Clone, Copy, Debug)]
pub(super) enum Limit {
    FiveHour,
    SevenDay,
    /// Not sampled; projected from the window average alone.
    Spend,
}

impl Limit {
    /// Use in percent and reset time recorded in `s` for this window.
    fn sampled(self, s: &Sample) -> Option<(f64, i64)> {
        match self {
            Limit::FiveHour => Some((s.h5?, s.h5_reset?)),
            Limit::SevenDay => Some((s.d7?, s.d7_reset?)),
            Limit::Spend => None,
        }
    }
}

/// When a window `length` seconds long reaches 100% (`used` is 0..1), if
/// that comes before it resets at `resets`. Uses the rate over the last
/// tenth of the window when history covers enough of it, else the average
/// since the window opened.
pub(super) fn run_out(ctx: &Ctx, limit: Limit, used: f64, resets: i64, length: i64) -> Option<i64> {
    if used >= 1.0 || resets <= ctx.now {
        return None;
    }
    let oldest_recent = ctx.state.samples.iter().find_map(|s| {
        let (pct, reset) = limit.sampled(s)?;
        // Same window (reset times jitter a little), within the lookback.
        let same = (reset - resets).abs() <= 120;
        (same && ctx.now - s.t <= length / 10).then_some((s.t, pct / 100.0))
    });
    let rate = match oldest_recent {
        Some((t, then)) if ctx.now - t >= QUOTA_MIN_SPAN => (used - then) / (ctx.now - t) as f64,
        _ => {
            let elapsed = length - (resets - ctx.now);
            if elapsed < QUOTA_MIN_ELAPSED {
                return None;
            }
            used / elapsed as f64
        }
    };
    if rate <= 0.0 {
        return None;
    }
    let at = ctx.now + ((1.0 - used) / rate).ceil() as i64;
    (at < resets).then_some(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{GitSource, find, read};
    use crate::models::Registry;
    use crate::scenarios::{self, BASE};
    use crate::style::GlyphTier;
    use chrono::FixedOffset;

    fn reading(metric: &str, scenario: &str) -> Option<Reading> {
        let s = scenarios::find(scenario).unwrap();
        let input = s.input(BASE);
        let reg = Registry::default();
        let ctx = Ctx::new(
            &input,
            &reg,
            BASE,
            FixedOffset::east_opt(0).unwrap(),
            GlyphTier::Unicode,
        )
        .with_git(GitSource::Fixed(None))
        .with_state(s.state(BASE));
        read(find(metric).unwrap(), &ctx, &Options::new())
    }

    fn field(r: &Reading, name: &str) -> String {
        r.get(name).map(|f| f.text.clone()).unwrap_or_default()
    }

    #[test]
    fn the_mods_compact_point_wins() {
        let s = scenarios::find("normal").unwrap();
        let input = s.input(BASE);
        let reg = Registry::default();
        let with = |edit: &dyn Fn(&mut crate::state::bridge::Context)| {
            let mut bridge = s.bridge(BASE).unwrap();
            edit(bridge.context.as_mut().unwrap());
            let state = crate::state::StateView {
                bridge: Some(std::rc::Rc::new(bridge)),
                ..s.state(BASE)
            };
            let ctx = Ctx::new(
                &input,
                &reg,
                BASE,
                FixedOffset::east_opt(0).unwrap(),
                GlyphTier::Unicode,
            )
            .with_state(state);
            read(find("context.to_compact").unwrap(), &ctx, &Options::new())
        };
        let r = with(&|c| c.threshold = Some(150_000)).unwrap();
        assert_eq!(field(&r, "at"), "150k");
        assert!(with(&|c| c.auto = false).is_none());
        // Without a threshold from the mod, Claude Code's rules still apply.
        let r = with(&|c| c.threshold = None).unwrap();
        assert_eq!(field(&r, "at"), "167k");
    }

    #[test]
    fn compact_point_follows_claude_code() {
        let ac = AutoCompact::default();
        assert_eq!(compact_point(200_000, &ac), Some(167_000));
        assert_eq!(compact_point(1_000_000, &ac), Some(967_000));
        let capped = AutoCompact {
            window: Some(400_000),
            ..ac
        };
        assert_eq!(compact_point(1_000_000, &capped), Some(367_000));
        assert_eq!(compact_point(200_000, &capped), Some(167_000));
        let pct = AutoCompact {
            pct: Some(50.0),
            ..ac
        };
        assert_eq!(compact_point(200_000, &pct), Some(90_000));
        let late = AutoCompact {
            pct: Some(99.0),
            ..ac
        };
        assert_eq!(compact_point(200_000, &late), Some(167_000));
        let off = AutoCompact {
            enabled: false,
            ..ac
        };
        assert_eq!(compact_point(200_000, &off), None);
    }

    #[test]
    fn trend_tracks_turns() {
        let r = reading("context.trend", "normal").unwrap();
        assert_eq!(field(&r, "delta"), "+12k");
        assert_eq!(field(&r, "turns"), "9");
        assert_eq!(r.series.unwrap().len(), 9);
        assert!(reading("context.trend", "fresh").is_none());
    }

    #[test]
    fn counts_down_to_compact() {
        let r = reading("context.to_compact", "normal").unwrap();
        assert_eq!(field(&r, "left"), "43k");
        assert_eq!(field(&r, "at"), "167k");
        assert_eq!(field(&r, "turns"), "5");
        assert_eq!(r.severity, None);

        let due = reading("context.to_compact", "near-full").unwrap();
        assert_eq!(field(&due, "due"), "compact due");
        assert_eq!(due.severity, Some(Severity::Crit));

        // Kimi's real 128k window ends before Claude Code's 167k compact point.
        let kimi = reading("context.to_compact", "third-party").unwrap();
        assert_eq!(field(&kimi, "at"), "128k");
        assert_eq!(field(&kimi, "left"), "24k");
    }

    #[test]
    fn rates_spend_over_the_last_hour() {
        let r = reading("cost.rate", "normal").unwrap();
        assert_eq!(field(&r, "rate"), "$1.65");
        assert!(reading("cost.rate", "fresh").is_none());
        let today = reading("cost.today", "normal").unwrap();
        assert_eq!(field(&today, "usd"), "$4.85");
    }

    #[test]
    fn prices_a_cold_cache() {
        // 182k tokens of Opus 4.8 at $5/M, written with the 5-minute TTL.
        let r = reading("cache.cold_cost", "near-full").unwrap();
        assert_eq!(field(&r, "usd"), "$1.14");
        // 270k tokens of Opus 5.5 written with the 1-hour TTL.
        let big = reading("cache.cold_cost", "big-context").unwrap();
        assert_eq!(field(&big, "usd"), "$2.70");
        assert!(reading("cache.cold_cost", "third-party").is_none());
    }

    #[test]
    fn projects_when_limits_run_out() {
        // 88% used, 14 points in the last 30 minutes: out in ~26 minutes,
        // before the reset in 30.
        let r = reading("quota.eta", "near-full").unwrap();
        assert_eq!(field(&r, "which"), "5h");
        assert_eq!(field(&r, "left"), "25:43");
        assert_eq!(r.severity, Some(Severity::Crit));
        assert_eq!(
            field(&reading("quota.5h", "near-full").unwrap(), "eta"),
            "00:25"
        );
        // Recent history is slow enough to outlast the window.
        assert!(reading("quota.eta", "normal").is_none());
        // Spend isn't sampled; the month's average pace runs out first.
        let gw = reading("quota.eta", "gateway").unwrap();
        assert_eq!(field(&gw, "which"), "spend");
    }
}
