//! Skins decide how items are framed and joined, and the painter turns
//! widget pieces into styled spans for the active palette.

use super::color::Color;
use super::glyphs::GlyphTier;
use super::palette::{Category, Palette, Token};
use super::piece::{Piece, Role, Severity};
use crate::render::span::{Line, Span, Style};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinKind {
    /// Items separated by a separator string.
    #[default]
    Plain,
    /// Each item in `[ ]`.
    Bracket,
    /// Each item on a filled background with rounded caps (Nerd Font).
    Pill,
    /// Filled backgrounds joined by arrows (Nerd Font).
    Powerline,
}

impl SkinKind {
    /// Skins that need Nerd Font glyphs fall back to brackets without them.
    pub fn effective(self, tier: GlyphTier) -> SkinKind {
        match self {
            SkinKind::Pill | SkinKind::Powerline if tier != GlyphTier::Nerd => SkinKind::Bracket,
            other => other,
        }
    }

    pub fn filled(self) -> bool {
        matches!(self, SkinKind::Pill | SkinKind::Powerline)
    }
}

/// Everything about appearance that rendering needs.
#[derive(Clone, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub skin: SkinKind,
    pub glyphs: GlyphTier,
    /// Separator for the plain skin; `None` uses the default.
    pub separator: Option<String>,
}

impl Theme {
    pub fn skin(&self) -> SkinKind {
        self.skin.effective(self.glyphs)
    }

    fn separator(&self) -> &str {
        match (&self.separator, self.glyphs) {
            (Some(s), _) => s,
            (None, GlyphTier::Ascii) => " | ",
            (None, _) => " │ ",
        }
    }

    fn token(&self, t: Token) -> Option<Color> {
        self.palette.color(t)
    }
}

/// One item as the painter and joiner see it.
#[derive(Clone, Debug)]
pub struct ItemView<'a> {
    pub pieces: &'a [Piece],
    pub category: Category,
    pub severity: Severity,
    /// Replaces the category color.
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

const PILL_LEFT: &str = "\u{e0b6}";
const PILL_RIGHT: &str = "\u{e0b4}";
const ARROW_RIGHT: &str = "\u{e0b0}";
const ARROW_LEFT: &str = "\u{e0b2}";

fn severity_token(sev: Severity) -> Option<Token> {
    match sev {
        Severity::Normal => None,
        Severity::Good => Some(Token::Ok),
        Severity::Warn => Some(Token::Warn),
        Severity::Crit => Some(Token::Crit),
    }
}

fn base_color(item: &ItemView, theme: &Theme) -> Option<Color> {
    item.color.or_else(|| theme.token(item.category.into()))
}

/// Background of a filled item: the severity color when it's a warning,
/// otherwise the item's own color.
fn fill_color(item: &ItemView, theme: &Theme) -> Option<Color> {
    match item.severity {
        Severity::Warn | Severity::Crit => {
            severity_token(item.severity).and_then(|t| theme.token(t))
        }
        _ => base_color(item, theme),
    }
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

fn gradient(theme: &Theme, t: f32) -> Option<Color> {
    let (ok, warn, crit) = (
        theme.token(Token::Ok)?,
        theme.token(Token::Warn)?,
        theme.token(Token::Crit)?,
    );
    let t = t.clamp(0.0, 1.0);
    let all_rgb = [ok, warn, crit].iter().all(|c| matches!(c, Color::Rgb(..)));
    if !all_rgb {
        return Some(if t < 0.5 {
            ok
        } else if t < 0.8 {
            warn
        } else {
            crit
        });
    }
    Some(if t < 0.5 {
        lerp(ok.to_rgb(), warn.to_rgb(), t / 0.5)
    } else {
        lerp(warn.to_rgb(), crit.to_rgb(), (t - 0.5) / 0.5)
    })
}

/// Style each piece of an item.
pub fn paint(item: &ItemView, theme: &Theme) -> Vec<Span> {
    let filled = theme.skin().filled();
    let base = base_color(item, theme);
    let sev = severity_token(item.severity).and_then(|t| theme.token(t));
    let muted = theme.token(Token::Muted);
    let bg = filled.then(|| fill_color(item, theme)).flatten();
    let on_fill = theme.token(Token::OnFill);

    item.pieces
        .iter()
        .map(|p| {
            let style = if filled {
                let dim = matches!(p.role, Role::Empty | Role::Muted);
                Style {
                    fg: on_fill,
                    bg,
                    bold: p.role == Role::Strong,
                    dim,
                    ..Style::default()
                }
            } else {
                let fg = match p.role {
                    Role::Icon => base,
                    Role::Text | Role::Strong => sev.or(base),
                    Role::Muted | Role::Empty => muted,
                    Role::Fill(s) => severity_token(s).and_then(|t| theme.token(t)).or(base),
                    Role::Gradient(t) => gradient(theme, t),
                    Role::Up => theme.token(Token::Ok),
                    Role::Down => theme.token(Token::Crit),
                };
                Style {
                    fg,
                    bold: p.role == Role::Strong,
                    ..Style::default()
                }
            };
            Span::styled(p.text.clone(), style).with_link(p.link.clone())
        })
        .collect()
}

/// Paint and join the items of one side of a line.
pub fn compose(items: &[ItemView], side: Side, theme: &Theme) -> Line {
    let mut line = Line::new();
    let muted = Style {
        fg: theme.token(Token::Muted),
        ..Style::default()
    };
    match theme.skin() {
        SkinKind::Plain => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    line.push(Span::styled(theme.separator(), muted.clone()));
                }
                line.extend(paint(item, theme));
            }
        }
        SkinKind::Bracket => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    line.push(Span::raw(" "));
                }
                line.push(Span::styled("[", muted.clone()));
                line.extend(paint(item, theme));
                line.push(Span::styled("]", muted.clone()));
            }
        }
        SkinKind::Pill => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    line.push(Span::raw(" "));
                }
                let bg = fill_color(item, theme);
                let cap = Style {
                    fg: bg,
                    ..Style::default()
                };
                line.push(Span::styled(PILL_LEFT, cap.clone()));
                line.push(Span::styled(" ", Style::default().with_bg(bg)));
                line.extend(paint(item, theme));
                line.push(Span::styled(" ", Style::default().with_bg(bg)));
                line.push(Span::styled(PILL_RIGHT, cap));
            }
        }
        SkinKind::Powerline => {
            let bgs: Vec<Option<Color>> = items.iter().map(|i| fill_color(i, theme)).collect();
            for (i, item) in items.iter().enumerate() {
                if side == Side::Right {
                    let prev = if i > 0 { bgs[i - 1] } else { None };
                    line.push(Span::styled(
                        ARROW_LEFT,
                        Style {
                            fg: bgs[i],
                            bg: prev,
                            ..Style::default()
                        },
                    ));
                }
                line.push(Span::styled(" ", Style::default().with_bg(bgs[i])));
                line.extend(paint(item, theme));
                line.push(Span::styled(" ", Style::default().with_bg(bgs[i])));
                if side == Side::Left {
                    let next = bgs.get(i + 1).copied().flatten();
                    line.push(Span::styled(
                        ARROW_RIGHT,
                        Style {
                            fg: bgs[i],
                            bg: next,
                            ..Style::default()
                        },
                    ));
                }
            }
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::span::line_text;
    use crate::style::palette;

    fn theme(skin: SkinKind, glyphs: GlyphTier) -> Theme {
        Theme {
            palette: palette::builtin("catppuccin-mocha").unwrap(),
            skin,
            glyphs,
            separator: None,
        }
    }

    fn item(pieces: &[Piece]) -> ItemView<'_> {
        ItemView {
            pieces,
            category: Category::Identity,
            severity: Severity::Normal,
            color: None,
        }
    }

    #[test]
    fn joins_per_skin() {
        let a = [Piece::new("a", Role::Text)];
        let b = [Piece::new("b", Role::Text)];
        let items = [item(&a), item(&b)];
        let text = |skin, tier| line_text(&compose(&items, Side::Left, &theme(skin, tier)));
        assert_eq!(text(SkinKind::Plain, GlyphTier::Nerd), "a │ b");
        assert_eq!(text(SkinKind::Plain, GlyphTier::Ascii), "a | b");
        assert_eq!(text(SkinKind::Bracket, GlyphTier::Nerd), "[a] [b]");
        assert_eq!(
            text(SkinKind::Pill, GlyphTier::Nerd),
            "\u{e0b6} a \u{e0b4} \u{e0b6} b \u{e0b4}"
        );
        assert_eq!(text(SkinKind::Pill, GlyphTier::Unicode), "[a] [b]");
        assert_eq!(
            text(SkinKind::Powerline, GlyphTier::Nerd),
            " a \u{e0b0} b \u{e0b0}"
        );
        let right = line_text(&compose(
            &items,
            Side::Right,
            &theme(SkinKind::Powerline, GlyphTier::Nerd),
        ));
        assert_eq!(right, "\u{e0b2} a \u{e0b2} b ");
    }

    #[test]
    fn powerline_arrows_bridge_backgrounds() {
        let a = [Piece::new("a", Role::Text)];
        let b = [Piece::new("b", Role::Text)];
        let mut second = item(&b);
        second.category = Category::Cost;
        let t = theme(SkinKind::Powerline, GlyphTier::Nerd);
        let line = compose(&[item(&a), second], Side::Left, &t);
        let arrow = line.iter().find(|s| s.text == ARROW_RIGHT).unwrap();
        assert_eq!(arrow.style.fg, t.palette.get(Token::Identity));
        assert_eq!(arrow.style.bg, t.palette.get(Token::Cost));
    }

    #[test]
    fn warnings_recolor_text_or_fill() {
        let p = [Piece::new("91%", Role::Text)];
        let mut it = item(&p);
        it.severity = Severity::Crit;
        let plain = theme(SkinKind::Plain, GlyphTier::Nerd);
        assert_eq!(
            paint(&it, &plain)[0].style.fg,
            plain.palette.get(Token::Crit)
        );
        let pill = theme(SkinKind::Pill, GlyphTier::Nerd);
        let span = &paint(&it, &pill)[0];
        assert_eq!(span.style.bg, pill.palette.get(Token::Crit));
        assert_eq!(span.style.fg, pill.palette.get(Token::OnFill));
    }

    #[test]
    fn gradient_steps_on_ansi_palettes() {
        let t = Theme {
            palette: palette::terminal(),
            ..theme(SkinKind::Plain, GlyphTier::Nerd)
        };
        assert_eq!(gradient(&t, 0.1), Some(Color::Ansi16(2)));
        assert_eq!(gradient(&t, 0.9), Some(Color::Ansi16(1)));
        let rgb = theme(SkinKind::Plain, GlyphTier::Nerd);
        assert!(matches!(gradient(&rgb, 0.3), Some(Color::Rgb(..))));
    }
}
