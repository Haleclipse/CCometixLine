use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Mutex;

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(rename = "model_remains")]
    model_remains: Vec<ModelRemain>,
}

#[derive(Debug, Deserialize)]
struct ModelRemain {
    #[serde(rename = "model_name")]
    model_name: String,
    #[serde(rename = "current_interval_total_count")]
    current_interval_total_count: u64,
    #[serde(rename = "current_interval_usage_count")]
    current_interval_usage_count: u64,
    #[serde(rename = "current_weekly_total_count")]
    current_weekly_total_count: u64,
    #[serde(rename = "current_weekly_usage_count")]
    current_weekly_usage_count: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct CacheData {
    weekly_remaining_pct: u32,
    interval_remaining_pct: u32,
    cached_at: String,
}

#[derive(Default)]
pub struct MiniMaxTokenPlanSegment {
    cached_config: Mutex<Option<CachedSegmentConfig>>,
}

#[derive(Clone)]
struct CachedSegmentConfig {
    cache_duration: u64,
    timeout: u64,
    preferred_model: String,
}

impl MiniMaxTokenPlanSegment {
    pub fn new() -> Self {
        Self {
            cached_config: Mutex::new(None),
        }
    }

    fn get_cached_config(&self) -> CachedSegmentConfig {
        let mut cached = self.cached_config.lock().unwrap();
        if let Some(ref config) = *cached {
            return config.clone();
        }

        let config = crate::config::Config::load()
            .ok()
            .and_then(|c| {
                c.segments
                    .iter()
                    .find(|s| s.id == SegmentId::MiniMaxTokenPlan)
                    .map(|sc| {
                        let cache_duration = sc.options.get("cache_duration").and_then(|v| v.as_u64()).unwrap_or(300);
                        let timeout = sc.options.get("timeout").and_then(|v| v.as_u64()).unwrap_or(10);
                        let preferred_model = sc.options.get("preferred_model")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "MiniMax-M2.7".to_string());
                        (cache_duration, timeout, preferred_model)
                    })
            })
            .map(|(cache_duration, timeout, preferred_model)| CachedSegmentConfig {
                cache_duration,
                timeout,
                preferred_model,
            })
            .unwrap_or_else(|| CachedSegmentConfig {
                cache_duration: 300,
                timeout: 10,
                preferred_model: "MiniMax-M2.7".to_string(),
            });

        *cached = Some(config.clone());
        config
    }

    fn get_circle_icon(remaining_pct: u32) -> String {
        match remaining_pct {
            88..=100 => "\u{f0a9e}".to_string(), // circle_slice_1 (almost full)
            75..=87 => "\u{f0a9f}".to_string(),  // circle_slice_2
            62..=74 => "\u{f0aa0}".to_string(),  // circle_slice_3
            50..=61 => "\u{f0aa1}".to_string(),  // circle_slice_4
            38..=49 => "\u{f0aa2}".to_string(),  // circle_slice_5
            25..=37 => "\u{f0aa3}".to_string(),  // circle_slice_6
            12..=24 => "\u{f0aa4}".to_string(),  // circle_slice_7
            _ => "\u{f0aa5}".to_string(),        // circle_slice_8 (almost empty)
        }
    }

    fn get_cache_path() -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        Some(
            home.join(".claude")
                .join("ccline")
                .join(".minimax_token_plan_cache.json"),
        )
    }

    fn load_cache(&self) -> Option<CacheData> {
        let cache_path = Self::get_cache_path()?;
        if !cache_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_cache(&self, cache: &CacheData) {
        if let Some(cache_path) = Self::get_cache_path() {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(cache) {
                let _ = std::fs::write(&cache_path, json);
            }
        }
    }

    fn is_cache_valid(&self, cache: &CacheData, cache_duration: u64) -> bool {
        if let Ok(cached_at) = DateTime::parse_from_rfc3339(&cache.cached_at) {
            let now = Utc::now();
            let elapsed = now.signed_duration_since(cached_at.with_timezone(&Utc));
            elapsed.num_seconds() < cache_duration as i64
        } else {
            false
        }
    }

    fn fetch_api(&self, api_key: &str, timeout_secs: u64) -> Option<ApiResponse> {
        let url = "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains";

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs)))
            .build()
            .new_agent();

        let response = agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .call()
            .ok()?;

        response.into_body().read_json().ok()
    }
}

impl Segment for MiniMaxTokenPlanSegment {
    fn collect(&self, _input: &InputData) -> Option<SegmentData> {
        let api_key = std::env::var("MINIMAX_API_KEY").ok()?;
        if api_key.is_empty() {
            return None;
        }

        let config = self.get_cached_config();
        let cache_duration = config.cache_duration;
        let timeout = config.timeout;
        let preferred_model = &config.preferred_model;

        let cached_data = self.load_cache();
        let use_cached = cached_data
            .as_ref()
            .map(|cache| self.is_cache_valid(cache, cache_duration))
            .unwrap_or(false);

        let (weekly_remaining_pct, interval_remaining_pct) = if use_cached {
            let cache = cached_data.unwrap();
            (cache.weekly_remaining_pct, cache.interval_remaining_pct)
        } else {
            match self.fetch_api(&api_key, timeout) {
                Some(response) => {
                    let model = response
                        .model_remains
                        .iter()
                        .find(|m| m.model_name == *preferred_model)
                        .or_else(|| response.model_remains.iter().next())?;

                    let weekly_remaining_pct = if model.current_weekly_total_count > 0 {
                        let usage_pct = (model.current_weekly_usage_count as f64
                            / model.current_weekly_total_count as f64)
                            * 100.0;
                        (100.0 - usage_pct) as u32
                    } else {
                        100
                    };

                    let interval_remaining_pct = if model.current_interval_total_count > 0 {
                        let usage_pct = (model.current_interval_usage_count as f64
                            / model.current_interval_total_count as f64)
                            * 100.0;
                        (100.0 - usage_pct) as u32
                    } else {
                        100
                    };

                    let cache = CacheData {
                        weekly_remaining_pct,
                        interval_remaining_pct,
                        cached_at: Utc::now().to_rfc3339(),
                    };
                    self.save_cache(&cache);

                    (weekly_remaining_pct, interval_remaining_pct)
                }
                None => {
                    if let Some(cache) = cached_data {
                        (cache.weekly_remaining_pct, cache.interval_remaining_pct)
                    } else {
                        return None;
                    }
                }
            }
        };

        let weekly_icon = Self::get_circle_icon(weekly_remaining_pct);
        let interval_icon = Self::get_circle_icon(interval_remaining_pct);

        let primary = format!(
            "{} {}% · {} {}% 5h",
            weekly_icon, weekly_remaining_pct, interval_icon, interval_remaining_pct
        );

        let mut metadata = HashMap::new();
        metadata.insert(
            "weekly_remaining_pct".to_string(),
            weekly_remaining_pct.to_string(),
        );
        metadata.insert(
            "interval_remaining_pct".to_string(),
            interval_remaining_pct.to_string(),
        );

        Some(SegmentData {
            primary,
            secondary: String::new(),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::MiniMaxTokenPlan
    }
}
