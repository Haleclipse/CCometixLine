//! Dialogs stack over the pages; the top one gets every key until it
//! closes.

mod color;
mod fonts;
mod form;
mod help;
mod prompts;
mod rule;
mod settings;

use super::doc::Doc;
use super::pages::Cx;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

pub use color::{ColorDialog, Target as ColorTarget};
pub use fonts::FontCheck;
pub use help::Help;
pub use prompts::{Conflict, Quit};
pub use rule::{ModifierDialog, RuleDialog};
pub use settings::{SettingsDiff, shown_path};

/// Something only the app can do, asked for by a closing dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Quit,
    Save {
        force: bool,
        then_quit: bool,
    },
    Reload,
    Toast(String),
    /// A toast for something that went wrong.
    Error(String),
}

pub enum Reply {
    Stay,
    Close,
    /// Close, then do this.
    Then(Action),
}

pub trait Dialog {
    /// Draw within `area` (the whole screen); dialogs center themselves.
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx);
    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply;
    /// Pasted text; returns whether a field took it.
    fn paste(&mut self, _text: &str) -> bool {
        false
    }
}
