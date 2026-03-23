pub mod minimax;

use serde_json::Value;
use std::collections::HashMap;

/// A single usage metric returned by a provider.
#[derive(Debug, Clone)]
pub struct UsageMetric {
    /// Display label, e.g. "7d" or "5h".
    /// Should contain only ASCII alphanumeric characters and hyphens
    /// so it is safe to use as a metadata key.
    pub label: String,
    /// Remaining quota as a percentage (0-100).
    pub remaining_pct: u32,
}

/// Trait for external usage data providers.
pub trait UsageProvider: Send + Sync {
    fn name(&self) -> &str;
    fn fetch(&self, options: &HashMap<String, Value>) -> Option<Vec<UsageMetric>>;
}

/// Build a provider implementation by name.
pub fn build(provider_name: &str) -> Option<Box<dyn UsageProvider>> {
    match provider_name {
        "minimax" => Some(Box::new(minimax::MinimaxProvider)),
        _ => None,
    }
}
