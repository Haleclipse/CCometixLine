//! The render pipeline: config + input → metrics → widgets → layout → lines.

use crate::config::{Config, ItemConfig, ItemSpec, LineConfig};
use crate::env::{AutoCompact, ClaudeSettings};
use crate::input::{Input, SubagentInput, Task};
use crate::layout::{self, LaidItem, LineSpec};
use crate::metrics::{self, Commands, Ctx, GitSource, Options};
use crate::models::Registry;
use crate::render::span::{Line, Span, Style};
use crate::state::StateView;
use crate::style::palette::{self, Palette, Token};
use crate::style::skin::{ItemView, Side, compose};
use crate::style::{Color, Severity, Theme, glyphs};
use crate::version;
use crate::widgets::{self, Level, Props};
use chrono::FixedOffset;

/// Time, width and the outside world for one render.
pub struct Frame {
    pub now: i64,
    pub tz: FixedOffset,
    /// Columns available to the line; `None` when unknown.
    pub width: Option<usize>,
    pub git: GitSource,
    pub state: StateView,
    pub compact: AutoCompact,
    pub commands: Commands,
}

pub fn theme(config: &Config) -> Theme {
    let mut palette = Palette::load(&config.palette).unwrap_or_else(palette::terminal);
    for (token, color) in &config.palette_overrides {
        if let (Some(t), Some(c)) = (Token::parse(token), Color::parse(color)) {
            palette.set(t, c);
        }
    }
    Theme {
        palette,
        skin: config.skin,
        glyphs: config.glyphs,
        separator: config.separator.clone(),
        fill: config.fill,
        edges: config.powerline,
    }
}

/// Columns the status line may use: the configured or reported terminal
/// width, minus Claude Code's `statusLine.padding` on both sides and any
/// configured reserve.
pub fn budget(config: &Config, settings: &ClaudeSettings) -> Option<usize> {
    let width = config
        .layout
        .width
        .0
        .map(usize::from)
        .or_else(crate::env::columns)?;
    let padding = config
        .layout
        .padding
        .0
        .map(usize::from)
        .unwrap_or_else(|| settings.statusline_padding());
    Some(width.saturating_sub(2 * padding + config.layout.reserve as usize))
}

pub fn lines(input: &Input, config: &Config, registry: &Registry, frame: Frame) -> Vec<Line> {
    let theme = theme(config);
    if let Some(v) = input.cc_version()
        && v < version::MIN_SUPPORTED
    {
        let text = format!(
            "ccline needs Claude Code {} or newer (running {v})",
            version::MIN_SUPPORTED
        );
        return vec![vec![Span::styled(
            text,
            Style {
                fg: theme.palette.color(Token::Warn),
                ..Style::default()
            },
        )]];
    }
    let ctx = context(input, registry, &frame, &theme);
    config
        .lines
        .iter()
        .map(|line| layout::fit(&line_spec(line, &ctx, &theme).0, frame.width, &theme))
        .collect()
}

/// What the layout made of one configured line, for the composer.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub line: Line,
    /// Columns the line needs with every item at its fullest.
    pub natural: usize,
    /// The layout's steps, with `index` pointing into the config's side.
    pub steps: Vec<layout::Step>,
    /// Config entries with nothing to show for this input.
    pub empty: Vec<(Side, usize)>,
}

/// Lay out `line` as `lines` would, reporting each step it took; `task`
/// is the background task for a subagent line.
pub fn report(
    input: &Input,
    task: Option<&Task>,
    config: &Config,
    registry: &Registry,
    frame: &Frame,
    line: &LineConfig,
) -> Report {
    let theme = theme(config);
    let mut ctx = context(input, registry, frame, &theme);
    if let Some(task) = task {
        ctx = ctx.with_task(task);
    }
    let (spec, [left, right]) = line_spec(line, &ctx, &theme);
    let plan = layout::plan(&spec, frame.width, &theme);
    let source = |side: Side, i: usize| match side {
        Side::Left => left[i],
        Side::Right => right[i],
    };
    let empty = [
        (Side::Left, &line.left, &left),
        (Side::Right, &line.right, &right),
    ]
    .into_iter()
    .flat_map(|(side, all, shown)| {
        (0..all.len())
            .filter(|i| !shown.contains(i))
            .map(move |i| (side, i))
    })
    .collect();
    Report {
        line: plan.line,
        natural: plan.natural,
        steps: plan
            .steps
            .into_iter()
            .map(|s| layout::Step {
                index: source(s.side, s.index),
                ..s
            })
            .collect(),
        empty,
    }
}

/// `item` drawn on its own at each level, `None` where it has nothing;
/// `None` overall when it shows nothing for this input.
pub fn item_levels(
    input: &Input,
    task: Option<&Task>,
    config: &Config,
    registry: &Registry,
    frame: &Frame,
    item: &ItemConfig,
) -> Option<[Option<Line>; 3]> {
    let theme = theme(config);
    let mut ctx = context(input, registry, frame, &theme);
    if let Some(task) = task {
        ctx = ctx.with_task(task);
    }
    let laid = lay_item(item, &ctx, &theme)?;
    Some(Level::ALL.map(|level| {
        let pieces = laid.variants[level.index()].as_deref()?;
        let view = ItemView {
            pieces,
            category: laid.category,
            severity: laid.severity,
            color: laid.color,
            fg: laid.fg,
        };
        Some(compose(&[view], Side::Left, &theme))
    }))
}

fn context<'a>(input: &'a Input, registry: &'a Registry, frame: &Frame, theme: &Theme) -> Ctx<'a> {
    Ctx::new(input, registry, frame.now, frame.tz, theme.glyphs)
        .with_git(frame.git.clone())
        .with_state(frame.state.clone())
        .with_compact(frame.compact)
        .with_commands(frame.commands.clone())
}

/// One line per background task, for `subagentStatusLine`, keyed by task
/// id. The width is the payload's `columns`, which already leaves out
/// Claude Code's own prefix.
pub fn subagent_lines(
    payload: &SubagentInput,
    config: &Config,
    registry: &Registry,
    frame: Frame,
) -> Vec<(String, Line)> {
    let theme = theme(config);
    let line = config
        .subagent
        .clone()
        .unwrap_or_else(crate::presets::default_subagent);
    let width = payload.columns.or(frame.width);
    payload
        .tasks
        .iter()
        .map(|task| {
            let input = task.input(payload);
            let ctx = context(&input, registry, &frame, &theme).with_task(task);
            let spec = line_spec(&line, &ctx, &theme).0;
            (task.id.clone(), layout::fit(&spec, width, &theme))
        })
        .collect()
}

/// The laid-out items of `line`, and for each side the config entries
/// they came from (entries with nothing to show are skipped).
fn line_spec(line: &LineConfig, ctx: &Ctx, theme: &Theme) -> (LineSpec, [Vec<usize>; 2]) {
    let side = |specs: &[ItemSpec]| -> (Vec<LaidItem>, Vec<usize>) {
        specs
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((lay_item(&s.config(), ctx, theme)?, i)))
            .unzip()
    };
    let (left, left_src) = side(&line.left);
    let (right, right_src) = side(&line.right);
    (LineSpec { left, right }, [left_src, right_src])
}

fn parse_severity(name: &str) -> Option<Severity> {
    match name {
        "normal" => Some(Severity::Normal),
        "ok" | "good" => Some(Severity::Good),
        "warn" => Some(Severity::Warn),
        "crit" => Some(Severity::Crit),
        _ => None,
    }
}

fn lay_item(cfg: &ItemConfig, ctx: &Ctx, theme: &Theme) -> Option<LaidItem> {
    let metric = metrics::find(&cfg.metric)?;
    let empty = Options::new();
    let reading = metrics::read(metric, ctx, cfg.options.as_ref().unwrap_or(&empty))?;
    let meta = &metric.meta;
    let template = |own: &Option<String>, i: usize| {
        own.clone().unwrap_or_else(|| meta.templates[i].to_string())
    };
    let props = Props {
        kind: cfg.widget.unwrap_or(meta.widget),
        bar: cfg.bar.unwrap_or_default(),
        cells: cfg.cells.map(usize::from).unwrap_or(10),
        templates: [
            template(&cfg.format, 0),
            template(&cfg.compact, 1),
            template(&cfg.minimal, 2),
        ],
        icon: cfg.icon.clone().unwrap_or_else(|| {
            glyphs::icon(reading.icon.unwrap_or(meta.icon), theme.glyphs).to_string()
        }),
        levels: match &cfg.levels {
            Some(levels) => levels
                .iter()
                .filter_map(|(at, name)| Some((*at, parse_severity(name)?)))
                .collect(),
            None => meta.levels.to_vec(),
        },
        coloring: cfg.coloring.unwrap_or_default(),
    };
    let variants = Level::ALL.map(|level| widgets::render(&reading, &props, level, theme.glyphs));
    if variants.iter().all(Option::is_none) {
        return None;
    }
    let resolve = |c: &str| match Token::parse(c) {
        Some(t) => theme.palette.color(t),
        None => Color::parse(c),
    };
    let color = cfg.color.as_deref().and_then(resolve);
    let fg = cfg.fg.as_deref().and_then(resolve);
    Some(LaidItem {
        variants,
        category: meta.category,
        severity: widgets::severity(&reading, &props.levels),
        color,
        fg,
        priority: cfg.priority.unwrap_or(meta.priority).clamp(1, 5),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::span::line_text;
    use crate::{presets, scenarios};

    fn render(preset: &str, scenario: &str, width: Option<usize>) -> Vec<String> {
        let s = scenarios::find(scenario).unwrap();
        let frame = Frame {
            now: scenarios::BASE,
            tz: FixedOffset::east_opt(0).unwrap(),
            width,
            git: GitSource::Fixed(s.git()),
            state: s.state(scenarios::BASE),
            compact: AutoCompact::default(),
            commands: Commands::Show,
        };
        lines(
            &s.input(scenarios::BASE),
            &presets::get(preset).unwrap(),
            &Registry::default(),
            frame,
        )
        .iter()
        .map(|l| line_text(l))
        .collect()
    }

    #[test]
    fn classic_matches_v1_shape() {
        let out = render("classic", "normal", None);
        assert_eq!(
            out,
            vec![
                "\u{e26d} Opus 4.8 | \u{f024b} app | \u{f02a2} main ● ↑2 | \u{f49b} 62% · 124k tokens"
            ]
        );
    }

    #[test]
    fn corrects_third_party_context() {
        let out = render("minimal", "third-party", None);
        assert_eq!(out, vec!["Kimi K2 Turbo · 81% · $0.84"]);
    }

    #[test]
    fn refuses_old_claude_code() {
        let out = render("classic", "old-cc", None);
        assert_eq!(
            out,
            vec!["ccline needs Claude Code 2.1.153 or newer (running 2.1.140)"]
        );
    }

    #[test]
    fn reports_steps_by_config_entry() {
        let s = scenarios::find("normal").unwrap();
        let frame = |width| Frame {
            now: scenarios::BASE,
            tz: FixedOffset::east_opt(0).unwrap(),
            width,
            git: GitSource::Fixed(s.git()),
            state: s.state(scenarios::BASE),
            compact: AutoCompact::default(),
            commands: Commands::Show,
        };
        let mut config = presets::get("classic").unwrap();
        config.glyphs = crate::style::GlyphTier::Ascii;
        let id = |m: &str| ItemSpec::Id(m.into());
        // `vim` has nothing in this scenario, so `git` is entry 2 but item 1.
        let line = LineConfig {
            left: vec![
                id("model"),
                id("vim"),
                ItemSpec::Item(Box::new(ItemConfig {
                    metric: "git".into(),
                    priority: Some(1),
                    ..ItemConfig::default()
                })),
            ],
            right: vec![id("cost")],
        };
        let input = s.input(scenarios::BASE);
        let reg = Registry::default();
        let r = report(&input, None, &config, &reg, &frame(Some(80)), &line);
        assert_eq!(r.empty, vec![(Side::Left, 1)]);
        assert!(r.steps.is_empty());
        assert!(r.natural < 80);

        let narrow = r.natural - 1;
        let r = report(&input, None, &config, &reg, &frame(Some(narrow)), &line);
        let first = r.steps[0];
        assert_eq!((first.side, first.index), (Side::Left, 2));
        assert_eq!(first.from, Level::Full);

        let levels = item_levels(
            &input,
            None,
            &config,
            &reg,
            &frame(None),
            &id("git").config(),
        )
        .unwrap();
        let text: Vec<String> = levels
            .iter()
            .map(|l| l.as_ref().map(|l| line_text(l)).unwrap_or_default())
            .collect();
        assert_eq!(text, vec!["main * +2", "main*", "main"]);
        assert!(
            item_levels(
                &input,
                None,
                &config,
                &reg,
                &frame(None),
                &id("vim").config()
            )
            .is_none()
        );
    }

    #[test]
    fn fits_width() {
        for width in [40, 60, 80, 120] {
            for line in render("dashboard", "near-full", Some(width)) {
                assert!(
                    crate::render::width::display_width(&line) <= width,
                    "{width}: {line}"
                );
            }
        }
    }
}
