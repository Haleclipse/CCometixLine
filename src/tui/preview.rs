//! The preview bar shared by the editing pages: the config being edited,
//! rendered by the real pipeline against a sample session.

use super::convert;
use super::doc::Doc;
use super::ui;
use crate::env::AutoCompact;
use crate::metrics::{Commands, GitSource};
use crate::models::Registry;
use crate::scenarios::{BASE, SCENARIOS};
use crate::statusline::{self, Frame};
use crate::style::GlyphTier;
use crate::style::color::ColorDepth;
use chrono::FixedOffset;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

/// Most lines the bar shows.
pub const MAX_LINES: usize = 4;
const MIN_WIDTH: u16 = 20;

const DEPTHS: [Option<ColorDepth>; 5] = [
    None,
    Some(ColorDepth::TrueColor),
    Some(ColorDepth::Ansi256),
    Some(ColorDepth::Ansi16),
    Some(ColorDepth::None),
];
const TIERS: [Option<GlyphTier>; 4] = [
    None,
    Some(GlyphTier::Nerd),
    Some(GlyphTier::Unicode),
    Some(GlyphTier::Ascii),
];

#[derive(Clone, Debug, Default)]
pub struct Preview {
    /// Index into `SCENARIOS`.
    pub scenario: usize,
    /// Columns to lay out in; `None` fits the bar.
    pub width: Option<u16>,
    depth: usize,
    tier: usize,
}

fn depth_name(d: ColorDepth) -> &'static str {
    match d {
        ColorDepth::TrueColor => "true",
        ColorDepth::Ansi256 => "256",
        ColorDepth::Ansi16 => "16",
        ColorDepth::None => "none",
    }
}

fn tier_name(t: GlyphTier) -> &'static str {
    match t {
        GlyphTier::Nerd => "nerd",
        GlyphTier::Unicode => "unicode",
        GlyphTier::Ascii => "ascii",
    }
}

impl Preview {
    pub fn select(&mut self, name: &str) {
        if let Some(i) = SCENARIOS.iter().position(|s| s.name == name) {
            self.scenario = i;
        }
    }

    pub fn next_scenario(&mut self, forward: bool) {
        let n = SCENARIOS.len();
        self.scenario = (self.scenario + if forward { 1 } else { n - 1 }) % n;
    }

    pub fn resize(&mut self, delta: i32, fit: u16) {
        let w = self.width.unwrap_or(fit) as i32 + delta;
        let w = w.clamp(MIN_WIDTH as i32, fit as i32) as u16;
        self.width = (w != fit).then_some(w);
    }

    pub fn cycle_depth(&mut self) {
        self.depth = (self.depth + 1) % DEPTHS.len();
    }

    pub fn cycle_glyphs(&mut self) {
        self.tier = (self.tier + 1) % TIERS.len();
    }

    pub fn depth(&self, doc: &Doc) -> ColorDepth {
        DEPTHS[self.depth].unwrap_or_else(|| doc.config.color_depth.resolve())
    }

    fn tier(&self, doc: &Doc) -> GlyphTier {
        TIERS[self.tier].unwrap_or(doc.config.glyphs)
    }

    fn width(&self, fit: u16) -> u16 {
        self.width.unwrap_or(fit).min(fit)
    }

    /// The edited config rendered for the current scenario.
    pub fn render_lines(
        &self,
        doc: &Doc,
        registry: &Registry,
        tz: FixedOffset,
        fit: u16,
    ) -> Vec<Line<'static>> {
        let mut config = doc.config.clone();
        config.glyphs = self.tier(doc);
        let s = &SCENARIOS[self.scenario];
        let frame = Frame {
            now: BASE,
            tz,
            width: Some(self.width(fit) as usize),
            git: GitSource::Fixed(s.git()),
            state: s.state(BASE),
            compact: AutoCompact::default(),
            commands: Commands::Show,
        };
        let depth = self.depth(doc);
        statusline::lines(&s.input(BASE), &config, registry, frame)
            .iter()
            .filter(|l| l.iter().any(|s| !s.text.trim().is_empty()))
            .take(MAX_LINES)
            .map(|l| convert::line(l, depth))
            .collect()
    }

    /// The title on the bar's top rule, labelled when `room` allows.
    pub fn title(&self, doc: &Doc, fit: u16, room: u16) -> Line<'static> {
        let depth = match DEPTHS[self.depth] {
            Some(d) => depth_name(d).to_string(),
            None => format!("auto·{}", depth_name(self.depth(doc))),
        };
        let tier = match TIERS[self.tier] {
            Some(t) => tier_name(t).to_string(),
            None => tier_name(doc.config.glyphs).to_string(),
        };
        let width = match self.width {
            Some(w) => w.to_string(),
            None => format!("fit {fit}"),
        };
        let values = [
            ("scenario", SCENARIOS[self.scenario].name.to_string()),
            ("width", width),
            ("depth", depth),
            ("glyphs", tier),
        ];
        let build = |labels: bool| {
            let mut spans = vec![Span::styled(" Preview ", ui::title())];
            for (label, value) in &values {
                spans.push(Span::raw(if labels {
                    format!(" {label} ")
                } else {
                    " ".into()
                }));
                spans.push(ui::choice(value));
            }
            spans.push(Span::styled("  [ ] - + { } ", ui::muted()));
            Line::from(spans)
        };
        let full = build(true);
        if full.width() <= room as usize {
            full
        } else {
            build(false)
        }
    }

    /// `·········1·········2…` marking every tenth column up to the width.
    pub fn ruler(&self, fit: u16) -> Line<'static> {
        let w = self.width(fit) as usize;
        let text: String = (1..=w)
            .map(|c| {
                if c % 10 == 0 {
                    char::from_digit(((c / 10) % 10) as u32, 10).unwrap_or('·')
                } else {
                    '·'
                }
            })
            .collect();
        Line::styled(text, ui::muted())
    }

    pub fn area_rows(lines: usize) -> u16 {
        lines.clamp(1, MAX_LINES) as u16 + 2
    }
}

/// The rows the preview occupies within `area`: title rule, lines, ruler.
pub fn split(area: Rect, lines: usize) -> (Rect, Rect, Rect) {
    let n = lines.clamp(1, MAX_LINES) as u16;
    let title = Rect { height: 1, ..area };
    let body = Rect {
        y: area.y + 1,
        height: n,
        ..area
    };
    let ruler = Rect {
        y: area.y + 1 + n,
        height: 1,
        ..area
    };
    (title, body, ruler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_and_clamps() {
        let mut p = Preview::default();
        p.next_scenario(false);
        assert_eq!(p.scenario, SCENARIOS.len() - 1);
        p.next_scenario(true);
        assert_eq!(SCENARIOS[p.scenario].name, "normal");
        p.resize(-8, 96);
        assert_eq!(p.width, Some(88));
        p.resize(100, 96);
        assert_eq!(p.width, None);
        p.resize(-500, 96);
        assert_eq!(p.width, Some(MIN_WIDTH));
        assert_eq!(p.ruler(96).spans[0].content.chars().count(), 20);
    }
}
