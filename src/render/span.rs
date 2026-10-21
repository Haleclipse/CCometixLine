use super::width::display_width;
use crate::style::color::Color;

/// Text attributes of a span. Colors are concrete; depth is applied on output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Style {
    pub fn fg(color: Color) -> Self {
        Style {
            fg: Some(color),
            ..Style::default()
        }
    }

    pub fn with_bg(mut self, color: Option<Color>) -> Self {
        self.bg = color;
        self
    }

    pub fn bold(mut self, on: bool) -> Self {
        self.bold = on;
        self
    }

    pub fn dim(mut self, on: bool) -> Self {
        self.dim = on;
        self
    }
}

/// A run of text with one style and an optional hyperlink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub link: Option<String>,
}

impl Span {
    pub fn raw(text: impl Into<String>) -> Self {
        Span {
            text: text.into(),
            style: Style::default(),
            link: None,
        }
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Span {
            text: text.into(),
            style,
            link: None,
        }
    }

    pub fn with_link(mut self, link: Option<String>) -> Self {
        self.link = link;
        self
    }

    pub fn width(&self) -> usize {
        display_width(&self.text)
    }
}

/// One output row.
pub type Line = Vec<Span>;

pub fn line_width(line: &[Span]) -> usize {
    line.iter().map(Span::width).sum()
}

/// Plain text of a line, without styling.
pub fn line_text(line: &[Span]) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}
