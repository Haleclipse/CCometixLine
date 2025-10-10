use super::{color_utils, threshold_utils, Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use crate::core::segments::usage::UsageSegment;
use std::collections::HashMap;

#[derive(Default)]
pub struct Usage7DaySegment;

impl Usage7DaySegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for Usage7DaySegment {
    fn collect(&self, _input: &InputData) -> Option<SegmentData> {
        // Load the shared cache created by UsageSegment
        let cache = UsageSegment::load_usage_cache()?;

        // Note: seven_day_utilization is a percentage (0-100) from the API
        let seven_day_util = cache.seven_day_utilization;
        let reset_time = UsageSegment::format_7day_reset_time(cache.seven_day_resets_at.as_deref());

        // Convert percentage (0-100) to normalized value (0-1) for get_circle_icon
        let dynamic_icon = UsageSegment::get_circle_icon(seven_day_util / 100.0);

        let seven_day_percent = seven_day_util.round() as u8;
        let primary = format!("{}%", seven_day_percent);
        let secondary = format!("→ {}", reset_time);

        let mut metadata = HashMap::new();
        metadata.insert("dynamic_icon".to_string(), dynamic_icon);
        metadata.insert("seven_day_utilization".to_string(), seven_day_util.to_string());

        // Check if we need to apply threshold-based color override
        if let Some(color) = threshold_utils::get_color_for_utilization(SegmentId::Usage7Day, seven_day_util) {
            // Serialize the color to JSON for metadata using shared helper
            let color_json = color_utils::serialize_ansi_color_to_json(&color);
            metadata.insert("text_color_override".to_string(), color_json);
        }

        // Check if we need to apply threshold-based bold override
        if let Some(should_bold) = threshold_utils::should_be_bold(SegmentId::Usage7Day, seven_day_util) {
            metadata.insert("text_bold_override".to_string(), should_bold.to_string());
        }

        Some(SegmentData {
            primary,
            secondary,
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Usage7Day
    }
}
