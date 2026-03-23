use super::{Segment, SegmentData};
use crate::config::{InputData, RateLimitPeriod, SegmentId};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// API fallback types (used only when stdin does not carry rate_limits)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ApiUsageResponse {
    five_hour: ApiUsagePeriod,
    seven_day: ApiUsagePeriod,
}

#[derive(Debug, Deserialize)]
struct ApiUsagePeriod {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiUsageCache {
    five_hour_utilization: f64,
    seven_day_utilization: f64,
    resets_at: Option<String>,
    cached_at: String,
}

// ---------------------------------------------------------------------------
// Segment
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct UsageSegment;

impl UsageSegment {
    pub fn new() -> Self {
        Self
    }

    fn get_circle_icon(utilization: f64) -> &'static str {
        let percent = (utilization * 100.0) as u8;
        match percent {
            0..=12  => "\u{f0a9e}", // circle_slice_1
            13..=25 => "\u{f0a9f}", // circle_slice_2
            26..=37 => "\u{f0aa0}", // circle_slice_3
            38..=50 => "\u{f0aa1}", // circle_slice_4
            51..=62 => "\u{f0aa2}", // circle_slice_5
            63..=75 => "\u{f0aa3}", // circle_slice_6
            76..=87 => "\u{f0aa4}", // circle_slice_7
            _       => "\u{f0aa5}", // circle_slice_8
        }
    }

    /// Format a Unix timestamp (seconds) into a human-readable reset time string.
    fn format_reset_time_unix(ts: i64) -> String {
        let dt = Utc.timestamp_opt(ts, 0).single();
        match dt {
            Some(utc_dt) => {
                let mut local_dt = utc_dt.with_timezone(&Local);
                if local_dt.minute() > 45 {
                    local_dt += Duration::hours(1);
                }
                format!("{}-{}-{}", local_dt.month(), local_dt.day(), local_dt.hour())
            }
            None => "?".to_string(),
        }
    }

    /// Format an RFC 3339 reset time string (used by the API fallback path).
    fn format_reset_time_rfc3339(reset_time_str: Option<&str>) -> String {
        if let Some(time_str) = reset_time_str {
            if let Ok(dt) = DateTime::parse_from_rfc3339(time_str) {
                let mut local_dt = dt.with_timezone(&Local);
                if local_dt.minute() > 45 {
                    local_dt += Duration::hours(1);
                }
                return format!(
                    "{}-{}-{}",
                    local_dt.month(),
                    local_dt.day(),
                    local_dt.hour()
                );
            }
        }
        "?".to_string()
    }

    /// Extract utilization percentage from a stdin RateLimitPeriod.
    /// Clamps to [0, 100] and returns None if the value is missing or non-finite.
    fn parse_utilization(period: &RateLimitPeriod) -> Option<f64> {
        let v = period.used_percentage?;
        if !v.is_finite() {
            return None;
        }
        Some(v.clamp(0.0, 100.0))
    }

    // --- API fallback helpers (unchanged from original) ---

    fn get_cache_path() -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        Some(
            home.join(".claude")
                .join("ccline")
                .join(".api_usage_cache.json"),
        )
    }

    fn load_cache(&self) -> Option<ApiUsageCache> {
        let cache_path = Self::get_cache_path()?;
        if !cache_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_cache(&self, cache: &ApiUsageCache) {
        if let Some(cache_path) = Self::get_cache_path() {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(cache) {
                let _ = std::fs::write(&cache_path, json);
            }
        }
    }

    fn is_cache_valid(&self, cache: &ApiUsageCache, cache_duration: u64) -> bool {
        if let Ok(cached_at) = DateTime::parse_from_rfc3339(&cache.cached_at) {
            let elapsed = Utc::now().signed_duration_since(cached_at.with_timezone(&Utc));
            let secs = elapsed.num_seconds();
            secs >= 0 && secs < cache_duration as i64
        } else {
            false
        }
    }

    fn get_claude_code_version() -> String {
        use std::process::Command;
        let output = Command::new("npm")
            .args(["view", "@anthropic-ai/claude-code", "version"])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !version.is_empty() {
                    return format!("claude-code/{}", version);
                }
            }
            _ => {}
        }
        "claude-code".to_string()
    }

    fn get_proxy_from_settings() -> Option<String> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;
        let settings_path = format!("{}/.claude/settings.json", home);
        let content = std::fs::read_to_string(&settings_path).ok()?;
        let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
        settings
            .get("env")?
            .get("HTTPS_PROXY")
            .or_else(|| settings.get("env")?.get("HTTP_PROXY"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn fetch_api_usage(
        &self,
        api_base_url: &str,
        token: &str,
        timeout_secs: u64,
    ) -> Option<ApiUsageResponse> {
        use crate::utils::credentials;
        let _ = credentials::get_oauth_token(); // ensure token is accessible
        let url = format!("{}/api/oauth/usage", api_base_url);
        let user_agent = Self::get_claude_code_version();

        let agent = if let Some(proxy_url) = Self::get_proxy_from_settings() {
            if let Ok(proxy) = ureq::Proxy::new(&proxy_url) {
                ureq::Agent::config_builder()
                    .proxy(Some(proxy))
                    .build()
                    .new_agent()
            } else {
                ureq::Agent::new_with_defaults()
            }
        } else {
            ureq::Agent::new_with_defaults()
        };

        let response = agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", token))
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("User-Agent", &user_agent)
            .config()
            .timeout_global(Some(std::time::Duration::from_secs(timeout_secs)))
            .build()
            .call()
            .ok()?;

        response.into_body().read_json().ok()
    }

    /// Collect from stdin rate_limits — no network, no credentials, always fresh.
    fn collect_from_stdin(&self, input: &InputData) -> Option<SegmentData> {
        let rate_limits = input.rate_limits.as_ref()?;
        let five_hour = rate_limits.five_hour.as_ref()?;
        let five_hour_util = Self::parse_utilization(five_hour)?;

        let seven_day_util = rate_limits
            .seven_day
            .as_ref()
            .and_then(|p| Self::parse_utilization(p))
            .unwrap_or(0.0);

        let reset_str = rate_limits
            .five_hour
            .as_ref()
            .and_then(|p| p.resets_at)
            .map(Self::format_reset_time_unix)
            .unwrap_or_else(|| "?".to_string());

        Some(self.build_segment_data(five_hour_util, seven_day_util, reset_str))
    }

    /// Fallback: collect via Anthropic API (for older Claude Code versions
    /// that do not include rate_limits in stdin).
    fn collect_from_api(&self, input: &InputData) -> Option<SegmentData> {
        use crate::utils::credentials;
        let token = credentials::get_oauth_token()?;

        let config = crate::config::Config::load().ok()?;
        let segment_config = config.segments.iter().find(|s| s.id == SegmentId::Usage);

        let api_base_url = segment_config
            .and_then(|sc| sc.options.get("api_base_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("https://api.anthropic.com");

        let cache_duration = segment_config
            .and_then(|sc| sc.options.get("cache_duration"))
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let timeout = segment_config
            .and_then(|sc| sc.options.get("timeout"))
            .and_then(|v| v.as_u64())
            .unwrap_or(2);

        let cached_data = self.load_cache();
        let use_cached = cached_data
            .as_ref()
            .map(|c| self.is_cache_valid(c, cache_duration))
            .unwrap_or(false);

        let (five_hour_util, seven_day_util, resets_at) = if use_cached {
            let c = cached_data.unwrap();
            (c.five_hour_utilization, c.seven_day_utilization, c.resets_at)
        } else {
            match self.fetch_api_usage(api_base_url, &token, timeout) {
                Some(response) => {
                    let cache = ApiUsageCache {
                        five_hour_utilization: response.five_hour.utilization,
                        seven_day_utilization: response.seven_day.utilization,
                        resets_at: response.seven_day.resets_at.clone(),
                        cached_at: Utc::now().to_rfc3339(),
                    };
                    self.save_cache(&cache);
                    (
                        response.five_hour.utilization,
                        response.seven_day.utilization,
                        response.seven_day.resets_at,
                    )
                }
                None => {
                    let c = cached_data?;
                    (c.five_hour_utilization, c.seven_day_utilization, c.resets_at)
                }
            }
        };

        let reset_str = Self::format_reset_time_rfc3339(resets_at.as_deref());
        Some(self.build_segment_data(five_hour_util, seven_day_util, reset_str))
    }

    fn build_segment_data(
        &self,
        five_hour_util: f64,
        seven_day_util: f64,
        reset_str: String,
    ) -> SegmentData {
        let dynamic_icon = Self::get_circle_icon(seven_day_util / 100.0);
        let five_hour_percent = five_hour_util.round() as u8;
        let primary = format!("{}%", five_hour_percent);
        let secondary = format!("· {}", reset_str);

        let mut metadata = HashMap::new();
        metadata.insert("dynamic_icon".to_string(), dynamic_icon.to_string());
        metadata.insert("five_hour_utilization".to_string(), five_hour_util.to_string());
        metadata.insert("seven_day_utilization".to_string(), seven_day_util.to_string());

        SegmentData { primary, secondary, metadata }
    }
}

impl Segment for UsageSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        // Prefer stdin data: zero latency, no credentials, always up to date.
        // Fall back to the API path for older Claude Code versions that do not
        // include rate_limits in stdin.
        self.collect_from_stdin(input)
            .or_else(|| self.collect_from_api(input))
    }

    fn id(&self) -> SegmentId {
        SegmentId::Usage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RateLimitPeriod, RateLimits};

    fn make_input(five_pct: Option<f64>, five_resets_at: Option<i64>, seven_pct: Option<f64>) -> InputData {
        InputData {
            model: crate::config::Model { id: "".into(), display_name: "".into() },
            workspace: crate::config::Workspace { current_dir: "".into() },
            transcript_path: "".into(),
            cost: None,
            output_style: None,
            rate_limits: Some(RateLimits {
                five_hour: Some(RateLimitPeriod {
                    used_percentage: five_pct,
                    resets_at: five_resets_at,
                }),
                seven_day: Some(RateLimitPeriod {
                    used_percentage: seven_pct,
                    resets_at: None,
                }),
            }),
        }
    }

    #[test]
    fn test_collect_from_stdin_basic() {
        let seg = UsageSegment::new();
        let input = make_input(Some(75.0), None, Some(45.0));
        let data = seg.collect_from_stdin(&input).unwrap();
        assert_eq!(data.primary, "75%");
        assert_eq!(data.metadata["five_hour_utilization"], "75");
        assert_eq!(data.metadata["seven_day_utilization"], "45");
    }

    #[test]
    fn test_collect_from_stdin_clamps_over_100() {
        let seg = UsageSegment::new();
        let input = make_input(Some(120.0), None, Some(0.0));
        let data = seg.collect_from_stdin(&input).unwrap();
        assert_eq!(data.primary, "100%");
    }

    #[test]
    fn test_collect_from_stdin_no_rate_limits() {
        let seg = UsageSegment::new();
        let mut input = make_input(Some(50.0), None, None);
        input.rate_limits = None;
        assert!(seg.collect_from_stdin(&input).is_none());
    }

    #[test]
    fn test_parse_utilization_non_finite() {
        let period = RateLimitPeriod { used_percentage: Some(f64::NAN), resets_at: None };
        assert!(UsageSegment::parse_utilization(&period).is_none());
    }

    #[test]
    fn test_circle_icon_boundaries() {
        assert_eq!(UsageSegment::get_circle_icon(0.0),   "\u{f0a9e}"); // 0% -> slice_1
        assert_eq!(UsageSegment::get_circle_icon(0.50),  "\u{f0aa1}"); // 50% -> slice_4 (38..=50)
        assert_eq!(UsageSegment::get_circle_icon(0.51),  "\u{f0aa2}"); // 51% -> slice_5
        assert_eq!(UsageSegment::get_circle_icon(1.0),   "\u{f0aa5}"); // 100% -> slice_8
    }
}
