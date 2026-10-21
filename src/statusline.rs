//! The render pipeline: config + input → metrics → widgets → layout → lines.

use crate::config::{Config, ItemConfig, ItemSpec};
use crate::input::Input;
use crate::layout::{self, LaidItem, LineSpec};
use crate::metrics::{self, Ctx, GitSource, Options};
use crate::models::Registry;
use crate::render::span::{Line, Span, Style};
use crate::style::palette::{self, Palette, Token};
use crate::style::{Color, Severity, Theme, glyphs};
use crate::version;
use crate::widgets::{self, Level, Props};
use chrono::FixedOffset;

/// Time, width and git for one render.
pub struct Frame {
    pub now: i64,
    pub tz: FixedOffset,
    /// Columns available to the line; `None` when unknown.
    pub width: Option<usize>,
    pub git: GitSource,
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
    }
}

/// Columns the status line may use: the configured or reported terminal
/// width, minus Claude Code's `statusLine.padding` on both sides and any
/// configured reserve.
pub fn budget(config: &Config, input: &Input) -> Option<usize> {
    let width = config
        .layout
        .width
        .0
        .map(usize::from)
        .or_else(crate::env::columns)?;
    let padding = config.layout.padding.0.map(usize::from).unwrap_or_else(|| {
        let project = input
            .workspace
            .as_ref()
            .and_then(|w| w.project_dir.clone())
            .or_else(|| std::env::var("CLAUDE_PROJECT_DIR").ok());
        crate::env::statusline_padding(project.as_deref())
    });
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
    let ctx = Ctx::new(input, registry, frame.now, frame.tz, theme.glyphs).with_git(frame.git);
    config
        .lines
        .iter()
        .map(|line| {
            let spec = LineSpec {
                left: items(&line.left, &ctx, &theme),
                right: items(&line.right, &ctx, &theme),
            };
            layout::fit(&spec, frame.width, &theme)
        })
        .collect()
}

fn items(specs: &[ItemSpec], ctx: &Ctx, theme: &Theme) -> Vec<LaidItem> {
    specs
        .iter()
        .filter_map(|s| lay_item(&s.config(), ctx, theme))
        .collect()
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
    let color = cfg.color.as_deref().and_then(|c| match Token::parse(c) {
        Some(t) => theme.palette.color(t),
        None => Color::parse(c),
    });
    Some(LaidItem {
        variants,
        category: meta.category,
        severity: widgets::severity(&reading, &props.levels),
        color,
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
