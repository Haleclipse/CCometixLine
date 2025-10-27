use crate::config::{Config, SegmentId, StyleMode};
use crate::core::segments::color_utils;
use crate::ui::components::{
    color_picker::{ColorPickerComponent, NavDirection},
    help::HelpComponent,
    icon_selector::IconSelectorComponent,
    name_input::NameInputComponent,
    preview::PreviewComponent,
    segment_list::{FieldSelection, Panel, SegmentListComponent},
    separator_editor::SeparatorEditorComponent,
    settings::SettingsComponent,
    theme_selector::ThemeSelectorComponent,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;

// Field count constants to avoid hardcoding
// These represent the number of configurable fields in the Settings panel
const DEFAULT_SEGMENT_FIELD_COUNT: usize = 7;  // Enabled, Icon, IconColor, TextColor, BackgroundColor, TextStyle, Options
const THRESHOLD_SEGMENT_FIELD_COUNT: usize = 13; // Default fields + WarningThreshold, CriticalThreshold, WarningColor, CriticalColor, WarningBold, CriticalBold
const GIT_SEGMENT_FIELD_COUNT: usize = 9; // Default fields + ShowSha, ShowDirtyCount

pub struct App {
    config: Config,
    selected_segment: usize,
    selected_panel: Panel,
    selected_field: FieldSelection,
    should_quit: bool,
    color_picker: ColorPickerComponent,
    icon_selector: IconSelectorComponent,
    name_input: NameInputComponent,
    preview: PreviewComponent,
    segment_list: SegmentListComponent,
    separator_editor: SeparatorEditorComponent,
    settings: SettingsComponent,
    theme_selector: ThemeSelectorComponent,
    help: HelpComponent,
    status_message: Option<String>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut app = Self {
            config: config.clone(),
            selected_segment: 0,
            selected_panel: Panel::SegmentList,
            selected_field: FieldSelection::Enabled,
            should_quit: false,
            color_picker: ColorPickerComponent::new(),
            icon_selector: IconSelectorComponent::new(),
            name_input: NameInputComponent::new(),
            preview: PreviewComponent::new(),
            segment_list: SegmentListComponent::new(),
            separator_editor: SeparatorEditorComponent::new(),
            settings: SettingsComponent::new(),
            theme_selector: ThemeSelectorComponent::new(),
            help: HelpComponent::new(),
            status_message: None,
        };
        app.preview.update_preview(&config);
        app
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        // Ensure themes directory and built-in themes exist
        if let Err(e) = crate::config::loader::ConfigLoader::init_themes() {
            eprintln!("Warning: Failed to initialize themes: {}", e);
        }

        // Load config
        let mut config = Config::load().unwrap_or_else(|_| Config::default());

        // If a theme is specified, reload it to get the latest changes
        if !config.theme.is_empty() && config.theme != "default" {
            if let Ok(theme_config) =
                crate::ui::themes::ThemePresets::load_theme_from_file(&config.theme)
            {
                config = theme_config;
            }
        }

        // Terminal setup
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut app = App::new(config);

        // Main loop
        let result = loop {
            terminal.draw(|f| app.ui(f))?;

            if let Event::Key(key) = event::read()? {
                // Only handle KeyDown events to prevent double triggering on Windows
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Handle popup events first
                if app.name_input.is_open {
                    match key.code {
                        KeyCode::Esc => app.name_input.close(),
                        KeyCode::Enter => {
                            if let Some(name) = app.name_input.get_input() {
                                app.save_as_new_theme(&name);
                            }
                            app.name_input.close();
                        }
                        KeyCode::Char(c) => app.name_input.input_char(c),
                        KeyCode::Backspace => app.name_input.backspace(),
                        _ => {}
                    }
                } else if app.separator_editor.is_open {
                    match key.code {
                        KeyCode::Esc => app.separator_editor.close(),
                        KeyCode::Enter => {
                            let new_separator = app.separator_editor.get_separator();
                            app.config.style.separator = new_separator;
                            app.separator_editor.close();
                            app.preview.update_preview(&app.config);
                            app.status_message = Some("Separator updated!".to_string());
                        }
                        KeyCode::Tab => {
                            app.separator_editor.input.clear();
                            app.separator_editor.selected_preset = None;
                        }
                        KeyCode::Up => app.separator_editor.move_preset_selection(-1),
                        KeyCode::Down => app.separator_editor.move_preset_selection(1),
                        KeyCode::Char(c) => app.separator_editor.input_char(c),
                        KeyCode::Backspace => app.separator_editor.backspace(),
                        _ => {}
                    }
                } else if app.color_picker.is_open {
                    match key.code {
                        KeyCode::Esc => app.color_picker.close(),
                        KeyCode::Up => app.color_picker.move_direction(NavDirection::Up),
                        KeyCode::Down => app.color_picker.move_direction(NavDirection::Down),
                        KeyCode::Left => app.color_picker.move_direction(NavDirection::Left),
                        KeyCode::Right => app.color_picker.move_direction(NavDirection::Right),
                        KeyCode::Tab => app.color_picker.cycle_mode(),
                        KeyCode::Char('r') => app.color_picker.switch_to_rgb(),
                        KeyCode::Enter => {
                            if let Some(color) = app.color_picker.get_selected_color() {
                                app.apply_selected_color(color);
                            }
                            app.color_picker.close();
                        }
                        KeyCode::Char(c) => app.color_picker.input_char(c),
                        KeyCode::Backspace => app.color_picker.backspace(),
                        _ => {}
                    }
                } else if app.icon_selector.is_open {
                    match key.code {
                        KeyCode::Esc => app.icon_selector.close(),
                        KeyCode::Up => app.icon_selector.move_selection(-1),
                        KeyCode::Down => app.icon_selector.move_selection(1),
                        KeyCode::Tab => app.icon_selector.toggle_style(),
                        KeyCode::Char('c') => app.icon_selector.start_custom_input(),
                        KeyCode::Enter => {
                            if app.icon_selector.editing_custom {
                                app.icon_selector.finish_custom_input();
                            } else {
                                if let Some(icon) = app.icon_selector.get_selected_icon() {
                                    app.apply_selected_icon(icon);
                                }
                                app.icon_selector.close();
                            }
                        }
                        KeyCode::Char(c) if app.icon_selector.editing_custom => {
                            app.icon_selector.input_char(c);
                        }
                        KeyCode::Backspace if app.icon_selector.editing_custom => {
                            app.icon_selector.backspace();
                        }
                        _ => {}
                    }
                } else {
                    // Handle main app events
                    match key.code {
                        KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char('s') => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                // Ctrl+S: Save as new theme with name input
                                app.name_input.open("Save as New Theme", "Enter theme name");
                            } else {
                                // s: Save config to config.toml
                                if let Err(e) = app.save_config() {
                                    app.status_message =
                                        Some(format!("Failed to save config: {}", e));
                                } else {
                                    app.status_message =
                                        Some("Configuration saved to config.toml!".to_string());
                                }
                            }
                        }
                        KeyCode::Char('w') | KeyCode::Char('W') => {
                            // w/W: Write config to current theme
                            app.write_to_current_theme();
                        }
                        KeyCode::Up => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.move_segment_up();
                            } else {
                                app.move_selection(-1);
                            }
                        }
                        KeyCode::Down => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.move_segment_down();
                            } else {
                                app.move_selection(1);
                            }
                        }
                        KeyCode::Enter => app.toggle_current(),
                        KeyCode::Tab => app.switch_panel(),
                        KeyCode::Char('1') => app.switch_to_theme("default"),
                        KeyCode::Char('2') => app.switch_to_theme("minimal"),
                        KeyCode::Char('3') => app.switch_to_theme("gruvbox"),
                        KeyCode::Char('4') => app.switch_to_theme("nord"),
                        KeyCode::Char('p') => app.cycle_theme(),
                        KeyCode::Char('r') => app.reset_to_theme_defaults(),
                        KeyCode::Char('e') | KeyCode::Char('E') => app.open_separator_editor(),
                        _ => {}
                    }
                }
            }

            if app.should_quit {
                break Ok(());
            }
        };

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn calculate_theme_selector_height(&self, total_width: u16) -> u16 {
        // Get all available themes dynamically
        let available_themes = crate::ui::themes::ThemePresets::list_available_themes();

        // Calculate available width (minus borders only)
        let content_width = total_width.saturating_sub(2); // Remove borders

        // Simulate the line wrapping logic
        let mut line_count = 1;
        let mut current_line_length = 0;
        let mut first_line = true;

        for (i, theme) in available_themes.iter().enumerate() {
            let marker = if self.config.theme == *theme {
                "[✓]"
            } else {
                "[ ]"
            };
            let theme_part = format!("{} {}", marker, theme);
            let separator = if i == 0 { "" } else { "  " };
            let part_with_sep = format!("{}{}", separator, theme_part);

            let would_fit = current_line_length + part_with_sep.len() <= content_width as usize;

            if would_fit || first_line {
                current_line_length += part_with_sep.len();
                first_line = false;
            } else {
                line_count += 1;
                current_line_length = theme_part.len();
            }
        }

        // Return height: content lines + borders (top + bottom)
        line_count + 2
    }

    fn calculate_help_height(&self, total_width: u16) -> u16 {
        // Use same help_items as in help.render
        let help_items = if self.color_picker.is_open {
            vec![
                "[↑↓] Navigate",
                "[Tab] Mode",
                "[Enter] Select",
                "[Esc] Cancel",
            ]
        } else if self.icon_selector.is_open {
            vec![
                "[↑↓] Navigate",
                "[Tab] Style",
                "[C] Custom",
                "[Enter] Select",
                "[Esc] Cancel",
            ]
        } else {
            vec![
                "[Tab] Switch Panel",
                "[Enter] Toggle/Edit",
                "[Shift+↑↓] Reorder",
                "[1-4] Theme",
                "[P] Switch Theme",
                "[R] Reset",
                "[E] Edit Separator",
                "[S] Save Config",
                "[W] Write Theme",
                "[Ctrl+S] Save Theme",
                "[Esc] Quit",
            ]
        };

        let content_width = total_width.saturating_sub(2); // Remove borders
        let mut lines_needed = 1u16;
        let mut current_width = 0usize;

        // Use same logic as help.render for line wrapping
        for (i, item) in help_items.iter().enumerate() {
            let item_width = item.chars().count();
            let needs_separator = i > 0 && current_width > 0;
            let separator_width = if needs_separator { 2 } else { 0 };
            let total_width = item_width + separator_width;

            if current_width + total_width > content_width as usize {
                // Need new line
                lines_needed += 1;
                current_width = item_width;
            } else {
                if needs_separator {
                    current_width += 2;
                }
                current_width += item_width;
            }
        }

        // Add line for status message if present
        if self.status_message.is_some() {
            lines_needed += 1;
        }

        // Return height: content lines + borders, max 6
        (lines_needed + 2).clamp(3, 6)
    }

    fn ui(&mut self, f: &mut Frame) {
        // Calculate required heights for dynamic sections (using full width as estimate)
        let theme_selector_height = self.calculate_theme_selector_height(f.area().width);
        let help_height = self.calculate_help_height(f.area().width);

        // Initial layout to measure preview width
        let initial_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),                     // Title
                Constraint::Min(3),                        // Preview (dynamic - will recalculate)
                Constraint::Length(theme_selector_height), // Theme selector (dynamic)
                Constraint::Min(10),                       // Main content
                Constraint::Length(help_height),           // Help (dynamic)
            ])
            .split(f.area());

        // Update preview with measured width
        self.preview
            .update_preview_with_width(&self.config, initial_layout[1].width);

        // Calculate actual preview height after content update
        let preview_height = self.preview.calculate_height();

        // Final layout with correct preview height
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),                     // Title
                Constraint::Length(preview_height),        // Preview (dynamic)
                Constraint::Length(theme_selector_height), // Theme selector (dynamic)
                Constraint::Min(10),                       // Main content
                Constraint::Length(help_height),           // Help (dynamic)
            ])
            .split(f.area());

        // Title
        let title_text = format!("CCometixLine Configurator v{}", env!("CARGO_PKG_VERSION"));
        let title = Paragraph::new(title_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(title, layout[0]);

        // Preview - use TUI-optimized statusline generation with smart segment wrapping
        // Update preview if layout width differs from initial measurement
        if layout[1].width != initial_layout[1].width {
            self.preview
                .update_preview_with_width(&self.config, layout[1].width);
        }

        // Render preview
        self.preview.render(f, layout[1]);

        // Theme selector
        self.theme_selector.render(f, layout[2], &self.config);

        // Main content (split horizontally)
        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(layout[3]);

        // Segment list
        self.segment_list.render(
            f,
            content_layout[0],
            &self.config,
            self.selected_segment,
            &self.selected_panel,
        );

        // Settings panel
        self.settings.render(
            f,
            content_layout[1],
            &self.config,
            self.selected_segment,
            &self.selected_panel,
            &self.selected_field,
        );

        // Help
        self.help.render(
            f,
            layout[4],
            self.status_message.as_deref(),
            self.color_picker.is_open,
            self.icon_selector.is_open,
        );

        // Render popups on top
        if self.color_picker.is_open {
            self.color_picker.render(f, f.area());
        }
        if self.icon_selector.is_open {
            self.icon_selector.render(f, f.area());
        }
        if self.name_input.is_open {
            self.name_input.render(f, f.area());
        }
        if self.separator_editor.is_open {
            self.separator_editor.render(f, f.area());
        }
    }

    fn move_selection(&mut self, delta: i32) {
        match self.selected_panel {
            Panel::SegmentList => {
                let new_selection = (self.selected_segment as i32 + delta)
                    .max(0)
                    .min((self.config.segments.len() - 1) as i32)
                    as usize;
                self.selected_segment = new_selection;
            }
            Panel::Settings => {
                // Check segment type to determine field count
                let is_usage_segment = self.config.segments.get(self.selected_segment)
                    .map(|s| matches!(s.id, crate::config::SegmentId::Usage5Hour | crate::config::SegmentId::Usage7Day | crate::config::SegmentId::ContextWindow))
                    .unwrap_or(false);

                let is_git_segment = self.config.segments.get(self.selected_segment)
                    .map(|s| matches!(s.id, crate::config::SegmentId::Git))
                    .unwrap_or(false);

                let field_count = if is_usage_segment {
                    THRESHOLD_SEGMENT_FIELD_COUNT
                } else if is_git_segment {
                    GIT_SEGMENT_FIELD_COUNT
                } else {
                    DEFAULT_SEGMENT_FIELD_COUNT
                };

                let current_field = match self.selected_field {
                    FieldSelection::Enabled => 0i32,
                    FieldSelection::Icon => 1,
                    FieldSelection::IconColor => 2,
                    FieldSelection::TextColor => 3,
                    FieldSelection::BackgroundColor => 4,
                    FieldSelection::TextStyle => 5,
                    FieldSelection::WarningThreshold => 6,
                    FieldSelection::CriticalThreshold => 7,
                    FieldSelection::WarningColor => 8,
                    FieldSelection::CriticalColor => 9,
                    FieldSelection::WarningBold => 10,
                    FieldSelection::CriticalBold => 11,
                    FieldSelection::ShowSha => 6,
                    FieldSelection::ShowDirtyCount => 7,
                    FieldSelection::Options => if is_usage_segment { 12 } else if is_git_segment { 8 } else { 6 },
                };
                let new_field = (current_field + delta).clamp(0, (field_count - 1) as i32) as usize;
                self.selected_field = match new_field {
                    0 => FieldSelection::Enabled,
                    1 => FieldSelection::Icon,
                    2 => FieldSelection::IconColor,
                    3 => FieldSelection::TextColor,
                    4 => FieldSelection::BackgroundColor,
                    5 => FieldSelection::TextStyle,
                    6 if is_usage_segment => FieldSelection::WarningThreshold,
                    7 if is_usage_segment => FieldSelection::CriticalThreshold,
                    8 if is_usage_segment => FieldSelection::WarningColor,
                    9 if is_usage_segment => FieldSelection::CriticalColor,
                    10 if is_usage_segment => FieldSelection::WarningBold,
                    11 if is_usage_segment => FieldSelection::CriticalBold,
                    12 if is_usage_segment => FieldSelection::Options,
                    6 if is_git_segment => FieldSelection::ShowSha,
                    7 if is_git_segment => FieldSelection::ShowDirtyCount,
                    8 if is_git_segment => FieldSelection::Options,
                    6 => FieldSelection::Options, // For default segments
                    _ => FieldSelection::Enabled,
                };
            }
        }
    }

    fn toggle_current(&mut self) {
        match self.selected_panel {
            Panel::SegmentList => {
                // Toggle segment enabled/disabled in segment list
                if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                    segment.enabled = !segment.enabled;
                    let segment_name = match segment.id {
                        SegmentId::Model => "Model",
                        SegmentId::Directory => "Directory",
                        SegmentId::Git => "Git",
                        SegmentId::ContextWindow => "Context Window",
                        SegmentId::Usage => "Usage",
                        SegmentId::Usage5Hour => "Usage (5-hour)",
                        SegmentId::Usage7Day => "Usage (7-day)",
                        SegmentId::Cost => "Cost",
                        SegmentId::Session => "Session",
                        SegmentId::OutputStyle => "Output Style",
                        SegmentId::Update => "Update",
                    };
                    let is_enabled = segment.enabled;
                    self.status_message = Some(format!(
                        "{} segment {}",
                        segment_name,
                        if is_enabled { "enabled" } else { "disabled" }
                    ));
                    self.preview.update_preview(&self.config);
                }
            }
            Panel::Settings => {
                // Edit field in settings panel
                match self.selected_field {
                    FieldSelection::Enabled => {
                        // Toggle enabled state in settings panel too
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            segment.enabled = !segment.enabled;
                            let segment_name = match segment.id {
                                SegmentId::Model => "Model",
                                SegmentId::Directory => "Directory",
                                SegmentId::Git => "Git",
                                SegmentId::ContextWindow => "Context Window",
                                SegmentId::Usage => "Usage",
                                SegmentId::Usage5Hour => "Usage (5-hour)",
                                SegmentId::Usage7Day => "Usage (7-day)",
                                SegmentId::Cost => "Cost",
                                SegmentId::Session => "Session",
                                SegmentId::OutputStyle => "Output Style",
                                SegmentId::Update => "Update",
                            };
                            let is_enabled = segment.enabled;
                            self.status_message = Some(format!(
                                "{} segment {}",
                                segment_name,
                                if is_enabled { "enabled" } else { "disabled" }
                            ));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::Icon => self.open_icon_selector(),
                    FieldSelection::IconColor
                    | FieldSelection::TextColor
                    | FieldSelection::BackgroundColor
                    | FieldSelection::WarningColor
                    | FieldSelection::CriticalColor => self.open_color_picker(),
                    FieldSelection::TextStyle => {
                        // Toggle text bold style
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            segment.styles.text_bold = !segment.styles.text_bold;
                            self.status_message = Some(format!(
                                "Text bold {}",
                                if segment.styles.text_bold {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            ));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::WarningThreshold => {
                        // Cycle through common warning thresholds
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            let current = segment
                                .options
                                .get("warning_threshold")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(60);
                            let new_value = match current {
                                x if x < 50 => 50,
                                50 => 60,
                                60 => 70,
                                70 => 80,
                                _ => 40,
                            };
                            segment.options.insert(
                                "warning_threshold".to_string(),
                                serde_json::Value::Number(new_value.into()),
                            );
                            self.status_message = Some(format!("Warning threshold set to {}%", new_value));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::CriticalThreshold => {
                        // Cycle through common critical thresholds
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            let current = segment
                                .options
                                .get("critical_threshold")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(80);
                            let new_value = match current {
                                x if x < 70 => 70,
                                70 => 80,
                                80 => 90,
                                90 => 95,
                                _ => 60,
                            };
                            segment.options.insert(
                                "critical_threshold".to_string(),
                                serde_json::Value::Number(new_value.into()),
                            );
                            self.status_message = Some(format!("Critical threshold set to {}%", new_value));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::WarningBold => {
                        // Toggle warning bold option
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            let current = segment
                                .options
                                .get("warning_bold")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let new_value = !current;
                            segment.options.insert(
                                "warning_bold".to_string(),
                                serde_json::Value::Bool(new_value),
                            );
                            self.status_message = Some(format!(
                                "Warning bold {}",
                                if new_value { "enabled" } else { "disabled" }
                            ));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::CriticalBold => {
                        // Toggle critical bold option
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            let current = segment
                                .options
                                .get("critical_bold")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(true);
                            let new_value = !current;
                            segment.options.insert(
                                "critical_bold".to_string(),
                                serde_json::Value::Bool(new_value),
                            );
                            self.status_message = Some(format!(
                                "Critical bold {}",
                                if new_value { "enabled" } else { "disabled" }
                            ));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::ShowSha => {
                        // Toggle show_sha option for Git segment
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            let current = segment
                                .options
                                .get("show_sha")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let new_value = !current;
                            segment.options.insert(
                                "show_sha".to_string(),
                                serde_json::Value::Bool(new_value),
                            );
                            self.status_message = Some(format!(
                                "Show SHA {}",
                                if new_value { "enabled" } else { "disabled" }
                            ));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::ShowDirtyCount => {
                        // Toggle show_dirty_count option for Git segment
                        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
                            let current = segment
                                .options
                                .get("show_dirty_count")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let new_value = !current;
                            segment.options.insert(
                                "show_dirty_count".to_string(),
                                serde_json::Value::Bool(new_value),
                            );
                            self.status_message = Some(format!(
                                "Show dirty count {}",
                                if new_value { "enabled" } else { "disabled" }
                            ));
                            self.preview.update_preview(&self.config);
                        }
                    }
                    FieldSelection::Options => {
                        // TODO: Implement options editor
                        self.status_message =
                            Some("Options editor not implemented yet".to_string());
                    }
                }
            }
        }
    }

    fn switch_panel(&mut self) {
        self.selected_panel = match self.selected_panel {
            Panel::SegmentList => Panel::Settings,
            Panel::Settings => Panel::SegmentList,
        };
    }

    fn open_color_picker(&mut self) {
        if self.selected_panel == Panel::Settings
            && (self.selected_field == FieldSelection::IconColor
                || self.selected_field == FieldSelection::TextColor
                || self.selected_field == FieldSelection::BackgroundColor
                || self.selected_field == FieldSelection::WarningColor
                || self.selected_field == FieldSelection::CriticalColor)
        {
            self.color_picker.open();
        }
    }

    fn open_icon_selector(&mut self) {
        if self.selected_panel == Panel::Settings && self.selected_field == FieldSelection::Icon {
            self.icon_selector.open(self.config.style.mode);
        }
    }

    fn apply_selected_color(&mut self, color: crate::config::AnsiColor) {
        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
            match self.selected_field {
                FieldSelection::IconColor => segment.colors.icon = Some(color),
                FieldSelection::TextColor => segment.colors.text = Some(color),
                FieldSelection::BackgroundColor => segment.colors.background = Some(color),
                FieldSelection::WarningColor => {
                    // Store warning color in options using shared helper
                    let color_str = color_utils::serialize_ansi_color_to_json(&color);
                    if let Ok(color_json) = serde_json::from_str::<serde_json::Value>(&color_str) {
                        segment.options.insert("warning_color".to_string(), color_json);
                    }
                }
                FieldSelection::CriticalColor => {
                    // Store critical color in options using shared helper
                    let color_str = color_utils::serialize_ansi_color_to_json(&color);
                    if let Ok(color_json) = serde_json::from_str::<serde_json::Value>(&color_str) {
                        segment.options.insert("critical_color".to_string(), color_json);
                    }
                }
                _ => {}
            }
            self.preview.update_preview(&self.config);
        }
    }

    fn apply_selected_icon(&mut self, icon: String) {
        if let Some(segment) = self.config.segments.get_mut(self.selected_segment) {
            match self.config.style.mode {
                StyleMode::Plain => segment.icon.plain = icon,
                StyleMode::NerdFont | StyleMode::Powerline => segment.icon.nerd_font = icon,
            }
            self.preview.update_preview(&self.config);
        }
    }

    fn cycle_theme(&mut self) {
        let themes = crate::ui::themes::ThemePresets::list_available_themes();
        let current_theme = &self.config.theme;
        let current_index = themes.iter().position(|t| t == current_theme).unwrap_or(0);
        let next_index = (current_index + 1) % themes.len();
        let next_theme = &themes[next_index];

        self.status_message = Some(format!("Switching to theme: {}", next_theme));
        self.switch_to_theme(next_theme);
    }

    fn switch_to_theme(&mut self, theme_name: &str) {
        self.config = crate::ui::themes::ThemePresets::get_theme(theme_name);
        self.selected_segment = 0;
        self.preview.update_preview(&self.config);
        self.status_message = Some(format!("Switched to {} theme", theme_name));
    }

    /// Reset current theme to its default configuration
    fn reset_to_theme_defaults(&mut self) {
        let current_theme = self.config.theme.clone();
        self.config = crate::ui::themes::ThemePresets::get_theme(&current_theme);
        self.selected_segment = 0;
        self.preview.update_preview(&self.config);
        self.status_message = Some(format!("Reset {} theme to defaults", current_theme));
    }

    fn save_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config.save()?;
        Ok(())
    }

    /// Move the currently selected segment up in the list
    fn move_segment_up(&mut self) {
        if self.selected_panel == Panel::SegmentList && self.selected_segment > 0 {
            let current_idx = self.selected_segment;
            self.config.segments.swap(current_idx, current_idx - 1);
            self.selected_segment -= 1;
            self.preview.update_preview(&self.config);
            self.status_message = Some("Moved segment up".to_string());
        }
    }

    /// Move the currently selected segment down in the list
    fn move_segment_down(&mut self) {
        if self.selected_panel == Panel::SegmentList
            && self.selected_segment < self.config.segments.len() - 1
        {
            let current_idx = self.selected_segment;
            self.config.segments.swap(current_idx, current_idx + 1);
            self.selected_segment += 1;
            self.preview.update_preview(&self.config);
            self.status_message = Some("Moved segment down".to_string());
        }
    }

    /// Write current config to the current theme file
    fn write_to_current_theme(&mut self) {
        let current_theme = &self.config.theme;
        match crate::ui::themes::ThemePresets::save_theme(current_theme, &self.config) {
            Ok(_) => {
                self.status_message = Some(format!("Wrote config to theme: {}", current_theme));
            }
            Err(e) => {
                self.status_message =
                    Some(format!("Failed to write to theme {}: {}", current_theme, e));
            }
        }
    }

    /// Save current config as a new theme with the given name
    fn save_as_new_theme(&mut self, theme_name: &str) {
        match crate::ui::themes::ThemePresets::save_theme(theme_name, &self.config) {
            Ok(_) => {
                // Update current theme to the new one
                self.config.theme = theme_name.to_string();
                self.status_message = Some(format!("Saved as new theme: {}", theme_name));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to save theme {}: {}", theme_name, e));
            }
        }
    }

    /// Open separator editor with current separator
    fn open_separator_editor(&mut self) {
        self.status_message = Some("Opening separator editor...".to_string());
        self.separator_editor.open(&self.config.style.separator);
    }
}
