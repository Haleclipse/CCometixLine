//! Stand-in for pages that arrive in later steps.

use super::{Cx, Env, Outcome, Page};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;

pub struct Placeholder {
    title: &'static str,
    summary: &'static str,
}

impl Placeholder {
    pub fn new(title: &'static str, summary: &'static str) -> Self {
        Placeholder { title, summary }
    }
}

impl Page for Placeholder {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Cx) {
        let text = Text::from(vec![
            Line::raw(""),
            Line::styled(format!("  {}", self.title), ui::strong()),
            Line::raw(""),
            Line::styled(format!("  {}", self.summary), ui::muted()),
        ]);
        f.render_widget(Paragraph::new(text), area);
    }

    fn handle(&mut self, _: KeyEvent, _: &mut Doc, _: &Env) -> Outcome {
        Outcome::Ignored
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("esc", "home")]
    }
}
