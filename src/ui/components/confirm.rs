use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Modal shown when quitting with unsaved changes
#[derive(Debug, Default)]
pub struct QuitConfirmComponent {
    pub is_open: bool,
}

impl QuitConfirmComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.is_open {
            return;
        }

        let popup_width = 56_u16.min(area.width.saturating_sub(4));
        let popup_height = 7_u16;
        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };

        f.render_widget(Clear, popup_area);

        let key = |k: &str| {
            Span::styled(
                k.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        };
        let lines = vec![
            Line::from("You have unsaved changes to config.toml."),
            Line::from(""),
            Line::from(vec![key("[S]"), Span::raw(" Save and quit")]),
            Line::from(vec![key("[Q]"), Span::raw(" Quit without saving")]),
            Line::from(vec![key("[Esc]"), Span::raw(" Keep editing")]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Unsaved changes")
            .border_style(Style::default().fg(Color::Yellow));
        f.render_widget(Paragraph::new(lines).block(block), popup_area);
    }
}
