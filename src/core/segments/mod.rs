pub mod burn_rate;
pub mod cost;
pub mod directory;
pub mod git;
pub mod model;
pub mod update;
pub mod usage;

use crate::config::InputData;

pub trait Segment {
    fn render(&self, input: &InputData) -> String;
    fn enabled(&self) -> bool;
}

// Re-export all segment types
pub use burn_rate::BurnRateSegment;
pub use cost::CostSegment;
pub use directory::DirectorySegment;
pub use git::GitSegment;
pub use model::ModelSegment;
pub use update::UpdateSegment;
pub use usage::UsageSegment;
