use crate::config::{Config, SegmentId};
use crate::core::segments::SegmentData;
use crate::core::StatusLineGenerator;
use ratatui::{
    layout::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::collections::HashMap;

pub struct PreviewComponent {
    preview_cache: String,
    preview_text: Text<'static>,
}

impl Default for PreviewComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewComponent {
    pub fn new() -> Self {
        Self {
            preview_cache: String::new(),
            preview_text: Text::default(),
        }
    }

    pub fn update_preview(&mut self, config: &Config) {
        self.update_preview_with_width(config, 80); // Default width
    }

    pub fn update_preview_with_width(&mut self, config: &Config, width: u16) {
        // Generate mock segments data directly for preview
        let segments_data = self.generate_mock_segments_data(config);

        // Generate both string and TUI text versions
        let renderer = StatusLineGenerator::new(config.clone());

        // Keep string version for compatibility (if needed elsewhere)
        self.preview_cache = renderer.generate(segments_data.clone());

        // Generate TUI-optimized text with smart segment wrapping for preview display
        // Use actual available width minus borders
        let content_width = width.saturating_sub(2);
        let preview_result = renderer.generate_for_tui_preview(segments_data, content_width);

        // Convert to owned text by cloning the spans
        let owned_lines: Vec<Line<'static>> = preview_result
            .lines
            .into_iter()
            .map(|line| {
                let owned_spans: Vec<ratatui::text::Span<'static>> = line
                    .spans
                    .into_iter()
                    .map(|span| ratatui::text::Span::styled(span.content.to_string(), span.style))
                    .collect();
                Line::from(owned_spans)
            })
            .collect();

        self.preview_text = Text::from(owned_lines);
    }

    pub fn calculate_height(&self) -> u16 {
        let line_count = self.preview_text.lines.len().max(1);
        // Min 3 (1 line + 2 borders), max 8 to prevent taking too much space
        ((line_count + 2).max(3) as u16).min(8)
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let preview = Paragraph::new(self.preview_text.clone())
            .block(Block::default().borders(Borders::ALL).title("Preview"))
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(preview, area);
    }

    pub fn get_preview_cache(&self) -> &str {
        &self.preview_cache
    }

    /// Get threshold-based color override for usage segments
    fn get_threshold_color(&self, segment_config: &crate::config::SegmentConfig, utilization: f64) -> Option<String> {
        // Get threshold values from options
        let warning_threshold = segment_config
            .options
            .get("warning_threshold")
            .and_then(|v| v.as_u64())
            .unwrap_or(60) as f64;

        let critical_threshold = segment_config
            .options
            .get("critical_threshold")
            .and_then(|v| v.as_u64())
            .unwrap_or(80) as f64;

        // Determine which color to use based on utilization
        if utilization >= critical_threshold {
            // Critical threshold exceeded - use critical color
            segment_config
                .options
                .get("critical_color")
                .map(|v| v.to_string())
        } else if utilization >= warning_threshold {
            // Warning threshold exceeded - use warning color
            segment_config
                .options
                .get("warning_color")
                .map(|v| v.to_string())
        } else {
            // Below warning threshold - no override
            None
        }
    }

    /// Generate mock segments data for preview display
    /// This creates perfect preview data without depending on real environment
    fn generate_mock_segments_data(
        &self,
        config: &Config,
    ) -> Vec<(crate::config::SegmentConfig, SegmentData)> {
        let mut segments_data = Vec::new();

        for segment_config in &config.segments {
            if !segment_config.enabled {
                continue;
            }

            let mock_data = match segment_config.id {
                SegmentId::Model => SegmentData {
                    primary: "Sonnet 4".to_string(),
                    secondary: "".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("model".to_string(), "claude-4-sonnet-20250512".to_string());
                        map
                    },
                },
                SegmentId::Directory => SegmentData {
                    primary: "CCometixLine".to_string(),
                    secondary: "".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("current_dir".to_string(), "~/CCometixLine".to_string());
                        map
                    },
                },
                SegmentId::Git => SegmentData {
                    primary: "master".to_string(),
                    secondary: "✓".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("branch".to_string(), "master".to_string());
                        map.insert("status".to_string(), "Clean".to_string());
                        map.insert("ahead".to_string(), "0".to_string());
                        map.insert("behind".to_string(), "0".to_string());
                        map
                    },
                },
                SegmentId::ContextWindow => SegmentData {
                    primary: "78.2%".to_string(),
                    secondary: "· 156.4k".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("total_tokens".to_string(), "156400".to_string());
                        map.insert("percentage".to_string(), "78.2".to_string());
                        map.insert("session_tokens".to_string(), "48200".to_string());
                        map
                    },
                },
                SegmentId::Usage => SegmentData {
                    primary: "24%".to_string(),
                    secondary: "· 10-7-2".to_string(),
                    metadata: HashMap::new(),
                },
                SegmentId::Usage5Hour => {
                    // Use mock utilization that demonstrates warning threshold (65% > default 60%)
                    let utilization = 65.0;
                    let mut metadata = HashMap::new();
                    metadata.insert("dynamic_icon".to_string(), "\u{f0aa3}".to_string()); // circle_slice_6
                    metadata.insert("five_hour_utilization".to_string(), utilization.to_string());

                    // Apply threshold-based color override
                    if let Some(color_override) = self.get_threshold_color(segment_config, utilization) {
                        metadata.insert("text_color_override".to_string(), color_override);
                    }

                    SegmentData {
                        primary: "65%".to_string(),
                        secondary: "→ 11am".to_string(),
                        metadata,
                    }
                },
                SegmentId::Usage7Day => {
                    // Use mock utilization that demonstrates critical threshold (85% > default 80%)
                    let utilization = 85.0;
                    let mut metadata = HashMap::new();
                    metadata.insert("dynamic_icon".to_string(), "\u{f0aa4}".to_string()); // circle_slice_7
                    metadata.insert("seven_day_utilization".to_string(), utilization.to_string());

                    // Apply threshold-based color override
                    if let Some(color_override) = self.get_threshold_color(segment_config, utilization) {
                        metadata.insert("text_color_override".to_string(), color_override);
                    }

                    SegmentData {
                        primary: "85%".to_string(),
                        secondary: "→ Oct 9:5am".to_string(),
                        metadata,
                    }
                },
                SegmentId::Cost => SegmentData {
                    primary: "$0.02".to_string(),
                    secondary: "".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("cost".to_string(), "0.01234".to_string());
                        map
                    },
                },
                SegmentId::Session => SegmentData {
                    primary: "3m45s".to_string(),
                    secondary: "+156 -23".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("duration_ms".to_string(), "225000".to_string());
                        map.insert("lines_added".to_string(), "156".to_string());
                        map.insert("lines_removed".to_string(), "23".to_string());
                        map
                    },
                },
                SegmentId::OutputStyle => SegmentData {
                    primary: "default".to_string(),
                    secondary: "".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("style_name".to_string(), "default".to_string());
                        map
                    },
                },
                SegmentId::Update => SegmentData {
                    primary: format!("v{}", env!("CARGO_PKG_VERSION")),
                    secondary: "".to_string(),
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert(
                            "current_version".to_string(),
                            env!("CARGO_PKG_VERSION").to_string(),
                        );
                        map.insert("update_available".to_string(), "false".to_string());
                        map
                    },
                },
            };

            segments_data.push((segment_config.clone(), mock_data));
        }

        segments_data
    }
}
