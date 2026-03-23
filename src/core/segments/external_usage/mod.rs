mod cache;
pub mod providers;

use super::{Segment, SegmentData};
use cache::{Cache, CacheEntry, CachedMetric};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

pub struct ExternalUsageSegment;

impl ExternalUsageSegment {
    pub fn new() -> Self {
        Self
    }

    // Higher remaining_pct = fuller icon (opposite of UsageSegment which shows consumed amount)
    fn get_circle_icon(remaining_pct: u32) -> String {
        match remaining_pct {
            88..=100 => "\u{f0aa5}".to_string(), // circle_slice_8 (fullest)
            75..=87  => "\u{f0aa4}".to_string(), // circle_slice_7
            62..=74  => "\u{f0aa3}".to_string(), // circle_slice_6
            50..=61  => "\u{f0aa2}".to_string(), // circle_slice_5
            38..=49  => "\u{f0aa1}".to_string(), // circle_slice_4
            25..=37  => "\u{f0aa0}".to_string(), // circle_slice_3
            12..=24  => "\u{f0a9f}".to_string(), // circle_slice_2
            _        => "\u{f0a9e}".to_string(), // circle_slice_1 (emptiest)
        }
    }
}

impl Default for ExternalUsageSegment {
    fn default() -> Self {
        Self::new()
    }
}

fn cached_to_usage(entry: &cache::CacheEntry) -> Vec<providers::UsageMetric> {
    entry
        .metrics
        .iter()
        .map(|m| providers::UsageMetric {
            label: m.label.clone(),
            remaining_pct: m.remaining_pct,
        })
        .collect()
}

fn metrics_to_cache(metrics: &[providers::UsageMetric]) -> CacheEntry {
    CacheEntry {
        metrics: metrics
            .iter()
            .map(|m| CachedMetric {
                label: m.label.clone(),
                remaining_pct: m.remaining_pct,
            })
            .collect(),
        cached_at: chrono::Utc::now().to_rfc3339(),
    }
}

impl Segment for ExternalUsageSegment {
    fn collect(&self, _input: &InputData) -> Option<SegmentData> {
        let config = crate::config::Config::load().ok()?;
        let seg_cfg = config
            .segments
            .iter()
            .find(|s| s.id == SegmentId::ExternalUsage)?;

        let options = &seg_cfg.options;

        let provider_name = options
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("minimax");

        let cache_duration = options
            .get("cache_duration")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let provider = providers::build(provider_name)?;
        let cache = Cache::new(provider_name)?;

        let metrics = if let Some(entry) = cache.load() {
            if Cache::is_valid(&entry, cache_duration) {
                // Cache hit: use stored metrics directly
                cached_to_usage(&entry)
            } else {
                // Cache expired: fetch fresh data, fall back to stale cache on failure
                match provider.fetch(options) {
                    Some(fresh) => {
                        cache.save(&metrics_to_cache(&fresh));
                        fresh
                    }
                    None => cached_to_usage(&entry),
                }
            }
        } else {
            // No cache: must fetch, return None on failure
            let fresh = provider.fetch(options)?;
            cache.save(&metrics_to_cache(&fresh));
            fresh
        };

        if metrics.is_empty() {
            return None;
        }

        // Format: " 97% 7d ·  54% 5h"
        let parts: Vec<String> = metrics
            .iter()
            .map(|m| format!("{} {}% {}", Self::get_circle_icon(m.remaining_pct), m.remaining_pct, m.label))
            .collect();
        let primary = parts.join(" · ");

        let mut metadata = HashMap::new();
        for m in &metrics {
            metadata.insert(
                format!("{}_remaining_pct", m.label),
                m.remaining_pct.to_string(),
            );
        }

        Some(SegmentData {
            primary,
            secondary: String::new(),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::ExternalUsage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_icon_full() {
        // 88-100% -> fullest icon
        assert_eq!(ExternalUsageSegment::get_circle_icon(100), "\u{f0aa5}");
        assert_eq!(ExternalUsageSegment::get_circle_icon(88), "\u{f0aa5}");
    }

    #[test]
    fn test_circle_icon_empty() {
        // 0-11% -> emptiest icon
        assert_eq!(ExternalUsageSegment::get_circle_icon(0), "\u{f0a9e}");
        assert_eq!(ExternalUsageSegment::get_circle_icon(11), "\u{f0a9e}");
    }

    #[test]
    fn test_circle_icon_mid() {
        // 50-61% -> circle_slice_5
        assert_eq!(ExternalUsageSegment::get_circle_icon(50), "\u{f0aa2}");
        assert_eq!(ExternalUsageSegment::get_circle_icon(60), "\u{f0aa2}");
    }

    #[test]
    fn test_circle_icon_boundary() {
        // boundary: 87% -> slice_7, 88% -> slice_8
        assert_eq!(ExternalUsageSegment::get_circle_icon(87), "\u{f0aa4}");
        assert_eq!(ExternalUsageSegment::get_circle_icon(88), "\u{f0aa5}");
    }
}
