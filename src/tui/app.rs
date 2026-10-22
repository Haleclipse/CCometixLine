//! The application: the frame around every page, key routing (top dialog,
//! then the page, then global keys) and the actions only it can take.

use super::dialogs::{self, Action, Dialog, Reply};
use super::doc::{Doc, SaveError};
use super::pages::{
    ClaudePage, Cx, Env, Home, LayoutPage, Models, Outcome, Page, PageId, Presets, StylePage,
};
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
    /// Whether keys go to the tab row (←→ switch pages) rather than the page.
    pub tabs_focused: bool,
}

impl App {
    pub fn new(doc: Doc, env: Env) -> App {
        let mut preview = Preview::default();
        preview.set_replay(
            env.session
                .as_ref()
                .and_then(super::preview::Replay::from_session),
        );
        let pages: Vec<Box<dyn Page>> = vec![
            Box::new(Home::default()),
            Box::new(Presets::default()),
            Box::new(LayoutPage::default()),
            Box::new(StylePage::default()),
            Box::new(Models::default()),
            Box::new(ClaudePage::default()),
        ];
        App {
            doc,
            env,
            page: PageId::Home,
            pages,
            dialogs: Vec::new(),
            preview,
            toast: None,
            quit: false,
            tabs_focused: true,
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
        let (file, show_preview, keys, target) = {
            let p = &self.pages[page.index()];
            (p.file(), p.preview(), p.keys(), p.target())
        };

        let cc = self
            .env
            .cc_version()
            .map(|v| format!("CC {v} · "))
            .unwrap_or_default();
        let star = if self.doc.dirty() { "* " } else { "" };
        let frame = Block::bordered().border_style(ui::muted()).title_top(
            Line::from(vec![
                Span::styled(
                    format!(" {cc}{file} "),
                    Style::default().fg(ratatui::style::Color::Reset),
                ),
                Span::styled(star, Style::default().fg(ui::WARN)),
            ])
            .right_aligned(),
        );
        let inner = frame.inner(area);
        f.render_widget(frame, area);

        self.preview.fit = inner.width;
        let registry = self.doc.registry();
        let lines = if show_preview {
            self.preview
                .render_lines(&self.doc, &registry, self.env.tz, target)
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
            preview: &self.preview,
            focused: !self.tabs_focused,
        };
        self.pages[page.index()].render(f, body, &cx);

        if show_preview {
            let (title, body, ruler) = preview::split(preview_area, lines.len());
            let heading = self.preview.title(&self.doc, target, area.width - 4);
            ui::rule(f.buffer_mut(), area.x, title.y, area.width, Some(heading));
            f.render_widget(Paragraph::new(lines), body);
            f.render_widget(Paragraph::new(self.preview.ruler()), ruler);
        }

        let footer_line = match &self.toast {
            Some(t) if t.at.elapsed() < TOAST_FOR => Line::styled(
                format!(" {}", t.text),
                Style::default().fg(if t.bad { ui::BAD } else { ui::OK }),
            ),
            _ => {
                let own = if self.tabs_focused {
                    vec![("←→", "switch page"), ("↓", "into the page")]
                } else {
                    keys
                };
                footer_hints(own, !self.tabs_focused, footer.width as usize)
            }
        };
        f.render_widget(Paragraph::new(footer_line), footer);

        for d in &mut self.dialogs {
            d.render(f, area, &cx);
        }
    }

    fn tab_line(&self, width: u16) -> Line<'static> {
        let mut spans = vec![Span::styled(" ccline", ui::brand()), Span::raw("  ")];
        for p in PageId::ALL {
            let label = format!(" {} ", p.title());
            if p == self.page {
                // Filled while the tab row has the keys.
                let style = if self.tabs_focused {
                    ui::tab_on()
                } else {
                    ui::tab_current()
                };
                spans.push(Span::styled(label, style));
            } else {
                spans.push(Span::raw(label));
            }
            spans.push(Span::raw(" "));
        }
        let used: usize = spans.iter().map(|s| s.width()).sum();
        let right = "^P commands   ? help ";
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
        // Control keys are the app's, whatever has focus.
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            self.global(key);
            return;
        }
        if self.tabs_focused {
            let letter = matches!(key.code, KeyCode::Char(c) if c.is_ascii_lowercase())
                && key.modifiers.is_empty();
            match key.code {
                // The page shows as soon as its tab is picked.
                KeyCode::Left => self.step_page(false),
                KeyCode::Right => self.step_page(true),
                KeyCode::Down | KeyCode::Enter => self.tabs_focused = false,
                _ if letter && self.pages[self.page.index()].shortcuts() => {
                    let outcome = {
                        let (doc, env) = (&mut self.doc, &self.env);
                        self.pages[self.page.index()].handle(key, doc, env)
                    };
                    self.outcome(outcome, key);
                }
                _ => self.global(key),
            }
            return;
        }
        let outcome = {
            let (doc, env) = (&mut self.doc, &self.env);
            self.pages[self.page.index()].handle(key, doc, env)
        };
        match outcome {
            // Up past the top of the page, or Esc, reaches the tab row.
            Outcome::Ignored if matches!(key.code, KeyCode::Up | KeyCode::Esc) => {
                self.tabs_focused = true;
            }
            other => self.outcome(other, key),
        }
    }

    fn outcome(&mut self, outcome: Outcome, key: KeyEvent) {
        match outcome {
            Outcome::Ignored => self.global(key),
            Outcome::Consumed => {}
            Outcome::Goto(p) => self.open_page(p),
            Outcome::Open(d) => self.dialogs.push(d),
            Outcome::Toast(t) => self.toast(t, false),
            Outcome::Copy(text) => self.copy(&text),
            Outcome::Quit => self.request_quit(),
        }
    }

    /// Bracketed paste goes to the top dialog, else the page.
    pub fn paste(&mut self, text: &str) {
        let taken = match self.dialogs.last_mut() {
            Some(d) => d.paste(text),
            None => self.pages[self.page.index()].paste(text),
        };
        if taken {
            self.tabs_focused = false;
        } else {
            self.toast("Nothing here takes pasted text", false);
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
            KeyCode::Char('z') if ctrl => self.act(Action::Undo),
            KeyCode::Char('y') if ctrl => self.act(Action::Redo),
            KeyCode::Char('p') if ctrl => self.dialogs.push(Box::new(dialogs::Commands::default())),
            _ if ctrl => {}
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('?') => self.act(Action::Help),
            KeyCode::Char('[') => self.preview.next_scenario(false),
            KeyCode::Char(']') => self.preview.next_scenario(true),
            KeyCode::Char('-') => self.preview.resize(-WIDTH_STEP),
            KeyCode::Char('+' | '=') => self.preview.resize(WIDTH_STEP),
            KeyCode::Char('R') => self.act(Action::Replay),
            KeyCode::Char('{') => self.preview.cycle_depth(),
            KeyCode::Char('}') => self.preview.cycle_glyphs(),
            _ => {}
        }
    }

    fn step_page(&mut self, forward: bool) {
        let n = PageId::ALL.len();
        let i = self.page.index();
        self.goto(PageId::ALL[(i + if forward { 1 } else { n - 1 }) % n]);
    }

    /// Show `page` with the keys going to it.
    pub fn open_page(&mut self, page: PageId) {
        self.goto(page);
        self.tabs_focused = false;
    }

    fn goto(&mut self, page: PageId) {
        if page != self.page
            && let Some(s) = self.pages[page.index()].scenario()
        {
            self.preview.select(s);
        }
        self.page = page;
    }

    /// Copy through the terminal (OSC 52), which works over SSH too.
    fn copy(&mut self, text: &str) {
        use std::io::Write;
        let mut out = std::io::stdout();
        let _ = write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()));
        let _ = out.flush();
        self.toast(format!("Copied: {text}"), false);
    }

    /// The first-run setup, over whatever is showing.
    pub fn open_setup(&mut self) {
        self.dialogs
            .push(Box::new(dialogs::Setup::new(self.env.settings.clone())));
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
            Action::Toast(t) => self.toast(t, false),
            Action::Error(t) => self.toast(t, true),
            Action::Goto(p) => self.open_page(p),
            Action::Scenario(i) => self.preview.scenario = i,
            Action::Replay => {
                if !self.preview.toggle_replay() {
                    self.toast("No session recorded yet to replay", false);
                }
            }
            Action::Undo => {
                if !self.doc.undo() {
                    self.toast("Nothing to undo", false);
                }
            }
            Action::Redo => {
                if !self.doc.redo() {
                    self.toast("Nothing to redo", false);
                }
            }
            Action::AskQuit => self.request_quit(),
            Action::Help => {
                let keys = self.page_mut().keys();
                self.dialogs.push(Box::new(dialogs::Help {
                    page: self.page.title(),
                    keys,
                }));
            }
            Action::Setup => self.open_setup(),
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

/// The footer's key hints in `width` columns: the page's own, then the
/// global ones. What doesn't fit is left out, quit first and the page's
/// last hints after the globals, but never `? help`, which lists them all.
fn footer_hints(own: Vec<(&str, &str)>, in_page: bool, width: usize) -> Line<'static> {
    let mut own = own;
    let mut global = vec![("^S", "save"), ("?", "help"), ("q", "quit")];
    if in_page {
        global.insert(0, ("↑", "tabs"));
    }
    loop {
        let all: Vec<(&str, &str)> = own.iter().chain(&global).copied().collect();
        let line = ui::hints(&all);
        if line.width() <= width {
            return line;
        }
        let next = ["q", "^S", "↑"]
            .iter()
            .find_map(|d| global.iter().position(|(k, _)| k == d));
        match next {
            Some(i) => {
                global.remove(i);
            }
            None if own.pop().is_some() => {}
            None => return line,
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
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
            now: crate::scenarios::BASE,
            tz: chrono::FixedOffset::east_opt(0).unwrap(),
            session: None,
            seen: Default::default(),
            settings: std::env::temp_dir().join("ccline-test-no-settings.json"),
            update: Default::default(),
        }
    }

    #[test]
    fn footer_keeps_help_when_short_of_room() {
        let page = vec![("tab", "pane"), ("x", "remove"), ("n", "new line")];
        let text = |w| {
            footer_hints(page.clone(), true, w)
                .spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert!(text(120).contains("q quit"));
        let mid = text(60);
        assert!(!mid.contains("q quit") && mid.contains("? help") && mid.contains("n new line"));
        let tight = text(30);
        assert!(
            tight.contains("? help") && !tight.contains("n new line"),
            "{tight}"
        );
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(
            base64(b"CLAUDE_CODE_MAX_CONTEXT_TOKENS=256000"),
            "Q0xBVURFX0NPREVfTUFYX0NPTlRFWFRfVE9LRU5TPTI1NjAwMA=="
        );
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

    /// How the first cell of `title` in the tab row is drawn.
    fn tab_style(app: &mut App, title: &str) -> Style {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| app.render(f)).unwrap();
        let buf = t.backend().buffer();
        let row: String = (0..100).map(|x| buf[(x, 1)].symbol().to_string()).collect();
        let x = row[..row.find(title).unwrap()].chars().count() as u16;
        buf[(x, 1)].style()
    }

    /// The tab is a filled block: accent while the tab row has focus,
    /// the terminal's foreground color otherwise.
    fn tab_on(app: &mut App, title: &str) -> bool {
        let s = tab_style(app, title);
        s.fg == Some(ui::ACCENT) && s.add_modifier.contains(ratatui::style::Modifier::REVERSED)
    }

    fn tab_current(app: &mut App, title: &str) -> bool {
        let s = tab_style(app, title);
        s.fg != Some(ui::ACCENT) && s.add_modifier.contains(ratatui::style::Modifier::REVERSED)
    }

    #[test]
    fn navigates_pages_from_the_tab_row() {
        let (dir, paths) = temp_paths("app-pages");
        let mut app = App::new(Doc::open(paths), env());
        let home = screen(&mut app, 100, 30);
        assert!(
            home.contains(" ccline   Home ") && !home.contains("[1 Home]"),
            "{home}"
        );
        assert!(
            home.contains("Health")
                && home.contains("Start")
                && home.contains("Your status line now")
        );
        // The tab row starts with the keys; left and right show pages.
        assert!(app.tabs_focused && tab_on(&mut app, "Home"));
        app.handle_key(key(KeyCode::Left));
        assert_eq!(app.page, PageId::Claude);
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Right));
        }
        assert_eq!(app.page, PageId::Models);
        assert!(tab_on(&mut app, "Models") && !tab_on(&mut app, "Home"));
        // Down goes into the page; there left and right are the page's.
        app.handle_key(key(KeyCode::Down));
        assert!(!app.tabs_focused && tab_current(&mut app, "Models"));
        assert!(!tab_current(&mut app, "Home"));
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.page, PageId::Models);
        // Up past the top of the page reaches the tab row again; so does Esc.
        app.handle_key(key(KeyCode::Up));
        assert!(app.tabs_focused);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.tabs_focused);
        // Digits no longer switch pages.
        app.handle_key(key(KeyCode::Char('1')));
        assert_eq!(app.page, PageId::Models);

        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.page, PageId::Home);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.page, PageId::Presets);
        assert!(!app.tabs_focused);
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
        app.page = PageId::Style;
        let _ = screen(&mut app, 100, 30);
        app.handle_key(key(KeyCode::Char(']')));
        app.handle_key(key(KeyCode::Char('-')));
        let s = screen(&mut app, 100, 30);
        assert!(s.contains("‹fresh›") && s.contains("‹94›"), "{s}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
