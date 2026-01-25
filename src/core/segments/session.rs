use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

#[derive(Default)]
pub struct SessionSegment;

impl SessionSegment {
    pub fn new() -> Self {
        Self
    }

    fn format_duration(ms: u64) -> String {
        if ms < 1000 {
            format!("{}ms", ms)
        } else if ms < 60_000 {
            let seconds = ms / 1000;
            format!("{}s", seconds)
        } else if ms < 3_600_000 {
            let minutes = ms / 60_000;
            let seconds = (ms % 60_000) / 1000;
            if seconds == 0 {
                format!("{}m", minutes)
            } else {
                format!("{}m{}s", minutes, seconds)
            }
        } else {
            let hours = ms / 3_600_000;
            let minutes = (ms % 3_600_000) / 60_000;
            if minutes == 0 {
                format!("{}h", hours)
            } else {
                format!("{}h{}m", hours, minutes)
            }
        }
    }
}

impl Segment for SessionSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let cost_data = input.cost.as_ref()?;

        // Primary display: total duration
        let primary = if let Some(duration) = cost_data.total_duration_ms {
            Self::format_duration(duration)
        } else {
            return None;
        };

        // Secondary display: line changes if available (green for +, red for -)
        let secondary = match (cost_data.total_lines_added, cost_data.total_lines_removed) {
            (Some(added), Some(removed)) if added > 0 || removed > 0 => {
                format!("\x1b[32m+{}\x1b[0m \x1b[31m-{}\x1b[0m", added, removed)
            }
            (Some(added), None) if added > 0 => {
                format!("\x1b[32m+{}\x1b[0m", added)
            }
            (None, Some(removed)) if removed > 0 => {
                format!("\x1b[31m-{}\x1b[0m", removed)
            }
            _ => String::new(),
        };

        let mut metadata = HashMap::new();
        if let Some(duration) = cost_data.total_duration_ms {
            metadata.insert("duration_ms".to_string(), duration.to_string());
        }
        if let Some(api_duration) = cost_data.total_api_duration_ms {
            metadata.insert("api_duration_ms".to_string(), api_duration.to_string());
        }
        if let Some(added) = cost_data.total_lines_added {
            metadata.insert("lines_added".to_string(), added.to_string());
        }
        if let Some(removed) = cost_data.total_lines_removed {
            metadata.insert("lines_removed".to_string(), removed.to_string());
        }

        Some(SegmentData {
            primary,
            secondary,
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Session
    }
}
