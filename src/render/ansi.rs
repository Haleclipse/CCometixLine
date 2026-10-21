//! ANSI output tuned to how Claude Code handles status line stdout.
//!
//! Claude Code trims every line and drops blank ones, and it carries escape
//! sequences seen on earlier lines over to the start of later lines. So each
//! line starts with a reset (which also keeps leading spaces from being
//! trimmed) and ends with one, and lines with no visible text are not emitted.

use super::span::{Line, Span, Style};
use crate::style::color::ColorDepth;

const RESET: &str = "\x1b[0m";

#[derive(Clone, Copy, Debug)]
pub struct EmitOptions {
    pub depth: ColorDepth,
    /// Wrap each line in resets. Off only for plain previews and tests.
    pub guards: bool,
}

impl EmitOptions {
    pub fn plain() -> Self {
        EmitOptions {
            depth: ColorDepth::None,
            guards: false,
        }
    }
}

pub fn emit(lines: &[Line], opts: EmitOptions) -> String {
    lines
        .iter()
        .filter(|line| line.iter().any(|s| !s.text.trim().is_empty()))
        .map(|line| emit_line(line, opts))
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit_line(line: &[Span], opts: EmitOptions) -> String {
    let mut out = String::new();
    if opts.guards {
        out.push_str(RESET);
    }
    let mut current = Style::default();
    for span in line {
        let text = sanitize(&span.text);
        if text.is_empty() {
            continue;
        }
        if opts.depth != ColorDepth::None && span.style != current {
            out.push_str(&sgr(&span.style, opts.depth));
            current = span.style.clone();
        }
        match span.link.as_deref().filter(|l| is_safe_link(l)) {
            Some(link) => {
                out.push_str("\x1b]8;;");
                out.push_str(link);
                out.push_str("\x1b\\");
                out.push_str(&text);
                out.push_str("\x1b]8;;\x1b\\");
            }
            None => out.push_str(&text),
        }
    }
    if opts.guards || current != Style::default() {
        out.push_str(RESET);
    }
    out
}

/// A reset followed by the attributes of `style`, as one SGR sequence.
fn sgr(style: &Style, depth: ColorDepth) -> String {
    let mut params = vec!["0".to_string()];
    if style.bold {
        params.push("1".into());
    }
    if style.dim {
        params.push("2".into());
    }
    if style.italic {
        params.push("3".into());
    }
    if style.underline {
        params.push("4".into());
    }
    if let Some(c) = style.fg.and_then(|c| c.downgrade(depth)) {
        params.push(c.sgr(false));
    }
    if let Some(c) = style.bg.and_then(|c| c.downgrade(depth)) {
        params.push(c.sgr(true));
    }
    format!("\x1b[{}m", params.join(";"))
}

/// Remove control characters so text from stdin, git or config can't inject
/// escape sequences or break the line structure.
pub fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

fn is_safe_link(link: &str) -> bool {
    (link.starts_with("https://") || link.starts_with("http://"))
        && link.len() <= 2048
        && !link.chars().any(|c| c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::color::Color;

    fn guarded(depth: ColorDepth) -> EmitOptions {
        EmitOptions {
            depth,
            guards: true,
        }
    }

    #[test]
    fn guards_every_line_and_skips_blank_ones() {
        let lines = vec![
            vec![Span::raw("  right")],
            vec![Span::raw("   ")],
            vec![Span::raw("next")],
        ];
        assert_eq!(
            emit(&lines, guarded(ColorDepth::TrueColor)),
            "\x1b[0m  right\x1b[0m\n\x1b[0mnext\x1b[0m"
        );
    }

    #[test]
    fn emits_sgr_only_on_style_change() {
        let red = Style::fg(Color::Ansi16(1)).bold(true);
        let line = vec![
            Span::styled("a", red.clone()),
            Span::styled("b", red),
            Span::raw("c"),
        ];
        assert_eq!(
            emit(&[line], guarded(ColorDepth::TrueColor)),
            "\x1b[0m\x1b[0;1;31mab\x1b[0mc\x1b[0m"
        );
    }

    #[test]
    fn applies_color_depth() {
        let lines = [vec![Span::styled("x", Style::fg(Color::Rgb(255, 0, 0)))]];
        assert!(emit(&lines, guarded(ColorDepth::Ansi256)).contains("38;5;196"));
        assert!(emit(&lines, guarded(ColorDepth::Ansi16)).contains("91"));
        assert_eq!(emit(&lines, EmitOptions::plain()), "x");
    }

    #[test]
    fn strips_control_characters() {
        let line = vec![Span::raw("evil\x1b]0;title\x07branch\nnext")];
        assert_eq!(
            emit(&[line], EmitOptions::plain()),
            "evil]0;titlebranchnext"
        );
    }

    #[test]
    fn writes_only_safe_links() {
        let ok = vec![Span::raw("PR").with_link(Some("https://github.com/a/b/pull/1".into()))];
        assert_eq!(
            emit(&[ok], EmitOptions::plain()),
            "\x1b]8;;https://github.com/a/b/pull/1\x1b\\PR\x1b]8;;\x1b\\"
        );
        let bad = vec![Span::raw("x").with_link(Some("file:///etc/passwd".into()))];
        assert_eq!(emit(&[bad], EmitOptions::plain()), "x");
    }
}
