//! The six pages. Each draws its body and handles the keys it understands;
//! anything it ignores falls through to the app's global keys.

mod home;
mod models;
mod placeholder;
mod presets;

use super::dialogs::Dialog;
use super::doc::Doc;
use crate::state::SessionState;
use crate::state::models_seen::Seen;
use crate::version::Version;
use chrono::FixedOffset;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

pub use home::Home;
pub use models::Models;
pub use placeholder::Placeholder;
pub use presets::Presets;

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
}

/// What the TUI knows about its surroundings.
pub struct Env {
    /// Unix epoch seconds at start.
    pub now: i64,
    pub tz: FixedOffset,
    /// The newest session ccline recorded, for replay and version checks.
    pub session: Option<SessionState>,
    /// Model ids seen in recent sessions.
    pub seen: BTreeMap<String, Seen>,
}

impl Env {
    pub fn detect() -> Env {
        Env {
            now: chrono::Utc::now().timestamp(),
            tz: *chrono::Local::now().offset(),
            session: crate::state::session::latest(),
            seen: crate::state::models_seen::load(),
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
    /// The preview bar's scenario and overrides, for pages that render.
    pub preview: &'a super::preview::Preview,
    /// Whether the page has the keys (else the tab row does), so it shows
    /// its selection.
    pub focused: bool,
}

pub enum Outcome {
    /// Not handled; the app's global keys get it.
    Ignored,
    Consumed,
    Goto(PageId),
    Open(Box<dyn Dialog>),
    Toast(String),
    /// Put this text on the clipboard.
    Copy(String),
    Quit,
}

pub trait Page {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx);
    /// Handle a key. Return `Ignored` for Up at the top of the page so
    /// focus moves to the tab row.
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
    /// Pasted text; returns whether a field took it.
    fn paste(&mut self, _text: &str) -> bool {
        false
    }
    /// The preview scenario that shows this page's effect best.
    fn scenario(&self) -> Option<&'static str> {
        None
    }
}
