//! E · Editing a model rule or a context modifier.

use super::form::{Field, Form, FormKey};
use super::{Action, Dialog, Reply};
use crate::metrics::fmt::grouped;
use crate::models::{MatchKind, Modifier, Rule, parse_window};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const KINDS: &[&str] = &["contains", "prefix", "exact", "regex"];
/// Recently seen ids listed against the draft.
const SHOWN: usize = 6;
const WIDTH: u16 = 76;

fn kind_index(k: MatchKind) -> usize {
    match k {
        MatchKind::Contains => 0,
        MatchKind::Prefix => 1,
        MatchKind::Exact => 2,
        MatchKind::Regex => 3,
    }
}

fn kind_at(i: usize) -> MatchKind {
    [
        MatchKind::Contains,
        MatchKind::Prefix,
        MatchKind::Exact,
        MatchKind::Regex,
    ][i]
}

/// `kimi-k3-preview` → `Kimi K3 Preview`, a starting point for the name.
pub fn guess_name(id: &str) -> String {
    id.split(['-', '_', '/', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `= 256,000 tokens`, or what's wrong with the text.
fn window_note(text: &str) -> Span<'static> {
    if text.trim().is_empty() {
        return Span::styled("keep Claude Code's", ui::muted());
    }
    match parse_window(text) {
        Some(n) => Span::styled(format!("= {} tokens", grouped(n)), ui::muted()),
        None => Span::styled("use 256k, 1m or 256000", Style::default().fg(ui::BAD)),
    }
}

pub struct RuleDialog {
    /// The rule being edited, or `None` for a new one.
    index: Option<usize>,
    form: Form,
    /// Recently seen ids, with the window Claude Code reported.
    seen: Vec<(String, Option<u64>)>,
    error: Option<String>,
}

impl RuleDialog {
    pub fn edit(index: usize, rule: &Rule, seen: Vec<(String, Option<u64>)>) -> Self {
        let window = rule
            .context_limit
            .map(|n| n.to_string())
            .unwrap_or_default();
        Self::build(
            Some(index),
            kind_index(rule.match_kind),
            &rule.pattern,
            rule.display_name.as_deref().unwrap_or(""),
            &window,
            seen,
        )
    }

    /// A new rule, optionally starting from an id nothing recognized.
    pub fn new(id: Option<&str>, seen: Vec<(String, Option<u64>)>) -> Self {
        let name = id.map(guess_name).unwrap_or_default();
        Self::build(None, 0, id.unwrap_or(""), &name, "", seen)
    }

    fn build(
        index: Option<usize>,
        kind: usize,
        pattern: &str,
        name: &str,
        window: &str,
        seen: Vec<(String, Option<u64>)>,
    ) -> Self {
        let form = Form::new(
            vec![
                Field::choice("Match", KINDS, kind),
                Field::text("Pattern", pattern, "part of the model id"),
                Field::text("Display name", name, "as shown on the line"),
                Field::text("Context", window, "e.g. 256k"),
            ],
            // A new rule from an id starts at the name; otherwise the pattern.
            if index.is_none() && !pattern.is_empty() {
                2
            } else {
                1
            },
        );
        RuleDialog {
            index,
            form,
            seen,
            error: None,
        }
    }

    /// The rule as typed, or why it can't be saved.
    fn draft(&self) -> Result<Rule, String> {
        let f = &self.form.fields;
        let pattern = f[1].value();
        if pattern.is_empty() {
            return Err("The pattern can't be empty.".into());
        }
        let window = f[3].value();
        let context_limit = if window.is_empty() {
            None
        } else {
            Some(parse_window(&window).ok_or("The context isn't a size like 256k.")?)
        };
        let name = f[2].value();
        let rule = Rule {
            pattern,
            display_name: (!name.is_empty()).then_some(name),
            context_limit,
            match_kind: kind_at(f[0].choice),
        };
        if let Some(e) = rule.regex_error() {
            return Err(format!(
                "Not a valid regex: {}",
                e.lines().last().unwrap_or("")
            ));
        }
        Ok(rule)
    }

    fn matches(&self) -> Vec<Line<'static>> {
        let rule = match self.draft() {
            Ok(rule) => rule,
            Err(e) => {
                return vec![Line::styled(
                    format!("   {e}"),
                    Style::default().fg(ui::BAD),
                )];
            }
        };
        if self.seen.is_empty() {
            return vec![Line::styled("   No model ids seen yet.", ui::muted())];
        }
        self.seen
            .iter()
            .take(SHOWN)
            .map(|(id, cc_window)| {
                if rule.matches(id) {
                    let name = rule
                        .display_name
                        .clone()
                        .unwrap_or_else(|| "(name unchanged)".into());
                    let window = rule
                        .context_limit
                        .or(*cc_window)
                        .map(|n| format!(" · {}", grouped(n)))
                        .unwrap_or_default();
                    Line::from(vec![
                        Span::styled("   ✓ ", Style::default().fg(ui::OK)),
                        Span::raw(format!("{id:<28} → {name}{window}")),
                    ])
                } else {
                    Line::styled(format!("   · {id:<28}   not matched"), ui::muted())
                }
            })
            .collect()
    }
}

impl Dialog for RuleDialog {
    fn paste(&mut self, text: &str) -> bool {
        self.error = None;
        self.form.paste(text)
    }

    fn render(&mut self, f: &mut Frame, area: Rect, _: &Doc) {
        let matches = self.matches();
        let height = 10 + matches.len() as u16 + u16::from(self.error.is_some());
        let rect = ui::popup(f, area, WIDTH, height);
        let notes = [
            None,
            None,
            None,
            Some(window_note(&self.form.fields[3].input.text())),
        ];
        let mut text = self.form.lines(WIDTH as usize - 2, &notes);
        text.push(Line::raw(""));
        text.push(Line::raw(" Against ids seen recently:"));
        text.extend(matches);
        if let Some(e) = &self.error {
            text.push(Line::styled(format!(" {e}"), Style::default().fg(ui::BAD)));
        }
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("enter", "save"),
            ("esc", "cancel"),
            ("tab", "next field"),
            ("←→", "match"),
        ]));
        let title = match self.index {
            Some(i) => format!("Edit rule #{}", i + 1),
            None => "New rule".into(),
        };
        f.render_widget(Paragraph::new(text).block(ui::panel(&title)), rect);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        match self.form.handle(key) {
            FormKey::Cancel => Reply::Close,
            FormKey::Submit => match self.draft() {
                Err(e) => {
                    self.error = Some(e);
                    Reply::Stay
                }
                Ok(rule) => {
                    let index = self.index;
                    let mut message = String::new();
                    doc.edit(|_, m| match index {
                        Some(i) => {
                            m.rules[i] = rule;
                            message = format!("Updated rule #{}", i + 1);
                        }
                        None => {
                            // Go before the first rule that would take this
                            // rule's ids, so the new rule applies.
                            let at = (rule.match_kind != MatchKind::Regex)
                                .then(|| m.rules.iter().position(|r| r.matches(&rule.pattern)))
                                .flatten()
                                .unwrap_or(m.rules.len());
                            m.rules.insert(at, rule);
                            message = format!("Added rule #{}", at + 1);
                        }
                    });
                    Reply::Then(Action::Toast(message))
                }
            },
            _ => {
                self.error = None;
                Reply::Stay
            }
        }
    }
}

pub struct ModifierDialog {
    index: Option<usize>,
    form: Form,
    error: Option<String>,
}

impl ModifierDialog {
    pub fn new(index: Option<usize>, modifier: Option<&Modifier>) -> Self {
        let m = modifier.cloned().unwrap_or(Modifier {
            pattern: String::new(),
            display_suffix: String::new(),
            context_limit: None,
        });
        let window = m.context_limit.map(|n| n.to_string()).unwrap_or_default();
        ModifierDialog {
            index,
            form: Form::new(
                vec![
                    Field::text("Pattern", &m.pattern, "e.g. [1m]"),
                    Field::text("Name suffix", &m.display_suffix, "e.g.  1M"),
                    Field::text("Context", &window, "e.g. 1m"),
                ],
                0,
            ),
            error: None,
        }
    }

    fn draft(&self) -> Result<Modifier, String> {
        let f = &self.form.fields;
        let pattern = f[0].value();
        if pattern.is_empty() {
            return Err("The pattern can't be empty.".into());
        }
        let window = f[2].value();
        let context_limit = if window.is_empty() {
            None
        } else {
            Some(parse_window(&window).ok_or("The context isn't a size like 1m.")?)
        };
        // Keep the suffix's leading space: it separates it from the name.
        Ok(Modifier {
            pattern,
            display_suffix: f[1].input.text(),
            context_limit,
        })
    }
}

impl Dialog for ModifierDialog {
    fn paste(&mut self, text: &str) -> bool {
        self.error = None;
        self.form.paste(text)
    }

    fn render(&mut self, f: &mut Frame, area: Rect, _: &Doc) {
        let rect = ui::popup(f, area, WIDTH, 9 + u16::from(self.error.is_some()));
        let notes = [
            None,
            None,
            Some(window_note(&self.form.fields[2].input.text())),
        ];
        let mut text = self.form.lines(WIDTH as usize - 2, &notes);
        text.push(Line::styled(
            " Ids containing the pattern get the suffix and, if set, the context.",
            ui::muted(),
        ));
        if let Some(e) = &self.error {
            text.push(Line::styled(format!(" {e}"), Style::default().fg(ui::BAD)));
        }
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("enter", "save"),
            ("esc", "cancel"),
            ("tab", "next field"),
        ]));
        let title = if self.index.is_some() {
            "Edit modifier"
        } else {
            "New modifier"
        };
        f.render_widget(Paragraph::new(text).block(ui::panel(title)), rect);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        match self.form.handle(key) {
            FormKey::Cancel => Reply::Close,
            FormKey::Submit => match self.draft() {
                Err(e) => {
                    self.error = Some(e);
                    Reply::Stay
                }
                Ok(modifier) => {
                    let index = self.index;
                    doc.edit(|_, m| match index {
                        Some(i) => m.context_modifiers[i] = modifier,
                        None => m.context_modifiers.push(modifier),
                    });
                    Reply::Then(Action::Toast("Saved modifier".into()))
                }
            },
            _ => {
                self.error = None;
                Reply::Stay
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::doc::tests::temp_paths;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typing(d: &mut dyn Dialog, doc: &mut Doc, text: &str) {
        for c in text.chars() {
            d.handle(key(KeyCode::Char(c)), doc);
        }
    }

    #[test]
    fn guesses_names() {
        assert_eq!(guess_name("kimi-k3-preview"), "Kimi K3 Preview");
        assert_eq!(guess_name("deepseek/v4_chat"), "Deepseek V4 Chat");
    }

    #[test]
    fn adds_a_rule_where_it_applies() {
        let (dir, paths) = temp_paths("rule-add");
        let mut doc = Doc::open(paths);
        doc.edit(|_, m| {
            m.rules.push(Rule {
                pattern: "kimi".into(),
                display_name: Some("Kimi".into()),
                context_limit: None,
                match_kind: MatchKind::Contains,
            })
        });
        let seen = vec![("kimi-k3-preview".to_string(), Some(200_000))];
        let mut d = RuleDialog::new(Some("kimi-k3-preview"), seen);
        // Focus starts on the display name, prefilled from the id.
        d.handle(key(KeyCode::Tab), &mut doc);
        typing(&mut d, &mut doc, "256x");
        assert!(matches!(
            d.handle(key(KeyCode::Enter), &mut doc),
            Reply::Stay
        ));
        assert!(d.error.as_deref().unwrap().contains("256k"));
        d.handle(key(KeyCode::Backspace), &mut doc);
        typing(&mut d, &mut doc, "k");
        assert!(matches!(
            d.handle(key(KeyCode::Enter), &mut doc),
            Reply::Then(Action::Toast(_))
        ));
        let r = &doc.models.rules[0];
        assert_eq!(
            (
                r.pattern.as_str(),
                r.display_name.as_deref(),
                r.context_limit
            ),
            ("kimi-k3-preview", Some("Kimi K3 Preview"), Some(256_000))
        );
        assert_eq!(doc.models.rules[1].pattern, "kimi");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_bad_regexes() {
        let (dir, paths) = temp_paths("rule-regex");
        let mut doc = Doc::open(paths);
        let mut d = RuleDialog::new(None, Vec::new());
        typing(&mut d, &mut doc, "(k3");
        d.handle(key(KeyCode::BackTab), &mut doc);
        for _ in 0..3 {
            d.handle(key(KeyCode::Right), &mut doc);
        }
        assert!(matches!(
            d.handle(key(KeyCode::Enter), &mut doc),
            Reply::Stay
        ));
        assert!(d.error.as_deref().unwrap().starts_with("Not a valid regex"));
        assert!(doc.models.rules.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
