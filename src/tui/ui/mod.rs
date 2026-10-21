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

/// The app's name at the start of the tab row.
pub fn brand() -> Style {
    accent().add_modifier(Modifier::BOLD)
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

/// `‹value›` for a setting cycled with keys.
pub fn choice(value: &str) -> Span<'static> {
    Span::styled(format!("‹{value}›"), accent())
}
