use crate::config::StyleMode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

#[derive(Debug, Clone, PartialEq)]
pub enum IconStyle {
    Plain,
    NerdFont,
}

#[derive(Debug, Clone)]
pub struct IconSelectorComponent {
    pub is_open: bool,
    pub icon_style: IconStyle,
    pub selected_plain: usize,
    pub selected_nerd: usize,
    pub custom_input: String,
    pub editing_custom: bool,
    pub current_icon: Option<String>,
    pub plain_list_state: ListState,
    pub plain_scrollbar_state: ScrollbarState,
    pub nerd_list_state: ListState,
    pub nerd_scrollbar_state: ScrollbarState,
}

impl Default for IconSelectorComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl IconSelectorComponent {
    pub fn new() -> Self {
        let plain_icons = get_plain_icons();
        let nerd_icons = get_nerd_font_icons();

        Self {
            is_open: false,
            icon_style: IconStyle::Plain,
            selected_plain: 0,
            selected_nerd: 0,
            custom_input: String::new(),
            editing_custom: false,
            current_icon: None,
            plain_list_state: ListState::default().with_selected(Some(0)),
            plain_scrollbar_state: ScrollbarState::new(plain_icons.len()),
            nerd_list_state: ListState::default().with_selected(Some(0)),
            nerd_scrollbar_state: ScrollbarState::new(nerd_icons.len()),
        }
    }

    pub fn open(&mut self, current_style: StyleMode) {
        self.is_open = true;
        self.icon_style = match current_style {
            StyleMode::Plain => IconStyle::Plain,
            StyleMode::NerdFont | StyleMode::Powerline => IconStyle::NerdFont,
        };
        self.update_current_icon();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.editing_custom = false;
    }

    pub fn toggle_style(&mut self) {
        self.icon_style = match self.icon_style {
            IconStyle::Plain => IconStyle::NerdFont,
            IconStyle::NerdFont => IconStyle::Plain,
        };
        self.update_current_icon();
    }

    /// Adjust scrolling offset for plain icons list
    fn adjust_plain_offset(&mut self, view_height: usize) {
        let selected = self.selected_plain;
        let offset = self.plain_list_state.offset();
        let view = view_height.max(1);

        let new_offset = if selected >= offset + view {
            selected + 1 - view
        } else if selected < offset {
            selected
        } else {
            offset
        };

        *self.plain_list_state.offset_mut() = new_offset;
        self.plain_scrollbar_state = self.plain_scrollbar_state.position(new_offset);
    }

    /// Adjust scrolling offset for nerd font icons list  
    fn adjust_nerd_offset(&mut self, view_height: usize) {
        let selected = self.selected_nerd;
        let offset = self.nerd_list_state.offset();
        let view = view_height.max(1);

        let new_offset = if selected >= offset + view {
            selected + 1 - view
        } else if selected < offset {
            selected
        } else {
            offset
        };

        *self.nerd_list_state.offset_mut() = new_offset;
        self.nerd_scrollbar_state = self.nerd_scrollbar_state.position(new_offset);
    }

    pub fn start_custom_input(&mut self) {
        self.editing_custom = true;
        self.custom_input.clear();
    }

    pub fn finish_custom_input(&mut self) -> bool {
        self.editing_custom = false;
        if !self.custom_input.is_empty() {
            self.current_icon = Some(self.custom_input.clone());
            return true;
        }
        false
    }

    pub fn input_char(&mut self, c: char) {
        if self.editing_custom {
            self.custom_input.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.editing_custom {
            self.custom_input.pop();
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.editing_custom {
            return;
        }

        match self.icon_style {
            IconStyle::Plain => {
                let plain_icons = get_plain_icons();
                let new_selection = (self.selected_plain as i32 + delta)
                    .max(0)
                    .min((plain_icons.len() - 1) as i32)
                    as usize;
                self.selected_plain = new_selection;
                self.plain_list_state.select(Some(new_selection));
                // Note: adjust_plain_offset will be called in render with actual view height
            }
            IconStyle::NerdFont => {
                let nerd_icons = get_nerd_font_icons();
                let new_selection = (self.selected_nerd as i32 + delta)
                    .max(0)
                    .min((nerd_icons.len() - 1) as i32)
                    as usize;
                self.selected_nerd = new_selection;
                self.nerd_list_state.select(Some(new_selection));
                // Note: adjust_nerd_offset will be called in render with actual view height
            }
        }
        self.update_current_icon();
    }

    fn update_current_icon(&mut self) {
        match self.icon_style {
            IconStyle::Plain => {
                let plain_icons = get_plain_icons();
                if let Some(icon) = plain_icons.get(self.selected_plain) {
                    self.current_icon = Some(icon.icon.to_string());
                }
            }
            IconStyle::NerdFont => {
                let nerd_icons = get_nerd_font_icons();
                if let Some(icon) = nerd_icons.get(self.selected_nerd) {
                    self.current_icon = Some(icon.icon.to_string());
                }
            }
        }
    }

    pub fn get_selected_icon(&self) -> Option<String> {
        self.current_icon.clone()
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.is_open {
            return;
        }

        let popup_area = centered_rect(60, 70, area);

        // Clear the popup area first
        f.render_widget(Clear, popup_area);

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .title("Icon Selector");
        let inner = popup_block.inner(popup_area);
        f.render_widget(popup_block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Style selector (需要3行：上边框+内容+下边框)
                Constraint::Min(10),   // Icon grid
                Constraint::Length(3), // Custom input
                Constraint::Length(3), // Actions
            ])
            .split(inner);

        // Style selector
        let style_text = match self.icon_style {
            IconStyle::Plain => "[•] Emoji  [ ] Nerd Font",
            IconStyle::NerdFont => "[ ] Emoji  [•] Nerd Font",
        };

        f.render_widget(
            Paragraph::new(style_text).block(Block::default().borders(Borders::ALL).title("Style")),
            chunks[0],
        );

        // Icon display
        match self.icon_style {
            IconStyle::Plain => self.render_plain_icons(f, chunks[1]),
            IconStyle::NerdFont => self.render_nerd_icons(f, chunks[1]),
        }

        // Custom input
        let custom_text = if self.editing_custom {
            format!("> {} <", self.custom_input)
        } else {
            "[Enter text to input custom icon]".to_string()
        };

        f.render_widget(
            Paragraph::new(custom_text)
                .style(if self.editing_custom {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                })
                .block(Block::default().borders(Borders::ALL).title("Custom")),
            chunks[2],
        );

        // Actions
        let actions = if self.editing_custom {
            "[Enter] Confirm  [Esc] Cancel"
        } else {
            "[Enter] Select  [Tab] Switch Style  [c] Custom  [Esc] Cancel"
        };

        f.render_widget(
            Paragraph::new(actions).block(Block::default().borders(Borders::ALL)),
            chunks[3],
        );
    }

    fn render_plain_icons(&mut self, f: &mut Frame, area: Rect) {
        let icons = get_plain_icons();
        let items: Vec<ListItem> = icons
            .iter()
            .map(|icon_info| {
                let line = format!("{} {}", icon_info.icon, icon_info.name);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default().borders(Borders::ALL).title("Emoji Icons");

        let inner = block.inner(area);
        let view_height = inner.height.saturating_sub(0) as usize; // No title inside list

        // Adjust scrolling offset
        self.adjust_plain_offset(view_height);

        // Render block
        f.render_widget(block, area);

        // Render list with state
        let list = List::new(items)
            .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED));

        f.render_stateful_widget(list, inner, &mut self.plain_list_state);

        // Render scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::Gray));
        f.render_stateful_widget(scrollbar, inner, &mut self.plain_scrollbar_state);
    }

    fn render_nerd_icons(&mut self, f: &mut Frame, area: Rect) {
        let icons = get_nerd_font_icons();
        let items: Vec<ListItem> = icons
            .iter()
            .map(|icon_info| {
                let line = format!("{} {}", icon_info.icon, icon_info.name);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Nerd Font Icons");

        let inner = block.inner(area);
        let view_height = inner.height.saturating_sub(0) as usize; // No title inside list

        // Adjust scrolling offset
        self.adjust_nerd_offset(view_height);

        // Render block
        f.render_widget(block, area);

        // Render list with state
        let list = List::new(items)
            .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED));

        f.render_stateful_widget(list, inner, &mut self.nerd_list_state);

        // Render scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::Gray));
        f.render_stateful_widget(scrollbar, inner, &mut self.nerd_scrollbar_state);
    }
}

#[derive(Debug, Clone)]
struct IconInfo {
    icon: &'static str,
    name: &'static str,
}

fn get_plain_icons() -> Vec<IconInfo> {
    vec![
        IconInfo {
            icon: "🤖",
            name: "Robot (Model)",
        },
        IconInfo {
            icon: "💻",
            name: "Laptop (Computer)",
        },
        IconInfo {
            icon: "🖥️",
            name: "Desktop",
        },
        IconInfo {
            icon: "⚙️",
            name: "Gear (Settings)",
        },
        IconInfo {
            icon: "📁",
            name: "Folder",
        },
        IconInfo {
            icon: "📂",
            name: "Open Folder",
        },
        IconInfo {
            icon: "🗿",
            name: "Card Index",
        },
        IconInfo {
            icon: "📊",
            name: "Bar Chart",
        },
        IconInfo {
            icon: "🌿",
            name: "Branch (Git)",
        },
        IconInfo {
            icon: "🌱",
            name: "Seedling",
        },
        IconInfo {
            icon: "🔧",
            name: "Wrench",
        },
        IconInfo {
            icon: "⚡",
            name: "Lightning (Usage)",
        },
        IconInfo {
            icon: "⭐",
            name: "Star",
        },
        IconInfo {
            icon: "✨",
            name: "Sparkles",
        },
        IconInfo {
            icon: "🔥",
            name: "Fire",
        },
        IconInfo {
            icon: "💎",
            name: "Gem",
        },
        IconInfo {
            icon: "✓",
            name: "Check Mark",
        },
        IconInfo {
            icon: "✗",
            name: "X Mark",
        },
        IconInfo {
            icon: "●",
            name: "Circle (Dirty)",
        },
        IconInfo {
            icon: "○",
            name: "Open Circle",
        },
        IconInfo {
            icon: "▶",
            name: "Play",
        },
        IconInfo {
            icon: "▼",
            name: "Down Triangle",
        },
        IconInfo {
            icon: "►",
            name: "Right Triangle",
        },
        IconInfo {
            icon: "◄",
            name: "Left Triangle",
        },
    ]
}

fn get_nerd_font_icons() -> Vec<IconInfo> {
    vec![
        IconInfo {
            icon: "\u{e26d}",
            name: "Robot (Model)",
        },
        IconInfo {
            icon: "\u{f02a2}",
            name: "Git Branch",
        },
        IconInfo {
            icon: "\u{f024b}",
            name: "Folder",
        },
        IconInfo {
            icon: "\u{f111}",
            name: "Circle",
        },
        IconInfo {
            icon: "\u{f135}",
            name: "Rocket",
        },
        IconInfo {
            icon: "\u{f49b}",
            name: "Chart",
        },
        IconInfo {
            icon: "\u{f0c6}",
            name: "Database",
        },
        IconInfo {
            icon: "\u{f0c9}",
            name: "List",
        },
        IconInfo {
            icon: "\u{f013}",
            name: "Cog",
        },
        IconInfo {
            icon: "\u{f015}",
            name: "Home",
        },
        IconInfo {
            icon: "\u{f07b}",
            name: "Folder Open",
        },
        IconInfo {
            icon: "\u{f0e7}",
            name: "Lightning",
        },
        IconInfo {
            icon: "\u{f121}",
            name: "Code",
        },
        IconInfo {
            icon: "\u{f126}",
            name: "Code Fork",
        },
        IconInfo {
            icon: "\u{f1c0}",
            name: "Database",
        },
        IconInfo {
            icon: "\u{f251}",
            name: "Headphones",
        },
        IconInfo {
            icon: "\u{f252}",
            name: "Terminal",
        },
        IconInfo {
            icon: "\u{f269}",
            name: "Map",
        },
        IconInfo {
            icon: "\u{f2d0}",
            name: "Chrome",
        },
        IconInfo {
            icon: "\u{f31b}",
            name: "Github",
        },
    ]
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
