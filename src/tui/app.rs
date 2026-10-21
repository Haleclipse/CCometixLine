//! The application: the frame around every page, key routing (top dialog,
//! then the page, then global keys) and the actions only it can take.

use super::dialogs::{self, Action, Dialog, Reply};
use super::doc::{Doc, SaveError};
use super::pages::{Cx, Env, Home, Outcome, Page, PageId, Placeholder};
use super::preview::{self, Preview};
use super::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use std::time::{Duration, Instant};

/// Smallest usable terminal.
pub const MIN_SIZE: (u16, u16) = (80, 24);
const TOAST_FOR: Duration = Duration::from_secs(4);
const WIDTH_STEP: i32 = 4;

struct Toast {
    text: String,
    bad: bool,
    at: Instant,
}

pub struct App {
    pub doc: Doc,
    pub env: Env,
    pub page: PageId,
    pages: Vec<Box<dyn Page>>,
    dialogs: Vec<Box<dyn Dialog>>,
    pub preview: Preview,
    toast: Option<Toast>,
    pub quit: bool,
    /// Columns the preview fits when its width isn't set, from the last draw.
    fit: u16,
}

impl App {
    pub fn new(doc: Doc, env: Env) -> App {
        let pages: Vec<Box<dyn Page>> = vec![
            Box::new(Home::default()),
            Box::new(Placeholder::new(
                "Presets",
                "The preset gallery arrives in a later step of the v2 TUI.",
            )),
            Box::new(Placeholder::new(
                "Layout",
                "The composer arrives in a later step of the v2 TUI.",
            )),
            Box::new(Placeholder::new(
                "Style",
                "Skins, palettes and glyphs arrive in a later step of the v2 TUI.",
            )),
            Box::new(Placeholder::new(
                "Models",
                "The model registry arrives in the next step of the v2 TUI.",
            )),
            Box::new(Placeholder::new(
                "Claude Code",
                "Claude Code integration arrives in a later step of the v2 TUI.",
            )),
        ];
        App {
            doc,
            env,
            page: PageId::Home,
            pages,
            dialogs: Vec::new(),
            preview: Preview::default(),
            toast: None,
            quit: false,
            fit: 96,
        }
    }

    fn page_mut(&mut self) -> &mut dyn Page {
        self.pages[self.page.index()].as_mut()
    }

    pub fn toast(&mut self, text: impl Into<String>, bad: bool) {
        self.toast = Some(Toast {
            text: text.into(),
            bad,
            at: Instant::now(),
        });
    }

    pub fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        if area.width < MIN_SIZE.0 || area.height < MIN_SIZE.1 {
            let msg = format!(
                "ccline needs a terminal of at least {}×{} (this one is {}×{}).",
                MIN_SIZE.0, MIN_SIZE.1, area.width, area.height
            );
            f.render_widget(Paragraph::new(msg), area);
            return;
        }
        let page = self.page;
        let (file, show_preview, keys) = {
            let p = &self.pages[page.index()];
            (p.file(), p.preview(), p.keys())
        };

        let cc = self
            .env
            .cc_version()
            .map(|v| format!("CC {v} · "))
            .unwrap_or_default();
        let star = if self.doc.dirty() { " *" } else { "" };
        let version = env!("CARGO_PKG_VERSION");
        let short = version.split('.').take(2).collect::<Vec<_>>().join(".");
        let frame = Block::bordered()
            .border_style(ui::muted())
            .title(Span::styled(format!(" ccline {short} "), ui::strong()))
            .title_top(Line::from(format!(" {cc}{file}{star} ")).right_aligned());
        let inner = frame.inner(area);
        f.render_widget(frame, area);

        self.fit = inner.width;
        let registry = self.doc.registry();
        let lines = if show_preview {
            self.preview
                .render_lines(&self.doc, &registry, self.env.tz, self.fit)
        } else {
            Vec::new()
        };
        let preview_rows = if show_preview {
            Preview::area_rows(lines.len())
        } else {
            0
        };
        let [tabs, _, body, preview_area, _, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(preview_rows),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        f.render_widget(Paragraph::new(self.tab_line(inner.width)), tabs);
        let buf = f.buffer_mut();
        ui::rule(buf, area.x, tabs.y + 1, area.width, None);
        ui::rule(buf, area.x, footer.y - 1, area.width, None);

        let cx = Cx {
            doc: &self.doc,
            env: &self.env,
        };
        self.pages[page.index()].render(f, body, &cx);

        if show_preview {
            let (title, body, ruler) = preview::split(preview_area, lines.len());
            let heading = self.preview.title(&self.doc, self.fit, area.width - 4);
            ui::rule(f.buffer_mut(), area.x, title.y, area.width, Some(heading));
            f.render_widget(Paragraph::new(lines), body);
            f.render_widget(Paragraph::new(self.preview.ruler(self.fit)), ruler);
        }

        let footer_line = match &self.toast {
            Some(t) if t.at.elapsed() < TOAST_FOR => Line::styled(
                format!(" {}", t.text),
                Style::default().fg(if t.bad { ui::BAD } else { ui::OK }),
            ),
            _ => {
                let mut all = keys;
                all.extend([("^S", "save"), ("?", "help"), ("q", "quit")]);
                let mut l = ui::hints(&all);
                l.spans.insert(0, Span::raw(" "));
                l
            }
        };
        f.render_widget(Paragraph::new(footer_line), footer);

        for d in &mut self.dialogs {
            d.render(f, area, &self.doc);
        }
    }

    fn tab_line(&self, width: u16) -> Line<'static> {
        let mut spans = Vec::new();
        for (i, p) in PageId::ALL.iter().enumerate() {
            let label = format!("{} {}", i + 1, p.title());
            if *p == self.page {
                spans.push(Span::styled(format!("[{label}]"), ui::accent()));
            } else {
                spans.push(Span::raw(format!(" {label} ")));
            }
            spans.push(Span::raw(" "));
        }
        let used: usize = spans.iter().map(|s| s.width()).sum();
        let right = "? help ";
        let gap = (width as usize).saturating_sub(used + right.len());
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(right, ui::muted()));
        Line::from(spans)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if let Some(d) = self.dialogs.last_mut() {
            match d.handle(key, &mut self.doc) {
                Reply::Stay => {}
                Reply::Close => {
                    self.dialogs.pop();
                }
                Reply::Then(action) => {
                    self.dialogs.pop();
                    self.act(action);
                }
            }
            return;
        }
        let outcome = {
            let (doc, env) = (&mut self.doc, &self.env);
            self.pages[self.page.index()].handle(key, doc, env)
        };
        match outcome {
            Outcome::Ignored => self.global(key),
            Outcome::Consumed => {}
            Outcome::Goto(p) => self.page = p,
            Outcome::Quit => self.request_quit(),
        }
    }

    fn global(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => self.request_quit(),
            KeyCode::Char('s') if ctrl => self.act(Action::Save {
                force: false,
                then_quit: false,
            }),
            KeyCode::Char('z') if ctrl => {
                if !self.doc.undo() {
                    self.toast("Nothing to undo", false);
                }
            }
            KeyCode::Char('y') if ctrl => {
                if !self.doc.redo() {
                    self.toast("Nothing to redo", false);
                }
            }
            _ if ctrl => {}
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('?') => {
                let keys = self.page_mut().keys();
                self.dialogs.push(Box::new(dialogs::Help {
                    page: self.page.title(),
                    keys,
                }));
            }
            KeyCode::Char(c @ '1'..='6') => {
                if let Some(p) = PageId::from_digit(c) {
                    self.page = p;
                }
            }
            KeyCode::Esc => self.page = PageId::Home,
            KeyCode::Char('[') => self.preview.next_scenario(false),
            KeyCode::Char(']') => self.preview.next_scenario(true),
            KeyCode::Char('-') => self.preview.resize(-WIDTH_STEP, self.fit),
            KeyCode::Char('+' | '=') => self.preview.resize(WIDTH_STEP, self.fit),
            KeyCode::Char('{') => self.preview.cycle_depth(),
            KeyCode::Char('}') => self.preview.cycle_glyphs(),
            _ => {}
        }
    }

    fn request_quit(&mut self) {
        if self.doc.dirty() {
            self.dialogs.push(Box::new(dialogs::Quit));
        } else {
            self.quit = true;
        }
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            Action::Reload => {
                self.doc.reload();
                self.toast("Reloaded from disk", false);
            }
            Action::Save { force, then_quit } => match self.doc.save(force) {
                Ok(()) => {
                    self.toast("Saved", false);
                    self.quit = then_quit;
                }
                Err(SaveError::Conflict(paths)) => {
                    self.dialogs
                        .push(Box::new(dialogs::Conflict { paths, then_quit }));
                }
                Err(SaveError::Io(e)) => self.toast(format!("Couldn't save: {e}"), true),
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tui::doc::tests::temp_paths;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEventKind;

    pub fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    pub fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new_with_kind(KeyCode::Char(c), KeyModifiers::CONTROL, KeyEventKind::Press)
    }

    pub fn env() -> Env {
        Env {
            tz: chrono::FixedOffset::east_opt(0).unwrap(),
            session: None,
        }
    }

    /// The screen as text, one row per line.
    pub fn screen(app: &mut App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| app.render(f)).unwrap();
        let buf = t.backend().buffer();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn switches_pages_and_draws_the_frame() {
        let (dir, paths) = temp_paths("app-pages");
        let mut app = App::new(Doc::open(paths), env());
        let home = screen(&mut app, 100, 30);
        assert!(home.contains("[1 Home]"), "{home}");
        assert!(home.contains("Health") && home.contains("Start"));
        assert!(home.contains("Preview"));
        app.handle_key(key(KeyCode::Char('5')));
        assert_eq!(app.page, PageId::Models);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.page, PageId::Home);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.page, PageId::Presets);
        assert!(screen(&mut app, 60, 20).contains("at least 80×24"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn quits_through_the_unsaved_prompt() {
        let (dir, paths) = temp_paths("app-quit");
        let mut app = App::new(Doc::open(paths.clone()), env());
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.quit);

        let mut app = App::new(Doc::open(paths.clone()), env());
        app.doc.edit(|c, _| c.palette = "nord".into());
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.quit && !app.dialogs.is_empty());
        assert!(screen(&mut app, 100, 30).contains("1 change since the last save"));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.dialogs.is_empty());
        app.handle_key(ctrl('z'));
        assert!(!app.doc.dirty());
        app.handle_key(ctrl('y'));
        app.handle_key(key(KeyCode::Char('q')));
        app.handle_key(key(KeyCode::Char('s')));
        assert!(app.quit);
        assert!(paths.config.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn asks_before_overwriting_outside_changes() {
        let (dir, paths) = temp_paths("app-conflict");
        let mut app = App::new(Doc::open(paths.clone()), env());
        app.doc.edit(|c, _| c.palette = "nord".into());
        std::fs::write(&paths.config, "version = 2\n").unwrap();
        app.handle_key(ctrl('s'));
        assert!(screen(&mut app, 100, 30).contains("Changed on disk"));
        app.handle_key(key(KeyCode::Char('o')));
        assert!(!app.doc.dirty() && app.dialogs.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn preview_keys_change_the_bar() {
        let (dir, paths) = temp_paths("app-preview");
        let mut app = App::new(Doc::open(paths), env());
        let _ = screen(&mut app, 100, 30);
        app.handle_key(key(KeyCode::Char(']')));
        app.handle_key(key(KeyCode::Char('-')));
        let s = screen(&mut app, 100, 30);
        assert!(s.contains("‹fresh›") && s.contains("‹94›"), "{s}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
