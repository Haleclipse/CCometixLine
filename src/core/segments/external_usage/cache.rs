use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub metrics: Vec<CachedMetric>,
    pub cached_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMetric {
    pub label: String,
    pub remaining_pct: u32,
}

pub struct Cache {
    path: std::path::PathBuf,
}

impl Cache {
    pub fn new(provider_name: &str) -> Option<Self> {
        let home = dirs::home_dir()?;
        let path = home
            .join(".claude")
            .join("ccline")
            .join(format!(".external_usage_{}_cache.json", provider_name));
        Some(Self { path })
    }

    pub fn load(&self) -> Option<CacheEntry> {
        if !self.path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self, entry: &CacheEntry) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(entry) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn is_valid(entry: &CacheEntry, cache_duration_secs: u64) -> bool {
        DateTime::parse_from_rfc3339(&entry.cached_at)
            .map(|cached_at| {
                let elapsed = Utc::now().signed_duration_since(cached_at.with_timezone(&Utc));
                elapsed.num_seconds() < cache_duration_secs as i64
            })
            .unwrap_or(false)
    }
}
