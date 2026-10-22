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

/// How filled skins color a segment when the palette doesn't set its
/// colors itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillStyle {
    /// The item's color as the background, text in a deep shade of it.
    #[default]
    Solid,
    /// The item's color as the text, on a dark background tinted with it.
    Tinted,
}

/// Everything about appearance that rendering needs.
#[derive(Clone, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub skin: SkinKind,
    pub glyphs: GlyphTier,
    /// Separator for the plain skin; `None` uses the default.
    pub separator: Option<String>,
    pub fill: FillStyle,
    pub edges: Edges,
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
    /// The text color: on the segment in filled skins, beside the icon in
    /// the others.
    pub fg: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

const PILL_LEFT: &str = "\u{e0b6}";
const PILL_RIGHT: &str = "\u{e0b4}";

/// The shape where powerline segments meet, and where a line's segments
/// begin and end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Arrow,
    Round,
    /// `/`
    Slant,
    /// `\`
    Backslant,
    Flame,
    Pixel,
    /// No glyph: segments meet edge to edge, lines stop square.
    #[default]
    Flat,
}

impl Edge {
    pub const ALL: [Edge; 7] = [
        Edge::Flat,
        Edge::Arrow,
        Edge::Round,
        Edge::Slant,
        Edge::Backslant,
        Edge::Flame,
        Edge::Pixel,
    ];

    /// The glyph pointing right (left side of the line) and left (right
    /// side); `None` for flat.
    pub fn glyphs(self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            Edge::Arrow => ("\u{e0b0}", "\u{e0b2}"),
            Edge::Round => ("\u{e0b4}", "\u{e0b6}"),
            Edge::Slant => ("\u{e0bc}", "\u{e0ba}"),
            Edge::Backslant => ("\u{e0b8}", "\u{e0be}"),
            Edge::Flame => ("\u{e0c0}", "\u{e0c2}"),
            Edge::Pixel => ("\u{e0c4}", "\u{e0c6}"),
            Edge::Flat => return None,
        })
    }
}

/// How powerline segments meet and how a line's run of them ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edges {
    pub join: Edge,
    pub end: Edge,
}

/// No glyphs: segments are told apart by their colors alone.
impl Default for Edges {
    fn default() -> Self {
        Edges {
            join: Edge::Flat,
            end: Edge::Flat,
        }
    }
}

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

/// WCAG relative luminance, 0..1.
fn luminance(c: Color) -> f32 {
    let (r, g, b) = c.to_rgb();
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio, 1..21; 21 when either color is unknown, so
/// nothing gets adjusted against the terminal's own colors.
fn contrast(a: Option<Color>, b: Option<Color>) -> f32 {
    let (Some(a), Some(b)) = (a, b) else {
        return 21.0;
    };
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `from` moved toward `toward` just far enough to read on `bg`, keeping
/// as much of its hue as it can.
fn readable(from: Option<Color>, toward: Option<Color>, bg: Option<Color>) -> Option<Color> {
    if contrast(from, bg) >= READABLE {
        return from;
    }
    (1..=10)
        .filter_map(|i| shade(from, toward, i as f32 / 10.0))
        .find(|c| contrast(Some(*c), bg) >= READABLE)
        .or(toward)
}

/// Text on `bg` when nothing sets it: a deep shade of the background
/// toward the palette's `on_fill`, readable on it.
fn ink_on(bg: Option<Color>, theme: &Theme) -> Option<Color> {
    let on_fill = theme.token(Token::OnFill);
    readable(shade(bg, on_fill, INK).or(on_fill), on_fill, bg)
}

/// The background and text a category's segments get, as `paint` works
/// them out for an item without colors of its own.
pub fn segment_colors(category: Category, theme: &Theme) -> (Option<Color>, Option<Color>) {
    let view = ItemView {
        pieces: &[],
        category,
        severity: Severity::Normal,
        color: None,
        fg: None,
    };
    segment(&view, theme)
}

/// A filled item's background and text. In order: the palette's segment
/// colors for the category (unless the item has its own color), then the
/// fill style; colors worked out rather than set are kept readable. A
/// warning or error turns the text its color when that reads on the
/// background, else the background.
fn segment(item: &ItemView, theme: &Theme) -> (Option<Color>, Option<Color>) {
    let main = base_color(item, theme);
    let (own_bg, own_fg) = match item.color {
        Some(_) => (None, None),
        None => (
            theme.token(Token::SegBg(item.category)),
            theme.token(Token::SegFg(item.category)),
        ),
    };
    let (bg, fg) = match theme.fill {
        FillStyle::Solid => {
            let bg = own_bg.or(main);
            (bg, own_fg.or_else(|| ink_on(bg, theme)))
        }
        FillStyle::Tinted => {
            let surface = theme.token(Token::Bg).or(theme.token(Token::OnFill));
            let bg = own_bg.or_else(|| shade(surface, main, TINT).or(surface));
            let fg = own_fg.or_else(|| readable(main, theme.token(Token::Text), bg));
            (bg, fg)
        }
    };
    let fg = item.fg.or(fg);
    let Some(sev) = severity_token(item.severity)
        .filter(|_| matches!(item.severity, Severity::Warn | Severity::Crit))
        .and_then(|t| theme.token(t))
    else {
        return (bg, fg);
    };
    // Text someone chose stays if it reads; worked-out text follows the
    // new background.
    let chosen = item.fg.or(own_fg);
    if contrast(Some(sev), bg) >= SEVERITY_TEXT {
        (bg, Some(sev))
    } else if chosen.is_some() && contrast(chosen, Some(sev)) >= READABLE {
        (Some(sev), chosen)
    } else {
        (Some(sev), ink_on(Some(sev), theme))
    }
}

/// Share of the text color mixed into a segment's background: for a solid
/// bar's track, and for unfilled graph cells, quiet text and a pace bar's
/// elapsed half.
const TRACK: f32 = 0.22;
const MID: f32 = 0.5;
/// Share of `on_fill` in default text on a solid segment, before making
/// sure it reads.
const INK: f32 = 0.75;
/// Contrast text needs on its background (WCAG AA for normal text).
const READABLE: f32 = 4.5;
/// Contrast a warning color needs to be shown as text on the segment.
const SEVERITY_TEXT: f32 = 3.0;
/// Share of the item's color in a tinted segment's background.
const TINT: f32 = 0.22;

/// `bg` moved toward `ink` by `amount`; `None` unless both are RGB, since
/// terminal colors can't be mixed.
fn shade(bg: Option<Color>, ink: Option<Color>, amount: f32) -> Option<Color> {
    match (bg?, ink?) {
        (bg @ Color::Rgb(..), ink @ Color::Rgb(..)) => {
            Some(lerp(bg.to_rgb(), ink.to_rgb(), amount))
        }
        _ => None,
    }
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// The color at `t` (0..1) along ok → warn → crit; stepped when the palette
/// isn't all RGB.
pub fn gradient(theme: &Theme, t: f32) -> Option<Color> {
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
    let base = base_color(item, theme);
    let sev = severity_token(item.severity).and_then(|t| theme.token(t));
    let muted = theme.token(Token::Muted);
    if !theme.skin().filled() {
        return item
            .pieces
            .iter()
            .map(|p| {
                let fg = match p.role {
                    Role::Icon => base,
                    Role::Text | Role::Strong => sev.or(item.fg).or(base),
                    Role::Muted | Role::Empty => muted,
                    Role::Fill(s) => severity_token(s).and_then(|t| theme.token(t)).or(base),
                    Role::Gradient(t) => gradient(theme, t),
                    Role::Good => theme.token(Token::Ok),
                    Role::Bad => theme.token(Token::Crit),
                    Role::Pace { used, .. } => {
                        if used {
                            sev.or(base)
                        } else {
                            muted
                        }
                    }
                };
                let bg = match p.role {
                    Role::Pace { elapsed: true, .. } => muted,
                    _ => None,
                };
                let style = Style {
                    fg,
                    bg,
                    bold: p.role == Role::Strong,
                    ..Style::default()
                };
                Span::styled(p.text.clone(), style).with_link(p.link.clone())
            })
            .collect();
    }

    // Everything in a segment is a tone between its background and text,
    // so bars and quiet text match whatever colors the segment has. Where
    // colors can't be mixed, quiet text is dimmed and tracks are muted.
    let (bg, fg) = segment(item, theme);
    let track = shade(bg, fg, TRACK).or(muted);
    let mid = shade(bg, fg, MID);
    let style = |fg, bg| Style {
        fg,
        bg,
        ..Style::default()
    };
    item.pieces
        .iter()
        .map(|p| {
            let s = match (p.role, p.inset) {
                // Top half: usage. Bottom half: time passed in the window.
                (Role::Pace { used, elapsed }, _) => style(
                    if used { fg } else { track },
                    if elapsed { mid.or(muted) } else { track },
                ),
                // A solid bar sits in a track; its frame is the track's ends.
                (Role::Muted, true) => style(track, bg),
                (Role::Empty, true) => style(track, track),
                (Role::Gradient(t), true) => style(gradient(theme, t).or(fg), track),
                (_, true) => style(fg, track),
                (Role::Muted | Role::Empty, false) => Style {
                    fg: mid.or(fg),
                    bg,
                    dim: mid.is_none(),
                    ..Style::default()
                },
                (role, false) => Style {
                    fg,
                    bg,
                    bold: role == Role::Strong,
                    ..Style::default()
                },
            };
            Span::styled(p.text.clone(), s).with_link(p.link.clone())
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
                let bg = segment(item, theme).0;
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
            let bgs: Vec<Option<Color>> = items.iter().map(|i| segment(i, theme).0).collect();
            let n = items.len();
            // A glyph drawn in `fg` over `bg`, if the edge has one.
            let edge = |line: &mut Line, edge: Edge, fg: Option<Color>, bg: Option<Color>| {
                if let Some((right, left)) = edge.glyphs() {
                    let glyph = if side == Side::Left { right } else { left };
                    line.push(Span::styled(
                        glyph,
                        Style {
                            fg,
                            bg,
                            ..Style::default()
                        },
                    ));
                }
            };
            for (i, item) in items.iter().enumerate() {
                // The right side's segments point left, toward the gap.
                if side == Side::Right {
                    let (kind, prev) = match i {
                        0 => (theme.edges.end, None),
                        _ => (theme.edges.join, bgs[i - 1]),
                    };
                    edge(&mut line, kind, bgs[i], prev);
                }
                line.push(Span::styled(" ", Style::default().with_bg(bgs[i])));
                line.extend(paint(item, theme));
                line.push(Span::styled(" ", Style::default().with_bg(bgs[i])));
                if side == Side::Left {
                    let (kind, next) = match i + 1 {
                        j if j == n => (theme.edges.end, None),
                        j => (theme.edges.join, bgs[j]),
                    };
                    edge(&mut line, kind, bgs[i], next);
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
            fill: FillStyle::Solid,
            edges: Edges {
                join: Edge::Arrow,
                end: Edge::Arrow,
            },
        }
    }

    fn item(pieces: &[Piece]) -> ItemView<'_> {
        ItemView {
            pieces,
            category: Category::Identity,
            severity: Severity::Normal,
            color: None,
            fg: None,
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
        let right = Edge::Arrow.glyphs().unwrap().0;
        let arrow = line.iter().find(|s| s.text == right).unwrap();
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
        // Red text wouldn't read on a light segment, so the segment turns
        // red, with text worked out for red.
        let pill = theme(SkinKind::Pill, GlyphTier::Nerd);
        let span = &paint(&it, &pill)[0];
        let crit = pill.palette.get(Token::Crit);
        assert_eq!(span.style.bg, crit);
        assert_eq!(span.style.fg, ink_on(crit, &pill));
        assert!(contrast(span.style.fg, crit) >= READABLE);
        // A dark segment keeps its background and turns its text red.
        let tinted = Theme {
            fill: FillStyle::Tinted,
            ..pill
        };
        let span = &paint(&it, &tinted)[0];
        assert_eq!(span.style.fg, crit);
        assert_ne!(span.style.bg, crit);
        // White text on a bright segment: red text wouldn't read, so the
        // segment turns red and keeps its white text.
        let light = Theme {
            palette: palette::builtin("powerline-light").unwrap(),
            ..theme(SkinKind::Powerline, GlyphTier::Nerd)
        };
        it.category = Category::Vcs;
        let span = &paint(&it, &light)[0];
        assert_eq!(span.style.bg, light.palette.get(Token::Crit));
        assert_eq!(span.style.fg, Color::parse("#ffffff"));
    }

    #[test]
    fn segments_take_palette_item_and_fill_style_colors() {
        let p = [Piece::new("x", Role::Text)];
        let pill = theme(SkinKind::Pill, GlyphTier::Nerd);
        let identity = pill.palette.get(Token::Identity);
        let on_fill = pill.palette.get(Token::OnFill);

        // Solid: the category color behind text in a deep shade of it.
        let (bg, fg) = segment(&item(&p), &pill);
        assert_eq!(bg, identity);
        assert_eq!(fg, ink_on(identity, &pill));
        assert_ne!(fg, on_fill);
        assert!(contrast(fg, bg) >= READABLE);

        // Tinted: the category color as text on a dark tinted surface.
        let tinted = Theme {
            fill: FillStyle::Tinted,
            ..pill.clone()
        };
        let (bg, fg) = segment(&item(&p), &tinted);
        assert_eq!(fg, identity);
        assert!(luminance(bg.unwrap()) < luminance(identity.unwrap()));
        assert!(contrast(fg, bg) >= READABLE);

        // The palette's own segment colors win over the fill style.
        let v1 = Theme {
            palette: palette::builtin("powerline-tokyo-night").unwrap(),
            ..pill.clone()
        };
        let (bg, fg) = segment(&item(&p), &v1);
        assert_eq!(bg, Color::parse("#191b29"));
        assert_eq!(fg, Color::parse("#fca7ea"));

        // An item's own text color wins over everything.
        let mut own = item(&p);
        own.fg = Color::parse("#00ff00");
        assert_eq!(segment(&own, &v1).1, own.fg);
        assert_eq!(paint(&own, &v1)[0].style.fg, own.fg);
    }

    #[test]
    fn solid_bars_sit_in_a_track() {
        let pieces = [
            Piece::new("██", Role::Fill(Severity::Normal)).inset(),
            Piece::new("  ", Role::Empty).inset(),
        ];
        let pill = theme(SkinKind::Pill, GlyphTier::Nerd);
        let it = item(&pieces);
        let (bg, fg) = segment(&it, &pill);
        let spans = paint(&it, &pill);
        let track = shade(bg, fg, TRACK);
        assert_eq!((spans[0].style.fg, spans[0].style.bg), (fg, track));
        assert_eq!((spans[1].style.fg, spans[1].style.bg), (track, track));
        // Plain skins ignore the track.
        let plain = theme(SkinKind::Plain, GlyphTier::Nerd);
        assert_eq!(paint(&it, &plain)[1].style.bg, None);
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
