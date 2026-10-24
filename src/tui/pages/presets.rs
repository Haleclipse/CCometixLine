//! 2 Presets: the gallery. Each card is the preset rendered by the real
//! pipeline for the preview's scenario at the card's width; Enter applies
//! all of it, L only the layout, S only the look. The user's own presets
//! come first: N saves the current setup as one, W replaces one with it, E
//! renames one and X deletes one.

use super::{Cx, Env, Outcome, Page};
use crate::config::Config;
use crate::presets::{self, Entry, Library, Origin, Parts};
use crate::tui::dialogs::{PresetDialog, PresetPrompt, shown_path};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use std::time::UNIX_EPOCH;

const FILTERS: [&str; 8] = [
    "all",
    "mine",
    "minimal",
    "multi-line",
    "powerline",
    "quota",
    "cache",
    "context",
];
/// Narrowest card; the grid fits as many columns as this allows.
const CARD_MIN: u16 = 34;
/// Status line rows a card shows.
const CARD_LINES: usize = 3;
/// Borders, the preview rows and the footer row.
const CARD_HEIGHT: u16 = CARD_LINES as u16 + 3;

#[derive(Default)]
pub struct Presets {
    filter: usize,
    /// Index into the filtered presets.
    selected: usize,
    /// First visible row of cards.
    scroll: usize,
    /// Columns in the last drawn grid, for moving up and down.
    cols: usize,
    /// The library's generation last shown, to follow its changes.
    seen: u64,
    /// Whether the selected card is one of the user's, for the key hints.
    on_mine: bool,
}

/// `cache-aware` → `Cache-aware`.
fn title(name: &str) -> String {
    let mut c = name.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// `edited 2d ago`, from a file's modification time.
fn edited(modified: Option<std::time::SystemTime>, now: i64) -> Option<String> {
    let at = modified?.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let secs = (now - at).max(0);
    Some(match secs {
        0..=59 => "edited just now".into(),
        60..=3_599 => format!("edited {}m ago", secs / 60),
        3_600..=86_399 => format!("edited {}h ago", secs / 3_600),
        _ => format!("edited {}d ago", secs / 86_400),
    })
}

/// The card's last row: where the preset comes from and its shape.
fn footer(entry: &Entry, now: i64) -> String {
    let lines = entry
        .config
        .as_ref()
        .ok()
        .map(|c| plural(c.lines.len(), "line"));
    let parts: Vec<String> = match &entry.origin {
        Origin::Builtin => lines
            .into_iter()
            .chain(entry.tags.iter().map(|t| t.to_string()))
            .collect(),
        Origin::Mine {
            modified, replaces, ..
        } => std::iter::once("mine".to_string())
            .chain(replaces.then(|| "replaces built-in".to_string()))
            .chain(lines)
            .chain(edited(*modified, now))
            .collect(),
    };
    parts.join(" · ")
}

impl Presets {
    fn shown<'a>(&self, lib: &'a Library) -> Vec<&'a Entry> {
        lib.entries()
            .iter()
            .filter(|e| self.filter == 0 || e.tags.contains(&FILTERS[self.filter]))
            .collect()
    }

    /// After the library changes, select the preset the change made,
    /// showing every preset if the filter hides it.
    fn follow(&mut self, lib: &Library) {
        if lib.generation() == self.seen {
            return;
        }
        self.seen = lib.generation();
        let Some(name) = lib.last() else {
            return;
        };
        if !self.shown(lib).iter().any(|e| e.name == name) {
            self.filter = 0;
        }
        if let Some(i) = self.shown(lib).iter().position(|e| e.name == name) {
            self.selected = i;
        }
    }

    fn filter_line(&self) -> Line<'static> {
        let mut spans = vec![Span::styled(" Filter  ", ui::muted())];
        for (i, f) in FILTERS.iter().enumerate() {
            let on = i == self.filter;
            spans.push(Span::styled(
                format!("{} {f}  ", ui::radio(on)),
                if on { ui::accent() } else { Style::default() },
            ));
        }
        Line::from(spans)
    }

    fn card(&self, f: &mut Frame, area: Rect, cx: &Cx, entry: &Entry, selected: bool) {
        let in_use = cx.doc.config.preset.as_deref() == Some(entry.name.as_str());
        let mut block = Block::bordered()
            .border_type(if selected {
                BorderType::Double
            } else {
                BorderType::Rounded
            })
            .border_style(if selected { ui::accent() } else { ui::muted() })
            .title(Span::styled(
                format!(" {} ", title(&entry.name)),
                ui::title(),
            ));
        if in_use {
            let note = match &entry.config {
                Ok(config) if presets::matches(&cx.doc.config, config) => " ✓ in use ",
                _ => " ✓ in use, modified ",
            };
            block =
                block.title_top(Line::styled(note, Style::default().fg(ui::OK)).right_aligned());
        }
        let inner = block.inner(area);
        let mut lines = match &entry.config {
            Ok(config) => {
                let registry = cx.doc.registry();
                cx.preview
                    .render(config, cx.doc, &registry, cx.env.tz, inner.width)
            }
            Err(e) => vec![
                Line::styled("Can't read this file:", Style::default().fg(ui::BAD)),
                Line::styled(e.clone(), ui::muted()),
            ],
        };
        lines.truncate(CARD_LINES);
        lines.resize(CARD_LINES, Line::raw(""));
        lines.push(Line::styled(footer(entry, cx.env.now), ui::muted()));
        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    /// The two rows under the grid: the selected preset's summary and make.
    fn info(&self, entry: Option<&Entry>) -> Vec<Line<'static>> {
        let Some(e) = entry else {
            let empty = if FILTERS[self.filter] == "mine" {
                " No presets of yours yet. N saves the current setup as one."
            } else {
                " No preset has this tag."
            };
            return vec![Line::styled(empty, ui::muted())];
        };
        let summary = if e.summary.is_empty() {
            Span::styled("— your preset", ui::muted())
        } else {
            Span::raw(format!("— {}", e.summary))
        };
        let file = match &e.origin {
            Origin::Mine { path, .. } => format!(" · {}", shown_path(path)),
            Origin::Builtin => String::new(),
        };
        let make = match &e.config {
            Ok(config) => Line::styled(
                format!(
                    " {} · skin {} · palette {} · glyphs {}{file}",
                    plural(config.lines.len(), "line"),
                    format!("{:?}", config.skin).to_lowercase(),
                    config.palette,
                    format!("{:?}", config.glyphs).to_lowercase(),
                ),
                ui::muted(),
            ),
            Err(err) => Line::styled(format!(" {err}{file}"), Style::default().fg(ui::BAD)),
        };
        vec![
            Line::from(vec![
                Span::styled(format!(" {} ", title(&e.name)), ui::title()),
                summary,
            ]),
            make,
        ]
    }
}

impl Page for Presets {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let lib = &cx.doc.presets;
        self.follow(lib);
        let shown = self.shown(lib);
        self.selected = self.selected.min(shown.len().saturating_sub(1));
        self.on_mine = shown.get(self.selected).is_some_and(|e| e.is_mine());
        let [filter, grid, info] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(area);
        f.render_widget(Paragraph::new(self.filter_line()), filter);

        let cols = (grid.width / CARD_MIN).max(1) as usize;
        self.cols = cols;
        let visible_rows = (grid.height / CARD_HEIGHT).max(1) as usize;
        let row = self.selected / cols;
        if row < self.scroll {
            self.scroll = row;
        } else if row >= self.scroll + visible_rows {
            self.scroll = row + 1 - visible_rows;
        }
        let card_width = grid.width / cols as u16;
        for (i, entry) in shown
            .iter()
            .enumerate()
            .skip(self.scroll * cols)
            .take(visible_rows * cols)
        {
            let r = i / cols - self.scroll;
            let c = i % cols;
            let rect = Rect {
                x: grid.x + c as u16 * card_width,
                y: grid.y + r as u16 * CARD_HEIGHT,
                width: if c + 1 == cols {
                    grid.width - c as u16 * card_width
                } else {
                    card_width - 1
                },
                height: CARD_HEIGHT,
            };
            self.card(f, rect, cx, entry, cx.focused && i == self.selected);
        }
        let info_lines = self.info(shown.get(self.selected).copied());
        f.render_widget(Paragraph::new(info_lines), info);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, _: &Env) -> Outcome {
        self.follow(&doc.presets);
        let shown = self.shown(&doc.presets);
        let n = shown.len();
        let cols = self.cols.max(1);
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let k = FILTERS.len();
                self.filter = (self.filter + if key.code == KeyCode::Tab { 1 } else { k - 1 }) % k;
                self.selected = 0;
                self.scroll = 0;
            }
            KeyCode::Left if !self.selected.is_multiple_of(cols) => self.selected -= 1,
            KeyCode::Right if self.selected % cols + 1 < cols && self.selected + 1 < n => {
                self.selected += 1
            }
            KeyCode::Left | KeyCode::Right => {}
            KeyCode::Up if self.selected < cols => return Outcome::Ignored,
            KeyCode::Up => self.selected -= cols,
            KeyCode::Down if self.selected + cols < n => self.selected += cols,
            // From a short last row, Down still lands on the last card.
            KeyCode::Down if self.selected / cols < (n.saturating_sub(1)) / cols => {
                self.selected = n - 1
            }
            KeyCode::Down => {}
            KeyCode::Enter | KeyCode::Char('l' | 'L' | 's' | 'S') if n > 0 => {
                let entry = shown[self.selected];
                let config = match &entry.config {
                    Ok(c) => c.clone(),
                    Err(e) => return Outcome::Toast(format!("Can't use {}: {e}", entry.name)),
                };
                let name = entry.name.clone();
                let (parts, what) = match key.code {
                    KeyCode::Enter => (Parts::All, ""),
                    KeyCode::Char('l' | 'L') => (Parts::Layout, "the layout of "),
                    _ => (Parts::Style, "the look of "),
                };
                doc.edit(|c, _| *c = presets::apply(c, &config, parts));
                return Outcome::Toast(format!("Applied {what}{name} (^Z undoes)"));
            }
            KeyCode::Char('n' | 'N') => return Outcome::Open(Box::new(PresetDialog::new())),
            KeyCode::Char('w' | 'W' | 'e' | 'E' | 'x' | 'X') | KeyCode::Delete if n > 0 => {
                let entry = shown[self.selected];
                if !entry.is_mine() {
                    return Outcome::Toast(
                        "Built-in presets stay as they are; N saves yours as a new one".into(),
                    );
                }
                let name = entry.name.clone();
                return Outcome::Open(match key.code {
                    KeyCode::Char('w' | 'W') => Box::new(PresetPrompt::replace(name)),
                    KeyCode::Char('e' | 'E') => Box::new(PresetDialog::edit(&name, &entry.summary)),
                    _ => Box::new(PresetPrompt::delete(name)),
                });
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Consumed
    }

    /// The preset under the cursor, applied as Enter would apply it.
    fn previewing(&self, doc: &Doc) -> Option<(String, Config)> {
        let entry = self.shown(&doc.presets).get(self.selected).copied()?;
        let config = entry.config.as_ref().ok()?;
        Some((
            format!("{} · enter applies", entry.name),
            presets::apply(&doc.config, config, Parts::All),
        ))
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        let mut keys = vec![
            ("←→↑↓", "choose"),
            ("enter", "apply"),
            ("L", "layout only"),
            ("S", "style only"),
            ("N", "save as preset"),
        ];
        if self.on_mine {
            keys.extend([("W", "replace"), ("E", "rename"), ("X", "delete")]);
        }
        keys.push(("tab", "filter"));
        keys
    }
}

#[cfg(test)]
mod tests {
    use crate::tui::app::App;
    use crate::tui::app::tests::{ctrl, env, key, screen};
    use crate::tui::doc::Doc;
    use crate::tui::doc::tests::temp_paths;
    use crate::tui::pages::PageId;
    use ratatui::crossterm::event::KeyCode;

    #[test]
    fn browses_filters_and_applies_presets() {
        let (dir, paths) = temp_paths("presets");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Presets);
        let s = screen(&mut app, 112, 33);
        for name in [
            "Cometix",
            "Minimal",
            "Dashboard",
            "Budget",
            "Cache-aware",
            "Focus",
        ] {
            assert!(s.contains(name), "{name}: {s}");
        }
        assert!(s.contains("✓ in use"), "the default config is cometix: {s}");
        assert!(s.contains("Classic look"));

        // Right then down: from cometix to the card under minimal. The
        // preview bar shows the card under the cursor, nothing applied yet.
        app.handle_key(key(KeyCode::Right));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("Model, context, cache and cost"));
        assert!(s.contains("Preview · minimal · enter applies"), "{s}");
        assert!(!app.doc.dirty());
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Char('s')));
        assert_eq!(app.doc.config.lines.len(), 1, "style only keeps the layout");
        assert_ne!(app.doc.config.palette, "terminal");
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("✓ in use, modified"), "{s}");
        app.handle_key(ctrl('z'));
        assert!(!app.doc.dirty());

        // Filter down to cache presets and apply all of it.
        for _ in 0..6 {
            app.handle_key(key(KeyCode::Tab));
        }
        let s = screen(&mut app, 112, 33);
        // Every preset shows the cache; the tag marks the one about it.
        assert!(
            s.contains("● cache")
                && s.contains("Cache-aware")
                && !s.contains("Classic look")
                && !s.contains("Minimal"),
            "{s}"
        );
        assert!(s.contains("Preview · cache-aware"), "{s}");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.doc.config.preset.as_deref(), Some("cache-aware"));
        assert_eq!(app.doc.config.lines.len(), 2);
        assert!(screen(&mut app, 112, 33).contains("Applied cache-aware"));

        // Up from the first row reaches the tab row.
        app.handle_key(key(KeyCode::Up));
        assert!(app.tabs_focused);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn typed(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.handle_key(key(KeyCode::Char(ch)));
        }
    }

    #[test]
    fn saves_renames_replaces_and_deletes_your_presets() {
        let (dir, paths) = temp_paths("presets-mine");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Presets);
        app.handle_key(key(KeyCode::Tab));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("No presets of yours yet. N saves"), "{s}");

        // Built-ins don't change.
        app.handle_key(key(KeyCode::BackTab));
        app.handle_key(key(KeyCode::Char('x')));
        assert!(screen(&mut app, 112, 33).contains("Built-in presets stay as they are"));

        // Apply a look, then save the setup under a name typed loosely.
        app.handle_key(key(KeyCode::Char('s')));
        app.handle_key(key(KeyCode::Char('n')));
        typed(&mut app, "cometix");
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("Save as preset") && s.contains("built-in preset"),
            "{s}"
        );
        for _ in 0.."cometix".len() {
            app.handle_key(key(KeyCode::Backspace));
        }
        typed(&mut app, "Work Laptop");
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("presets/work-laptop.toml"), "{s}");
        app.handle_key(key(KeyCode::Tab));
        typed(&mut app, "Quota first");
        app.handle_key(key(KeyCode::Enter));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("Saved preset work-laptop"), "{s}");
        assert!(dir.join("presets/work-laptop.toml").exists());
        assert_eq!(app.doc.config.preset.as_deref(), Some("work-laptop"));
        // It comes first, selected, in use, with its summary below.
        assert!(s.contains("Work-laptop") && s.contains("✓ in use"), "{s}");
        assert!(
            s.contains("— Quota first") && s.contains("mine · 1 line"),
            "{s}"
        );
        app.handle_key(key(KeyCode::Char('?')));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("replace") && s.contains("rename"), "{s}");
        app.handle_key(key(KeyCode::Esc));

        // Rename it; the config follows the new name.
        app.handle_key(key(KeyCode::Char('e')));
        for _ in 0.."work-laptop".len() {
            app.handle_key(key(KeyCode::Backspace));
        }
        typed(&mut app, "office");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.doc.config.preset.as_deref(), Some("office"));
        assert!(!dir.join("presets/work-laptop.toml").exists());
        assert!(screen(&mut app, 112, 33).contains("Renamed work-laptop to office"));

        // Change the setup: the card says so, and W takes the change in.
        app.doc.edit(|c, _| c.palette = "nord".into());
        assert!(screen(&mut app, 112, 33).contains("✓ in use, modified"));
        app.handle_key(key(KeyCode::Char('w')));
        assert!(screen(&mut app, 112, 33).contains("Replace office with your current setup?"));
        app.handle_key(key(KeyCode::Char('y')));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("✓ in use ") && !s.contains("modified"), "{s}");
        let file = std::fs::read_to_string(dir.join("presets/office.toml")).unwrap();
        assert!(file.contains("palette = \"nord\""), "{file}");

        // Delete it after asking.
        app.handle_key(key(KeyCode::Char('x')));
        assert!(screen(&mut app, 112, 33).contains("Delete office?"));
        app.handle_key(key(KeyCode::Char('n')));
        assert!(dir.join("presets/office.toml").exists());
        app.handle_key(key(KeyCode::Delete));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(!dir.join("presets/office.toml").exists());
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("Deleted office") && !s.contains("Office"), "{s}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shows_a_preset_file_it_cannot_read() {
        let (dir, paths) = temp_paths("presets-broken");
        std::fs::create_dir_all(dir.join("presets")).unwrap();
        std::fs::write(dir.join("presets/broken.toml"), "skin = 3\n").unwrap();
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Presets);
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("Broken") && s.contains("Can't read this file"),
            "{s}"
        );
        app.handle_key(key(KeyCode::Enter));
        assert!(screen(&mut app, 112, 33).contains("Can't use broken"));
        assert!(!app.doc.dirty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scrolls_on_a_small_terminal() {
        let (dir, paths) = temp_paths("presets-small");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Presets);
        for _ in 0..8 {
            app.handle_key(key(KeyCode::Down));
        }
        let s = screen(&mut app, 80, 24);
        assert!(
            s.contains("Powerline"),
            "the last card scrolls into view: {s}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
