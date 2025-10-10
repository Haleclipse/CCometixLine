use super::segment_list::{FieldSelection, Panel};
use crate::config::{Config, SegmentId, StyleMode};
use crate::core::segments::color_utils;
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
                SegmentId::Usage5Hour => "Usage (5-hour)",
                SegmentId::Usage7Day => "Usage (7-day)",
                SegmentId::Cost => "Cost",
                SegmentId::Session => "Session",
                SegmentId::OutputStyle => "Output Style",
                SegmentId::Update => "Update",
            };
            let current_icon = match config.style.mode {
                StyleMode::Plain => &segment.icon.plain,
                StyleMode::NerdFont | StyleMode::Powerline => &segment.icon.nerd_font,
            };
            // Convert AnsiColor to ratatui Color using shared helper
            let icon_ratatui_color = segment.colors.icon
                .as_ref()
                .map(|c| color_utils::ansi_color_to_ratatui(c))
                .unwrap_or(Color::White);
            let text_ratatui_color = segment.colors.text
                .as_ref()
                .map(|c| color_utils::ansi_color_to_ratatui(c))
                .unwrap_or(Color::White);
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
            let background_ratatui_color = segment.colors.background
                .as_ref()
                .map(|c| color_utils::ansi_color_to_ratatui(c))
                .unwrap_or(Color::White);
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

            // Check if this is a usage segment to show threshold fields
            let is_usage_segment = matches!(
                segment.id,
                SegmentId::Usage5Hour | SegmentId::Usage7Day
            );

            let mut lines = vec![
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
            ];

            // Add threshold fields for usage segments
            if is_usage_segment {
                let warning_threshold = segment
                    .options
                    .get("warning_threshold")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(60);
                let critical_threshold = segment
                    .options
                    .get("critical_threshold")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(80);

                // Get warning color description and ratatui color using shared helper
                let (warning_color_desc, warning_ratatui_color) = if let Some(color_val) = segment.options.get("warning_color") {
                    if let Some(c256) = color_val.get("c256").and_then(|v| v.as_u64()) {
                        (format!("256:{}", c256), Some(Color::Indexed(c256 as u8)))
                    } else if let Some(c16) = color_val.get("c16").and_then(|v| v.as_u64()) {
                        let color = color_utils::c16_to_ratatui_color(c16 as u8);
                        (format!("16:{}", c16), Some(color))
                    } else {
                        ("Not set".to_string(), None)
                    }
                } else {
                    ("Not set".to_string(), None)
                };

                // Get critical color description and ratatui color using shared helper
                let (critical_color_desc, critical_ratatui_color) = if let Some(color_val) = segment.options.get("critical_color") {
                    if let Some(c256) = color_val.get("c256").and_then(|v| v.as_u64()) {
                        (format!("256:{}", c256), Some(Color::Indexed(c256 as u8)))
                    } else if let Some(c16) = color_val.get("c16").and_then(|v| v.as_u64()) {
                        let color = color_utils::c16_to_ratatui_color(c16 as u8);
                        (format!("16:{}", c16), Some(color))
                    } else {
                        ("Not set".to_string(), None)
                    }
                } else {
                    ("Not set".to_string(), None)
                };

                // Get bold settings
                let warning_bold = segment
                    .options
                    .get("warning_bold")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let critical_bold = segment
                    .options
                    .get("critical_bold")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                lines.extend(vec![
                    create_field_line(
                        FieldSelection::WarningThreshold,
                        vec![Span::raw(format!("├─ Warning Threshold: {}%", warning_threshold))],
                    ),
                    create_field_line(
                        FieldSelection::CriticalThreshold,
                        vec![Span::raw(format!("├─ Critical Threshold: {}%", critical_threshold))],
                    ),
                    create_field_line(
                        FieldSelection::WarningColor,
                        {
                            let mut spans = vec![Span::raw(format!("├─ Warning Color: {} ", warning_color_desc))];
                            if let Some(color) = warning_ratatui_color {
                                spans.push(Span::styled("██".to_string(), Style::default().fg(color)));
                            } else {
                                spans.push(Span::styled("--".to_string(), Style::default().fg(Color::DarkGray)));
                            }
                            spans
                        },
                    ),
                    create_field_line(
                        FieldSelection::CriticalColor,
                        {
                            let mut spans = vec![Span::raw(format!("├─ Critical Color: {} ", critical_color_desc))];
                            if let Some(color) = critical_ratatui_color {
                                spans.push(Span::styled("██".to_string(), Style::default().fg(color)));
                            } else {
                                spans.push(Span::styled("--".to_string(), Style::default().fg(Color::DarkGray)));
                            }
                            spans
                        },
                    ),
                    create_field_line(
                        FieldSelection::WarningBold,
                        vec![Span::raw(format!(
                            "├─ Warning Bold: {}",
                            if warning_bold { "[✓]" } else { "[ ]" }
                        ))],
                    ),
                    create_field_line(
                        FieldSelection::CriticalBold,
                        vec![Span::raw(format!(
                            "├─ Critical Bold: {}",
                            if critical_bold { "[✓]" } else { "[ ]" }
                        ))],
                    ),
                ]);
            }

            // Add Options field (always last)
            lines.push(create_field_line(
                FieldSelection::Options,
                vec![Span::raw(format!(
                    "└─ Options: {} items",
                    segment.options.len()
                ))],
            ));
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
