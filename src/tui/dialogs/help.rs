//! `?`: every key that works here.

use super::{Dialog, Reply};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub const GLOBAL: [(&str, &str); 11] = [
    ("1-6", "switch page"),
    ("esc", "back to Home"),
    ("^S", "save"),
    ("^Z ^Y", "undo, redo"),
    ("[ ]", "preview scenario"),
    ("- +", "preview width"),
    ("{", "preview color depth"),
    ("}", "preview glyphs"),
    ("?", "this help"),
    ("q", "quit"),
    ("^C", "quit"),
];

pub struct Help {
    pub page: &'static str,
    pub keys: Vec<(&'static str, &'static str)>,
}

fn rows(keys: &[(&str, &str)]) -> Vec<Line<'static>> {
    keys.iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(format!("  {k:<10}"), ui::accent()),
                Span::raw(d.to_string()),
            ])
        })
        .collect()
}

impl Dialog for Help {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Doc) {
        let height = (GLOBAL.len() + self.keys.len() + 6) as u16;
        let rect = ui::popup(f, area, 56, height);
        let mut text = vec![Line::styled(" Everywhere", ui::strong())];
        text.extend(rows(&GLOBAL));
        text.push(Line::raw(""));
        text.push(Line::styled(format!(" {}", self.page), ui::strong()));
        text.extend(rows(&self.keys));
        f.render_widget(
            Paragraph::new(text).block(
                ui::panel("Keys").title_bottom(Line::styled(" any key closes ", ui::muted())),
            ),
            rect,
        );
    }

    fn handle(&mut self, _: KeyEvent, _: &mut Doc) -> Reply {
        Reply::Close
    }
}
