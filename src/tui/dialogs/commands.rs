//! G · The command palette (^P): every action, found by typing a few
//! letters of it.

use super::{Action, Dialog, Reply};
use crate::config::{Config, ItemSpec, LineConfig};
use crate::metrics;
use crate::presets::{self, PRESETS, Parts};
use crate::scenarios::SCENARIOS;
use crate::style::palette;
use crate::style::{GlyphTier, SkinKind};
use crate::tui::doc::Doc;
use crate::tui::pages::{Cx, PageId};
use crate::tui::ui::{self, TextInput};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const WIDTH: u16 = 70;
/// Matches shown at once.
const SHOWN: usize = 10;

type Edit = Box<dyn Fn(&mut Config)>;

enum Run {
    App(Action),
    Edit(Edit),
}

struct Command {
    group: &'static str,
    label: String,
    run: Run,
}

fn app(group: &'static str, label: impl Into<String>, action: Action) -> Command {
    Command {
        group,
        label: label.into(),
        run: Run::App(action),
    }
}

fn edit(
    group: &'static str,
    label: impl Into<String>,
    f: impl Fn(&mut Config) + 'static,
) -> Command {
    Command {
        group,
        label: label.into(),
        run: Run::Edit(Box::new(f)),
    }
}

fn lower<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}").to_lowercase()
}

fn all() -> Vec<Command> {
    let mut out = Vec::new();
    for p in PageId::ALL {
        out.push(app("Go", format!("to {}", p.title()), Action::Goto(p)));
    }
    out.push(app(
        "File",
        "save",
        Action::Save {
            force: false,
            then_quit: false,
        },
    ));
    out.push(app("File", "undo", Action::Undo));
    out.push(app("File", "redo", Action::Redo));
    out.push(app("File", "reload from disk", Action::Reload));
    out.push(app("File", "quit", Action::AskQuit));
    out.push(app("Setup", "run the first-run setup", Action::Setup));
    out.push(app("Help", "keys for this page", Action::Help));
    for p in PRESETS {
        let name = p.name;
        out.push(edit("Preset", format!("apply {name}"), move |c| {
            if let Some(preset) = presets::get(name) {
                *c = presets::apply(c, &preset, Parts::All);
            }
        }));
    }
    for name in palette::available() {
        let label = name.clone();
        out.push(edit("Palette", label, move |c| c.palette = name.clone()));
    }
    for skin in [
        SkinKind::Plain,
        SkinKind::Bracket,
        SkinKind::Pill,
        SkinKind::Powerline,
    ] {
        out.push(edit("Skin", lower(skin), move |c| c.skin = skin));
    }
    for tier in [GlyphTier::Nerd, GlyphTier::Unicode, GlyphTier::Ascii] {
        out.push(edit("Glyphs", lower(tier), move |c| c.glyphs = tier));
    }
    for m in metrics::catalog() {
        let id = m.meta.id;
        if id.starts_with("task.") {
            continue;
        }
        // Like the library: on the lines once, or not at all.
        out.push(edit("Layout", format!("add or remove {id}"), move |c| {
            let had = c
                .lines
                .iter()
                .flat_map(|l| l.left.iter().chain(&l.right))
                .any(|s| s.config().metric == id);
            if had && id != "cmd" {
                for line in &mut c.lines {
                    line.left.retain(|s| s.config().metric != id);
                    line.right.retain(|s| s.config().metric != id);
                }
                return;
            }
            if c.lines.is_empty() {
                c.lines.push(LineConfig::default());
            }
            if let Some(line) = c.lines.last_mut() {
                line.left.push(ItemSpec::Id(id.to_string()));
            }
        }));
    }
    for (i, s) in SCENARIOS.iter().enumerate() {
        out.push(app(
            "Preview",
            format!("scenario {}", s.name),
            Action::Scenario(i),
        ));
    }
    out.push(app("Preview", "replay your last session", Action::Replay));
    out
}

/// Whether `query`'s letters appear in `text` in order, and how well:
/// lower is better, `None` for no match.
fn score(query: &str, text: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    if let Some(at) = text.find(query) {
        return Some(at);
    }
    let mut chars = text.char_indices();
    let mut first = None;
    let mut last = 0;
    for q in query.chars() {
        let (i, _) = chars.find(|(_, c)| *c == q)?;
        first.get_or_insert(i);
        last = i;
    }
    // Spread-out matches rank after tight ones and after substrings.
    Some(100 + last - first.unwrap_or(0))
}

pub struct Commands {
    input: TextInput,
    row: usize,
    all: Vec<Command>,
}

impl Default for Commands {
    fn default() -> Self {
        Commands {
            input: TextInput::default(),
            row: 0,
            all: all(),
        }
    }
}

impl Commands {
    fn matches(&self) -> Vec<&Command> {
        let q = self.input.text().trim().to_lowercase();
        let mut found: Vec<(usize, &Command)> = self
            .all
            .iter()
            .filter_map(|c| {
                let text = format!("{} {}", c.group, c.label).to_lowercase();
                Some((score(&q, &text)?, c))
            })
            .collect();
        found.sort_by_key(|(s, _)| *s);
        found.into_iter().map(|(_, c)| c).collect()
    }
}

impl Dialog for Commands {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Cx) {
        let count = self.matches().len();
        self.row = self.row.min(count.saturating_sub(1));
        let matches = self.matches();
        let start = self.row.saturating_sub(SHOWN - 1);
        let rect = ui::popup(f, area, WIDTH, SHOWN as u16 + 6);
        let mut text = vec![
            Line::from(
                [
                    vec![Span::styled(" > ", ui::accent())],
                    self.input
                        .line(WIDTH as usize - 6, true, "type to search")
                        .spans,
                ]
                .concat(),
            ),
            Line::raw(""),
        ];
        for (i, c) in matches.iter().enumerate().skip(start).take(SHOWN) {
            let on = i == self.row;
            text.push(Line::from(vec![
                Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
                Span::styled(format!("{:<9}", c.group), ui::muted()),
                Span::styled(
                    c.label.clone(),
                    if on { ui::accent() } else { Style::default() },
                ),
            ]));
        }
        if matches.is_empty() {
            text.push(Line::styled("  nothing matches", ui::muted()));
        }
        while text.len() < SHOWN + 2 {
            text.push(Line::raw(""));
        }
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("↑↓", "choose"),
            ("enter", "run"),
            ("esc", "close"),
        ]));
        f.render_widget(Paragraph::new(text).block(ui::panel("Commands")), rect);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        match key.code {
            KeyCode::Esc => return Reply::Close,
            KeyCode::Up => self.row = self.row.saturating_sub(1),
            KeyCode::Down => self.row += 1,
            KeyCode::Enter => {
                let Some(c) = self.matches().into_iter().nth(self.row) else {
                    return Reply::Stay;
                };
                return match &c.run {
                    Run::App(action) => Reply::Then(action.clone()),
                    Run::Edit(f) => {
                        doc.edit(|config, _| f(config));
                        Reply::Then(Action::Toast(format!("{} {}", c.group, c.label)))
                    }
                };
            }
            _ => {
                if self.input.handle(key) {
                    self.row = 0;
                }
            }
        }
        Reply::Stay
    }

    fn paste(&mut self, text: &str) -> bool {
        self.input.insert_str(text.trim());
        self.row = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::doc::tests::temp_paths;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn finds_and_runs_commands() {
        assert_eq!(score("pill", "skin pill"), Some(5));
        assert!(score("sknp", "skin pill").is_some_and(|s| s >= 100));
        assert_eq!(score("zz", "skin pill"), None);

        let (dir, paths) = temp_paths("commands");
        let mut doc = Doc::open(paths);
        let mut c = Commands::default();
        for ch in "skin pill".chars() {
            c.handle(key(KeyCode::Char(ch)), &mut doc);
        }
        assert_eq!(c.matches()[0].label, "pill");
        c.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(doc.config.skin, SkinKind::Pill);

        let mut c = Commands::default();
        for ch in "go lay".chars() {
            c.handle(key(KeyCode::Char(ch)), &mut doc);
        }
        assert!(matches!(
            c.handle(key(KeyCode::Enter), &mut doc),
            Reply::Then(Action::Goto(PageId::Layout))
        ));

        let run = |doc: &mut Doc| {
            let mut c = Commands::default();
            for ch in "remove cost.today".chars() {
                c.handle(key(KeyCode::Char(ch)), doc);
            }
            c.handle(key(KeyCode::Enter), doc);
        };
        run(&mut doc);
        let last = doc.config.lines.last().unwrap();
        assert_eq!(last.left.last(), Some(&ItemSpec::Id("cost.today".into())));
        // Run again, it comes off: never twice.
        run(&mut doc);
        let all: Vec<_> = doc
            .config
            .lines
            .iter()
            .flat_map(|l| l.left.iter())
            .collect();
        assert!(!all.contains(&&ItemSpec::Id("cost.today".into())));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
