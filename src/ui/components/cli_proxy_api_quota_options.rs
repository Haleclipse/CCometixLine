use crate::config::{AnsiColor, Config, SegmentId};
use crate::core::segments::TrackedModel;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliProxyApiQuotaOptionField {
    Alias(TrackedModel),
    Color(TrackedModel),
    Separator,
}

#[derive(Debug, Clone)]
pub struct CliProxyApiQuotaOptionsComponent {
    pub is_open: bool,
    selected: usize,
}

impl Default for CliProxyApiQuotaOptionsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl CliProxyApiQuotaOptionsComponent {
    pub fn new() -> Self {
        Self {
            is_open: false,
            selected: 0,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn move_selection(&mut self, delta: i32) {
        let max = Self::fields().len().saturating_sub(1) as i32;
        self.selected = (self.selected as i32 + delta).clamp(0, max) as usize;
    }

    pub fn selected_field(&self) -> CliProxyApiQuotaOptionField {
        Self::fields()
            .get(self.selected)
            .copied()
            .unwrap_or(CliProxyApiQuotaOptionField::Separator)
    }

    fn fields() -> &'static [CliProxyApiQuotaOptionField] {
        &[
            CliProxyApiQuotaOptionField::Alias(TrackedModel::Opus),
            CliProxyApiQuotaOptionField::Color(TrackedModel::Opus),
            CliProxyApiQuotaOptionField::Alias(TrackedModel::Gemini3Pro),
            CliProxyApiQuotaOptionField::Color(TrackedModel::Gemini3Pro),
            CliProxyApiQuotaOptionField::Alias(TrackedModel::Gemini3Flash),
            CliProxyApiQuotaOptionField::Color(TrackedModel::Gemini3Flash),
            CliProxyApiQuotaOptionField::Separator,
        ]
    }

    fn get_alias(options: &HashMap<String, Value>, model: TrackedModel) -> String {
        options
            .get(model.alias_key())
            .and_then(|v| v.as_str())
            .unwrap_or(model.default_alias())
            .to_string()
    }

    fn get_color(options: &HashMap<String, Value>, model: TrackedModel) -> Option<AnsiColor> {
        options
            .get(model.color_key())
            .and_then(|v| serde_json::from_value::<AnsiColor>(v.clone()).ok())
    }

    fn color_to_desc(color: &Option<AnsiColor>) -> String {
        match color {
            Some(AnsiColor::Color16 { c16 }) => format!("c16:{}", c16),
            Some(AnsiColor::Color256 { c256 }) => format!("c256:{}", c256),
            Some(AnsiColor::Rgb { r, g, b }) => format!("rgb({},{},{})", r, g, b),
            None => "default".to_string(),
        }
    }

    fn to_ratatui_color(color: &AnsiColor) -> Color {
        match color {
            AnsiColor::Color16 { c16 } => match c16 {
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
            AnsiColor::Color256 { c256 } => Color::Indexed(*c256),
            AnsiColor::Rgb { r, g, b } => Color::Rgb(*r, *g, *b),
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect, config: &Config, selected_segment: usize) {
        if !self.is_open {
            return;
        }

        let Some(segment) = config.segments.get(selected_segment) else {
            return;
        };
        if segment.id != SegmentId::CliProxyApiQuota {
            return;
        }

        // Avoid covering bottom help area
        let popup_width = 70_u16.min(area.width.saturating_sub(4));
        let popup_height = 16_u16;
        let max_y = area.height.saturating_sub(popup_height + 4);
        let popup_y = if max_y > 2 {
            (area.height.saturating_sub(popup_height)) / 2
        } else {
            2
        };
        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: popup_y.min(max_y),
            width: popup_width,
            height: popup_height,
        };

        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("CLI Proxy API Quota Options");
        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(3)])
            .split(inner);

        let mut lines: Vec<Line<'static>> = Vec::new();

        for (idx, field) in Self::fields().iter().enumerate() {
            let is_selected = idx == self.selected;
            let cursor = if is_selected { "▶ " } else { "  " };
            let cursor_style = if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };

            let mut spans = vec![Span::styled(cursor.to_string(), cursor_style)];

            match field {
                CliProxyApiQuotaOptionField::Alias(model) => {
                    let alias = Self::get_alias(&segment.options, *model);
                    spans.push(Span::raw(format!("{} Alias: ", model.display_name())));
                    spans.push(Span::styled(
                        alias,
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ));
                }
                CliProxyApiQuotaOptionField::Color(model) => {
                    let color = Self::get_color(&segment.options, *model);
                    spans.push(Span::raw(format!("{} Color: ", model.display_name())));
                    spans.push(Span::styled(
                        Self::color_to_desc(&color),
                        Style::default().fg(Color::Yellow),
                    ));
                    if let Some(c) = &color {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            "██".to_string(),
                            Style::default().fg(Self::to_ratatui_color(c)),
                        ));
                    }
                }
                CliProxyApiQuotaOptionField::Separator => {
                    let sep = segment
                        .options
                        .get("separator")
                        .and_then(|v| v.as_str())
                        .unwrap_or(" | ");
                    spans.push(Span::raw("Separator: ".to_string()));
                    spans.push(Span::styled(
                        sep.to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ));
                }
            }

            lines.push(Line::from(spans));
        }

        let text = Text::from(lines);
        f.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Fields (↑↓, Enter)"),
            ),
            chunks[0],
        );

        f.render_widget(
            Paragraph::new("[↑↓] Navigate  [Enter] Edit  [Esc] Close")
                .block(Block::default().borders(Borders::ALL)),
            chunks[1],
        );
    }
}
