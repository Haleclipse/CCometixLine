use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

#[derive(Default)]
pub struct MainMenu {
    selected_item: usize,
    should_quit: bool,
    show_about: bool,
    status_message: Option<StatusMessage>,
}

/// Status message to display in the footer
struct StatusMessage {
    message: String,
    is_error: bool,
}

#[derive(Debug)]
pub enum MenuResult {
    LaunchConfigurator,
    InitConfig,
    CheckConfig,
    Exit,
}

impl MainMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run() -> Result<Option<MenuResult>, Box<dyn std::error::Error>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut app = MainMenu::new();
        let result = app.main_loop(&mut terminal)?;

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        Ok(result)
    }

    fn main_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<Option<MenuResult>, Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if self.show_about {
                    // In about dialog, any key closes it
                    self.show_about = false;
                    continue;
                }

                // Clear status message on any key press
                self.status_message = None;

                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.should_quit = true;
                    }
                    KeyCode::Up => {
                        if self.selected_item > 0 {
                            self.selected_item -= 1;
                        }
                    }
                    KeyCode::Down => {
                        let menu_items = self.get_menu_items();
                        if self.selected_item < menu_items.len() - 1 {
                            self.selected_item += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(result) = self.handle_selection() {
                            return Ok(Some(result));
                        }
                        // None means stay in menu (action was handled internally)
                    }
                    _ => {}
                }
            }

            if self.should_quit {
                return Ok(Some(MenuResult::Exit));
            }
        }
    }

    fn get_menu_items(&self) -> Vec<(&str, &str)> {
        vec![
            (" Configuration Mode", "Enter TUI configuration interface"),
            (" Initialize Config", "Create default configuration"),
            (" Check Configuration", "Validate configuration file"),
            (" About", "Show application information"),
            (" Exit", "Exit CCometixLine"),
        ]
    }

    fn handle_selection(&mut self) -> Option<MenuResult> {
        match self.selected_item {
            0 => Some(MenuResult::LaunchConfigurator),
            1 => {
                // Initialize config and show result in footer
                use crate::config::InitResult;
                match crate::config::Config::init() {
                    Ok(InitResult::Created(path)) => {
                        self.status_message = Some(StatusMessage {
                            message: format!("✓ Created config at {}", path.display()),
                            is_error: false,
                        });
                    }
                    Ok(InitResult::AlreadyExists(path)) => {
                        self.status_message = Some(StatusMessage {
                            message: format!("Config already exists at {}", path.display()),
                            is_error: false,
                        });
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage {
                            message: format!("✗ Error: {}", e),
                            is_error: true,
                        });
                    }
                }
                None // Stay in menu
            }
            2 => {
                // Check config and show result in footer
                match crate::config::Config::load() {
                    Ok(config) => match config.check() {
                        Ok(_) => {
                            self.status_message = Some(StatusMessage {
                                message: "✓ Configuration is valid!".to_string(),
                                is_error: false,
                            });
                        }
                        Err(e) => {
                            self.status_message = Some(StatusMessage {
                                message: format!("✗ Invalid: {}", e),
                                is_error: true,
                            });
                        }
                    },
                    Err(e) => {
                        self.status_message = Some(StatusMessage {
                            message: format!("✗ Failed to load: {}", e),
                            is_error: true,
                        });
                    }
                }
                None // Stay in menu
            }
            3 => {
                self.show_about = true;
                None // Stay in menu
            }
            4 => Some(MenuResult::Exit),
            _ => Some(MenuResult::Exit),
        }
    }

    fn ui(&mut self, f: &mut Frame) {
        let size = f.area();

        // Calculate footer height based on status message
        let footer_height = if self.status_message.is_some() { 5 } else { 3 };

        // Main layout
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),             // Header
                Constraint::Min(10),               // Menu
                Constraint::Length(footer_height), // Footer/Help
            ])
            .split(size);

        // Header
        let header_text = Text::from(vec![
            Line::from(vec![
                Span::styled(
                    "CCometixLine",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" v", Style::default().fg(Color::Gray)),
                Span::styled(
                    env!("CARGO_PKG_VERSION"),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "High-performance Claude Code StatusLine Configuration",
                Style::default().fg(Color::Gray),
            )),
        ]);

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).title("Welcome"))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(header, main_layout[0]);

        // Menu
        let menu_items = self.get_menu_items();
        let list_items: Vec<ListItem> = menu_items
            .iter()
            .enumerate()
            .map(|(i, (title, desc))| {
                let style = if i == self.selected_item {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                let content = Line::from(vec![
                    Span::styled(*title, style),
                    Span::styled(format!(" - {}", desc), Style::default().fg(Color::Gray)),
                ]);

                ListItem::new(content).style(style)
            })
            .collect();

        let menu_list = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Main Menu")
                    .title_style(Style::default().fg(Color::Green)),
            )
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black))
            .highlight_symbol("▶ ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected_item));

        f.render_stateful_widget(menu_list, main_layout[1], &mut list_state);

        // Footer/Help - with optional status message
        let mut footer_lines = vec![Line::from(vec![
            Span::styled(
                "[↑↓]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Navigate  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Select  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "[Esc/Q]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Exit", Style::default().fg(Color::Gray)),
        ])];

        // Add status message if present
        if let Some(ref status) = self.status_message {
            let color = if status.is_error {
                Color::Red
            } else {
                Color::Green
            };
            footer_lines.push(Line::from(""));
            footer_lines.push(Line::from(Span::styled(
                status.message.as_str(),
                Style::default().fg(color),
            )));
        }

        let footer = Paragraph::new(Text::from(footer_lines))
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .alignment(Alignment::Center);

        f.render_widget(footer, main_layout[2]);

        // About dialog overlay
        if self.show_about {
            self.render_about_dialog(f, size);
        }
    }

    fn render_about_dialog(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        // Calculate popup area (centered)
        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(area)[1];

        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(popup_area)[1];

        // Clear the background
        f.render_widget(Clear, popup_area);

        let about_text = Text::from(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "CCometixLine ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("v", Style::default().fg(Color::Gray)),
                Span::styled(
                    env!("CARGO_PKG_VERSION"),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Features:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("• 🎨 TUI Configuration Interface"),
            Line::from("• 🎯 Multiple Built-in Themes"),
            Line::from("• ⚡ Real-time Usage Tracking"),
            Line::from("• 💰 Cost Monitoring"),
            Line::from("• 📊 Session Statistics"),
            Line::from("• 🎨 Nerd Font Support"),
            Line::from("• 🔧 Highly Customizable"),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to continue...",
                Style::default().fg(Color::Yellow),
            )),
        ]);

        let about_dialog = Paragraph::new(about_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("About CCometixLine")
                    .title_style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(about_dialog, popup_area);
    }
}
