//! F · Changing Claude Code's `settings.json`: the change as a diff, then
//! a backup and the write.

use super::{Action, Dialog, Reply};
use crate::cc_settings::{Mark, Settings};
use crate::tui::doc::Doc;
use crate::tui::pages::Cx;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const WIDTH: u16 = 72;
/// Most diff lines shown.
const MAX_LINES: usize = 16;

pub struct SettingsDiff {
    /// The file as read.
    pub base: Settings,
    /// The settings to write.
    pub draft: Settings,
}

/// `path` with the home directory as `~`.
pub fn shown_path(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(h).ok().map(|p| p.to_path_buf())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

impl Dialog for SettingsDiff {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Cx) {
        let diff = self.draft.diff(&self.base);
        let shown = diff.len().min(MAX_LINES);
        let more = u16::from(diff.len() > shown);
        let rect = ui::popup(f, area, WIDTH, shown as u16 + more + 7);
        let room = WIDTH as usize - 2;
        let mut text = vec![
            Line::raw(" ccline will write this to your Claude Code settings:"),
            Line::raw(""),
        ];
        for (mark, line) in diff.iter().take(shown) {
            let (sign, style) = match mark {
                Mark::Same => ("  ", Style::default()),
                Mark::Added => ("+ ", Style::default().fg(ui::OK)),
                Mark::Removed => ("- ", Style::default().fg(ui::BAD)),
            };
            text.push(Line::from(ui::clip(
                vec![Span::styled(format!("  {sign}{line}"), style)],
                room,
            )));
        }
        if diff.len() > shown {
            text.push(Line::styled(
                format!("    … {} more lines", diff.len() - shown),
                ui::muted(),
            ));
        }
        text.push(Line::raw(""));
        text.push(Line::from(vec![
            Span::styled(" Backup  ", ui::muted()),
            Span::raw(format!(
                "{}.ccline-bak",
                self.base
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )),
        ]));
        text.push(ui::hints(&[("Y", "apply"), ("N", "cancel")]));
        let title = format!("Change {}", shown_path(&self.base.path));
        f.render_widget(Paragraph::new(text).block(ui::panel(&title)), rect);
    }

    fn handle(&mut self, key: KeyEvent, _: &mut Doc) -> Reply {
        match key.code {
            KeyCode::Char('y' | 'Y') => match self.draft.save(&self.base) {
                Ok(backup) => Reply::Then(Action::Toast(match backup {
                    Some(b) => format!("Updated settings.json · old file in {}", shown_path(&b)),
                    None => "Created settings.json".into(),
                })),
                Err(e) => Reply::Then(Action::Error(format!("Couldn't write settings.json: {e}"))),
            },
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Reply::Close,
            _ => Reply::Stay,
        }
    }
}
