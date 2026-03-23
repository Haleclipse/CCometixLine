use super::{UsageMetric, UsageProvider};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct ApiResponse {
    model_remains: Vec<ModelRemain>,
}

#[derive(Debug, Deserialize)]
struct ModelRemain {
    model_name: String,
    current_interval_total_count: u64,
    current_interval_usage_count: u64,
    current_weekly_total_count: u64,
    current_weekly_usage_count: u64,
}

pub struct MinimaxProvider;

impl UsageProvider for MinimaxProvider {
    fn name(&self) -> &str {
        "minimax"
    }

    fn fetch(&self, options: &HashMap<String, Value>) -> Option<Vec<UsageMetric>> {
        let auth_env = options
            .get("auth_env")
            .and_then(|v| v.as_str())
            .unwrap_or("MINIMAX_API_KEY");

        let api_key = std::env::var(auth_env).ok().filter(|k| !k.is_empty())?;

        let timeout_secs = options
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);

        let preferred_model = options
            .get("preferred_model")
            .and_then(|v| v.as_str())
            .unwrap_or("MiniMax-M2.7");

        let response = Self::fetch_api(&api_key, timeout_secs)?;

        // 优先匹配指定模型，回退到第一个可用模型
        let model = response
            .model_remains
            .iter()
            .find(|m| m.model_name == preferred_model)
            .or_else(|| response.model_remains.first())?;

        let weekly_remaining_pct = remaining_pct(
            model.current_weekly_usage_count,
            model.current_weekly_total_count,
        );
        let interval_remaining_pct = remaining_pct(
            model.current_interval_usage_count,
            model.current_interval_total_count,
        );

        Some(vec![
            UsageMetric {
                label: "7d".to_string(),
                remaining_pct: weekly_remaining_pct,
            },
            UsageMetric {
                label: "5h".to_string(),
                remaining_pct: interval_remaining_pct,
            },
        ])
    }
}

impl MinimaxProvider {
    fn fetch_api(api_key: &str, timeout_secs: u64) -> Option<ApiResponse> {
        let url = "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains";

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs)))
            .build()
            .new_agent();

        agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .call()
            .ok()?
            .into_body()
            .read_json()
            .ok()
    }
}

fn remaining_pct(usage: u64, total: u64) -> u32 {
    if total == 0 {
        return 100;
    }
    let used_pct = (usage as f64 / total as f64 * 100.0) as u32;
    100u32.saturating_sub(used_pct)
}
