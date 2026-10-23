//! 4 Style: skin, palette, glyph tier, color depth and fill style, and
//! per-token color overrides on top of the palette, including each
//! category's segment background and text.

use super::{Cx, Env, Outcome, Page};
use crate::config::{Config, DepthSetting, ItemSpec, LineConfig};
use crate::style::palette::{self, Category, Palette, TOKENS, Token};
use crate::style::{Edge, FillStyle, GlyphTier, SkinKind};
use crate::tui::dialogs::{ColorDialog, ColorTarget, FontCheck};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

const SKINS: [SkinKind; 4] = [
    SkinKind::Plain,
    SkinKind::Bracket,
    SkinKind::Pill,
    SkinKind::Powerline,
];
/// Separators for the plain skin; `None` is the skin's own.
const SEPARATORS: [Option<&str>; 5] = [None, Some(" | "), Some(" · "), Some(" / "), Some("  ")];
const TIERS: [GlyphTier; 3] = [GlyphTier::Nerd, GlyphTier::Unicode, GlyphTier::Ascii];
const FILLS: [FillStyle; 2] = [FillStyle::Solid, FillStyle::Tinted];
/// Rows of the glyphs & color pane; the last opens the font check.
const LOOK_ROWS: usize = 4;
const DEPTHS: [DepthSetting; 5] = [
    DepthSetting::Auto,
    DepthSetting::TrueColor,
    DepthSetting::Ansi256,
    DepthSetting::Ansi16,
    DepthSetting::None,
];
/// Category tokens shown as a palette's swatches.
const SWATCHES: [Token; 8] = [
    Token::Identity,
    Token::Workspace,
    Token::Vcs,
    Token::Context,
    Token::Quota,
    Token::Cost,
    Token::Cache,
    Token::Session,
];
/// Semantic tokens come first in `TOKENS`; the rest are categories.
const SEMANTIC: usize = 9;
/// Columns of a semantic cell: marker, name, swatch, value, mark.
const SEMANTIC_CELL: usize = 2 + 10 + 1 + 9 + 1;
/// The category table's columns: the color, then segment bg and fg.
const SEGMENT_COLUMNS: [&str; 3] = ["color", "bg", "fg"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    #[default]
    Skin,
    Palette,
    Look,
    Tokens,
}

const PANES: [Pane; 4] = [Pane::Skin, Pane::Palette, Pane::Look, Pane::Tokens];

#[derive(Default)]
pub struct StylePage {
    pane: Pane,
    /// Row in the skin pane: the skins, then the separator.
    skin_row: usize,
    palette_row: usize,
    look_row: usize,
    /// Token cursor: column 0 is the semantic list, 1–3 the category
    /// table's color, bg and fg.
    tcol: usize,
    trow: usize,
    palettes: Vec<String>,
}

fn lower<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}").to_lowercase()
}

fn depth_label(d: DepthSetting) -> String {
    match d {
        DepthSetting::Auto => format!("auto: {}", lower(d.resolve())),
        DepthSetting::TrueColor => "truecolor".into(),
        DepthSetting::Ansi256 => "256".into(),
        DepthSetting::Ansi16 => "16".into(),
        DepthSetting::None => "none".into(),
    }
}

fn separator_label(s: Option<&str>) -> String {
    match s {
        None => "skin default".into(),
        Some(s) => format!("\"{s}\""),
    }
}

/// The palette with the config's overrides applied.
fn effective_palette(config: &Config) -> Palette {
    let mut p = Palette::load(&config.palette).unwrap_or_else(palette::terminal);
    for (name, value) in &config.palette_overrides {
        if let (Some(t), Some(c)) = (Token::parse(name), crate::style::Color::parse(value)) {
            p.set(t, c);
        }
    }
    p
}

/// The token at a cursor position in the tokens pane.
fn token_at(col: usize, row: usize) -> Token {
    if col == 0 {
        return TOKENS[row.min(SEMANTIC - 1)];
    }
    let cat = Category::ALL[row.min(Category::ALL.len() - 1)];
    match col {
        1 => Token::from(cat),
        2 => Token::SegBg(cat),
        _ => Token::SegFg(cat),
    }
}

/// Where a token's color comes from.
enum Source {
    Palette,
    Override,
    /// Worked out from the fill style.
    Auto,
}

fn cycle<T: PartialEq + Copy>(all: &[T], current: T, forward: bool) -> T {
    let i = all.iter().position(|v| *v == current).unwrap_or(0);
    let n = all.len();
    all[(i + if forward { 1 } else { n - 1 }) % n]
}

impl StylePage {
    fn palettes(&mut self) -> &[String] {
        if self.palettes.is_empty() {
            self.palettes = palette::available();
        }
        &self.palettes
    }

    fn block(&self, title: &str, pane: Pane, cx: &Cx) -> Block<'static> {
        let b = ui::panel(title);
        if cx.focused && self.pane == pane {
            b.border_style(ui::accent())
        } else {
            b
        }
    }

    fn row_marker(&self, pane: Pane, row: usize, at: usize, cx: &Cx) -> (Span<'static>, Style) {
        let on = cx.focused && self.pane == pane && row == at;
        (
            Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
            if on { ui::accent() } else { Style::default() },
        )
    }

    fn render_skin(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let inner = area.width.saturating_sub(18);
        let registry = cx.doc.registry();
        let mut lines: Vec<Line> = SKINS
            .iter()
            .enumerate()
            .map(|(i, skin)| {
                let (marker, style) = self.row_marker(Pane::Skin, i, self.skin_row, cx);
                let radio = ui::radio(cx.doc.config.skin == *skin);
                // A sample drawn by the real painter: three items in this skin.
                let mut sample = cx.doc.config.clone();
                sample.skin = *skin;
                sample.lines = vec![LineConfig {
                    left: ["model", "dir", "git"]
                        .iter()
                        .map(|m| ItemSpec::Id(m.to_string()))
                        .collect(),
                    right: Vec::new(),
                }];
                let mut spans = vec![
                    marker,
                    Span::styled(format!("{radio} {:<10} ", lower(skin)), style),
                ];
                if let Some(line) = cx
                    .preview
                    .render(&sample, cx.doc, &registry, cx.env.tz, inner)
                    .into_iter()
                    .next()
                {
                    spans.extend(line.spans);
                }
                Line::from(spans)
            })
            .collect();
        let row = |i: usize, label: &str, value: String, note: String| {
            let (marker, style) = self.row_marker(Pane::Skin, SKINS.len() + i, self.skin_row, cx);
            Line::from(vec![
                marker,
                Span::styled(format!("{label:<11}"), style),
                ui::choice(&value),
                Span::styled(format!("  {note}"), ui::muted()),
            ])
        };
        // The edge's glyphs, so the choice shows what it draws.
        let sample = |e: Edge| match e.glyphs() {
            Some((right, left)) => format!("{right} {left}  powerline"),
            None => "powerline".to_string(),
        };
        let edges = cx.doc.config.powerline;
        lines.push(row(
            0,
            "Separator",
            separator_label(cx.doc.config.separator.as_deref()),
            "plain skin".into(),
        ));
        lines.push(row(1, "Join", lower(edges.join), sample(edges.join)));
        lines.push(row(2, "End", lower(edges.end), sample(edges.end)));
        f.render_widget(
            Paragraph::new(lines).block(self.block("Skin", Pane::Skin, cx)),
            area,
        );
    }

    fn render_palettes(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let depth = cx.preview.depth(cx.doc);
        let rows = area.height.saturating_sub(2) as usize;
        let start = self.palette_row.saturating_sub(rows.saturating_sub(1));
        let names = self.palettes().to_vec();
        let lines: Vec<Line> = names
            .iter()
            .enumerate()
            .skip(start)
            .take(rows)
            .map(|(i, name)| {
                let (marker, style) = self.row_marker(Pane::Palette, i, self.palette_row, cx);
                let radio = ui::radio(cx.doc.config.palette == *name);
                let p = Palette::load(name).unwrap_or_else(palette::terminal);
                let mut spans = vec![marker, Span::styled(format!("{radio} {name:<22}"), style)];
                for t in SWATCHES {
                    spans.push(ui::swatch(p.color(t), depth));
                }
                if p.light {
                    spans.push(Span::styled(" light", ui::muted()));
                }
                Line::from(spans)
            })
            .collect();
        f.render_widget(
            Paragraph::new(lines).block(self.block("Palette", Pane::Palette, cx)),
            area,
        );
    }

    fn render_look(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let c = &cx.doc.config;
        let row = |i: usize, label: &str, value: String, note: &str| {
            let (marker, style) = self.row_marker(Pane::Look, i, self.look_row, cx);
            Line::from(vec![
                marker,
                Span::styled(format!("{label:<13}"), style),
                ui::choice(&value),
                Span::styled(format!("  {note}"), ui::muted()),
            ])
        };
        let tier_note = match c.glyphs {
            GlyphTier::Nerd => "icons from a Nerd Font",
            GlyphTier::Unicode => "◆ ◔ ⎇ ▰, no special font",
            GlyphTier::Ascii => "plain ASCII, any terminal",
        };
        let fill_note = match c.fill {
            FillStyle::Solid => "color behind dark text",
            FillStyle::Tinted => "colored text on dark",
        };
        let (marker, style) = self.row_marker(Pane::Look, 3, self.look_row, cx);
        let lines = vec![
            row(0, "Glyph tier", lower(c.glyphs), tier_note),
            row(1, "Color depth", depth_label(c.color_depth), ""),
            row(2, "Fill", lower(c.fill), fill_note),
            Line::from(vec![
                marker,
                Span::styled(format!("{:<13}", "Font check"), style),
                Span::styled("enter", ui::accent()),
                Span::styled("  show test glyphs", ui::muted()),
            ]),
        ];
        f.render_widget(
            Paragraph::new(lines).block(self.block("Glyphs & color", Pane::Look, cx)),
            area,
        );
    }

    fn render_tokens(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let depth = cx.preview.depth(cx.doc);
        let config = &cx.doc.config;
        let p = effective_palette(config);
        let theme = crate::statusline::theme(config);
        let overrides = &config.palette_overrides;
        let focused = cx.focused && self.pane == Pane::Tokens;
        // The color a token shows as, and where it comes from.
        let resolve = |t: Token| -> (Option<crate::style::Color>, Source) {
            let source = if overrides.contains_key(t.name()) {
                Source::Override
            } else if p.get(t).is_some()
                || !matches!(t, Token::SegBg(_) | Token::SegFg(_) | Token::Separator)
            {
                Source::Palette
            } else {
                Source::Auto
            };
            let color = match (t, &source) {
                (Token::SegBg(c), Source::Auto) => crate::style::skin::segment_colors(c, &theme).0,
                (Token::SegFg(c), Source::Auto) => crate::style::skin::segment_colors(c, &theme).1,
                (Token::Separator, Source::Auto) => p.color(t),
                _ => p.get(t),
            };
            (color, source)
        };
        let mark = |s: &Source| match s {
            Source::Override => Span::styled("*", Style::default().fg(ui::WARN)),
            Source::Auto => Span::styled("·", ui::muted()),
            Source::Palette => Span::raw(" "),
        };
        let on = |col: usize, row: usize| focused && self.tcol == col && self.trow == row;

        let semantic = |r: usize| -> Vec<Span<'static>> {
            let t = TOKENS[r];
            let (color, source) = resolve(t);
            let sel = on(0, r);
            let value = color
                .map(|c| c.notation())
                .unwrap_or_else(|| "default".into());
            vec![
                Span::styled(if sel { ui::SELECTED } else { "  " }, ui::accent()),
                Span::styled(
                    format!("{:<10}", t.name()),
                    if sel { ui::accent() } else { Style::default() },
                ),
                ui::swatch(color, depth),
                Span::raw(format!(" {:<8}", crate::render::width::truncate(&value, 8))),
                mark(&source),
            ]
        };
        let category = |r: usize| -> Vec<Span<'static>> {
            let cat = Category::ALL[r];
            let row_on = focused && self.tcol > 0 && self.trow == r;
            let name = Token::from(cat);
            let name = if cat == Category::Custom {
                "custom"
            } else {
                name.name()
            };
            let mut spans = vec![
                Span::styled(if row_on { ui::SELECTED } else { "  " }, ui::accent()),
                Span::styled(
                    format!("{name:<10}"),
                    if row_on {
                        ui::accent()
                    } else {
                        Style::default()
                    },
                ),
            ];
            for col in 1..=3 {
                let (color, source) = resolve(token_at(col, r));
                let sel = on(col, r);
                let edge = |c: &'static str| Span::styled(if sel { c } else { " " }, ui::accent());
                spans.push(edge("‹"));
                spans.push(ui::swatch(color, depth));
                spans.push(edge("›"));
                spans.push(mark(&source));
                spans.push(Span::raw("  "));
            }
            spans
        };

        let mut lines = vec![Line::from(vec![
            Span::styled(
                format!("  {:<w$}", "Semantic", w = SEMANTIC_CELL),
                ui::muted(),
            ),
            Span::styled(
                format!(
                    "    {:<10}{}",
                    "Category",
                    SEGMENT_COLUMNS.map(|c| format!(" {c:<5}")).concat()
                ),
                ui::muted(),
            ),
        ])];
        for r in 0..Category::ALL.len() {
            let mut spans = if r < SEMANTIC {
                semantic(r)
            } else {
                vec![Span::raw(" ".repeat(SEMANTIC_CELL))]
            };
            spans.push(Span::raw("  "));
            spans.extend(category(r));
            lines.push(Line::from(spans));
        }

        // The selected token in full.
        let t = token_at(self.tcol, self.trow);
        let (color, source) = resolve(t);
        let from = match source {
            Source::Override => "your override",
            Source::Palette if color.is_none() => "terminal default",
            Source::Palette => "from the palette",
            Source::Auto if t == Token::Separator => "auto, as muted",
            Source::Auto => "auto, from the fill style",
        };
        // On the bottom border, so the table keeps every row.
        let detail = Line::from(vec![
            Span::styled(format!(" {} ", t.name()), ui::title()),
            ui::swatch(color, depth),
            Span::styled(
                format!(" {}  ", color.map(|c| c.notation()).unwrap_or_default()),
                Style::default().fg(ratatui::style::Color::Reset),
            ),
            Span::styled(format!("{from} "), ui::muted()),
        ]);
        let overridden = overrides.len();
        let block = self
            .block("Tokens", Pane::Tokens, cx)
            .title_top(
                Line::styled(format!(" {overridden} overridden "), ui::muted()).right_aligned(),
            )
            .title_bottom(detail);
        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    /// Up past the first row of the pane reaches the tab row.
    fn up(row: &mut usize) -> Outcome {
        if *row == 0 {
            return Outcome::Ignored;
        }
        *row -= 1;
        Outcome::Consumed
    }
}

impl Page for StylePage {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let [top, bottom] =
            Layout::vertical([Constraint::Length(9), Constraint::Fill(1)]).areas(area);
        let [skin, look] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .spacing(1)
                .areas(top);
        let [palettes, tokens] =
            Layout::horizontal([Constraint::Percentage(44), Constraint::Percentage(56)])
                .spacing(1)
                .areas(bottom);
        self.render_skin(f, skin, cx);
        self.render_look(f, look, cx);
        self.render_palettes(f, palettes, cx);
        self.render_tokens(f, tokens, cx);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, _: &Env) -> Outcome {
        let forward = matches!(
            key.code,
            KeyCode::Right | KeyCode::Char(' ') | KeyCode::Enter
        );
        match key.code {
            KeyCode::Tab => {
                self.pane = cycle(&PANES, self.pane, true);
                return Outcome::Consumed;
            }
            KeyCode::BackTab => {
                self.pane = cycle(&PANES, self.pane, false);
                return Outcome::Consumed;
            }
            _ => {}
        }
        match self.pane {
            Pane::Skin => match key.code {
                KeyCode::Up => return Self::up(&mut self.skin_row),
                KeyCode::Down => self.skin_row = (self.skin_row + 1).min(SKINS.len() + 2),
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                    if self.skin_row < SKINS.len() =>
                {
                    let skin = SKINS[self.skin_row];
                    doc.edit(|c, _| c.skin = skin);
                }
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                    match self.skin_row - SKINS.len() {
                        0 => doc.edit(|c, _| {
                            let next = cycle(&SEPARATORS, c.separator.as_deref(), forward);
                            c.separator = next.map(str::to_string);
                        }),
                        1 => doc.edit(|c, _| {
                            c.powerline.join = cycle(&Edge::ALL, c.powerline.join, forward)
                        }),
                        _ => doc.edit(|c, _| {
                            c.powerline.end = cycle(&Edge::ALL, c.powerline.end, forward)
                        }),
                    }
                }
                _ => return Outcome::Ignored,
            },
            Pane::Palette => {
                let n = self.palettes().len();
                match key.code {
                    KeyCode::Up => return Self::up(&mut self.palette_row),
                    KeyCode::Down => {
                        self.palette_row = (self.palette_row + 1).min(n.saturating_sub(1))
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        let row = self.palette_row;
                        let name = self.palettes()[row].clone();
                        doc.edit(|c, _| c.palette = name);
                    }
                    _ => return Outcome::Ignored,
                }
            }
            Pane::Look => match key.code {
                KeyCode::Up => return Self::up(&mut self.look_row),
                KeyCode::Down => self.look_row = (self.look_row + 1).min(LOOK_ROWS - 1),
                KeyCode::Enter if self.look_row == LOOK_ROWS - 1 => {
                    return Outcome::Open(Box::new(FontCheck));
                }
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => match self
                    .look_row
                {
                    0 => doc.edit(|c, _| c.glyphs = cycle(&TIERS, c.glyphs, forward)),
                    1 => doc.edit(|c, _| c.color_depth = cycle(&DEPTHS, c.color_depth, forward)),
                    2 => doc.edit(|c, _| c.fill = cycle(&FILLS, c.fill, forward)),
                    _ => return Outcome::Open(Box::new(FontCheck)),
                },
                _ => return Outcome::Ignored,
            },
            Pane::Tokens => {
                let rows = |col: usize| {
                    if col == 0 {
                        SEMANTIC
                    } else {
                        Category::ALL.len()
                    }
                };
                match key.code {
                    KeyCode::Up => return Self::up(&mut self.trow),
                    KeyCode::Down => self.trow = (self.trow + 1).min(rows(self.tcol) - 1),
                    KeyCode::Left => {
                        self.tcol = self.tcol.saturating_sub(1);
                        self.trow = self.trow.min(rows(self.tcol) - 1);
                    }
                    KeyCode::Right => self.tcol = (self.tcol + 1).min(3),
                    KeyCode::Enter => {
                        let t = token_at(self.tcol, self.trow);
                        let mut current = effective_palette(&doc.config).get(t);
                        if current.is_none() {
                            let theme = crate::statusline::theme(&doc.config);
                            current = match t {
                                Token::SegBg(c) => crate::style::skin::segment_colors(c, &theme).0,
                                Token::SegFg(c) => crate::style::skin::segment_colors(c, &theme).1,
                                _ => None,
                            };
                        }
                        return Outcome::Open(Box::new(ColorDialog::new(
                            ColorTarget::Token(t),
                            current,
                        )));
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        let name = token_at(self.tcol, self.trow).name();
                        if !doc.config.palette_overrides.contains_key(name) {
                            return Outcome::Toast(format!("{name} already follows the palette"));
                        }
                        doc.edit(|c, _| {
                            c.palette_overrides.remove(name);
                        });
                        return Outcome::Toast(format!("{name} follows the palette again"));
                    }
                    _ => return Outcome::Ignored,
                }
            }
        }
        Outcome::Consumed
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        match self.pane {
            Pane::Tokens => vec![
                ("tab", "pane"),
                ("↑↓←→", "move"),
                ("enter", "edit color"),
                ("del", "reset"),
            ],
            _ => vec![("tab", "pane"), ("↑↓", "move"), ("enter ←→", "pick")],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::style::SkinKind;
    use crate::tui::app::App;
    use crate::tui::app::tests::{ctrl, env, key, screen};
    use crate::tui::doc::Doc;
    use crate::tui::doc::tests::temp_paths;
    use crate::tui::pages::PageId;
    use ratatui::crossterm::event::KeyCode;

    #[test]
    fn changes_skin_palette_glyphs_and_tokens() {
        let (dir, paths) = temp_paths("style");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Style);
        let s = screen(&mut app, 112, 33);
        for text in [
            "Skin",
            "Palette",
            "Glyphs & color",
            "Tokens",
            "catppuccin-mocha",
            "● plain",
            "0 overridden",
        ] {
            assert!(s.contains(text), "{text}: {s}");
        }

        // Skin: pill.
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.doc.config.skin, SkinKind::Pill);

        // Palette: the second entry.
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.doc.config.palette, "catppuccin-mocha");

        // Glyphs: nerd → unicode.
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.doc.config.glyphs, crate::style::GlyphTier::Unicode);

        // Tokens: override warn through the color dialog, then reset it.
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert!(screen(&mut app, 112, 33).contains("Color · warn"));
        for _ in 0..7 {
            app.handle_key(key(KeyCode::Backspace));
        }
        for c in "#ff8800".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            app.doc
                .config
                .palette_overrides
                .get("warn")
                .map(String::as_str),
            Some("#ff8800")
        );
        assert!(screen(&mut app, 112, 33).contains("1 overridden"));
        app.handle_key(key(KeyCode::Delete));
        assert!(app.doc.config.palette_overrides.is_empty());
        app.handle_key(ctrl('z'));
        assert_eq!(app.doc.config.palette_overrides.len(), 1);

        // A category's segment background, in the table's bg column.
        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Right));
        assert!(screen(&mut app, 112, 33).contains("workspace.bg"));
        app.handle_key(key(KeyCode::Enter));
        for _ in 0..7 {
            app.handle_key(key(KeyCode::Backspace));
        }
        for c in "#112233".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            app.doc
                .config
                .palette_overrides
                .get("workspace.bg")
                .map(String::as_str),
            Some("#112233")
        );

        // Powerline join: flat → arrow, on the skin pane's join row.
        for _ in 0..3 {
            app.handle_key(key(KeyCode::BackTab));
        }
        for _ in 0..3 {
            app.handle_key(key(KeyCode::Down));
        }
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.doc.config.powerline.join, crate::style::Edge::Arrow);
        for _ in 0..3 {
            app.handle_key(key(KeyCode::Tab));
        }

        // Fill: solid → tinted.
        app.handle_key(key(KeyCode::BackTab));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.doc.config.fill, crate::style::FillStyle::Tinted);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_separator_has_a_token_of_its_own() {
        let (dir, paths) = temp_paths("style-sep");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Style);
        for _ in 0..3 {
            app.handle_key(key(KeyCode::Tab));
        }
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Down));
        }
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains(" separator ") && s.contains("auto, as muted"),
            "{s}"
        );
        app.handle_key(key(KeyCode::Enter));
        for _ in 0..7 {
            app.handle_key(key(KeyCode::Backspace));
        }
        for c in "#ff8800".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        let set = &app.doc.config.palette_overrides;
        assert_eq!(set.get("separator").map(String::as_str), Some("#ff8800"));
        assert!(screen(&mut app, 112, 33).contains("your override"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
