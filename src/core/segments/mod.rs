pub mod color_utils;
pub mod context_window;
pub mod threshold_utils;
pub mod cost;
pub mod directory;
pub mod git;
pub mod model;
pub mod output_style;
pub mod session;
pub mod update;
pub mod usage;
pub mod usage_5hour;
pub mod usage_7day;

use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

// New Segment trait for data collection only
pub trait Segment {
    fn collect(&self, input: &InputData) -> Option<SegmentData>;
    fn id(&self) -> SegmentId;
}

#[derive(Debug, Clone)]
pub struct SegmentData {
    pub primary: String,
    pub secondary: String,
    pub metadata: HashMap<String, String>,
}

// Re-export all segment types
pub use context_window::ContextWindowSegment;
pub use cost::CostSegment;
pub use directory::DirectorySegment;
pub use git::GitSegment;
pub use model::ModelSegment;
pub use output_style::OutputStyleSegment;
pub use session::SessionSegment;
pub use update::UpdateSegment;
pub use usage::UsageSegment;
pub use usage_5hour::Usage5HourSegment;
pub use usage_7day::Usage7DaySegment;
