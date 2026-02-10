use crate::config::{Config, SegmentId};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Panel {
    SegmentList,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldSelection {
    Enabled,
    Icon,
    IconColor,
    TextColor,
    BackgroundColor,
    TextStyle,
    Options,
}

#[derive(Default)]
pub struct SegmentListComponent;

impl SegmentListComponent {
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
    ) {
        let items: Vec<ListItem> = config
            .segments
            .iter()
            .enumerate()
            .map(|(i, segment)| {
                let is_selected = i == selected_segment && *selected_panel == Panel::SegmentList;
                let enabled_marker = if segment.enabled { "●" } else { "○" };
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

                if is_selected {
                    // Selected item with colored cursor
                    ListItem::new(Line::from(vec![
                        Span::styled("▶ ", Style::default().fg(Color::Cyan)),
                        Span::raw(format!("{} {}", enabled_marker, segment_name)),
                    ]))
                } else {
                    // Non-selected item
                    ListItem::new(format!("  {} {}", enabled_marker, segment_name))
                }
            })
            .collect();
        let segments_block = Block::default()
            .borders(Borders::ALL)
            .title("Segments")
            .border_style(if *selected_panel == Panel::SegmentList {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });
        let segments_list = List::new(items).block(segments_block);
        f.render_widget(segments_list, area);
    }
}
