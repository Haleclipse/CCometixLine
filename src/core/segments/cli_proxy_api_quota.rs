use super::{Segment, SegmentData};
use crate::config::{AnsiColor, InputData, SegmentId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CLI Proxy API Quota response structures
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthFile {
    #[serde(rename = "type")]
    auth_type: String,
    auth_index: String,
    label: Option<String>,
    name: Option<String>,
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthFilesResponse {
    files: Vec<AuthFile>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiCallResponse {
    body: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct QuotaInfo {
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ModelInfo {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "quotaInfo")]
    quota_info: Option<QuotaInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AntigravityModelsResponse {
    models: Option<HashMap<String, ModelInfo>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiBucket {
    #[serde(rename = "modelId")]
    model_id: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiQuotaResponse {
    buckets: Option<Vec<GeminiBucket>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackedModel {
    Opus,
    Gemini3Pro,
    Gemini3Flash,
}

impl TrackedModel {
    pub fn alias_key(&self) -> &'static str {
        match self {
            Self::Opus => "opus_alias",
            Self::Gemini3Pro => "gemini3pro_alias",
            Self::Gemini3Flash => "gemini3flash_alias",
        }
    }

    pub fn color_key(&self) -> &'static str {
        match self {
            Self::Opus => "opus_color",
            Self::Gemini3Pro => "gemini3pro_color",
            Self::Gemini3Flash => "gemini3flash_color",
        }
    }

    pub fn default_alias(&self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Gemini3Pro => "3pro",
            Self::Gemini3Flash => "3flash",
        }
    }

    pub fn default_color(&self) -> AnsiColor {
        match self {
            Self::Opus => AnsiColor::Color256 { c256: 214 },
            Self::Gemini3Pro => AnsiColor::Color256 { c256: 129 },
            Self::Gemini3Flash => AnsiColor::Color256 { c256: 45 },
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Opus => "Opus",
            Self::Gemini3Pro => "Gemini 3 Pro",
            Self::Gemini3Flash => "Gemini 3 Flash",
        }
    }

    pub fn all() -> &'static [TrackedModel] {
        &[Self::Opus, Self::Gemini3Pro, Self::Gemini3Flash]
    }
}

/// Cache structure for CLI Proxy API quota data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CliProxyApiQuotaCache {
    quotas: Vec<ModelQuota>,
    cached_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelQuota {
    model_id: String,
    display_name: String,
    remaining_fraction: f64,
    auth_type: String,
}

#[derive(Default)]
pub struct CliProxyApiQuotaSegment;

impl CliProxyApiQuotaSegment {
    pub fn new() -> Self {
        Self
    }

    fn normalize_model_text(text: &str) -> String {
        let mut s = text.trim().to_lowercase();
        for suffix in ["-preview", " preview"] {
            if s.ends_with(suffix) {
                let new_len = s.len().saturating_sub(suffix.len());
                s.truncate(new_len);
                s = s.trim_end().to_string();
            }
        }
        s
    }

    fn tracked_model_for(model_id: &str, display_name: &str) -> Option<TrackedModel> {
        let id = Self::normalize_model_text(model_id);
        let name = Self::normalize_model_text(display_name);

        if id.contains("opus") || name.contains("opus") {
            return Some(TrackedModel::Opus);
        }
        if id.contains("gemini-3-pro") || name.contains("gemini 3 pro") {
            return Some(TrackedModel::Gemini3Pro);
        }
        if id.contains("gemini-3-flash") || name.contains("gemini 3 flash") {
            return Some(TrackedModel::Gemini3Flash);
        }

        None
    }

    fn tracked_model_for_quota(quota: &ModelQuota) -> Option<TrackedModel> {
        Self::tracked_model_for(&quota.model_id, &quota.display_name)
    }

    fn get_alias(&self, options: &HashMap<String, serde_json::Value>, model: TrackedModel) -> String {
        options
            .get(model.alias_key())
            .and_then(|v| v.as_str())
            .unwrap_or(model.default_alias())
            .to_string()
    }

    fn get_color(&self, options: &HashMap<String, serde_json::Value>, model: TrackedModel) -> AnsiColor {
        options
            .get(model.color_key())
            .and_then(|v| serde_json::from_value::<AnsiColor>(v.clone()).ok())
            .unwrap_or_else(|| model.default_color())
    }

    /// Apply ANSI foreground color to text (resets only foreground, keeps background)
    pub fn apply_foreground_color(text: &str, color: &AnsiColor) -> String {
        let prefix = match color {
            AnsiColor::Color16 { c16 } => {
                let code = if *c16 < 8 { 30 + c16 } else { 90 + (c16 - 8) };
                format!("\x1b[{}m", code)
            }
            AnsiColor::Color256 { c256 } => format!("\x1b[38;5;{}m", c256),
            AnsiColor::Rgb { r, g, b } => format!("\x1b[38;2;{};{};{}m", r, g, b),
        };
        // Use 39m to reset foreground only (keeps background intact if set)
        format!("{}{}\x1b[39m", prefix, text)
    }

    fn format_tracked_output(
        &self,
        quotas: &[ModelQuota],
        options: &HashMap<String, serde_json::Value>,
        separator: &str,
    ) -> String {
        #[derive(Default)]
        struct SumCount {
            sum: f64,
            count: u32,
        }

        let mut agg: HashMap<TrackedModel, SumCount> = HashMap::new();
        for quota in quotas {
            let Some(model) = Self::tracked_model_for_quota(quota) else {
                continue;
            };
            let entry = agg.entry(model).or_default();
            entry.sum += quota.remaining_fraction;
            entry.count += 1;
        }

        let mut parts = Vec::new();
        for model in [
            TrackedModel::Opus,
            TrackedModel::Gemini3Pro,
            TrackedModel::Gemini3Flash,
        ] {
            let Some(entry) = agg.get(&model) else {
                continue;
            };
            if entry.count == 0 {
                continue;
            }

            let avg = entry.sum / entry.count as f64;
            let percent = (avg * 100.0).round().clamp(0.0, 100.0) as u8;
            let alias = self.get_alias(options, model);
            let color = self.get_color(options, model);
            let label = format!("{}:{}%", alias, percent);
            parts.push(Self::apply_foreground_color(&label, &color));
        }

        parts.join(separator)
    }

    fn get_cache_path() -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        Some(
            home.join(".claude")
                .join("ccline")
                .join(".cli_proxy_api_quota_cache.json"),
        )
    }

    fn load_cache(&self) -> Option<CliProxyApiQuotaCache> {
        let cache_path = Self::get_cache_path()?;
        if !cache_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_cache(&self, cache: &CliProxyApiQuotaCache) {
        if let Some(cache_path) = Self::get_cache_path() {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(cache) {
                let _ = std::fs::write(&cache_path, json);
            }
        }
    }

    fn is_cache_valid(&self, cache: &CliProxyApiQuotaCache, cache_duration: u64) -> bool {
        if let Ok(cached_at) = DateTime::parse_from_rfc3339(&cache.cached_at) {
            let now = Utc::now();
            let elapsed = now.signed_duration_since(cached_at.with_timezone(&Utc));
            elapsed.num_seconds() < cache_duration as i64
        } else {
            false
        }
    }

    fn get_auth_files(&self, host: &str, key: &str) -> Option<Vec<AuthFile>> {
        let url = format!("{}/v0/management/auth-files", host);

        let agent = ureq::AgentBuilder::new().build();
        let response = agent
            .get(&url)
            .set("Authorization", &format!("Bearer {}", key))
            .timeout(std::time::Duration::from_secs(5))
            .call()
            .ok()?;

        if response.status() == 200 {
            let resp: AuthFilesResponse = response.into_json().ok()?;
            Some(resp.files)
        } else {
            None
        }
    }

    fn api_call(
        &self,
        host: &str,
        key: &str,
        auth_index: &str,
        method: &str,
        url: &str,
        data: &str,
        extra_headers: Option<HashMap<&str, &str>>,
    ) -> Option<ApiCallResponse> {
        let api_url = format!("{}/v0/management/api-call", host);

        let mut headers = HashMap::new();
        headers.insert("Authorization", "Bearer $TOKEN$");
        headers.insert("Content-Type", "application/json");
        if let Some(extra) = extra_headers {
            for (k, v) in extra {
                headers.insert(k, v);
            }
        }

        let payload = serde_json::json!({
            "authIndex": auth_index,
            "method": method,
            "url": url,
            "header": headers,
            "data": data
        });

        let agent = ureq::AgentBuilder::new().build();
        let response = agent
            .post(&api_url)
            .set("Authorization", &format!("Bearer {}", key))
            .set("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(10))
            .send_json(&payload)
            .ok()?;

        if response.status() == 200 {
            response.into_json().ok()
        } else {
            None
        }
    }

    fn get_antigravity_quota(&self, host: &str, key: &str, auth_index: &str) -> Vec<ModelQuota> {
        let mut extra_headers = HashMap::new();
        extra_headers.insert("User-Agent", "antigravity/1.11.5 windows/amd64");

        let result = self.api_call(
            host,
            key,
            auth_index,
            "POST",
            "https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
            "{}",
            Some(extra_headers),
        );

        let mut quotas = Vec::new();

        if let Some(response) = result {
            if let Some(body) = response.body {
                if let Ok(models_resp) = serde_json::from_str::<AntigravityModelsResponse>(&body) {
                    if let Some(models) = models_resp.models {
                        for (model_id, model_info) in models {
                            if let Some(quota_info) = model_info.quota_info {
                                if let Some(remaining) = quota_info.remaining_fraction {
                                    let display_name = model_info
                                        .display_name
                                        .clone()
                                        .unwrap_or_else(|| model_id.clone());

                                    // Only keep Opus / Gemini 3 Pro / Gemini 3 Flash
                                    if Self::tracked_model_for(&model_id, &display_name).is_none() {
                                        continue;
                                    }

                                    quotas.push(ModelQuota {
                                        model_id: model_id.clone(),
                                        display_name,
                                        remaining_fraction: remaining,
                                        auth_type: "antigravity".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        quotas
    }

    fn extract_project_from_name(&self, name: &str) -> Option<String> {
        // gemini-gaakki@gmail.com-airy-lodge-481706-r3.json -> airy-lodge-481706-r3
        let name = name.replace(".json", "");
        let parts: Vec<&str> = name.split('-').collect();
        if parts.len() >= 4 {
            for (i, part) in parts.iter().enumerate() {
                if part.contains('@') {
                    return Some(parts[i + 1..].join("-"));
                }
            }
        }
        None
    }

    fn get_gemini_cli_quota(
        &self,
        host: &str,
        key: &str,
        auth_index: &str,
        project: &str,
    ) -> Vec<ModelQuota> {
        let data = serde_json::json!({"project": project}).to_string();

        let result = self.api_call(
            host,
            key,
            auth_index,
            "POST",
            "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota",
            &data,
            None,
        );

        let mut quotas = Vec::new();

        if let Some(response) = result {
            if let Some(body) = response.body {
                if let Ok(quota_resp) = serde_json::from_str::<GeminiQuotaResponse>(&body) {
                    if let Some(buckets) = quota_resp.buckets {
                        for bucket in buckets {
                            if let (Some(model_id), Some(remaining)) =
                                (bucket.model_id, bucket.remaining_fraction)
                            {
                                // Only keep Opus / Gemini 3 Pro / Gemini 3 Flash
                                if Self::tracked_model_for(&model_id, &model_id).is_none() {
                                    continue;
                                }

                                quotas.push(ModelQuota {
                                    model_id: model_id.clone(),
                                    display_name: model_id,
                                    remaining_fraction: remaining,
                                    auth_type: "gemini-cli".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        quotas
    }

    fn fetch_all_quotas(&self, host: &str, key: &str, auth_type_filter: &str) -> Vec<ModelQuota> {
        let mut all_quotas = Vec::new();

        let auth_files = match self.get_auth_files(host, key) {
            Some(files) => files,
            None => return all_quotas,
        };

        for file in auth_files {
            // Skip disabled accounts
            if file.disabled.unwrap_or(false) {
                continue;
            }

            // Apply type filter
            if auth_type_filter != "all" && file.auth_type != auth_type_filter {
                continue;
            }

            let quotas = match file.auth_type.as_str() {
                "antigravity" => self.get_antigravity_quota(host, key, &file.auth_index),
                "gemini-cli" => {
                    if let Some(project) =
                        self.extract_project_from_name(file.name.as_deref().unwrap_or(""))
                    {
                        self.get_gemini_cli_quota(host, key, &file.auth_index, &project)
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };

            all_quotas.extend(quotas);
        }

        all_quotas
    }
}

impl Segment for CliProxyApiQuotaSegment {
    fn collect(&self, _input: &InputData) -> Option<SegmentData> {
        // This method loads config from disk - use collect_with_options for better performance
        let config = crate::config::Config::load().ok()?;
        let segment_config = config.segments.iter().find(|s| s.id == SegmentId::CliProxyApiQuota)?;
        self.collect_with_options(&segment_config.options)
    }

    fn id(&self) -> SegmentId {
        SegmentId::CliProxyApiQuota
    }
}

impl CliProxyApiQuotaSegment {
    /// Collect quota data using provided options (avoids loading config from disk)
    pub fn collect_with_options(&self, options: &HashMap<String, serde_json::Value>) -> Option<SegmentData> {
        let host = options
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8317");

        let key = options
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("nbkey");

        let cache_duration = options
            .get("cache_duration")
            .and_then(|v| v.as_u64())
            .unwrap_or(180);

        let auth_type = options
            .get("auth_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let separator = options
            .get("separator")
            .and_then(|v| v.as_str())
            .unwrap_or(" | ");

        // Try to use cache first
        let cached_data = self.load_cache();
        let use_cached = cached_data
            .as_ref()
            .map(|cache| self.is_cache_valid(cache, cache_duration))
            .unwrap_or(false);

        let quotas = if use_cached {
            cached_data.unwrap().quotas
        } else {
            let fetched = self.fetch_all_quotas(host, key, auth_type);
            if !fetched.is_empty() {
                let cache = CliProxyApiQuotaCache {
                    quotas: fetched.clone(),
                    cached_at: Utc::now().to_rfc3339(),
                };
                self.save_cache(&cache);
            }
            if fetched.is_empty() {
                // Fall back to cached data if fetch fails
                cached_data.map(|c| c.quotas).unwrap_or_default()
            } else {
                fetched
            }
        };

        if quotas.is_empty() {
            return None;
        }

        let primary = self.format_tracked_output(&quotas, options, separator);

        if primary.is_empty() {
            return None;
        }

        let mut metadata = HashMap::new();
        metadata.insert("raw_text".to_string(), "true".to_string());

        Some(SegmentData {
            primary,
            secondary: String::new(),
            metadata,
        })
    }
}
