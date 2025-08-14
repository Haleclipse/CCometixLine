use super::Segment;
use crate::billing::{
    block::{find_active_block, identify_session_blocks_with_overrides},
    calculator::{calculate_daily_total, calculate_session_cost, format_remaining_time},
    ModelPricing,
};
use crate::config::InputData;
use crate::utils::{data_loader::DataLoader, transcript::extract_session_id};
use std::time::Instant;

pub struct CostSegment {
    enabled: bool,
    show_timing: bool,
}

impl CostSegment {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            show_timing: std::env::var("CCLINE_SHOW_TIMING").is_ok(),
        }
    }

    fn render_with_pricing(&self, input: &InputData) -> String {
        // Performance timing
        let start = Instant::now();
        let mut timings = Vec::new();

        // 1. Load all project data
        let load_start = Instant::now();
        let data_loader = DataLoader::new();
        let mut all_entries = data_loader.load_all_projects();
        timings.push(("L", load_start.elapsed().as_millis()));

        // 2. Get pricing data (create a runtime to handle async)
        let pricing_start = Instant::now();
        let pricing_map = {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async { ModelPricing::get_pricing_with_fallback().await })
        };
        timings.push(("P", pricing_start.elapsed().as_millis()));

        // 3. Calculate costs for all entries
        let calc_start = Instant::now();
        for entry in &mut all_entries {
            if let Some(pricing) = ModelPricing::get_model_pricing(&pricing_map, &entry.model) {
                entry.cost = Some(pricing.calculate_cost(entry));
            }
        }
        timings.push(("C", calc_start.elapsed().as_millis()));

        // 4. Calculate session and daily costs
        let analyze_start = Instant::now();
        let transcript_path = std::path::Path::new(&input.transcript_path);
        let session_id = extract_session_id(transcript_path);
        let session_cost = calculate_session_cost(&all_entries, &session_id, &pricing_map);
        let daily_total = calculate_daily_total(&all_entries, &pricing_map);
        timings.push(("A", analyze_start.elapsed().as_millis()));

        // 5. Calculate dynamic blocks with override support
        let block_start = Instant::now();
        let blocks = identify_session_blocks_with_overrides(&all_entries);
        let active_block = find_active_block(&blocks);
        timings.push(("B", block_start.elapsed().as_millis()));

        // Format basic output
        let cost_display = match active_block {
            Some(block) => format!(
                "\u{f155} ${:.2} session · ${:.2} today · ${:.2} block ({})",
                session_cost,
                daily_total,
                block.cost,
                format_remaining_time(block.remaining_minutes)
            ),
            None => format!(
                "\u{f155} ${:.2} session · ${:.2} today · No active block",
                session_cost, daily_total
            ),
        };

        // Add performance timing if enabled
        if self.show_timing {
            let total_ms = start.elapsed().as_millis();
            let timing_str = format!(
                " [{}ms: L{}|P{}|C{}|A{}|B{}]",
                total_ms,
                timings[0].1, // Load
                timings[1].1, // Pricing
                timings[2].1, // Calculate
                timings[3].1, // Analyze
                timings[4].1  // Block
            );
            format!("{}{}", cost_display, timing_str)
        } else {
            cost_display
        }
    }
}

impl Segment for CostSegment {
    fn render(&self, input: &InputData) -> String {
        if !self.enabled {
            return String::new();
        }

        // Handle potential errors gracefully
        match std::panic::catch_unwind(|| self.render_with_pricing(input)) {
            Ok(result) => result,
            Err(_) => {
                // Fallback display on error
                "\u{f155} $0.00 session · $0.00 today · Error loading data".to_string()
            }
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Model, Workspace};

    #[test]
    fn test_cost_segment_disabled() {
        let segment = CostSegment::new(false);
        let input = InputData {
            model: Model {
                display_name: "test-model".to_string(),
            },
            workspace: Workspace {
                current_dir: "/test".to_string(),
            },
            transcript_path: "/test/transcript.jsonl".to_string(),
        };

        assert_eq!(segment.render(&input), "");
        assert!(!segment.enabled());
    }

    #[test]
    fn test_cost_segment_enabled() {
        let segment = CostSegment::new(true);
        assert!(segment.enabled());
    }
}
