//! The six pages. Each draws its body and handles the keys it understands;
//! anything it ignores falls through to the app's global keys.

mod home;
mod placeholder;

use super::doc::Doc;
use crate::state::SessionState;
use crate::version::Version;
use chrono::FixedOffset;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

pub use home::Home;
pub use placeholder::Placeholder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageId {
    Home,
    Presets,
    Layout,
    Style,
    Models,
    Claude,
}

impl PageId {
    pub const ALL: [PageId; 6] = [
        PageId::Home,
        PageId::Presets,
        PageId::Layout,
        PageId::Style,
        PageId::Models,
        PageId::Claude,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            PageId::Home => "Home",
            PageId::Presets => "Presets",
            PageId::Layout => "Layout",
            PageId::Style => "Style",
            PageId::Models => "Models",
            PageId::Claude => "Claude Code",
        }
    }

    pub fn from_digit(c: char) -> Option<PageId> {
        let n = c.to_digit(10)? as usize;
        PageId::ALL.get(n.checked_sub(1)?).copied()
    }
}

/// What the TUI knows about its surroundings.
pub struct Env {
    pub tz: FixedOffset,
    /// The newest session ccline recorded, for replay and version checks.
    pub session: Option<SessionState>,
}

impl Env {
    pub fn detect() -> Env {
        Env {
            tz: *chrono::Local::now().offset(),
            session: crate::state::session::latest(),
        }
    }

    /// Claude Code's version as last seen by a render.
    pub fn cc_version(&self) -> Option<Version> {
        let input = self.session.as_ref()?.last_input.as_ref()?;
        Version::parse(input.get("version")?.as_str()?)
    }
}

/// Read-only context for drawing.
pub struct Cx<'a> {
    pub doc: &'a Doc,
    pub env: &'a Env,
}

pub enum Outcome {
    /// Not handled; the app's global keys get it.
    Ignored,
    Consumed,
    Goto(PageId),
    Quit,
}

pub trait Page {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx);
    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome;
    /// Key hints for the footer.
    fn keys(&self) -> Vec<(&'static str, &'static str)>;
    /// Whether the preview bar shows under this page.
    fn preview(&self) -> bool {
        true
    }
    /// The file this page edits, for the title bar.
    fn file(&self) -> &'static str {
        "config.toml"
    }
}
