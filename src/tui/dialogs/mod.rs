//! Dialogs stack over the pages; the top one gets every key until it
//! closes.

mod help;
mod prompts;

use super::doc::Doc;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

pub use help::Help;
pub use prompts::{Conflict, Quit};

/// Something only the app can do, asked for by a closing dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Quit,
    Save { force: bool, then_quit: bool },
    Reload,
}

pub enum Reply {
    Stay,
    Close,
    /// Close, then do this.
    Then(Action),
}

pub trait Dialog {
    /// Draw within `area` (the whole screen); dialogs center themselves.
    fn render(&mut self, f: &mut Frame, area: Rect, doc: &Doc);
    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply;
}
