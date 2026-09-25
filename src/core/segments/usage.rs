use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use crate::utils::credentials;
use chrono::{DateTime, Datelike, Duration, Local, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct ApiUsageResponse {
    five_hour: UsagePeriod,
    seven_day: UsagePeriod,
    #[serde(default)]
    extra_usage: Option<ExtraUsage>,
}

/// Usage credits spent beyond the plan limits (monthly, per seat)
#[derive(Debug, Deserialize)]
struct ExtraUsage {
    #[serde(default)]
    is_enabled: bool,
    /// Amounts are in minor units (cents when decimal_places == 2)
    #[serde(default)]
    monthly_limit: Option<f64>,
    #[serde(default)]
    used_credits: Option<f64>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    decimal_places: Option<u32>,
}

/// Extra-usage credits normalized to major units (e.g. dollars)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExtraCredits {
    pub(crate) used: f64,
    pub(crate) limit: f64,
    pub(crate) currency: String,
}

impl ExtraCredits {
    fn from_api(extra: &ExtraUsage) -> Option<Self> {
        if !extra.is_enabled {
            return None;
        }
        let divisor = 10f64.powi(extra.decimal_places.unwrap_or(2) as i32);
        Some(Self {
            used: extra.used_credits? / divisor,
            limit: extra.monthly_limit? / divisor,
            currency: extra.currency.clone().unwrap_or_else(|| "USD".to_string()),
        })
    }

    pub(crate) fn format(&self) -> String {
        let symbol = match self.currency.as_str() {
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            _ => "",
        };
        format!(
            "{}{}/{}{}",
            symbol,
            Self::format_amount(self.used),
            symbol,
            Self::format_amount(self.limit)
        )
    }

    fn format_amount(amount: f64) -> String {
        if amount.fract() == 0.0 {
            format!("{:.0}", amount)
        } else {
            format!("{:.2}", amount)
        }
    }
}

#[derive(Debug, Deserialize)]
struct UsagePeriod {
    utilization: f64,
    resets_at: Option<String>,
}

/// Bump when the cache layout changes so stale files from older builds are ignored
const CACHE_VERSION: u32 = 3;

#[derive(Debug, Serialize, Deserialize)]
struct ApiUsageCache {
    #[serde(default)]
    version: u32,
    five_hour_utilization: f64,
    seven_day_utilization: f64,
    five_hour_resets_at: Option<String>,
    seven_day_resets_at: Option<String>,
    #[serde(default)]
    extra_credits: Option<ExtraCredits>,
    cached_at: String,
}

/// Normalized usage data, independent of where it came from (API, cache or
/// the `rate_limits` block Claude Code passes in the statusline input).
#[derive(Debug, Clone)]
pub(crate) struct UsageData {
    pub(crate) five_hour_utilization: f64,
    pub(crate) seven_day_utilization: f64,
    pub(crate) five_hour_resets_at: Option<String>,
    pub(crate) seven_day_resets_at: Option<String>,
    pub(crate) extra_credits: Option<ExtraCredits>,
}

impl UsageData {
    fn from_cache(cache: &ApiUsageCache) -> Self {
        Self {
            five_hour_utilization: cache.five_hour_utilization,
            seven_day_utilization: cache.seven_day_utilization,
            five_hour_resets_at: cache.five_hour_resets_at.clone(),
            seven_day_resets_at: cache.seven_day_resets_at.clone(),
            extra_credits: cache.extra_credits.clone(),
        }
    }

    fn from_api(response: &ApiUsageResponse) -> Self {
        Self {
            five_hour_utilization: response.five_hour.utilization,
            seven_day_utilization: response.seven_day.utilization,
            five_hour_resets_at: response.five_hour.resets_at.clone(),
            seven_day_resets_at: response.seven_day.resets_at.clone(),
            extra_credits: response
                .extra_usage
                .as_ref()
                .and_then(ExtraCredits::from_api),
        }
    }

    /// Build from the `rate_limits` block Claude Code passes. Each window is
    /// independently optional; `None` only when neither is present.
    fn from_input(limits: &crate::config::RateLimits) -> Option<Self> {
        let five_hour = limits.five_hour.as_ref();
        let seven_day = limits.seven_day.as_ref();
        if five_hour.is_none() && seven_day.is_none() {
            return None;
        }
        Some(Self {
            five_hour_utilization: five_hour.and_then(|w| w.used_percentage).unwrap_or(0.0),
            seven_day_utilization: seven_day.and_then(|w| w.used_percentage).unwrap_or(0.0),
            five_hour_resets_at: five_hour
                .and_then(|w| w.resets_at.as_ref())
                .map(|r| r.to_rfc3339()),
            seven_day_resets_at: seven_day
                .and_then(|w| w.resets_at.as_ref())
                .map(|r| r.to_rfc3339()),
            // Claude Code does not pass credit information in the statusline input
            extra_credits: None,
        })
    }

    fn to_cache(&self) -> ApiUsageCache {
        ApiUsageCache {
            version: CACHE_VERSION,
            five_hour_utilization: self.five_hour_utilization,
            seven_day_utilization: self.seven_day_utilization,
            five_hour_resets_at: self.five_hour_resets_at.clone(),
            seven_day_resets_at: self.seven_day_resets_at.clone(),
            extra_credits: self.extra_credits.clone(),
            cached_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Options of the `usage` segment, read from the active configuration
#[derive(Debug, Clone)]
pub struct UsageOptions {
    pub api_base_url: String,
    pub cache_duration: u64,
    pub timeout: u64,
    /// "time" (default): clock time / weekday. "countdown": time remaining, e.g. "4h 52m"
    pub countdown: bool,
    pub show_weekly: bool,
}

impl Default for UsageOptions {
    fn default() -> Self {
        Self {
            api_base_url: "https://api.anthropic.com".to_string(),
            cache_duration: 300,
            timeout: 2,
            countdown: false,
            show_weekly: true,
        }
    }
}

impl UsageOptions {
    /// Read the options from a segment's `options` map, keeping defaults for missing keys
    pub fn from_map(options: &HashMap<String, serde_json::Value>) -> Self {
        let defaults = Self::default();
        Self {
            api_base_url: options
                .get("api_base_url")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or(defaults.api_base_url),
            cache_duration: options
                .get("cache_duration")
                .and_then(|v| v.as_u64())
                .unwrap_or(defaults.cache_duration),
            timeout: options
                .get("timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(defaults.timeout),
            countdown: options
                .get("reset_format")
                .and_then(|v| v.as_str())
                .map(|v| v == "countdown")
                .unwrap_or(defaults.countdown),
            show_weekly: options
                .get("show_weekly")
                .and_then(|v| v.as_bool())
                .unwrap_or(defaults.show_weekly),
        }
    }
}

#[derive(Default)]
pub struct UsageSegment {
    options: UsageOptions,
}

impl UsageSegment {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use the options of the active configuration instead of the defaults
    pub fn with_options(mut self, options: UsageOptions) -> Self {
        self.options = options;
        self
    }

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

    fn parse_local(reset_time_str: Option<&str>) -> Option<DateTime<Local>> {
        let time_str = reset_time_str?;
        DateTime::parse_from_rfc3339(time_str)
            .ok()
            .map(|dt| dt.with_timezone(&Local))
    }

    /// Time remaining until reset, e.g. "4h 52m", "35m", "2d 3h"
    fn format_reset_countdown(reset_time_str: Option<&str>) -> String {
        let Some(time_str) = reset_time_str else {
            return "?".to_string();
        };
        let Ok(dt) = DateTime::parse_from_rfc3339(time_str) else {
            return "?".to_string();
        };
        let remaining = dt.with_timezone(&Utc).signed_duration_since(Utc::now());
        if remaining.num_seconds() <= 0 {
            return "now".to_string();
        }
        let total_minutes = remaining.num_minutes();
        let days = total_minutes / (24 * 60);
        let hours = (total_minutes % (24 * 60)) / 60;
        let minutes = total_minutes % 60;
        if days > 0 {
            format!("{}d {}h", days, hours)
        } else if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }

    /// Five-hour window reset: local time of day, e.g. "14:35"
    fn format_session_reset(reset_time_str: Option<&str>) -> String {
        match Self::parse_local(reset_time_str) {
            Some(local_dt) => format!("{:02}:{:02}", local_dt.hour(), local_dt.minute()),
            None => "?".to_string(),
        }
    }

    /// Weekly window reset: local weekday and hour, e.g. "Thu 00h"
    fn format_weekly_reset(reset_time_str: Option<&str>) -> String {
        match Self::parse_local(reset_time_str) {
            Some(mut local_dt) => {
                if local_dt.minute() > 45 {
                    local_dt += Duration::hours(1);
                }
                format!("{} {:02}h", local_dt.weekday(), local_dt.hour())
            }
            None => "?".to_string(),
        }
    }

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
        let cache: ApiUsageCache = serde_json::from_str(&content).ok()?;
        (cache.version == CACHE_VERSION).then_some(cache)
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
            let now = Utc::now();
            let elapsed = now.signed_duration_since(cached_at.with_timezone(&Utc));
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
        let settings_path = format!("{}/.claude/settings.json", home);

        let content = std::fs::read_to_string(&settings_path).ok()?;
        let settings: serde_json::Value = serde_json::from_str(&content).ok()?;

        // Try HTTPS_PROXY first, then HTTP_PROXY
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
}

/// Load usage data: API (cached on disk with `options.cache_duration`), falling
/// back to the `rate_limits` block Claude Code passes in the input. Shared by
/// the Usage and Credits segments; the on-disk cache means the second caller in
/// a render reads the response the first one fetched instead of hitting the API.
pub(crate) fn load_usage_data(input: &InputData, options: &UsageOptions) -> Option<UsageData> {
    UsageSegment::new()
        .with_options(options.clone())
        .load(input)
}

impl UsageSegment {
    fn load(&self, input: &InputData) -> Option<UsageData> {
        let api_base_url = self.options.api_base_url.as_str();
        let cache_duration = self.options.cache_duration;
        let timeout = self.options.timeout;

        // Rate limits passed by Claude Code itself: no network needed, used as fallback
        let input_data = input.rate_limits.as_ref().and_then(UsageData::from_input);

        match credentials::get_oauth_token() {
            Some(token) => {
                let cached_data = self.load_cache();
                let use_cached = cached_data
                    .as_ref()
                    .map(|cache| self.is_cache_valid(cache, cache_duration))
                    .unwrap_or(false);

                if use_cached {
                    cached_data.as_ref().map(UsageData::from_cache)
                } else {
                    match self.fetch_api_usage(api_base_url, &token, timeout) {
                        Some(response) => {
                            let data = UsageData::from_api(&response);
                            self.save_cache(&data.to_cache());
                            Some(data)
                        }
                        None => cached_data
                            .as_ref()
                            .map(UsageData::from_cache)
                            .or(input_data.clone()),
                    }
                }
            }
            None => input_data,
        }
    }
}

impl Segment for UsageSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let countdown = self.options.countdown;
        let show_weekly = self.options.show_weekly;

        let usage = self.load(input)?;

        let dynamic_icon = Self::get_circle_icon(usage.seven_day_utilization / 100.0);
        let five_hour_percent = usage.five_hour_utilization.round() as u8;
        let seven_day_percent = usage.seven_day_utilization.round() as u8;

        // Primary: five-hour session usage; its reset time is the session reset, not the weekly one
        let primary = format!("{}%", five_hour_percent);
        let session_reset = if countdown {
            Self::format_reset_countdown(usage.five_hour_resets_at.as_deref())
        } else {
            Self::format_session_reset(usage.five_hour_resets_at.as_deref())
        };
        // Blocks: "5h% · reset" │ "7d% · reset", each block uses "·" internally
        const BLOCK_DIVIDER: &str = " \u{2502} ";
        let mut secondary = format!("· {}", session_reset);
        if show_weekly {
            let weekly_reset = if countdown {
                Self::format_reset_countdown(usage.seven_day_resets_at.as_deref())
            } else {
                Self::format_weekly_reset(usage.seven_day_resets_at.as_deref())
            };
            secondary.push_str(&format!(
                "{}7d {}% · {}",
                BLOCK_DIVIDER, seven_day_percent, weekly_reset
            ));
        }

        let mut metadata = HashMap::new();
        metadata.insert("dynamic_icon".to_string(), dynamic_icon);
        metadata.insert(
            "five_hour_utilization".to_string(),
            usage.five_hour_utilization.to_string(),
        );
        metadata.insert(
            "seven_day_utilization".to_string(),
            usage.seven_day_utilization.to_string(),
        );
        if let Some(resets_at) = &usage.five_hour_resets_at {
            metadata.insert("five_hour_resets_at".to_string(), resets_at.clone());
        }
        if let Some(resets_at) = &usage.seven_day_resets_at {
            metadata.insert("seven_day_resets_at".to_string(), resets_at.clone());
        }

        Some(SegmentData {
            primary,
            secondary,
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Usage
    }
}
