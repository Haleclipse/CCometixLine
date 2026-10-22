//! 2 Presets: the gallery. Each card is the preset rendered by the real
//! pipeline for the preview's scenario at the card's width; Enter applies
//! all of it, L only the layout, S only the look.

use super::{Cx, Env, Outcome, Page};
use crate::config::Config;
use crate::presets::{self, PRESETS, Parts, Preset};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

const FILTERS: [&str; 7] = [
    "all",
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
    /// Parsed presets, by name.
    configs: Vec<(&'static str, Config)>,
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

impl Presets {
    fn shown(&self) -> Vec<&'static Preset> {
        PRESETS
            .iter()
            .filter(|p| self.filter == 0 || p.tags.contains(&FILTERS[self.filter]))
            .collect()
    }

    fn config(&mut self, name: &'static str) -> &Config {
        if !self.configs.iter().any(|(n, _)| *n == name) {
            let c = presets::get(name).expect("listed presets exist");
            self.configs.push((name, c));
        }
        &self
            .configs
            .iter()
            .find(|(n, _)| *n == name)
            .expect("just added")
            .1
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

    fn card(
        &mut self,
        f: &mut Frame,
        area: Rect,
        cx: &Cx,
        preset: &'static Preset,
        selected: bool,
    ) {
        let config = self.config(preset.name).clone();
        let in_use = cx.doc.config.preset.as_deref() == Some(preset.name);
        let mut block = Block::bordered()
            .border_type(if selected {
                BorderType::Double
            } else {
                BorderType::Rounded
            })
            .border_style(if selected { ui::accent() } else { ui::muted() })
            .title(Span::styled(
                format!(" {} ", title(preset.name)),
                ui::title(),
            ));
        if in_use {
            let note = if presets::matches(&cx.doc.config, &config) {
                " ✓ in use "
            } else {
                " ✓ in use, modified "
            };
            block =
                block.title_top(Line::styled(note, Style::default().fg(ui::OK)).right_aligned());
        }
        let inner = block.inner(area);
        let registry = cx.doc.registry();
        let mut lines = cx
            .preview
            .render(&config, cx.doc, &registry, cx.env.tz, inner.width);
        lines.truncate(CARD_LINES);
        lines.resize(CARD_LINES, Line::raw(""));
        let mut footer = vec![plural(config.lines.len(), "line")];
        footer.extend(preset.tags.iter().map(|t| t.to_string()));
        lines.push(Line::styled(footer.join(" · "), ui::muted()));
        f.render_widget(Paragraph::new(lines).block(block), area);
    }
}

impl Page for Presets {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let shown = self.shown();
        self.selected = self.selected.min(shown.len().saturating_sub(1));
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
        for (i, preset) in shown
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
            self.card(f, rect, cx, preset, cx.focused && i == self.selected);
        }

        if let Some(p) = shown.get(self.selected) {
            let config = self.config(p.name).clone();
            let text = vec![
                Line::from(vec![
                    Span::styled(format!(" {} ", title(p.name)), ui::title()),
                    Span::raw(format!("— {}", p.summary)),
                ]),
                Line::styled(
                    format!(
                        " {} · skin {} · palette {} · glyphs {}",
                        plural(config.lines.len(), "line"),
                        format!("{:?}", config.skin).to_lowercase(),
                        config.palette,
                        format!("{:?}", config.glyphs).to_lowercase(),
                    ),
                    ui::muted(),
                ),
            ];
            f.render_widget(Paragraph::new(text), info);
        } else {
            f.render_widget(
                Paragraph::new(Line::styled(" No preset has this tag.", ui::muted())),
                info,
            );
        }
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, _: &Env) -> Outcome {
        let shown = self.shown();
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
                let preset = shown[self.selected];
                let (parts, what) = match key.code {
                    KeyCode::Enter => (Parts::All, ""),
                    KeyCode::Char('l' | 'L') => (Parts::Layout, "the layout of "),
                    _ => (Parts::Style, "the look of "),
                };
                let config = self.config(preset.name).clone();
                doc.edit(|c, _| *c = presets::apply(c, &config, parts));
                return Outcome::Toast(format!("Applied {what}{} (^Z undoes)", preset.name));
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Consumed
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("←→↑↓", "choose"),
            ("enter", "apply"),
            ("L", "layout only"),
            ("S", "style only"),
            ("tab", "filter"),
        ]
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
            "Classic",
            "Minimal",
            "Dashboard",
            "Budget",
            "Cache-aware",
            "Focus",
        ] {
            assert!(s.contains(name), "{name}: {s}");
        }
        assert!(s.contains("✓ in use"), "the default config is classic: {s}");
        assert!(s.contains("The v1 look"));

        // Right then down: from classic to the card under minimal.
        app.handle_key(key(KeyCode::Right));
        assert!(screen(&mut app, 112, 33).contains("Model, context and cost"));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Char('s')));
        assert_eq!(app.doc.config.lines.len(), 1, "style only keeps the layout");
        assert_ne!(app.doc.config.palette, "terminal");
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("✓ in use, modified"), "{s}");
        app.handle_key(ctrl('z'));
        assert!(!app.doc.dirty());

        // Filter down to cache presets and apply all of it.
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Tab));
        }
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("● cache") && s.contains("Cache-aware") && !s.contains("Classic"),
            "{s}"
        );
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.doc.config.preset.as_deref(), Some("cache-aware"));
        assert_eq!(app.doc.config.lines.len(), 2);
        assert!(screen(&mut app, 112, 33).contains("Applied cache-aware"));

        // Up from the first row reaches the tab row.
        app.handle_key(key(KeyCode::Up));
        assert!(app.tabs_focused);
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
