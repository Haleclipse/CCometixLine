use super::usage::{load_usage_data, UsageOptions};
use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

/// Extra-usage credits spent this month against the seat's spend limit,
/// e.g. `$23.75/$50 · 48%`. Only rendered when the organization enables credits.
pub struct CreditsSegment {
    /// Whether the used percentage is appended (`show_percent` option, default: true)
    show_percent: bool,
    /// API / cache settings shared with the `usage` segment
    usage_options: UsageOptions,
}

impl Default for CreditsSegment {
    fn default() -> Self {
        Self {
            show_percent: true,
            usage_options: UsageOptions::default(),
        }
    }
}

impl CreditsSegment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_show_percent(mut self, show_percent: bool) -> Self {
        self.show_percent = show_percent;
        self
    }

    /// Use the `usage` segment's API / cache options from the active configuration
    pub fn with_usage_options(mut self, options: UsageOptions) -> Self {
        self.usage_options = options;
        self
    }
}

impl Segment for CreditsSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let usage = load_usage_data(input, &self.usage_options)?;
        let credits = usage.extra_credits?;

        let percent = if credits.limit > 0.0 {
            (credits.used / credits.limit * 100.0).round() as u8
        } else {
            0
        };

        let secondary = if self.show_percent {
            format!("· {}%", percent)
        } else {
            String::new()
        };

        let mut metadata = HashMap::new();
        metadata.insert("credits_used".to_string(), credits.used.to_string());
        metadata.insert("credits_limit".to_string(), credits.limit.to_string());
        metadata.insert("credits_percent".to_string(), percent.to_string());
        metadata.insert("currency".to_string(), credits.currency.clone());

        Some(SegmentData {
            primary: credits.format(),
            secondary,
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Credits
    }
}
