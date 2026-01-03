use crate::config::models::{ModelAlias, ModelConfig};
use crate::ui::components::name_input::NameInputComponent;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    Normal,
    EditingId,
    EditingName,
    EditingContext,
}

pub struct AliasEditorApp {
    config: ModelConfig,
    config_path: PathBuf,
    state: ListState,
    should_quit: bool,
    input_mode: InputMode,
    name_input: NameInputComponent,
    status_message: Option<String>,
    // For editing
    editing_index: Option<usize>,
    temp_alias: Option<ModelAlias>,
}

impl AliasEditorApp {
    pub fn new() -> Self {
        let config = ModelConfig::load();
        let config_path = dirs::home_dir()
            .map(|d| d.join(".claude").join("ccline").join("models.toml"))
            .unwrap_or_else(|| PathBuf::from("models.toml"));

        let mut state = ListState::default();
        if !config.model_aliases.is_empty() {
            state.select(Some(0));
        }

        Self {
            config,
            config_path,
            state,
            should_quit: false,
            input_mode: InputMode::Normal,
            name_input: NameInputComponent::new(),
            status_message: None,
            editing_index: None,
            temp_alias: None,
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // Terminal setup
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut app = Self::new();
        let result = app.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Handle popup events first
                if self.name_input.is_open {
                    match key.code {
                        KeyCode::Esc => {
                            self.name_input.close();
                            self.input_mode = InputMode::Normal;
                            self.temp_alias = None;
                            self.editing_index = None;
                        }
                        KeyCode::Enter => {
                            if let Some(input) = self.name_input.get_input() {
                                self.handle_input_submission(input);
                            }
                        }
                        KeyCode::Char(c) => self.name_input.input_char(c),
                        KeyCode::Backspace => self.name_input.backspace(),
                        _ => {}
                    }
                    continue;
                }

                // Main navigation
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.should_quit = true;
                    }
                    KeyCode::Up => self.previous(),
                    KeyCode::Down => self.next(),
                    KeyCode::Char('a') => self.start_add_alias(),
                    KeyCode::Char('e') | KeyCode::Enter => self.start_edit_alias(),
                    KeyCode::Char('d') | KeyCode::Delete => self.delete_alias(),
                    KeyCode::Char('s') => self.save_config()?,
                    _ => {}
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn next(&mut self) {
        if self.config.model_aliases.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.config.model_aliases.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.config.model_aliases.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.config.model_aliases.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn start_add_alias(&mut self) {
        self.input_mode = InputMode::EditingId;
        self.editing_index = None;
        self.temp_alias = Some(ModelAlias {
            id: String::new(),
            display_name: String::new(),
            context_limit: None,
        });
        self.name_input.open("Add New Alias", "Enter Model ID (exact match):");
    }

    fn start_edit_alias(&mut self) {
        if let Some(i) = self.state.selected() {
            if let Some(alias) = self.config.model_aliases.get(i) {
                self.input_mode = InputMode::EditingId;
                self.editing_index = Some(i);
                self.temp_alias = Some(alias.clone());
                self.name_input.open_with_value(
                    "Edit Alias",
                    "Enter Model ID (exact match):",
                    &alias.id
                );
            }
        }
    }

    fn delete_alias(&mut self) {
        if let Some(i) = self.state.selected() {
            if i < self.config.model_aliases.len() {
                let removed = self.config.model_aliases.remove(i);
                self.status_message = Some(format!("Deleted alias: {}", removed.display_name));

                // Adjust selection
                if self.config.model_aliases.is_empty() {
                    self.state.select(None);
                } else if i >= self.config.model_aliases.len() {
                    self.state.select(Some(self.config.model_aliases.len() - 1));
                }
            }
        }
    }

    fn handle_input_submission(&mut self, input: String) {
        if let Some(alias) = &mut self.temp_alias {
            match self.input_mode {
                InputMode::EditingId => {
                    let trimmed_id = input.trim().to_string();

                    // Validate: ID must not be empty
                    if trimmed_id.is_empty() {
                        self.status_message = Some("Error: Model ID cannot be empty".to_string());
                        return;
                    }

                    // Validate: ID must be unique (except when editing the same entry)
                    let is_duplicate = self.config.model_aliases.iter().enumerate().any(|(i, a)| {
                        a.id == trimmed_id && self.editing_index != Some(i)
                    });
                    if is_duplicate {
                        self.status_message = Some(format!("Error: Model ID '{}' already exists", trimmed_id));
                        return;
                    }

                    alias.id = trimmed_id;
                    self.input_mode = InputMode::EditingName;
                    self.name_input.open_with_value(
                        if self.editing_index.is_some() { "Edit Alias" } else { "Add New Alias" },
                        "Enter Display Name:",
                        &alias.display_name
                    );
                }
                InputMode::EditingName => {
                    let trimmed_name = input.trim().to_string();

                    // Validate: Display name must not be empty
                    if trimmed_name.is_empty() {
                        self.status_message = Some("Error: Display name cannot be empty".to_string());
                        return;
                    }

                    alias.display_name = trimmed_name;
                    self.input_mode = InputMode::EditingContext;
                    let limit_str = alias.context_limit.map(|l| l.to_string()).unwrap_or_default();
                    self.name_input.open_with_value(
                        if self.editing_index.is_some() { "Edit Alias" } else { "Add New Alias" },
                        "Enter Context Limit (optional, press Enter to skip):",
                        &limit_str
                    );
                }
                InputMode::EditingContext => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        alias.context_limit = None;
                    } else if let Ok(limit) = trimmed.parse::<u32>() {
                        if limit == 0 {
                            self.status_message = Some("Error: Context limit must be greater than 0".to_string());
                            return;
                        }
                        alias.context_limit = Some(limit);
                    } else {
                        self.status_message = Some(format!("Error: '{}' is not a valid number", trimmed));
                        return;
                    }

                    // Save to list
                    if let Some(index) = self.editing_index {
                        self.config.model_aliases[index] = alias.clone();
                        self.status_message = Some("Alias updated".to_string());
                    } else {
                        self.config.model_aliases.push(alias.clone());
                        self.status_message = Some("Alias added".to_string());
                        // Select new item
                        self.state.select(Some(self.config.model_aliases.len() - 1));
                    }

                    // Reset state
                    self.temp_alias = None;
                    self.editing_index = None;
                    self.input_mode = InputMode::Normal;
                    self.name_input.close();
                }
                _ => {}
            }
        }
    }

    /// Escape a string for TOML (handle quotes and backslashes)
    fn escape_toml_string(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn save_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try to preserve existing file content (comments, models section, etc.)
        let existing_content = std::fs::read_to_string(&self.config_path).unwrap_or_default();

        // Generate only the aliases section with proper TOML escaping
        let aliases_toml = if self.config.model_aliases.is_empty() {
            String::new()
        } else {
            let mut aliases_str = String::new();
            for alias in &self.config.model_aliases {
                aliases_str.push_str("[[aliases]]\n");
                aliases_str.push_str(&format!("id = \"{}\"\n", Self::escape_toml_string(&alias.id)));
                aliases_str.push_str(&format!("display_name = \"{}\"\n", Self::escape_toml_string(&alias.display_name)));
                if let Some(limit) = alias.context_limit {
                    aliases_str.push_str(&format!("context_limit = {}\n", limit));
                }
                aliases_str.push('\n');
            }
            aliases_str
        };

        let new_content = if existing_content.is_empty() {
            // Create new file with header and aliases
            format!(
                "# CCometixLine Model Configuration\n\
                 # File location: {}\n\
                 \n\
                 # =============================================================================\n\
                 # Model Aliases (Exact Match - Highest Priority)\n\
                 # =============================================================================\n\
                 \n\
                 {}\
                 # =============================================================================\n\
                 # Model Patterns (Fuzzy Match - Fallback)\n\
                 # =============================================================================\n\
                 # Add [[models]] entries below for pattern matching\n",
                self.config_path.display(),
                aliases_toml
            )
        } else {
            // Preserve existing content, only update aliases section
            // Strategy: Remove old [[aliases]] entries and insert new ones

            let lines: Vec<&str> = existing_content.lines().collect();
            let mut new_lines: Vec<String> = Vec::new();
            let mut in_alias_block = false;
            let mut aliases_inserted = false;

            for line in lines.iter() {
                let trimmed = line.trim();

                // Detect start of [[aliases]] block
                if trimmed == "[[aliases]]" {
                    in_alias_block = true;

                    // Insert all new aliases at the position of first [[aliases]]
                    if !aliases_inserted {
                        for alias_line in aliases_toml.lines() {
                            new_lines.push(alias_line.to_string());
                        }
                        aliases_inserted = true;
                    }
                    continue;
                }

                // Inside alias block: skip until we hit another section or empty line followed by non-alias content
                if in_alias_block {
                    // Check if this line starts a new section
                    if trimmed.starts_with("[[") || (trimmed.starts_with('[') && !trimmed.starts_with("[[")) {
                        in_alias_block = false;
                        new_lines.push(line.to_string());
                    }
                    // Skip alias content (key = value lines and empty lines within alias blocks)
                    continue;
                }

                new_lines.push(line.to_string());
            }

            // If no aliases existed in file, insert at appropriate position
            if !aliases_inserted && !aliases_toml.is_empty() {
                // Find position after header comments
                let mut insert_pos = 0;
                for (i, line) in new_lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        insert_pos = i;
                        break;
                    }
                    insert_pos = i + 1;
                }

                // Insert aliases
                let alias_lines: Vec<String> = aliases_toml.lines().map(|s| s.to_string()).collect();
                for (i, alias_line) in alias_lines.into_iter().enumerate() {
                    new_lines.insert(insert_pos + i, alias_line);
                }
            }

            // Clean up: remove excessive empty lines (more than 2 consecutive)
            let mut result_lines: Vec<String> = Vec::new();
            let mut empty_count = 0;
            for line in new_lines {
                if line.trim().is_empty() {
                    empty_count += 1;
                    if empty_count <= 2 {
                        result_lines.push(line);
                    }
                } else {
                    empty_count = 0;
                    result_lines.push(line);
                }
            }

            result_lines.join("\n")
        };

        std::fs::write(&self.config_path, new_content)?;
        self.status_message = Some(format!("Saved to {}", self.config_path.display()));
        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        let size = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(5),    // List
                Constraint::Length(3), // Help/Status
            ])
            .split(size);

        // Title
        let title = Paragraph::new(format!("Model Aliases Editor ({})", self.config_path.display()))
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(title, chunks[0]);

        // List
        let items: Vec<ListItem> = self.config.model_aliases
            .iter()
            .map(|alias| {
                let limit_str = alias.context_limit
                    .map(|l| format!(" ({}k)", l / 1000))
                    .unwrap_or_default();

                let content = Line::from(vec![
                    Span::styled(format!("{:<30}", alias.display_name), Style::default().fg(Color::Green)),
                    Span::raw(" │ "),
                    Span::styled(&alias.id, Style::default().fg(Color::Cyan)),
                    Span::styled(limit_str, Style::default().fg(Color::Yellow)),
                ]);
                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Aliases"))
            .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, chunks[1], &mut self.state);

        // Status / Help
        let status_text = if let Some(msg) = &self.status_message {
            msg.clone()
        } else {
            "[A] Add  [E/Enter] Edit  [D/Del] Delete  [S] Save  [Esc/Q] Quit".to_string()
        };

        let status = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(if self.status_message.is_some() { Color::Yellow } else { Color::Gray }));
        f.render_widget(status, chunks[2]);

        // Popup
        if self.name_input.is_open {
            self.name_input.render(f, size);
        }
    }
}
