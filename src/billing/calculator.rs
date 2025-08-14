use crate::billing::{BillingBlock, BurnRate, BurnRateTrend, ModelPricing, UsageEntry};
use chrono::{Duration, Local, Utc};
use std::collections::HashMap;

/// Calculate cost for a single usage entry
pub fn calculate_entry_cost(entry: &UsageEntry, pricing: &ModelPricing) -> f64 {
    let input_cost = (entry.input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k;
    let output_cost = (entry.output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k;
    let cache_creation_cost =
        (entry.cache_creation_tokens as f64 / 1000.0) * pricing.cache_creation_cost_per_1k;
    let cache_read_cost =
        (entry.cache_read_tokens as f64 / 1000.0) * pricing.cache_read_cost_per_1k;

    input_cost + output_cost + cache_creation_cost + cache_read_cost
}

/// Calculate total cost for a session
pub fn calculate_session_cost(
    entries: &[UsageEntry],
    session_id: &str,
    pricing_map: &HashMap<String, ModelPricing>,
) -> f64 {
    entries
        .iter()
        .filter(|e| e.session_id == session_id)
        .filter_map(|entry| {
            // Find pricing for this model
            ModelPricing::get_model_pricing(pricing_map, &entry.model)
                .map(|pricing| calculate_entry_cost(entry, pricing))
        })
        .sum()
}

/// Calculate total cost for today
pub fn calculate_daily_total(
    entries: &[UsageEntry],
    pricing_map: &HashMap<String, ModelPricing>,
) -> f64 {
    let today = Local::now().date_naive();

    entries
        .iter()
        .filter(|e| e.timestamp.with_timezone(&Local).date_naive() == today)
        .filter_map(|entry| {
            // Find pricing for this model
            ModelPricing::get_model_pricing(pricing_map, &entry.model)
                .map(|pricing| calculate_entry_cost(entry, pricing))
        })
        .sum()
}

/// Calculate burn rate based on recent activity
pub fn calculate_burn_rate(block: &BillingBlock, entries: &[UsageEntry]) -> Option<BurnRate> {
    let now = Utc::now();
    let five_minutes_ago = now - Duration::minutes(5);

    // Filter entries from the last 5 minutes within this block
    let recent_entries: Vec<&UsageEntry> = entries
        .iter()
        .filter(|e| {
            e.timestamp >= block.start_time
                && e.timestamp <= block.end_time
                && e.timestamp >= five_minutes_ago
        })
        .collect();

    if recent_entries.is_empty() {
        return None;
    }

    // Calculate time span
    let time_span = if recent_entries.len() == 1 {
        Duration::minutes(1) // Assume at least 1 minute for single entry
    } else {
        let first = recent_entries.first()?.timestamp;
        let last = recent_entries.last()?.timestamp;
        last - first
    };

    let minutes = time_span.num_seconds() as f64 / 60.0;
    if minutes <= 0.0 {
        return None;
    }

    // Calculate total tokens (all types)
    let total_tokens: u32 = recent_entries
        .iter()
        .map(|e| e.input_tokens + e.output_tokens + e.cache_creation_tokens + e.cache_read_tokens)
        .sum();

    // Calculate tokens excluding cache (for indicator thresholds)
    let non_cache_tokens: u32 = recent_entries
        .iter()
        .map(|e| e.input_tokens + e.output_tokens)
        .sum();

    let tokens_per_minute = total_tokens as f64 / minutes;
    let tokens_per_minute_for_indicator = non_cache_tokens as f64 / minutes;

    // Calculate cost per hour (simplified - assumes same rate)
    let cost_per_hour = (block.cost / block.total_tokens as f64) * tokens_per_minute * 60.0;

    // Determine trend (simplified)
    let trend = if recent_entries.len() >= 2 {
        let mid_point = recent_entries.len() / 2;
        let first_half_tokens: u32 = recent_entries[..mid_point]
            .iter()
            .map(|e| {
                e.input_tokens + e.output_tokens + e.cache_creation_tokens + e.cache_read_tokens
            })
            .sum();
        let second_half_tokens: u32 = recent_entries[mid_point..]
            .iter()
            .map(|e| {
                e.input_tokens + e.output_tokens + e.cache_creation_tokens + e.cache_read_tokens
            })
            .sum();

        if second_half_tokens > first_half_tokens {
            BurnRateTrend::Rising
        } else if second_half_tokens < first_half_tokens {
            BurnRateTrend::Falling
        } else {
            BurnRateTrend::Stable
        }
    } else {
        BurnRateTrend::Stable
    };

    Some(BurnRate {
        tokens_per_minute,
        tokens_per_minute_for_indicator,
        cost_per_hour,
        trend,
    })
}

/// Format remaining time in human-readable format
pub fn format_remaining_time(minutes: i64) -> String {
    if minutes <= 0 {
        return "expired".to_string();
    }

    let hours = minutes / 60;
    let mins = minutes % 60;

    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_calculate_entry_cost() {
        let entry = UsageEntry {
            timestamp: Utc::now(),
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_tokens: 100,
            cache_read_tokens: 50,
            model: "claude-3-5-sonnet".to_string(),
            cost: None,
            session_id: "test".to_string(),
        };

        let pricing = ModelPricing {
            model_name: "claude-3-5-sonnet".to_string(),
            input_cost_per_1k: 3.0,
            output_cost_per_1k: 15.0,
            cache_creation_cost_per_1k: 3.75,
            cache_read_cost_per_1k: 0.3,
        };

        let cost = calculate_entry_cost(&entry, &pricing);
        // 1000/1000 * 3.0 + 500/1000 * 15.0 + 100/1000 * 3.75 + 50/1000 * 0.3
        // = 3.0 + 7.5 + 0.375 + 0.015 = 10.89
        assert!((cost - 10.89).abs() < 0.001);
    }

    #[test]
    fn test_format_remaining_time() {
        assert_eq!(format_remaining_time(0), "expired");
        assert_eq!(format_remaining_time(-10), "expired");
        assert_eq!(format_remaining_time(30), "30m");
        assert_eq!(format_remaining_time(90), "1h 30m");
        assert_eq!(format_remaining_time(125), "2h 5m");
    }

    #[test]
    fn test_calculate_daily_total() {
        let now = Utc::now();
        let entries = vec![
            UsageEntry {
                timestamp: now,
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                model: "claude-3-5-sonnet".to_string(),
                cost: None,
                session_id: "test1".to_string(),
            },
            UsageEntry {
                timestamp: now - Duration::days(1), // Yesterday
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                model: "claude-3-5-sonnet".to_string(),
                cost: None,
                session_id: "test2".to_string(),
            },
        ];

        let mut pricing_map = HashMap::new();
        pricing_map.insert(
            "claude-3-5-sonnet".to_string(),
            ModelPricing {
                model_name: "claude-3-5-sonnet".to_string(),
                input_cost_per_1k: 3.0,
                output_cost_per_1k: 15.0,
                cache_creation_cost_per_1k: 0.0,
                cache_read_cost_per_1k: 0.0,
            },
        );

        let total = calculate_daily_total(&entries, &pricing_map);
        // Only today's entry: 1000/1000 * 3.0 + 500/1000 * 15.0 = 3.0 + 7.5 = 10.5
        assert!((total - 10.5).abs() < 0.001);
    }
}
