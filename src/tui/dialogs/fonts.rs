//! Font check: show Nerd Font and Unicode glyphs and let the user pick the
//! tier their font can draw.

use super::{Action, Dialog, Reply};
use crate::style::glyphs::{self, GlyphTier, IconKey};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const SAMPLE: [IconKey; 8] = [
    IconKey::Model,
    IconKey::Dir,
    IconKey::Git,
    IconKey::Context,
    IconKey::Quota,
    IconKey::Cost,
    IconKey::Cache,
    IconKey::Pr,
];

pub struct FontCheck;

fn row(label: &str, glyphs: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("   {label:<9}"), ui::muted()),
        Span::styled(glyphs, ui::strong()),
    ])
}

impl Dialog for FontCheck {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Doc) {
        let rect = ui::popup(f, area, 64, 12);
        let nerd: Vec<&str> = SAMPLE
            .iter()
            .map(|k| glyphs::icon(*k, GlyphTier::Nerd))
            .collect();
        let unicode: Vec<&str> = SAMPLE
            .iter()
            .map(|k| glyphs::icon(*k, GlyphTier::Unicode))
            .collect();
        let text = vec![
            Line::raw(" Do these show as icons, not boxes or question marks?"),
            Line::raw(""),
            row(
                "Nerd",
                format!("{}    \u{e0b6}\u{e0b4} \u{e0b0}\u{e0b2}", nerd.join("  ")),
            ),
            row("Unicode", format!("{}    ▰▱ ▁▃▆█", unicode.join("  "))),
            Line::raw(""),
            Line::styled(" Pill and powerline skins need the Nerd row.", ui::muted()),
            Line::raw(""),
            ui::hints(&[
                ("N", "Nerd Font"),
                ("U", "Unicode only"),
                ("A", "ASCII only"),
                ("esc", "cancel"),
            ]),
        ];
        f.render_widget(Paragraph::new(text).block(ui::panel("Font check")), rect);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        let tier = match key.code {
            KeyCode::Char('n' | 'N') => GlyphTier::Nerd,
            KeyCode::Char('u' | 'U') => GlyphTier::Unicode,
            KeyCode::Char('a' | 'A') => GlyphTier::Ascii,
            KeyCode::Esc => return Reply::Close,
            _ => return Reply::Stay,
        };
        doc.edit(|c, _| c.glyphs = tier);
        Reply::Then(Action::Toast(format!(
            "Glyphs: {}",
            format!("{tier:?}").to_lowercase()
        )))
    }
}
