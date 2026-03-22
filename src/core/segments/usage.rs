use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use crate::utils::credentials;
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── API response types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ApiUsageResponse {
    five_hour: UsagePeriod,
    seven_day: UsagePeriod,
}

#[derive(Debug, Deserialize)]
struct UsagePeriod {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiUsageCache {
    five_hour_utilization: f64,
    #[serde(default)]
    five_hour_resets_at: Option<String>,
    seven_day_utilization: f64,
    resets_at: Option<String>, // 7d reset, kept for backward compat
    cached_at: String,
}

// ── Shared rate limit data ────────────────────────────────────────────────────

#[derive(Clone)]
struct RateLimitData {
    five_hour_util: f64,
    five_hour_resets_at: Option<String>,
    seven_day_util: f64,
    seven_day_resets_at: Option<String>,
}

// Process-lifetime cache: computed once per statusline render, shared across
// all Usage segments (Usage, Usage5h, Usage7d) to avoid redundant I/O.
static RATE_LIMIT_CACHE: std::sync::OnceLock<Option<RateLimitData>> =
    std::sync::OnceLock::new();

// ── Shared helpers ────────────────────────────────────────────────────────────

fn get_circle_icon(utilization: f64) -> String {
    let percent = (utilization * 100.0) as u8;
    match percent {
        0..=12 => "\u{f0a9e}".to_string(),  // circle_slice_1
        13..=25 => "\u{f0a9f}".to_string(), // circle_slice_2
        26..=37 => "\u{f0aa0}".to_string(), // circle_slice_3
        38..=50 => "\u{f0aa1}".to_string(), // circle_slice_4
        51..=62 => "\u{f0aa2}".to_string(), // circle_slice_5
        63..=75 => "\u{f0aa3}".to_string(), // circle_slice_6
        76..=87 => "\u{f0aa4}".to_string(), // circle_slice_7
        _ => "\u{f0aa5}".to_string(),       // circle_slice_8
    }
}

fn format_reset_time(reset_time_str: Option<&str>) -> String {
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

fn get_cache_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    Some(
        home.join(".claude")
            .join("ccline")
            .join(".api_usage_cache.json"),
    )
}

fn load_cache() -> Option<ApiUsageCache> {
    let cache_path = get_cache_path()?;
    if !cache_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&cache_path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_cache(cache: &ApiUsageCache) {
    if let Some(cache_path) = get_cache_path() {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(cache) {
            let _ = std::fs::write(&cache_path, json);
        }
    }
}

fn is_cache_valid(cache: &ApiUsageCache, cache_duration: u64) -> bool {
    if let Ok(cached_at) = DateTime::parse_from_rfc3339(&cache.cached_at) {
        let elapsed = Utc::now().signed_duration_since(cached_at.with_timezone(&Utc));
        elapsed.num_seconds() < cache_duration as i64
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
    let content = std::fs::read_to_string(format!("{}/.claude/settings.json", home)).ok()?;
    let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
    settings
        .get("env")?
        .get("HTTPS_PROXY")
        .or_else(|| settings.get("env")?.get("HTTP_PROXY"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn fetch_api_usage(api_base_url: &str, token: &str, timeout_secs: u64) -> Option<ApiUsageResponse> {
    let url = format!("{}/api/oauth/usage", api_base_url);
    let user_agent = get_claude_code_version();

    let agent = if let Some(proxy_url) = get_proxy_from_settings() {
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

/// Fetch rate limit data: prefer Claude Code's rate_limits field, fallback to API.
/// Result is cached for the process lifetime so multiple segments share one fetch.
fn fetch_rate_limit_data(input: &InputData) -> Option<RateLimitData> {
    if let Some(cached) = RATE_LIMIT_CACHE.get() {
        return cached.clone();
    }
    let result = fetch_rate_limit_data_inner(input);
    RATE_LIMIT_CACHE.get_or_init(|| result.clone());
    result
}

fn fetch_rate_limit_data_inner(input: &InputData) -> Option<RateLimitData> {
    if let Some(rate_limits) = &input.rate_limits {
        let ts_to_rfc3339 = |ts: i64| {
            Utc.timestamp_opt(ts, 0)
                .single()
                .map(|dt| dt.to_rfc3339())
        };
        return Some(RateLimitData {
            five_hour_util: rate_limits
                .five_hour
                .as_ref()
                .map(|p| p.used_percentage)
                .unwrap_or(0.0),
            five_hour_resets_at: rate_limits
                .five_hour
                .as_ref()
                .and_then(|p| p.resets_at)
                .and_then(ts_to_rfc3339),
            seven_day_util: rate_limits
                .seven_day
                .as_ref()
                .map(|p| p.used_percentage)
                .unwrap_or(0.0),
            seven_day_resets_at: rate_limits
                .seven_day
                .as_ref()
                .and_then(|p| p.resets_at)
                .and_then(ts_to_rfc3339),
        });
    }

    // Fallback: fetch from Anthropic API with cache
    let token = credentials::get_oauth_token()?;

    let config = crate::config::Config::load().ok()?;
    let segment_config = config.segments.iter().find(|s| {
        matches!(s.id, SegmentId::Usage | SegmentId::Usage5h | SegmentId::Usage7d)
    });

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

    let cached_data = load_cache();
    let use_cached = cached_data
        .as_ref()
        .map(|c| is_cache_valid(c, cache_duration))
        .unwrap_or(false);

    if use_cached {
        let cache = cached_data.unwrap();
        return Some(RateLimitData {
            five_hour_util: cache.five_hour_utilization,
            five_hour_resets_at: cache.five_hour_resets_at,
            seven_day_util: cache.seven_day_utilization,
            seven_day_resets_at: cache.resets_at,
        });
    }

    match fetch_api_usage(api_base_url, &token, timeout) {
        Some(response) => {
            save_cache(&ApiUsageCache {
                five_hour_utilization: response.five_hour.utilization,
                five_hour_resets_at: response.five_hour.resets_at.clone(),
                seven_day_utilization: response.seven_day.utilization,
                resets_at: response.seven_day.resets_at.clone(),
                cached_at: Utc::now().to_rfc3339(),
            });
            Some(RateLimitData {
                five_hour_util: response.five_hour.utilization,
                five_hour_resets_at: response.five_hour.resets_at,
                seven_day_util: response.seven_day.utilization,
                seven_day_resets_at: response.seven_day.resets_at,
            })
        }
        None => cached_data.map(|cache| RateLimitData {
            five_hour_util: cache.five_hour_utilization,
            five_hour_resets_at: cache.five_hour_resets_at,
            seven_day_util: cache.seven_day_utilization,
            seven_day_resets_at: cache.resets_at,
        }),
    }
}

// ── Usage5hSegment ────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Usage5hSegment;

impl Usage5hSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for Usage5hSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let data = fetch_rate_limit_data(input)?;
        let percent = data.five_hour_util.round() as u8;
        let reset = format_reset_time(data.five_hour_resets_at.as_deref());

        let mut metadata = HashMap::new();
        metadata.insert(
            "dynamic_icon".to_string(),
            get_circle_icon(data.five_hour_util / 100.0),
        );

        Some(SegmentData {
            primary: format!("{}%", percent),
            secondary: format!("· {}", reset),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Usage5h
    }
}

// ── Usage7dSegment ────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Usage7dSegment;

impl Usage7dSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for Usage7dSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let data = fetch_rate_limit_data(input)?;
        let percent = data.seven_day_util.round() as u8;
        let reset = format_reset_time(data.seven_day_resets_at.as_deref());

        let mut metadata = HashMap::new();
        metadata.insert(
            "dynamic_icon".to_string(),
            get_circle_icon(data.seven_day_util / 100.0),
        );

        Some(SegmentData {
            primary: format!("{}%", percent),
            secondary: format!("· {}", reset),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Usage7d
    }
}

// ── UsageSegment (legacy, combined view) ─────────────────────────────────────

#[derive(Default)]
pub struct UsageSegment;

impl UsageSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for UsageSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let data = fetch_rate_limit_data(input)?;
        let five_hour_percent = data.five_hour_util.round() as u8;
        let seven_day_reset = format_reset_time(data.seven_day_resets_at.as_deref());

        let mut metadata = HashMap::new();
        metadata.insert(
            "dynamic_icon".to_string(),
            get_circle_icon(data.seven_day_util / 100.0),
        );

        Some(SegmentData {
            primary: format!("{}%", five_hour_percent),
            secondary: format!("· {}", seven_day_reset),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Usage
    }
}
