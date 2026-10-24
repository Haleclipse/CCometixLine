//! A one-line text field with a cursor.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextInput {
    chars: Vec<char>,
    /// Position in `chars`, 0..=len.
    cursor: usize,
}

impl TextInput {
    pub fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        TextInput {
            cursor: chars.len(),
            chars,
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars().filter(|c| !c.is_control()) {
            self.chars.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Apply an editing key. Returns false for keys it doesn't use, which
    /// the caller may handle (Enter, Esc, Tab, arrows up and down).
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.chars.len(),
            KeyCode::Char('u') if ctrl => {
                self.chars.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('k') if ctrl => self.chars.truncate(self.cursor),
            KeyCode::Char('w') if ctrl => {
                let mut start = self.cursor;
                while start > 0 && self.chars[start - 1] == ' ' {
                    start -= 1;
                }
                while start > 0 && self.chars[start - 1] != ' ' {
                    start -= 1;
                }
                self.chars.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Char(c) if !ctrl && !alt => {
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            KeyCode::Backspace | KeyCode::Delete => {}
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            _ => return false,
        }
        true
    }

    /// The field in `width` columns, scrolled so the cursor shows, with the
    /// cursor drawn when `focused` and `placeholder` when empty.
    pub fn line(&self, width: usize, focused: bool, placeholder: &str) -> Line<'static> {
        let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
        if self.chars.is_empty() && !focused {
            return Line::styled(placeholder.to_string(), crate::tui::ui::muted());
        }
        let w = |c: &char| c.width().unwrap_or(0);
        // Start far enough right that text up to the cursor (plus the
        // cursor cell) fits.
        let mut start = 0;
        let budget = width.saturating_sub(1).max(1);
        while self.chars[start..self.cursor].iter().map(w).sum::<usize>() > budget {
            start += 1;
        }
        let mut spans = Vec::new();
        let mut used = 0;
        for (i, c) in self.chars.iter().enumerate().skip(start) {
            used += w(c);
            if used > width {
                break;
            }
            let style = if focused && i == self.cursor {
                cursor_style
            } else {
                Style::default()
            };
            spans.push(Span::styled(c.to_string(), style));
        }
        if focused && self.cursor == self.chars.len() && used < width {
            spans.push(Span::styled(" ", cursor_style));
        }
        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn edits_around_the_cursor() {
        let mut t = TextInput::new("kimi-k3");
        t.handle(k(KeyCode::Left));
        t.handle(k(KeyCode::Left));
        t.handle(k(KeyCode::Char('x')));
        assert_eq!(t.text(), "kimi-xk3");
        t.handle(k(KeyCode::Backspace));
        t.handle(k(KeyCode::Delete));
        assert_eq!(t.text(), "kimi-3");
        t.handle(ctrl('k'));
        assert_eq!(t.text(), "kimi-");
        t.insert_str("k2 0905\n");
        t.handle(ctrl('w'));
        assert_eq!(t.text(), "kimi-k2 ");
        t.handle(ctrl('u'));
        assert!(t.text().is_empty());
        assert!(!t.handle(k(KeyCode::Enter)));
    }

    #[test]
    fn scrolls_to_keep_the_cursor_visible() {
        let t = TextInput::new("abcdefghij");
        let text: String = t
            .line(5, true, "")
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect();
        assert_eq!(text, "ghij ");
        assert_eq!(
            TextInput::default().line(5, false, "id…").spans[0].content,
            "id…"
        );
    }
}
