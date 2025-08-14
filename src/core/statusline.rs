use crate::config::{Config, InputData};
use crate::core::segments::{
    BurnRateSegment, CostSegment, DirectorySegment, GitSegment, ModelSegment, Segment,
    UpdateSegment, UsageSegment,
};

pub struct StatusLineGenerator {
    config: Config,
}

impl StatusLineGenerator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate(&self, input: &InputData) -> String {
        let mut segments = Vec::new();

        // Assemble segments with proper colors
        if self.config.segments.model {
            let model_segment = ModelSegment::new(true);
            let content = model_segment.render(input);
            segments.push(format!("\x1b[1;36m{}\x1b[0m", content));
        }

        if self.config.segments.directory {
            let dir_segment = DirectorySegment::new(true);
            let content = dir_segment.render(input);
            // Extract directory name without icon
            let dir_name = content.trim_start_matches('\u{f024b}').trim_start();
            segments.push(format!(
                "\x1b[1;33m\u{f024b}\x1b[0m \x1b[1;32m{}\x1b[0m",
                dir_name
            ));
        }

        if self.config.segments.git {
            let git_segment = GitSegment::new(true);
            let git_output = git_segment.render(input);
            if !git_output.is_empty() {
                segments.push(format!("\x1b[1;34m{}\x1b[0m", git_output));
            }
        }

        if self.config.segments.usage {
            let usage_segment = UsageSegment::new(true);
            let content = usage_segment.render(input);
            segments.push(format!("\x1b[1;35m{}\x1b[0m", content));
        }

        // Add cost segment
        if self.config.segments.cost {
            let cost_segment = CostSegment::new(true);
            let content = cost_segment.render(input);
            segments.push(format!("\x1b[1;33m{}\x1b[0m", content)); // Yellow
        }

        // Add burn rate segment
        if self.config.segments.burn_rate {
            let burn_rate_segment = BurnRateSegment::new(true);
            let content = burn_rate_segment.render(input);
            segments.push(format!("\x1b[1;31m{}\x1b[0m", content)); // Red
        }

        // Add update segment (always enabled when there's an update)
        let update_segment = UpdateSegment::new();
        if update_segment.enabled() {
            let content = update_segment.render(input);
            segments.push(format!("\x1b[1;37m{}\x1b[0m", content));
        }

        // Join segments with white separator
        segments.join("\x1b[37m | \x1b[0m")
    }
}
