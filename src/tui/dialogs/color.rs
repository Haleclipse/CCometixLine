//! B · Picking a color: a palette token (for items, so they follow the
//! palette), typed (hex, `256:n`, `16:n`, a name) or from the 16- and
//! 256-color grids.

use super::{Action, Dialog, Reply};
use crate::style::Color;
use crate::style::palette::{TOKENS, Token};
use crate::tui::doc::Doc;
use crate::tui::pages::Cx;
use crate::tui::ui::{self, TextInput};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color as TuiColor, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const WIDTH: u16 = 72;
/// Cells per row of the 256-color grid.
const ROW_256: usize = 32;
/// Tokens per row of the palette grid.
const ROW_TOKENS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Palette,
    Typed,
    Ansi16,
    Ansi256,
}

/// Stores a picked token name or color notation.
pub type Apply = Box<dyn Fn(&mut Doc, String)>;

/// What the picked color is for.
pub enum Target {
    /// A palette override for this token.
    Token(Token),
    /// Something drawn in a token or a fixed color, such as an item.
    Custom { title: String, apply: Apply },
}

impl Target {
    fn title(&self) -> String {
        match self {
            Target::Token(t) => t.name().to_string(),
            Target::Custom { title, .. } => title.clone(),
        }
    }

    fn modes(&self) -> &'static [Mode] {
        match self {
            Target::Token(_) => &[Mode::Typed, Mode::Ansi16, Mode::Ansi256],
            Target::Custom { .. } => &[Mode::Palette, Mode::Typed, Mode::Ansi16, Mode::Ansi256],
        }
    }
}

/// What was picked: a token, or a fixed color.
enum Picked {
    Token(Token),
    Color(Color),
}

impl Picked {
    fn notation(&self) -> String {
        match self {
            Picked::Token(t) => t.name().to_string(),
            Picked::Color(c) => c.notation(),
        }
    }
}

pub struct ColorDialog {
    target: Target,
    mode: Mode,
    input: TextInput,
    token: usize,
    at16: usize,
    at256: usize,
    error: Option<String>,
}

impl ColorDialog {
    pub fn new(target: Target, current: Option<Color>) -> Self {
        let (mode, at16, at256) = match current {
            Some(Color::Ansi16(n)) => (Mode::Ansi16, n as usize, 0),
            Some(Color::Ansi256(n)) => (Mode::Ansi256, 0, n as usize),
            _ => (Mode::Typed, 0, 0),
        };
        ColorDialog {
            target,
            mode,
            input: TextInput::new(&current.map(Color::notation).unwrap_or_default()),
            token: 0,
            at16,
            at256,
            error: None,
        }
    }

    /// Pick for `title`, starting from `current` (a token name or color),
    /// on the palette tab unless it is a fixed color.
    pub fn custom(title: String, current: Option<&str>, apply: Apply) -> Self {
        let target = Target::Custom { title, apply };
        if let Some(c) = current.and_then(Color::parse)
            && current.and_then(Token::parse).is_none()
        {
            return ColorDialog::new(target, Some(c));
        }
        let token = current
            .and_then(Token::parse)
            .and_then(|t| TOKENS.iter().position(|x| *x == t))
            .unwrap_or(0);
        ColorDialog {
            mode: Mode::Palette,
            token,
            ..ColorDialog::new(target, None)
        }
    }

    fn picked(&self) -> Result<Picked, String> {
        match self.mode {
            Mode::Palette => Ok(Picked::Token(TOKENS[self.token])),
            Mode::Typed => Color::parse(&self.input.text())
                .map(Picked::Color)
                .ok_or_else(|| "Use #rrggbb, 256:n, 16:n or a name like bright_blue.".into()),
            Mode::Ansi16 => Ok(Picked::Color(Color::Ansi16(self.at16 as u8))),
            Mode::Ansi256 => Ok(Picked::Color(Color::Ansi256(self.at256 as u8))),
        }
    }

    fn tabs(&self) -> Line<'static> {
        let mut spans = vec![Span::raw(" ")];
        for m in self.target.modes() {
            let label = match m {
                Mode::Palette => "Palette",
                Mode::Typed => "Typed",
                Mode::Ansi16 => "16",
                Mode::Ansi256 => "256",
            };
            let style = if self.mode == *m {
                ui::tab_current()
            } else {
                Style::default()
            };
            spans.push(Span::styled(format!(" {label} "), style));
            spans.push(Span::raw(" "));
        }
        Line::from(spans)
    }

    fn palette_grid(&self, cx: &Cx) -> Vec<Line<'static>> {
        let palette = crate::statusline::theme(&cx.doc.config).palette;
        let depth = cx.preview.depth(cx.doc);
        let mut lines = vec![Line::styled(
            " Tokens follow the palette when you switch it:",
            ui::muted(),
        )];
        for row in TOKENS.chunks(ROW_TOKENS).enumerate() {
            let (r, tokens) = row;
            let mut spans = vec![Span::raw(" ")];
            for (c, t) in tokens.iter().enumerate() {
                let on = r * ROW_TOKENS + c == self.token;
                spans.push(Span::styled(
                    if on { ui::SELECTED } else { "  " },
                    ui::accent(),
                ));
                spans.push(ui::swatch(palette.color(*t), depth));
                spans.push(Span::styled(
                    format!(" {:<12}", t.name()),
                    if on { ui::accent() } else { Style::default() },
                ));
            }
            lines.push(Line::from(spans));
        }
        lines
    }

    /// One grid cell for color `n`. The picked one is filled with the color
    /// and marked `<>` in black or white, whichever reads on it.
    fn cell(n: u8, on: bool, width: usize) -> Span<'static> {
        if !on {
            return Span::styled("█".repeat(width), Style::default().fg(TuiColor::Indexed(n)));
        }
        let (r, g, b) = Color::Ansi256(n).to_rgb();
        let light = 299 * r as u32 + 587 * g as u32 + 114 * b as u32 > 128_000;
        let ink = if light {
            TuiColor::Black
        } else {
            TuiColor::White
        };
        let mark = format!("<{}>", " ".repeat(width.saturating_sub(2)));
        Span::styled(
            mark,
            Style::default()
                .fg(ink)
                .bg(TuiColor::Indexed(n))
                .add_modifier(ratatui::style::Modifier::BOLD),
        )
    }

    /// The terminal's own color for ANSI colors, so names follow its theme.
    fn shown(c: Color) -> TuiColor {
        match c {
            Color::Rgb(r, g, b) => TuiColor::Rgb(r, g, b),
            Color::Ansi256(n) | Color::Ansi16(n) => TuiColor::Indexed(n),
        }
    }

    fn body(&self, cx: &Cx) -> Vec<Line<'static>> {
        match self.mode {
            Mode::Palette => self.palette_grid(cx),
            Mode::Typed => {
                let preview = match Color::parse(&self.input.text()) {
                    Some(c) => Span::styled("██████", Style::default().fg(Self::shown(c))),
                    None => Span::styled("??????", ui::muted()),
                };
                vec![
                    Line::from(
                        [
                            vec![Span::raw(" Color  ")],
                            self.input.line(24, true, "").spans,
                            vec![Span::raw("   "), preview],
                        ]
                        .concat(),
                    ),
                    Line::styled(
                        " #rrggbb, #rgb, 256:n, 16:n or a name like bright_blue",
                        ui::muted(),
                    ),
                ]
            }
            Mode::Ansi16 => {
                let row = |start: usize| {
                    let mut spans = vec![Span::raw(" ")];
                    for i in start..start + 8 {
                        spans.push(Self::cell(i as u8, i == self.at16, 4));
                        spans.push(Span::raw(" "));
                    }
                    Line::from(spans)
                };
                vec![
                    row(0),
                    row(8),
                    Line::styled(
                        format!(" {}", Color::Ansi16(self.at16 as u8).notation()),
                        ui::accent(),
                    ),
                ]
            }
            Mode::Ansi256 => {
                let mut lines: Vec<Line> = (0..256 / ROW_256)
                    .map(|r| {
                        let mut spans = vec![Span::raw(" ")];
                        for i in r * ROW_256..(r + 1) * ROW_256 {
                            spans.push(Self::cell(i as u8, i == self.at256, 2));
                        }
                        Line::from(spans)
                    })
                    .collect();
                lines.push(Line::styled(format!(" 256:{}", self.at256), ui::accent()));
                lines
            }
        }
    }
}

impl Dialog for ColorDialog {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let body = self.body(cx);
        let height = body.len() as u16 + 6 + u16::from(self.error.is_some());
        let rect = ui::popup(f, area, WIDTH, height);
        let mut text = vec![self.tabs(), Line::raw("")];
        text.extend(body);
        if let Some(e) = &self.error {
            text.push(Line::styled(format!(" {e}"), Style::default().fg(ui::BAD)));
        }
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("tab", "mode"),
            ("arrows", "move"),
            ("enter", "pick"),
            ("esc", "cancel"),
        ]));
        f.render_widget(
            Paragraph::new(text).block(ui::panel(&format!("Color · {}", self.target.title()))),
            rect,
        );
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        self.error = None;
        match key.code {
            KeyCode::Esc => return Reply::Close,
            KeyCode::Tab | KeyCode::BackTab => {
                let order = self.target.modes();
                let n = order.len();
                let i = order.iter().position(|m| *m == self.mode).unwrap_or(0);
                let step = if key.code == KeyCode::Tab { 1 } else { n - 1 };
                self.mode = order[(i + step) % n];
                return Reply::Stay;
            }
            KeyCode::Enter => {
                return match self.picked() {
                    Ok(picked) => {
                        let value = picked.notation();
                        match &self.target {
                            Target::Token(token) => {
                                let name = token.name().to_string();
                                let v = value.clone();
                                doc.edit(|c, _| {
                                    c.palette_overrides.insert(name, v);
                                });
                            }
                            Target::Custom { apply, .. } => apply(doc, value.clone()),
                        }
                        Reply::Then(Action::Toast(format!(
                            "{} is now {value}",
                            self.target.title()
                        )))
                    }
                    Err(e) => {
                        self.error = Some(e);
                        Reply::Stay
                    }
                };
            }
            _ => {}
        }
        match self.mode {
            Mode::Palette => {
                self.token = step_grid(self.token, key.code, ROW_TOKENS, TOKENS.len());
            }
            Mode::Typed => {
                self.input.handle(key);
            }
            Mode::Ansi16 => {
                self.at16 = step_grid(self.at16, key.code, 8, 16);
            }
            Mode::Ansi256 => {
                self.at256 = step_grid(self.at256, key.code, ROW_256, 256);
            }
        }
        Reply::Stay
    }

    fn paste(&mut self, text: &str) -> bool {
        self.mode = Mode::Typed;
        self.input = TextInput::new(text.trim());
        true
    }
}

/// Move `at` in a grid `cols` wide holding `n` cells.
fn step_grid(at: usize, key: KeyCode, cols: usize, n: usize) -> usize {
    match key {
        KeyCode::Left => at.saturating_sub(1),
        KeyCode::Right => (at + 1).min(n - 1),
        KeyCode::Up => at.checked_sub(cols).unwrap_or(at),
        KeyCode::Down => (at + cols).min(n - 1),
        _ => at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::doc::tests::temp_paths;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn picks_typed_and_grid_colors() {
        let (dir, paths) = temp_paths("color");
        let mut doc = Doc::open(paths);
        let mut d = ColorDialog::new(Target::Token(Token::Warn), Some(Color::Rgb(249, 226, 175)));
        assert_eq!(d.input.text(), "#f9e2af");
        d.handle(key(KeyCode::Char('x')), &mut doc);
        assert!(matches!(
            d.handle(key(KeyCode::Enter), &mut doc),
            Reply::Stay
        ));
        assert!(d.error.is_some());

        d.handle(key(KeyCode::Tab), &mut doc);
        d.handle(key(KeyCode::Down), &mut doc);
        d.handle(key(KeyCode::Right), &mut doc);
        assert!(matches!(
            d.handle(key(KeyCode::Enter), &mut doc),
            Reply::Then(_)
        ));
        assert_eq!(
            doc.config.palette_overrides.get("warn").map(String::as_str),
            Some("bright_red")
        );

        let mut d = ColorDialog::new(Target::Token(Token::Crit), Some(Color::Ansi256(200)));
        assert!(d.mode == Mode::Ansi256 && d.at256 == 200);
        d.handle(key(KeyCode::Up), &mut doc);
        d.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(
            doc.config.palette_overrides.get("crit").map(String::as_str),
            Some("256:168")
        );
        assert_eq!(step_grid(0, KeyCode::Up, 8, 16), 0);

        // An item's color: a token from the palette tab, or a fixed color.
        let apply: Apply = Box::new(|doc: &mut Doc, value: String| {
            doc.edit(|c, _| c.separator = Some(value));
        });
        let mut d = ColorDialog::custom("git".into(), Some("crit"), apply);
        assert!(d.mode == Mode::Palette && TOKENS[d.token] == Token::Crit);
        d.handle(key(KeyCode::Right), &mut doc);
        d.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(doc.config.separator.as_deref(), Some(TOKENS[3].name()));
        let d = ColorDialog::custom("git".into(), Some("#ff8800"), Box::new(|_, _| {}));
        assert!(d.mode == Mode::Typed && d.input.text() == "#ff8800");
        assert_eq!(step_grid(15, KeyCode::Right, 8, 16), 15);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
