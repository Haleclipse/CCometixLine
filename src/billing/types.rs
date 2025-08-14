use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Session usage data aggregated from transcript files
#[derive(Debug, Clone, Default)]
pub struct SessionUsage {
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
    pub entries: Vec<UsageEntry>,
    pub session_id: String,
    pub start_time: Option<DateTime<Utc>>,
    pub last_update: Option<DateTime<Utc>>,
}

/// Single usage record from a transcript entry
#[derive(Debug, Clone)]
pub struct UsageEntry {
    pub timestamp: DateTime<Utc>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
    pub model: String,
    pub cost: Option<f64>, // Optional until pricing is calculated
    pub session_id: String,
}

/// 5-hour billing block with dynamic start time support
#[derive(Debug, Clone)]
pub struct BillingBlock {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub cost: f64,
    pub remaining_minutes: i64,
    pub is_active: bool,
    pub session_count: usize,
    pub total_tokens: u32,
    /// Source of the block start time
    pub start_time_source: BlockStartSource,
    /// Whether this is a gap block (no activity)
    pub is_gap: bool,
}

/// Source of block start time
#[derive(Debug, Clone, PartialEq)]
pub enum BlockStartSource {
    /// Automatically determined from first activity
    Auto,
    /// Manually set by user override
    Manual,
    /// Fixed 5-hour system (legacy mode)
    Fixed,
}

/// Burn rate calculation
#[derive(Debug, Clone)]
pub struct BurnRate {
    pub tokens_per_minute: f64,
    pub tokens_per_minute_for_indicator: f64, // Excludes cache tokens
    pub cost_per_hour: f64,
    pub trend: BurnRateTrend,
}

/// Burn rate trend indicator
#[derive(Debug, Clone, PartialEq)]
pub enum BurnRateTrend {
    Rising,
    Falling,
    Stable,
}

/// Burn rate thresholds for indicator display
#[derive(Debug, Clone)]
pub struct BurnRateThresholds {
    pub high: f64,   // Default 5000 tokens/minute
    pub medium: f64, // Default 2000 tokens/minute
}

impl Default for BurnRateThresholds {
    fn default() -> Self {
        Self {
            high: 5000.0,
            medium: 2000.0,
        }
    }
}

impl BurnRateThresholds {
    /// Create thresholds from environment variables
    pub fn from_env() -> Self {
        let mut thresholds = Self::default();

        if let Ok(high) = std::env::var("CCLINE_BURN_HIGH") {
            if let Ok(value) = high.parse::<f64>() {
                thresholds.high = value;
            }
        }

        if let Ok(medium) = std::env::var("CCLINE_BURN_MEDIUM") {
            if let Ok(value) = medium.parse::<f64>() {
                thresholds.medium = value;
            }
        }

        thresholds
    }
}

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub model_name: String,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
    pub cache_creation_cost_per_1k: f64,
    pub cache_read_cost_per_1k: f64,
}

impl ModelPricing {
    /// Calculate cost for a usage entry
    pub fn calculate_cost(&self, entry: &UsageEntry) -> f64 {
        let input_cost = (entry.input_tokens as f64 / 1000.0) * self.input_cost_per_1k;
        let output_cost = (entry.output_tokens as f64 / 1000.0) * self.output_cost_per_1k;
        let cache_creation_cost =
            (entry.cache_creation_tokens as f64 / 1000.0) * self.cache_creation_cost_per_1k;
        let cache_read_cost =
            (entry.cache_read_tokens as f64 / 1000.0) * self.cache_read_cost_per_1k;

        input_cost + output_cost + cache_creation_cost + cache_read_cost
    }
}

impl SessionUsage {
    /// Calculate total cost given pricing
    pub fn calculate_cost(&self, pricing: &ModelPricing) -> f64 {
        let input_cost = (self.total_input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k;
        let output_cost = (self.total_output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k;
        let cache_creation_cost =
            (self.cache_creation_tokens as f64 / 1000.0) * pricing.cache_creation_cost_per_1k;
        let cache_read_cost =
            (self.cache_read_tokens as f64 / 1000.0) * pricing.cache_read_cost_per_1k;

        input_cost + output_cost + cache_creation_cost + cache_read_cost
    }

    /// Get total tokens (all types)
    pub fn total_tokens(&self) -> u32 {
        self.total_input_tokens
            + self.total_output_tokens
            + self.cache_creation_tokens
            + self.cache_read_tokens
    }
}

impl BillingBlock {
    /// Check if the block is currently active
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        now >= self.start_time && now <= self.end_time
    }

    /// Calculate remaining minutes in the block
    pub fn remaining_minutes(&self) -> i64 {
        let now = Utc::now();
        if now > self.end_time {
            return 0;
        }
        (self.end_time - now).num_minutes()
    }
}
