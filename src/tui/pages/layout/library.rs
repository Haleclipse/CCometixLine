//! The library: every metric by category, marked with what it needs.

use crate::metrics::{self, Metric, MetricMeta};
use crate::style::Category;
use crate::tui::preview::Target;
use crate::tui::ui::{self, TextInput};
use crate::version::Version;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

const GROUPS: [(Category, &str); 12] = [
    (Category::Identity, "Identity"),
    (Category::Workspace, "Workspace"),
    (Category::Vcs, "Git"),
    (Category::Context, "Context"),
    (Category::Quota, "Quota"),
    (Category::Cost, "Cost"),
    (Category::Cache, "Cache"),
    (Category::Session, "Session"),
    (Category::Timing, "Timing"),
    (Category::Activity, "Activity"),
    (Category::System, "System"),
    (Category::Custom, "Custom"),
];

/// Whether a metric can go on the lines being edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Avail {
    Yes,
    /// The Claude Code in use is older than this.
    Needs(Version),
    /// It describes a background task, so only the subagent line has it.
    TaskOnly,
}

fn task_only(meta: &MetricMeta) -> bool {
    meta.id.starts_with("task.")
}

pub fn avail(meta: &MetricMeta, target: Target, cc: Option<Version>) -> Avail {
    if let (Some(min), Some(cc)) = (meta.min_cc, cc)
        && cc < min
    {
        return Avail::Needs(min);
    }
    if target == Target::Status && task_only(meta) {
        return Avail::TaskOnly;
    }
    Avail::Yes
}

/// `S` keeps history, `mod` needs the mod, `251+` needs CC 2.1.251, `sub`
/// only on the subagent line.
fn marks(meta: &MetricMeta) -> String {
    let mut m = Vec::new();
    if meta.stateful {
        m.push("S".to_string());
    }
    if meta.needs_mod {
        m.push("mod".into());
    }
    if let Some(v) = meta.min_cc {
        m.push(if (v.major, v.minor) == (2, 1) {
            format!("{}+", v.patch)
        } else {
            format!("{v}+")
        });
    }
    if task_only(meta) {
        m.push("sub".into());
    }
    m.join(" ")
}

#[derive(Clone, Copy)]
pub enum Row {
    Group {
        cat: Category,
        name: &'static str,
        count: usize,
        open: bool,
    },
    Metric(&'static Metric),
}

pub enum Key {
    /// Add the metric, or remove it if the lines have it.
    Toggle(&'static Metric),
    Consumed,
    Ignored,
}

#[derive(Default)]
pub struct Library {
    open: Vec<Category>,
    row: usize,
    offset: usize,
    search: TextInput,
    /// Keys go to the search field.
    pub typing: bool,
}

impl Library {
    fn query(&self) -> String {
        self.search.text().trim().to_lowercase()
    }

    pub fn rows(&self) -> Vec<Row> {
        let q = self.query();
        let mut out = Vec::new();
        for (cat, name) in GROUPS {
            let found: Vec<&'static Metric> = metrics::catalog()
                .iter()
                .filter(|m| m.meta.category == cat)
                .filter(|m| {
                    q.is_empty()
                        || m.meta.id.contains(&q)
                        || m.meta.name.to_lowercase().contains(&q)
                })
                .collect();
            if found.is_empty() {
                continue;
            }
            let open = !q.is_empty() || self.open.contains(&cat);
            out.push(Row::Group {
                cat,
                name,
                count: found.len(),
                open,
            });
            if open {
                out.extend(found.into_iter().map(Row::Metric));
            }
        }
        out
    }

    pub fn selected(&self) -> Option<&'static Metric> {
        match self.rows().get(self.row)? {
            Row::Metric(m) => Some(m),
            Row::Group { .. } => None,
        }
    }

    fn toggle(&mut self, cat: Category, open: bool) {
        self.open.retain(|c| *c != cat);
        if open {
            self.open.push(cat);
        }
    }

    pub fn paste(&mut self, text: &str) -> bool {
        if !self.typing {
            return false;
        }
        self.search.insert_str(text.trim());
        self.row = 0;
        true
    }

    pub fn handle(&mut self, key: KeyEvent) -> Key {
        if self.typing {
            match key.code {
                KeyCode::Esc => {
                    self.search = TextInput::default();
                    self.typing = false;
                    self.row = 0;
                }
                KeyCode::Enter | KeyCode::Down => {
                    self.typing = false;
                    // Land on the first match.
                    self.row = usize::from(!self.rows().is_empty());
                }
                _ => {
                    if self.search.handle(key) {
                        self.row = 0;
                    }
                }
            }
            return Key::Consumed;
        }
        let rows = self.rows();
        let Some(&row) = rows.get(self.row) else {
            self.row = 0;
            return Key::Ignored;
        };
        match key.code {
            KeyCode::Char('/') => self.typing = true,
            KeyCode::Esc if !self.query().is_empty() => {
                self.search = TextInput::default();
                self.row = 0;
            }
            KeyCode::Up if self.row == 0 => return Key::Ignored,
            KeyCode::Up => self.row -= 1,
            KeyCode::Down => self.row = (self.row + 1).min(rows.len() - 1),
            KeyCode::Right => {
                if let Row::Group { cat, .. } = row {
                    self.toggle(cat, true);
                }
            }
            KeyCode::Left => match row {
                Row::Group { cat, .. } => self.toggle(cat, false),
                Row::Metric(m) => {
                    self.toggle(m.meta.category, false);
                    self.row = self
                        .rows()
                        .iter()
                        .position(
                            |r| matches!(r, Row::Group { cat, .. } if *cat == m.meta.category),
                        )
                        .unwrap_or(0);
                }
            },
            KeyCode::Enter | KeyCode::Char(' ') => match row {
                Row::Group { cat, open, .. } => self.toggle(cat, !open),
                Row::Metric(m) => return Key::Toggle(m),
            },
            _ => return Key::Ignored,
        }
        Key::Consumed
    }

    /// Draw within `block`; `used` are the metrics already on the lines.
    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        block: Block<'static>,
        focused: bool,
        avail_of: &dyn Fn(&MetricMeta) -> Avail,
        used: &[String],
    ) {
        let inner = block.inner(area);
        f.render_widget(block, area);
        let [search, list, about] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(4),
        ])
        .areas(inner);
        let width = inner.width as usize;

        let search_line = if self.typing {
            let mut spans = vec![Span::styled("/ ", ui::accent())];
            spans.extend(self.search.line(width.saturating_sub(3), true, "").spans);
            Line::from(spans)
        } else if self.query().is_empty() {
            Line::styled("/ search", ui::muted())
        } else {
            Line::from(vec![
                Span::styled("/ ", ui::muted()),
                Span::styled(self.search.text(), ui::accent()),
                Span::styled("  esc clear", ui::muted()),
            ])
        };
        f.render_widget(Paragraph::new(search_line), search);

        let rows = self.rows();
        self.row = self.row.min(rows.len().saturating_sub(1));
        let height = list.height as usize;
        if self.row < self.offset {
            self.offset = self.row;
        } else if height > 0 && self.row >= self.offset + height {
            self.offset = self.row + 1 - height;
        }
        let lines: Vec<Line> = rows
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(height)
            .map(|(i, row)| {
                let on = focused && !self.typing && i == self.row;
                let marker = Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent());
                let style = if on { ui::accent() } else { Style::default() };
                match row {
                    Row::Group {
                        name, count, open, ..
                    } => {
                        let arrow = if *open { "▼" } else { "▶" };
                        let label = format!("{arrow} {name}");
                        let count = count.to_string();
                        let gap = width.saturating_sub(2 + label.chars().count() + count.len() + 1);
                        Line::from(vec![
                            marker,
                            Span::styled(label, style.add_modifier(ratatui::style::Modifier::BOLD)),
                            Span::raw(" ".repeat(gap)),
                            Span::styled(count, ui::muted()),
                        ])
                    }
                    Row::Metric(m) => {
                        let a = avail_of(&m.meta);
                        let style = if a == Avail::Yes { style } else { ui::muted() };
                        let id = m.meta.id;
                        let dot = if used.iter().any(|u| u == id) {
                            "•"
                        } else {
                            " "
                        };
                        let marks = marks(&m.meta);
                        let gap =
                            width.saturating_sub(4 + id.chars().count() + 1 + marks.len() + 1);
                        Line::from(vec![
                            marker,
                            Span::raw("  "),
                            Span::styled(id, style),
                            Span::styled(dot, ui::accent()),
                            Span::raw(" ".repeat(gap)),
                            Span::styled(marks, ui::muted()),
                        ])
                    }
                }
            })
            .collect();
        f.render_widget(Paragraph::new(lines), list);

        let mut text = Vec::new();
        if let Some(m) = self.selected() {
            let note = match avail_of(&m.meta) {
                Avail::Yes => m.meta.summary.to_string(),
                Avail::Needs(v) => format!("Needs Claude Code {v}"),
                Avail::TaskOnly => "Only on the subagent line (t)".into(),
            };
            text.push(Line::styled(note, ui::muted()));
        }
        let about_rows = about.height as usize;
        let legend = Line::styled("S history · 251+ CC · sub task", ui::muted());
        let mut p = text;
        while p.len() + 1 < about_rows {
            p.push(Line::raw(""));
        }
        p.truncate(about_rows.saturating_sub(1));
        p.push(legend);
        f.render_widget(Paragraph::new(p).wrap(Wrap { trim: true }), about);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn opens_groups_and_searches() {
        let mut lib = Library::default();
        assert_eq!(lib.rows().len(), GROUPS.len());
        lib.handle(key(KeyCode::Enter));
        assert!(matches!(lib.rows()[1], Row::Metric(m) if m.meta.id == "model"));
        lib.handle(key(KeyCode::Down));
        assert!(matches!(lib.handle(key(KeyCode::Enter)), Key::Toggle(m) if m.meta.id == "model"));
        lib.handle(key(KeyCode::Left));
        assert_eq!(lib.rows().len(), GROUPS.len());

        lib.handle(key(KeyCode::Char('/')));
        for c in "spend".chars() {
            lib.handle(key(KeyCode::Char(c)));
        }
        lib.handle(key(KeyCode::Enter));
        assert!(matches!(lib.selected(), Some(m) if m.meta.id == "quota.spend"));
        lib.handle(key(KeyCode::Esc));
        assert_eq!(lib.rows().len(), GROUPS.len());
    }

    #[test]
    fn gates_by_version_and_line() {
        let cache = metrics::find("cache").unwrap();
        let old = Version::parse("2.1.200");
        assert_eq!(
            avail(&cache.meta, Target::Status, old),
            Avail::Needs(Version::parse("2.1.251").unwrap())
        );
        assert_eq!(avail(&cache.meta, Target::Status, None), Avail::Yes);
        let task = metrics::find("task.name").unwrap();
        assert_eq!(avail(&task.meta, Target::Status, None), Avail::TaskOnly);
        assert_eq!(avail(&task.meta, Target::Subagent, None), Avail::Yes);
        assert_eq!(marks(&cache.meta), "251+");
        assert_eq!(marks(&metrics::find("cost.rate").unwrap().meta), "S");
    }
}
