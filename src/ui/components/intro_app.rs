use crate::claude_config::ClaudeConfig;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::intro_content::{get_step_content, get_step_title};

pub struct IntroApp {
    current_step: usize,
    total_steps: usize,
    should_quit: bool,
    should_continue_to_config: bool,
    claude_config_exists: bool,
    show_overwrite_prompt: bool,
    awaiting_config_choice: bool,
}

impl Default for IntroApp {
    fn default() -> Self {
        Self::new()
    }
}

impl IntroApp {
    pub fn new() -> Self {
        let claude_config_exists = ClaudeConfig::has_statusline_config();
        Self {
            current_step: 0,
            total_steps: 3,
            should_quit: false,
            should_continue_to_config: false,
            claude_config_exists,
            show_overwrite_prompt: false,
            awaiting_config_choice: false,
        }
    }

    pub fn next_step(&mut self) {
        if self.current_step < self.total_steps - 1 {
            self.current_step += 1;
            // When reaching the last step, show the config choice
            if self.current_step == self.total_steps - 1 {
                self.awaiting_config_choice = true;
            }
        }
    }

    pub fn configure_and_continue(&mut self) {
        // Configure Claude Code
        if let Err(_e) = ClaudeConfig::configure_statusline(true) {
            // Silently handle error in TUI mode
        }
        self.should_continue_to_config = true;
    }

    pub fn handle_config_choice(&mut self, choice: char) {
        match choice {
            'y' | 'Y' => {
                // Yes - configure automatically
                if self.claude_config_exists {
                    self.show_overwrite_prompt = true;
                } else {
                    self.configure_and_continue();
                }
            }
            'n' | 'N' => {
                // No - skip configuration, start configurator
                self.should_continue_to_config = true;
            }
            's' | 'S' => {
                // Skip - start configurator
                self.should_continue_to_config = true;
            }
            _ => return, // Invalid choice, do nothing
        }
        self.awaiting_config_choice = false;
    }

    pub fn is_awaiting_config_choice(&self) -> bool {
        self.awaiting_config_choice
    }

    pub fn handle_overwrite_response(&mut self, overwrite: bool) {
        if overwrite {
            self.configure_and_continue();
        } else {
            self.should_continue_to_config = true;
        }
        self.show_overwrite_prompt = false;
    }

    pub fn is_showing_overwrite_prompt(&self) -> bool {
        self.show_overwrite_prompt
    }

    pub fn prev_step(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            // Reset awaiting config choice if going back
            if self.awaiting_config_choice {
                self.awaiting_config_choice = false;
            }
        }
    }

    pub fn skip_intro(&mut self) {
        self.should_quit = true;
    }

    pub fn should_continue(&self) -> bool {
        self.should_continue_to_config
    }

    pub fn should_exit(&self) -> bool {
        self.should_quit
    }

    pub fn render(&self, f: &mut Frame) {
        let area = f.area();

        if self.show_overwrite_prompt {
            self.render_overwrite_prompt(f, area);
        } else {
            self.render_normal_view(f, area);
        }
    }

    fn render_normal_view(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        // Main content layout
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(10),   // Content
                Constraint::Length(4), // Progress + Help
            ])
            .split(area);

        // Title
        self.render_title(f, layout[0]);

        // Main content
        self.render_content(f, layout[1]);

        // Bottom section
        let bottom_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(2)])
            .split(layout[2]);

        // Progress
        self.render_progress(f, bottom_layout[0]);

        // Help
        self.render_help(f, bottom_layout[1]);
    }

    fn render_overwrite_prompt(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(10),   // Content
                Constraint::Length(3), // Help
            ])
            .split(area);

        // Title
        self.render_title(f, layout[0]);

        // Overwrite prompt
        let prompt_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Claude Code statusLine already configured!",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
            Line::from("Would you like to overwrite the existing configuration?"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Y", Style::default().fg(Color::Green)),
                Span::raw(" - Yes, overwrite"),
            ]),
            Line::from(vec![
                Span::styled("N", Style::default().fg(Color::Red)),
                Span::raw(" - No, skip configuration"),
            ]),
        ];

        let prompt_widget = Paragraph::new(prompt_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Configuration Conflict "),
            )
            .alignment(Alignment::Center);

        f.render_widget(prompt_widget, layout[1]);

        // Help
        let help_text = "[Y] Yes  [N] No  [Esc] Skip";
        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::TOP))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        f.render_widget(help, layout[2]);
    }

    fn render_title(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let title = Paragraph::new(format!(
            "CCometixLine Intro - v{}",
            env!("CARGO_PKG_VERSION")
        ))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center);
        f.render_widget(title, area);
    }

    fn render_content(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let content = get_step_content(self.current_step);
        let content_widget = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(get_step_title(self.current_step)),
            )
            .wrap(ratatui::widgets::Wrap { trim: true })
            .alignment(Alignment::Left);
        f.render_widget(content_widget, area);
    }

    fn render_progress(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let progress_text = format!("Step {} of {}", self.current_step + 1, self.total_steps);

        let progress_dots = (0..self.total_steps)
            .map(|i| {
                if i == self.current_step {
                    Span::styled("●", Style::default().fg(Color::Cyan))
                } else {
                    Span::styled("○", Style::default().fg(Color::DarkGray))
                }
            })
            .fold(Vec::new(), |mut acc, span| {
                if !acc.is_empty() {
                    acc.push(Span::raw(" "));
                }
                acc.push(span);
                acc
            });

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let progress_bar = Paragraph::new(Line::from(progress_dots)).alignment(Alignment::Center);

        let text = Paragraph::new(progress_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        f.render_widget(progress_bar, content_layout[0]);
        f.render_widget(text, content_layout[1]);
    }

    fn render_help(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let help_text = if self.awaiting_config_choice {
            "[Y] Yes  [N] No  [S] Skip  [Esc] Exit"
        } else if self.current_step == 0 {
            "[→/Enter] Next  [Esc] Skip"
        } else if self.current_step >= self.total_steps - 1 {
            "[←] Back  [Y/N/S] Choose  [Esc] Exit"
        } else {
            "[←] Back  [→/Enter] Next  [Esc] Skip"
        };

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::TOP))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        f.render_widget(help, area);
    }
}
