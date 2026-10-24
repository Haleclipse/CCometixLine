//! The status line's span IR as ratatui text, so previews come from the same
//! renderer as the real output with no ANSI round trip.

use crate::render::span;
use crate::style::color::{Color, ColorDepth};
use ratatui::style::{Color as TuiColor, Modifier, Style};
use ratatui::text::{Line, Span};

pub fn color(c: Color, depth: ColorDepth) -> Option<TuiColor> {
    Some(match c.downgrade(depth)? {
        Color::Rgb(r, g, b) => TuiColor::Rgb(r, g, b),
        Color::Ansi256(n) => TuiColor::Indexed(n),
        Color::Ansi16(n) => TuiColor::Indexed(n % 16),
    })
}

fn style(s: &span::Style, depth: ColorDepth) -> Style {
    let mut out = Style::default();
    if let Some(fg) = s.fg.and_then(|c| color(c, depth)) {
        out = out.fg(fg);
    }
    if let Some(bg) = s.bg.and_then(|c| color(c, depth)) {
        out = out.bg(bg);
    }
    let mut m = Modifier::empty();
    m.set(Modifier::BOLD, s.bold);
    m.set(Modifier::DIM, s.dim);
    m.set(Modifier::ITALIC, s.italic);
    m.set(Modifier::UNDERLINED, s.underline);
    out.add_modifier(m)
}

pub fn line(spans: &span::Line, depth: ColorDepth) -> Line<'static> {
    Line::from(
        spans
            .iter()
            .map(|s| Span::styled(s.text.clone(), style(&s.style, depth)))
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_colors_and_attributes_at_depth() {
        let spans = vec![span::Span::styled(
            "62%",
            span::Style::fg(Color::Rgb(250, 179, 135)).bold(true),
        )];
        let full = line(&spans, ColorDepth::TrueColor);
        assert_eq!(full.spans[0].style.fg, Some(TuiColor::Rgb(250, 179, 135)));
        assert!(full.spans[0].style.add_modifier.contains(Modifier::BOLD));
        let none = line(&spans, ColorDepth::None);
        assert_eq!(none.spans[0].style.fg, None);
        assert!(matches!(
            line(&spans, ColorDepth::Ansi256).spans[0].style.fg,
            Some(TuiColor::Indexed(_))
        ));
    }
}
