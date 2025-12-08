use crate::config::{AnsiColor, Config, SegmentId};
use once_cell::sync::OnceCell;
use std::sync::Mutex;

/// Cache for loaded config to avoid repeated disk I/O
static CONFIG_CACHE: OnceCell<Mutex<Option<Config>>> = OnceCell::new();

/// Load config with caching to avoid repeated disk reads
fn get_cached_config() -> Option<Config> {
    let cache = CONFIG_CACHE.get_or_init(|| Mutex::new(None));
    let mut cache_guard = cache.lock().ok()?;

    if cache_guard.is_none() {
        *cache_guard = Config::load().ok();
    }

    cache_guard.clone()
}

/// Helper to get warning and critical thresholds for a segment
pub fn get_thresholds_for_segment(segment_id: SegmentId) -> Option<(f64, f64)> {
    let config = get_cached_config()?;
    let segment_config = config.segments.iter().find(|s| s.id == segment_id)?;

    let warning_threshold = segment_config
        .options
        .get("warning_threshold")
        .and_then(|v| v.as_u64())
        .unwrap_or(60) as f64;

    let critical_threshold = segment_config
        .options
        .get("critical_threshold")
        .and_then(|v| v.as_u64())
        .unwrap_or(80) as f64;

    Some((warning_threshold, critical_threshold))
}

/// Get color override based on utilization percentage
pub fn get_color_for_utilization(segment_id: SegmentId, utilization: f64) -> Option<AnsiColor> {
    let config = get_cached_config()?;
    let segment_config = config.segments.iter().find(|s| s.id == segment_id)?;
    let (warning_threshold, critical_threshold) = get_thresholds_for_segment(segment_id)?;

    // Determine which color to use based on utilization
    if utilization >= critical_threshold {
        // Critical threshold exceeded - use critical color
        segment_config
            .options
            .get("critical_color")
            .and_then(|v| {
                if let Some(c256) = v.get("c256").and_then(|c| c.as_u64()) {
                    Some(AnsiColor::Color256 { c256: c256 as u8 })
                } else if let Some(c16) = v.get("c16").and_then(|c| c.as_u64()) {
                    Some(AnsiColor::Color16 { c16: c16 as u8 })
                } else {
                    None
                }
            })
    } else if utilization >= warning_threshold {
        // Warning threshold exceeded - use warning color
        segment_config
            .options
            .get("warning_color")
            .and_then(|v| {
                if let Some(c256) = v.get("c256").and_then(|c| c.as_u64()) {
                    Some(AnsiColor::Color256 { c256: c256 as u8 })
                } else if let Some(c16) = v.get("c16").and_then(|c| c.as_u64()) {
                    Some(AnsiColor::Color16 { c16: c16 as u8 })
                } else {
                    None
                }
            })
    } else {
        // Below warning threshold - use default color
        None
    }
}

/// Check if text should be bold based on utilization percentage
pub fn should_be_bold(segment_id: SegmentId, utilization: f64) -> Option<bool> {
    let config = get_cached_config()?;
    let segment_config = config.segments.iter().find(|s| s.id == segment_id)?;
    let (warning_threshold, critical_threshold) = get_thresholds_for_segment(segment_id)?;

    // Determine if text should be bold based on utilization
    if utilization >= critical_threshold {
        // Critical threshold - check critical_bold option
        segment_config
            .options
            .get("critical_bold")
            .and_then(|v| v.as_bool())
    } else if utilization >= warning_threshold {
        // Warning threshold - check warning_bold option
        segment_config
            .options
            .get("warning_bold")
            .and_then(|v| v.as_bool())
    } else {
        // Below warning threshold - no bold override
        None
    }
}

/// Invalidate the config cache (useful for tests or when config changes)
#[allow(dead_code)]
pub fn invalidate_config_cache() {
    if let Some(cache) = CONFIG_CACHE.get() {
        if let Ok(mut cache_guard) = cache.lock() {
            *cache_guard = None;
        }
    }
}
