//! The configuration TUI: `ccline` run from a terminal, or `ccline -c`.

mod app;
mod convert;
mod dialogs;
pub mod doc;
mod pages;
mod preview;
mod ui;

use app::App;
use doc::{Doc, Paths};
use pages::Env;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use std::time::Duration;

/// How often the screen redraws without input, so messages expire; the
/// name's glint asks for more while it crosses the name.
const TICK: Duration = Duration::from_millis(500);

pub fn run() -> std::io::Result<()> {
    let mut app = App::new(Doc::open(Paths::user()), Env::detect());
    if app.doc.note == crate::config::LoadNote::Missing {
        app.open_setup();
    }
    // `ratatui::run` restores the terminal on return and on panic.
    ratatui::run(|terminal| {
        let _ = execute!(std::io::stdout(), EnableBracketedPaste);
        let result = event_loop(terminal, &mut app);
        let _ = execute!(std::io::stdout(), DisableBracketedPaste);
        result
    })
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    while !app.quit {
        terminal.draw(|f| app.render(f))?;
        let wait = app.next_frame().map_or(TICK, |f| f.min(TICK));
        if event::poll(wait)? {
            // Resizes only need the redraw at the top of the loop.
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                Event::Paste(text) => app.paste(&text),
                _ => {}
            }
        }
    }
    Ok(())
}
