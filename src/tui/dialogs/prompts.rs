//! Small yes/no prompts: quitting with unsaved edits, and saving over a
//! file that changed on disk.

use super::{Action, Dialog, Reply};
use crate::tui::doc::Doc;
use crate::tui::pages::Cx;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::path::PathBuf;

/// I · Unsaved changes.
pub struct Quit;

impl Dialog for Quit {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let rect = ui::popup(f, area, 64, 7);
        let n = cx.doc.edits();
        let text = vec![
            Line::raw(format!(
                " {n} change{} since the last save.",
                if n == 1 { "" } else { "s" }
            )),
            Line::raw(""),
            ui::hints(&[("S", "save and quit"), ("D", "discard"), ("esc", "stay")]),
        ];
        f.render_widget(
            Paragraph::new(text).block(ui::panel("Unsaved changes")),
            rect,
        );
    }

    fn handle(&mut self, key: KeyEvent, _: &mut Doc) -> Reply {
        match key.code {
            KeyCode::Char('s' | 'S') => Reply::Then(Action::Save {
                force: false,
                then_quit: true,
            }),
            KeyCode::Char('d' | 'D') => Reply::Then(Action::Quit),
            KeyCode::Esc => Reply::Close,
            _ => Reply::Stay,
        }
    }
}

/// A file changed on disk after it was loaded.
pub struct Conflict {
    pub paths: Vec<PathBuf>,
    pub then_quit: bool,
}

impl Dialog for Conflict {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Cx) {
        let rect = ui::popup(f, area, 72, 8 + self.paths.len() as u16);
        let mut text = vec![Line::raw(" Changed outside ccline since it was opened:")];
        text.extend(
            self.paths
                .iter()
                .map(|p| Line::styled(format!("   {}", p.display()), ui::accent())),
        );
        text.push(Line::raw(""));
        text.push(Line::raw(
            " Overwriting loses those changes; reloading loses yours.",
        ));
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("O", "overwrite"),
            ("R", "reload"),
            ("esc", "cancel"),
        ]));
        f.render_widget(
            Paragraph::new(text).block(ui::panel("Changed on disk")),
            rect,
        );
    }

    fn handle(&mut self, key: KeyEvent, _: &mut Doc) -> Reply {
        match key.code {
            KeyCode::Char('o' | 'O') => Reply::Then(Action::Save {
                force: true,
                then_quit: self.then_quit,
            }),
            KeyCode::Char('r' | 'R') => Reply::Then(Action::Reload),
            KeyCode::Esc => Reply::Close,
            _ => Reply::Stay,
        }
    }
}
