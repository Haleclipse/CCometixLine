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

    fn get_circle_icon(remaining_pct: u32) -> String {
        // 剩余越多图标越满（与 UsageSegment 的语义相反，UsageSegment 显示的是已用量）
        match remaining_pct {
            88..=100 => "\u{f0aa5}".to_string(), // circle_slice_8（最满）
            75..=87  => "\u{f0aa4}".to_string(), // circle_slice_7
            62..=74  => "\u{f0aa3}".to_string(), // circle_slice_6
            50..=61  => "\u{f0aa2}".to_string(), // circle_slice_5
            38..=49  => "\u{f0aa1}".to_string(), // circle_slice_4
            25..=37  => "\u{f0aa0}".to_string(), // circle_slice_3
            12..=24  => "\u{f0a9f}".to_string(), // circle_slice_2
            _        => "\u{f0a9e}".to_string(), // circle_slice_1（最空）
        }
    }
}

impl Default for ExternalUsageSegment {
    fn default() -> Self {
        Self::new()
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

        // 尝试使用有效缓存
        let metrics = if let Some(entry) = cache.load() {
            if Cache::is_valid(&entry, cache_duration) {
                entry.metrics.iter().map(|m| providers::UsageMetric {
                    label: m.label.clone(),
                    remaining_pct: m.remaining_pct,
                }).collect()
            } else {
                // 缓存过期，重新拉取；失败时降级使用旧缓存
                match provider.fetch(options) {
                    Some(fresh) => {
                        let entry = CacheEntry {
                            metrics: fresh.iter().map(|m| CachedMetric {
                                label: m.label.clone(),
                                remaining_pct: m.remaining_pct,
                            }).collect(),
                            cached_at: chrono::Utc::now().to_rfc3339(),
                        };
                        cache.save(&entry);
                        fresh
                    }
                    None => entry.metrics.iter().map(|m| providers::UsageMetric {
                        label: m.label.clone(),
                        remaining_pct: m.remaining_pct,
                    }).collect(),
                }
            }
        } else {
            // 无缓存，必须拉取
            let fresh = provider.fetch(options)?;
            let entry = CacheEntry {
                metrics: fresh.iter().map(|m| CachedMetric {
                    label: m.label.clone(),
                    remaining_pct: m.remaining_pct,
                }).collect(),
                cached_at: chrono::Utc::now().to_rfc3339(),
            };
            cache.save(&entry);
            fresh
        };

        if metrics.is_empty() {
            return None;
        }

        // 格式化：`◉ 97% 7d · ◐ 94% 5h`
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
