use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub struct NameInputComponent {
    pub is_open: bool,
    pub input: String,
    pub title: String,
    pub placeholder: String,
}

impl Default for NameInputComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl NameInputComponent {
    pub fn new() -> Self {
        Self {
            is_open: false,
            input: String::new(),
            title: "Input Name".to_string(),
            placeholder: "Enter name...".to_string(),
        }
    }

    pub fn open(&mut self, title: &str, placeholder: &str) {
        self.is_open = true;
        self.input.clear();
        self.title = title.to_string();
        self.placeholder = placeholder.to_string();
    }

    pub fn open_with_value(&mut self, title: &str, placeholder: &str, initial_value: &str) {
        self.is_open = true;
        self.input = initial_value.to_string();
        self.title = title.to_string();
        self.placeholder = placeholder.to_string();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.input.clear();
    }

    pub fn input_char(&mut self, c: char) {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            self.input.push(c);
        }
    }

    pub fn backspace(&mut self) {
        self.input.pop();
    }

    pub fn get_input(&self) -> Option<String> {
        if self.input.trim().is_empty() {
            None
        } else {
            Some(self.input.trim().to_string())
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.is_open {
            return;
        }

        // Calculate popup area that avoids covering the bottom help area
        let popup_width = 60_u16.min(area.width.saturating_sub(4));
        let popup_height = 8_u16; // Fixed height for content

        // Ensure popup doesn't cover bottom help area (reserve at least 4 lines for help)
        let max_y = area.height.saturating_sub(popup_height + 4);
        let popup_y = if max_y > 2 {
            (area.height.saturating_sub(popup_height)) / 2
        } else {
            2 // Minimum top margin
        };

        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: popup_y.min(max_y),
            width: popup_width,
            height: popup_height,
        };

        // Clear the popup area first
        f.render_widget(Clear, popup_area);

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title(self.title.as_str());
        let inner = popup_block.inner(popup_area);
        f.render_widget(popup_block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Input field
                Constraint::Length(3), // Actions
            ])
            .split(inner);

        // Input field
        let input_text = if self.input.is_empty() {
            format!("> {} <", self.placeholder)
        } else {
            format!("> {} <", self.input)
        };

        f.render_widget(
            Paragraph::new(input_text)
                .style(if self.input.is_empty() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Yellow)
                })
                .block(Block::default().borders(Borders::ALL).title("Name")),
            chunks[0],
        );

        // Actions
        f.render_widget(
            Paragraph::new("[Enter] Confirm  [Esc] Cancel")
                .block(Block::default().borders(Borders::ALL)),
            chunks[1],
        );
    }
}
