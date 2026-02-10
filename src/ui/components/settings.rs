use super::segment_list::{FieldSelection, Panel};
use crate::config::{Config, SegmentId, StyleMode};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Default)]
pub struct SettingsComponent;

impl SettingsComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        config: &Config,
        selected_segment: usize,
        selected_panel: &Panel,
        selected_field: &FieldSelection,
    ) {
        if let Some(segment) = config.segments.get(selected_segment) {
            let segment_name = match segment.id {
                SegmentId::Model => "Model",
                SegmentId::Directory => "Directory",
                SegmentId::Git => "Git",
                SegmentId::ContextWindow => "Context Window",
                SegmentId::Usage => "Usage",
                SegmentId::Cost => "Cost",
                SegmentId::Session => "Session",
                SegmentId::OutputStyle => "Output Style",
                SegmentId::Update => "Update",
                SegmentId::ConfigCounts => "Config Counts",
            };
            let current_icon = match config.style.mode {
                StyleMode::Plain => &segment.icon.plain,
                StyleMode::NerdFont | StyleMode::Powerline => &segment.icon.nerd_font,
            };
            // Convert AnsiColor to ratatui Color
            let icon_ratatui_color = match &segment.colors.icon {
                Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
                    0 => Color::Black,
                    1 => Color::Red,
                    2 => Color::Green,
                    3 => Color::Yellow,
                    4 => Color::Blue,
                    5 => Color::Magenta,
                    6 => Color::Cyan,
                    7 => Color::White,
                    8 => Color::DarkGray,
                    9 => Color::LightRed,
                    10 => Color::LightGreen,
                    11 => Color::LightYellow,
                    12 => Color::LightBlue,
                    13 => Color::LightMagenta,
                    14 => Color::LightCyan,
                    15 => Color::Gray,
                    _ => Color::White,
                },
                Some(crate::config::AnsiColor::Color256 { c256 }) => Color::Indexed(*c256),
                Some(crate::config::AnsiColor::Rgb { r, g, b }) => Color::Rgb(*r, *g, *b),
                None => Color::White,
            };
            let text_ratatui_color = match &segment.colors.text {
                Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
                    0 => Color::Black,
                    1 => Color::Red,
                    2 => Color::Green,
                    3 => Color::Yellow,
                    4 => Color::Blue,
                    5 => Color::Magenta,
                    6 => Color::Cyan,
                    7 => Color::White,
                    8 => Color::DarkGray,
                    9 => Color::LightRed,
                    10 => Color::LightGreen,
                    11 => Color::LightYellow,
                    12 => Color::LightBlue,
                    13 => Color::LightMagenta,
                    14 => Color::LightCyan,
                    15 => Color::Gray,
                    _ => Color::White,
                },
                Some(crate::config::AnsiColor::Color256 { c256 }) => Color::Indexed(*c256),
                Some(crate::config::AnsiColor::Rgb { r, g, b }) => Color::Rgb(*r, *g, *b),
                None => Color::White,
            };
            let icon_color_desc = match &segment.colors.icon {
                Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
                    0 => "Black".to_string(),
                    1 => "Red".to_string(),
                    2 => "Green".to_string(),
                    3 => "Yellow".to_string(),
                    4 => "Blue".to_string(),
                    5 => "Magenta".to_string(),
                    6 => "Cyan".to_string(),
                    7 => "White".to_string(),
                    8 => "Dark Gray".to_string(),
                    9 => "Light Red".to_string(),
                    10 => "Light Green".to_string(),
                    11 => "Light Yellow".to_string(),
                    12 => "Light Blue".to_string(),
                    13 => "Light Magenta".to_string(),
                    14 => "Light Cyan".to_string(),
                    15 => "Gray".to_string(),
                    _ => format!("ANSI {}", c16),
                },
                Some(crate::config::AnsiColor::Color256 { c256 }) => format!("256:{}", c256),
                Some(crate::config::AnsiColor::Rgb { r, g, b }) => {
                    format!("RGB({},{},{})", r, g, b)
                }
                None => "Default".to_string(),
            };
            let text_color_desc = match &segment.colors.text {
                Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
                    0 => "Black".to_string(),
                    1 => "Red".to_string(),
                    2 => "Green".to_string(),
                    3 => "Yellow".to_string(),
                    4 => "Blue".to_string(),
                    5 => "Magenta".to_string(),
                    6 => "Cyan".to_string(),
                    7 => "White".to_string(),
                    8 => "Dark Gray".to_string(),
                    9 => "Light Red".to_string(),
                    10 => "Light Green".to_string(),
                    11 => "Light Yellow".to_string(),
                    12 => "Light Blue".to_string(),
                    13 => "Light Magenta".to_string(),
                    14 => "Light Cyan".to_string(),
                    15 => "Gray".to_string(),
                    _ => format!("ANSI {}", c16),
                },
                Some(crate::config::AnsiColor::Color256 { c256 }) => format!("256:{}", c256),
                Some(crate::config::AnsiColor::Rgb { r, g, b }) => {
                    format!("RGB({},{},{})", r, g, b)
                }
                None => "Default".to_string(),
            };
            let background_ratatui_color = match &segment.colors.background {
                Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
                    0 => Color::Black,
                    1 => Color::Red,
                    2 => Color::Green,
                    3 => Color::Yellow,
                    4 => Color::Blue,
                    5 => Color::Magenta,
                    6 => Color::Cyan,
                    7 => Color::White,
                    8 => Color::DarkGray,
                    9 => Color::LightRed,
                    10 => Color::LightGreen,
                    11 => Color::LightYellow,
                    12 => Color::LightBlue,
                    13 => Color::LightMagenta,
                    14 => Color::LightCyan,
                    15 => Color::Gray,
                    _ => Color::White,
                },
                Some(crate::config::AnsiColor::Color256 { c256 }) => Color::Indexed(*c256),
                Some(crate::config::AnsiColor::Rgb { r, g, b }) => Color::Rgb(*r, *g, *b),
                None => Color::White,
            };
            let background_color_desc = match &segment.colors.background {
                Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
                    0 => "Black".to_string(),
                    1 => "Red".to_string(),
                    2 => "Green".to_string(),
                    3 => "Yellow".to_string(),
                    4 => "Blue".to_string(),
                    5 => "Magenta".to_string(),
                    6 => "Cyan".to_string(),
                    7 => "White".to_string(),
                    8 => "Dark Gray".to_string(),
                    9 => "Light Red".to_string(),
                    10 => "Light Green".to_string(),
                    11 => "Light Yellow".to_string(),
                    12 => "Light Blue".to_string(),
                    13 => "Light Magenta".to_string(),
                    14 => "Light Cyan".to_string(),
                    15 => "Gray".to_string(),
                    _ => format!("ANSI {}", c16),
                },
                Some(crate::config::AnsiColor::Color256 { c256 }) => format!("256:{}", c256),
                Some(crate::config::AnsiColor::Rgb { r, g, b }) => {
                    format!("RGB({},{},{})", r, g, b)
                }
                None => "None".to_string(),
            };
            let create_field_line = |field: FieldSelection, content: Vec<Span<'static>>| {
                let is_selected = *selected_panel == Panel::Settings && *selected_field == field;
                let mut spans = vec![];

                if is_selected {
                    spans.push(Span::styled(
                        "▶ ".to_string(),
                        Style::default().fg(Color::Cyan),
                    ));
                } else {
                    spans.push(Span::raw("  ".to_string()));
                }

                spans.extend(content);
                Line::from(spans)
            };
            let lines = vec![
                Line::from(format!("{} Segment", segment_name)),
                create_field_line(
                    FieldSelection::Enabled,
                    vec![Span::raw(format!(
                        "├─ Enabled: {}",
                        if segment.enabled { "✓" } else { "✗" }
                    ))],
                ),
                create_field_line(
                    FieldSelection::Icon,
                    vec![
                        Span::raw("├─ Icon: ".to_string()),
                        Span::styled(
                            current_icon.to_string(),
                            Style::default().fg(icon_ratatui_color),
                        ),
                    ],
                ),
                create_field_line(
                    FieldSelection::IconColor,
                    vec![
                        Span::raw(format!("├─ Icon Color: {} ", icon_color_desc)),
                        Span::styled("██".to_string(), Style::default().fg(icon_ratatui_color)),
                    ],
                ),
                create_field_line(
                    FieldSelection::TextColor,
                    vec![
                        Span::raw(format!("├─ Text Color: {} ", text_color_desc)),
                        Span::styled("██".to_string(), Style::default().fg(text_ratatui_color)),
                    ],
                ),
                create_field_line(
                    FieldSelection::BackgroundColor,
                    vec![
                        Span::raw(format!("├─ Background Color: {} ", background_color_desc)),
                        if segment.colors.background.is_some() {
                            Span::styled(
                                "██".to_string(),
                                Style::default().fg(background_ratatui_color),
                            )
                        } else {
                            Span::styled("--".to_string(), Style::default().fg(Color::DarkGray))
                        },
                    ],
                ),
                create_field_line(
                    FieldSelection::TextStyle,
                    vec![Span::raw(format!(
                        "├─ Text Style: Bold {}",
                        if segment.styles.text_bold {
                            "[✓]"
                        } else {
                            "[ ]"
                        }
                    ))],
                ),
                create_field_line(
                    FieldSelection::Options,
                    vec![Span::raw(format!(
                        "└─ Options: {} items",
                        segment.options.len()
                    ))],
                ),
            ];
            let text = Text::from(lines);
            let settings_block = Block::default()
                .borders(Borders::ALL)
                .title("Settings")
                .border_style(if *selected_panel == Panel::Settings {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                });
            let settings_panel = Paragraph::new(text).block(settings_block);
            f.render_widget(settings_panel, area);
        } else {
            let settings_block = Block::default()
                .borders(Borders::ALL)
                .title("Settings")
                .border_style(if *selected_panel == Panel::Settings {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                });
            let settings_panel = Paragraph::new("No segment selected").block(settings_block);
            f.render_widget(settings_panel, area);
        }
    }
}
