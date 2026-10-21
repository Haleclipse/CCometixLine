//! Labelled fields for small editing dialogs: text fields and `‹choice›`
//! fields, Tab or ↑↓ to move, ←→ to cycle a choice.

use crate::tui::ui::{self, TextInput};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

const LABEL: usize = 15;

pub enum Kind {
    Text { placeholder: &'static str },
    Choice(&'static [&'static str]),
}

pub struct Field {
    pub label: &'static str,
    pub kind: Kind,
    pub input: TextInput,
    pub choice: usize,
}

impl Field {
    pub fn text(label: &'static str, value: &str, placeholder: &'static str) -> Field {
        Field {
            label,
            kind: Kind::Text { placeholder },
            input: TextInput::new(value),
            choice: 0,
        }
    }

    pub fn choice(label: &'static str, options: &'static [&'static str], selected: usize) -> Field {
        Field {
            label,
            kind: Kind::Choice(options),
            input: TextInput::default(),
            choice: selected.min(options.len().saturating_sub(1)),
        }
    }

    pub fn value(&self) -> String {
        match self.kind {
            Kind::Text { .. } => self.input.text().trim().to_string(),
            Kind::Choice(options) => options[self.choice].to_string(),
        }
    }
}

pub enum FormKey {
    Edited,
    Moved,
    Submit,
    Cancel,
    Ignored,
}

pub struct Form {
    pub fields: Vec<Field>,
    pub focus: usize,
}

impl Form {
    pub fn new(fields: Vec<Field>, focus: usize) -> Form {
        Form { fields, focus }
    }

    /// Paste into the focused field if it takes text.
    pub fn paste(&mut self, text: &str) -> bool {
        let field = &mut self.fields[self.focus];
        let takes = matches!(field.kind, Kind::Text { .. });
        if takes {
            field.input.insert_str(text);
        }
        takes
    }

    pub fn handle(&mut self, key: KeyEvent) -> FormKey {
        let n = self.fields.len();
        match key.code {
            KeyCode::Esc => return FormKey::Cancel,
            KeyCode::Enter => return FormKey::Submit,
            KeyCode::Tab | KeyCode::Down => {
                self.focus = (self.focus + 1) % n;
                return FormKey::Moved;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = (self.focus + n - 1) % n;
                return FormKey::Moved;
            }
            _ => {}
        }
        let field = &mut self.fields[self.focus];
        match field.kind {
            Kind::Choice(options) => match key.code {
                KeyCode::Left => {
                    field.choice = (field.choice + options.len() - 1) % options.len();
                    FormKey::Edited
                }
                KeyCode::Right | KeyCode::Char(' ') => {
                    field.choice = (field.choice + 1) % options.len();
                    FormKey::Edited
                }
                _ => FormKey::Ignored,
            },
            Kind::Text { .. } => {
                if field.input.handle(key) {
                    FormKey::Edited
                } else {
                    FormKey::Ignored
                }
            }
        }
    }

    /// One line per field: the label, the value in `width` columns, then
    /// the field's note, if any.
    pub fn lines(&self, width: usize, notes: &[Option<Span<'static>>]) -> Vec<Line<'static>> {
        self.fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let focused = i == self.focus;
                let label_style = if focused {
                    ui::accent()
                } else {
                    Style::default()
                };
                let mut spans = vec![Span::styled(format!(" {:<LABEL$}", f.label), label_style)];
                let room = width.saturating_sub(LABEL + 2);
                match f.kind {
                    Kind::Text { placeholder } => {
                        let field_width = room.min(28);
                        let line = f.input.line(field_width, focused, placeholder);
                        let used: usize = line.spans.iter().map(|s| s.width()).sum();
                        spans.extend(line.spans);
                        spans.push(Span::raw(" ".repeat(field_width.saturating_sub(used) + 2)));
                    }
                    Kind::Choice(options) => {
                        let style = if focused {
                            ui::accent()
                        } else {
                            Style::default()
                        };
                        spans.push(Span::styled(format!("‹ {} ›", options[f.choice]), style));
                        spans.push(Span::styled(
                            format!("   {}", options.join("·")),
                            ui::muted(),
                        ));
                    }
                }
                if let Some(Some(note)) = notes.get(i) {
                    spans.push(note.clone());
                }
                Line::from(spans)
            })
            .collect()
    }
}
