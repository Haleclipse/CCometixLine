//! 4 Style: skin, palette, glyph tier and color depth, and per-token color
//! overrides on top of the palette.

use super::{Cx, Env, Outcome, Page};
use crate::config::{Config, DepthSetting, ItemSpec, LineConfig};
use crate::style::palette::{self, Palette, TOKENS, Token};
use crate::style::{GlyphTier, SkinKind};
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
const SEMANTIC: usize = 8;
/// Columns of one token cell: marker, name, swatch, value, override mark.
const TOKEN_CELL: usize = 2 + 10 + 1 + 15 + 1;

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
    token: usize,
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
                let radio = if cx.doc.config.skin == *skin {
                    "(•)"
                } else {
                    "( )"
                };
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
        lines.push(Line::raw(""));
        let (marker, style) = self.row_marker(Pane::Skin, SKINS.len(), self.skin_row, cx);
        lines.push(Line::from(vec![
            marker,
            Span::styled("Separator  ", style),
            ui::choice(&separator_label(cx.doc.config.separator.as_deref())),
            Span::styled("  plain skin", ui::muted()),
        ]));
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
                let radio = if cx.doc.config.palette == *name {
                    "(•)"
                } else {
                    "( )"
                };
                let p = Palette::load(name).unwrap_or_else(palette::terminal);
                let mut spans = vec![marker, Span::styled(format!("{radio} {name:<18}"), style)];
                for t in SWATCHES {
                    spans.push(ui::swatch(p.color(t), depth));
                    spans.push(Span::raw(" "));
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
        let (marker, style) = self.row_marker(Pane::Look, 2, self.look_row, cx);
        let lines = vec![
            row(0, "Glyph tier", lower(c.glyphs), tier_note),
            row(1, "Color depth", depth_label(c.color_depth), ""),
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
        let p = effective_palette(&cx.doc.config);
        let overridden = cx.doc.config.palette_overrides.len();
        let cell = |i: usize| -> Vec<Span<'static>> {
            let t = TOKENS[i];
            let (marker, style) = self.row_marker(Pane::Tokens, i, self.token, cx);
            let value = p
                .get(t)
                .map(|c| c.notation())
                .unwrap_or_else(|| "default".into());
            let mark = if cx.doc.config.palette_overrides.contains_key(t.name()) {
                "*"
            } else {
                " "
            };
            vec![
                marker,
                Span::styled(format!("{:<10}", t.name()), style),
                ui::swatch(p.get(t), depth),
                Span::raw(format!(" {value:<14}")),
                Span::styled(mark, Style::default().fg(ui::WARN)),
            ]
        };
        let rows = TOKENS.len() - SEMANTIC;
        let mut lines = vec![Line::styled(
            format!("  {:<w$}{}", "Semantic", "Category", w = TOKEN_CELL),
            ui::muted(),
        )];
        for r in 0..rows {
            let mut spans = if r < SEMANTIC {
                cell(r)
            } else {
                vec![Span::raw(" ".repeat(TOKEN_CELL))]
            };
            spans.push(Span::raw("  "));
            spans.extend(cell(SEMANTIC + r));
            lines.push(Line::from(spans));
        }
        let hints = if cx.focused && self.pane == Pane::Tokens {
            ui::hints(&[
                ("enter", "edit"),
                ("del", "palette value"),
                ("←→", "column"),
            ])
        } else {
            Line::raw("")
        };
        let block = self
            .block("Tokens", Pane::Tokens, cx)
            .title_top(
                Line::styled(format!(" {overridden} overridden "), ui::muted()).right_aligned(),
            )
            .title_bottom(hints);
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
            Layout::vertical([Constraint::Length(8), Constraint::Fill(1)]).areas(area);
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
                KeyCode::Down => self.skin_row = (self.skin_row + 1).min(SKINS.len()),
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                    if self.skin_row < SKINS.len() =>
                {
                    let skin = SKINS[self.skin_row];
                    doc.edit(|c, _| c.skin = skin);
                }
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                    doc.edit(|c, _| {
                        let next = cycle(&SEPARATORS, c.separator.as_deref(), forward);
                        c.separator = next.map(str::to_string);
                    });
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
                KeyCode::Down => self.look_row = (self.look_row + 1).min(2),
                KeyCode::Enter if self.look_row == 2 => return Outcome::Open(Box::new(FontCheck)),
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => match self
                    .look_row
                {
                    0 => doc.edit(|c, _| c.glyphs = cycle(&TIERS, c.glyphs, forward)),
                    1 => doc.edit(|c, _| c.color_depth = cycle(&DEPTHS, c.color_depth, forward)),
                    _ => return Outcome::Open(Box::new(FontCheck)),
                },
                _ => return Outcome::Ignored,
            },
            Pane::Tokens => match key.code {
                KeyCode::Up => return Self::up(&mut self.token),
                KeyCode::Down => self.token = (self.token + 1).min(TOKENS.len() - 1),
                KeyCode::Left if self.token >= SEMANTIC => {
                    self.token = (self.token - SEMANTIC).min(SEMANTIC - 1);
                }
                KeyCode::Right if self.token < SEMANTIC => self.token += SEMANTIC,
                KeyCode::Left | KeyCode::Right => {}
                KeyCode::Enter => {
                    let t = TOKENS[self.token];
                    let current = effective_palette(&doc.config).get(t);
                    return Outcome::Open(Box::new(ColorDialog::new(
                        ColorTarget::Token(t),
                        current,
                    )));
                }
                KeyCode::Delete | KeyCode::Backspace => {
                    let name = TOKENS[self.token].name();
                    if !doc.config.palette_overrides.contains_key(name) {
                        return Outcome::Toast(format!("{name} already follows the palette"));
                    }
                    doc.edit(|c, _| {
                        c.palette_overrides.remove(name);
                    });
                    return Outcome::Toast(format!("{name} follows the palette again"));
                }
                _ => return Outcome::Ignored,
            },
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
            "(•) plain",
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
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
