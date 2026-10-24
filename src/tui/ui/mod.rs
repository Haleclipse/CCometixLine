//! Shared pieces of the TUI's own look: colors, rules, key hints, popups,
//! and small widgets.

mod input;

pub use input::TextInput;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear};
use std::time::Duration;

// Named ANSI colors only, so the TUI follows the terminal's theme.
pub const ACCENT: Color = Color::Blue;
pub const MUTED: Color = Color::DarkGray;
pub const OK: Color = Color::Green;
pub const WARN: Color = Color::Yellow;
pub const BAD: Color = Color::Red;

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn strong() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// The current page's tab while the tab row has the keys: a block in the
/// accent color. Reversed rather than painted, so the text takes the
/// terminal's background color whatever the theme.
pub fn tab_on() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

/// The current page's tab while the page has the keys: a block in the
/// terminal's foreground color.
pub fn tab_current() -> Style {
    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

/// The app's name, at the start of the tab row.
pub const BRAND: &str = "CCometixLine";

/// Claude Code's rainbow for `ultrathink`: the colors, then the lighter
/// shades its glint passes over them in. Its truecolor themes use these
/// values, its ANSI themes the named colors below; the name is the one
/// place the TUI draws in fixed colors, and only on a truecolor terminal.
const RAINBOW_RGB: [[(u8, u8, u8); 7]; 2] = [
    [
        (235, 95, 87),
        (245, 139, 87),
        (250, 195, 95),
        (145, 200, 130),
        (130, 170, 220),
        (155, 130, 200),
        (200, 130, 180),
    ],
    [
        (250, 155, 147),
        (255, 185, 137),
        (255, 225, 155),
        (185, 230, 180),
        (180, 205, 240),
        (195, 180, 230),
        (230, 180, 210),
    ],
];
const RAINBOW_ANSI: [[Color; 7]; 2] = [
    [
        Color::Red,
        Color::LightRed,
        Color::Yellow,
        Color::Green,
        Color::Cyan,
        Color::Blue,
        Color::Magenta,
    ],
    [
        Color::LightRed,
        Color::Yellow,
        Color::LightYellow,
        Color::LightGreen,
        Color::LightCyan,
        Color::LightBlue,
        Color::LightMagenta,
    ],
];

/// The glint moves a letter this often...
const GLINT_STEP: Duration = Duration::from_millis(50);
/// ...from this many letters before the name to as many past it.
const GLINT_RUN: i64 = 10;

fn brand_len() -> i64 {
    BRAND.chars().count() as i64
}

/// The name in Claude Code's rainbow, a color per letter, as its prompt
/// draws `ultrathink`; the glint at letter `glint` lights that letter and
/// its neighbours in the lighter shades.
pub fn brand(truecolor: bool, glint: Option<i64>) -> Vec<Span<'static>> {
    BRAND
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let lit = glint.is_some_and(|g| (i as i64 - g).abs() <= 1);
            let shade = usize::from(lit);
            let color = if truecolor {
                let (r, g, b) = RAINBOW_RGB[shade][i % 7];
                Color::Rgb(r, g, b)
            } else {
                RAINBOW_ANSI[shade][i % 7]
            };
            Span::styled(
                c.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        })
        .collect()
}

/// The letter the glint is on `elapsed` into the animation, over and over.
pub fn glint(elapsed: Duration) -> i64 {
    let step = (elapsed.as_millis() / GLINT_STEP.as_millis()) as i64;
    step % (brand_len() + 2 * GLINT_RUN) - GLINT_RUN
}

/// Time until the glint next changes how the name looks: the next step
/// while it is on the name or a step from it, else until it comes round.
pub fn next_glint(elapsed: Duration) -> Duration {
    let cycle = brand_len() + 2 * GLINT_RUN;
    let step_ms = GLINT_STEP.as_millis() as i64;
    let ms = elapsed.as_millis() as i64;
    let step = ms / step_ms;
    let steps = if (-2..=brand_len()).contains(&glint(elapsed)) {
        1
    } else {
        // Round to where the glint lights the first letter.
        (GLINT_RUN - 1 - step % cycle).rem_euclid(cycle)
    };
    Duration::from_millis((steps * step_ms - ms % step_ms) as u64)
}

/// Marker in front of the selected list row.
pub const SELECTED: &str = "❯ ";

/// Titles sit on a border drawn in the muted color; without their own
/// foreground they would take it on.
pub fn title() -> Style {
    Style::default()
        .fg(Color::Reset)
        .add_modifier(Modifier::BOLD)
}

/// A rule across `width` cells at row `y`, joined to the frame's side
/// borders, with an optional title after the first two cells.
pub fn rule(buf: &mut Buffer, x: u16, y: u16, width: u16, title: Option<Line>) {
    if width < 2 {
        return;
    }
    let style = muted();
    buf.set_string(x, y, "├", style);
    for cx in x + 1..x + width - 1 {
        buf.set_string(cx, y, "─", style);
    }
    buf.set_string(x + width - 1, y, "┤", style);
    if let Some(title) = title {
        let room = width.saturating_sub(4);
        buf.set_line(x + 2, y, &title, room);
    }
}

/// ` key desc   key desc` hints for the footer, dialogs and panel
/// borders (where the text needs its own color, not the border's).
pub fn hints(keys: &[(&str, &str)]) -> Line<'static> {
    let text = Style::default().fg(Color::Reset);
    let mut spans = vec![Span::styled(" ", text)];
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", text));
        }
        spans.push(Span::styled(key.to_string(), accent()));
        spans.push(Span::styled(format!(" {desc}"), text));
    }
    spans.push(Span::styled(" ", text));
    Line::from(spans)
}

/// A rounded, titled panel.
pub fn panel(title: &str) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(muted())
        .title(Span::styled(format!(" {title} "), self::title()))
}

/// A `width` × `height` area centered in `area`, cleared for a popup.
pub fn popup(f: &mut Frame, area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    rect
}

/// A `■` in `color` at `depth`; `□` for the terminal's default color.
pub fn swatch(
    color: Option<crate::style::Color>,
    depth: crate::style::color::ColorDepth,
) -> Span<'static> {
    match color.and_then(|c| super::convert::color(c, depth)) {
        Some(c) => Span::styled("■", Style::default().fg(c)),
        None => Span::styled("□", muted()),
    }
}

/// The mark for a choice: `●` picked, `○` not.
pub fn radio(on: bool) -> &'static str {
    if on { "●" } else { "○" }
}

/// `‹value›` for a setting cycled with keys.
pub fn choice(value: &str) -> Span<'static> {
    Span::styled(format!("‹{value}›"), accent())
}

/// `spans` cut to `width` columns, ending in `…` when anything was cut.
pub fn clip(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(Span::width).sum();
    if total <= width {
        return spans;
    }
    let mut out = Vec::new();
    let mut room = width.saturating_sub(1);
    for span in spans {
        let w = span.width();
        if w <= room {
            room -= w;
            out.push(span);
            continue;
        }
        let cut = crate::render::width::truncate(&span.content, room + 1);
        let cut = cut.trim_end_matches('…').to_string();
        out.push(Span::styled(cut, span.style));
        break;
    }
    out.push(Span::styled("…", muted()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_the_name_in_claude_codes_rainbow() {
        let fg = |spans: &[Span], i: usize| spans[i].style.fg;
        let ansi = brand(false, None);
        assert_eq!(ansi.len(), 12);
        assert_eq!(
            (fg(&ansi, 0), fg(&ansi, 1), fg(&ansi, 7)),
            (Some(Color::Red), Some(Color::LightRed), Some(Color::Red))
        );
        assert_eq!(fg(&brand(true, None), 1), Some(Color::Rgb(245, 139, 87)));
        // The glint lights its letter and the ones either side.
        let lit = brand(false, Some(1));
        assert_eq!(
            [0, 1, 2, 3].map(|i| fg(&lit, i)),
            [
                Color::LightRed,
                Color::Yellow,
                Color::LightYellow,
                Color::Green
            ]
            .map(Some)
        );
    }

    #[test]
    fn the_glint_sweeps_past_the_name_and_rests_between() {
        let ms = Duration::from_millis;
        assert_eq!(glint(ms(0)), -10);
        assert_eq!(glint(ms(500)), 0);
        assert_eq!(glint(ms(32 * 50)), -10);
        // Off the name it sleeps until it comes back; on it, a step.
        assert_eq!(next_glint(ms(0)), ms(450));
        assert_eq!(next_glint(ms(20)), ms(430));
        assert_eq!(next_glint(ms(500)), ms(50));
        assert_eq!(next_glint(ms(22 * 50)), ms(50));
        assert_eq!(next_glint(ms(23 * 50)), ms(18 * 50));
    }

    #[test]
    fn clips_to_width() {
        let text = |spans: Vec<Span<'static>>| -> String {
            spans.iter().map(|s| s.content.as_ref()).collect()
        };
        let spans = vec![Span::raw("{pct}%"), Span::raw("[ · {tokens}]")];
        assert_eq!(text(clip(spans.clone(), 40)), "{pct}%[ · {tokens}]");
        assert_eq!(text(clip(spans.clone(), 10)), "{pct}%[ ·…");
        assert_eq!(text(clip(spans, 4)), "{pc…");
    }
}
