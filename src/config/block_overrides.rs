use chrono::{DateTime, Local, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Block override configuration for a specific date
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockOverride {
    /// Block start time (UTC, floored to the hour)
    pub start_time: DateTime<Utc>,
    /// Override source ("manual", device ID, etc.)
    pub source: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Optional notes
    pub notes: Option<String>,
}

impl BlockOverride {
    pub fn new(start_time: DateTime<Utc>, source: String, notes: Option<String>) -> Self {
        Self {
            start_time,
            source,
            created_at: Utc::now(),
            notes,
        }
    }
}

/// Error types for block override operations
#[derive(Debug)]
pub enum BlockOverrideError {
    InvalidFormat,
    HourOutOfRange,
    TimeOutOfRange,
    FutureTime,
    FileAccess(std::io::Error),
    CorruptedConfig(String),
}

impl std::fmt::Display for BlockOverrideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockOverrideError::InvalidFormat => {
                write!(
                    f,
                    "Invalid time format. Expected: hour (0-23), HH:MM, or ISO timestamp"
                )
            }
            BlockOverrideError::HourOutOfRange => write!(f, "Hour must be between 0 and 23"),
            BlockOverrideError::TimeOutOfRange => write!(f, "Time values out of range"),
            BlockOverrideError::FutureTime => write!(f, "Cannot set future time"),
            BlockOverrideError::FileAccess(e) => {
                write!(f, "Failed to access configuration file: {}", e)
            }
            BlockOverrideError::CorruptedConfig(msg) => {
                write!(f, "Configuration file is corrupted: {}", msg)
            }
        }
    }
}

impl std::error::Error for BlockOverrideError {}

impl From<std::io::Error> for BlockOverrideError {
    fn from(error: std::io::Error) -> Self {
        BlockOverrideError::FileAccess(error)
    }
}

impl From<serde_json::Error> for BlockOverrideError {
    fn from(error: serde_json::Error) -> Self {
        BlockOverrideError::CorruptedConfig(format!("JSON error: {}", error))
    }
}

/// Block Override Manager handles configuration persistence and CRUD operations
pub struct BlockOverrideManager {
    config_path: PathBuf,
    overrides: HashMap<String, BlockOverride>,
}

impl BlockOverrideManager {
    /// Create a new BlockOverrideManager with default config path
    pub fn new() -> Result<Self, BlockOverrideError> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| {
                BlockOverrideError::FileAccess(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory",
                ))
            })?
            .join(".claude")
            .join("ccline");

        let config_path = config_dir.join("block_overrides.json");

        Ok(Self {
            config_path,
            overrides: HashMap::new(),
        })
    }

    /// Create BlockOverrideManager with custom config path (for testing)
    pub fn with_path(config_path: PathBuf) -> Self {
        Self {
            config_path,
            overrides: HashMap::new(),
        }
    }

    /// Ensure the configuration directory exists
    fn ensure_config_dir(&self) -> Result<(), BlockOverrideError> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Load configuration from file
    pub fn load(&mut self) -> Result<(), BlockOverrideError> {
        if !self.config_path.exists() {
            // File doesn't exist, start with empty configuration
            return Ok(());
        }

        let content = fs::read_to_string(&self.config_path)?;
        if content.trim().is_empty() {
            // Empty file, start with empty configuration
            return Ok(());
        }

        self.overrides = serde_json::from_str(&content).map_err(|e| {
            BlockOverrideError::CorruptedConfig(format!("JSON parsing failed: {}", e))
        })?;

        Ok(())
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), BlockOverrideError> {
        self.ensure_config_dir()?;

        let content = serde_json::to_string_pretty(&self.overrides)?;
        fs::write(&self.config_path, content)?;

        Ok(())
    }

    /// Set an override for a specific date
    pub fn set_override(
        &mut self,
        date: NaiveDate,
        start_time: DateTime<Utc>,
        source: String,
        notes: Option<String>,
    ) -> Result<(), BlockOverrideError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let override_config = BlockOverride::new(floor_to_hour(start_time), source, notes);
        self.overrides.insert(date_str, override_config);
        self.save()
    }

    /// Get an override for a specific date
    pub fn get_override(&self, date: NaiveDate) -> Option<&BlockOverride> {
        let date_str = date.format("%Y-%m-%d").to_string();
        self.overrides.get(&date_str)
    }

    /// Clear an override for a specific date
    pub fn clear_override(&mut self, date: NaiveDate) -> Result<bool, BlockOverrideError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let removed = self.overrides.remove(&date_str).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Clean up expired overrides (older than retention_days)
    pub fn cleanup_expired(&mut self, retention_days: u32) -> Result<usize, BlockOverrideError> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let initial_count = self.overrides.len();

        self.overrides
            .retain(|_date, override_config| override_config.created_at > cutoff);

        let removed_count = initial_count - self.overrides.len();
        if removed_count > 0 {
            self.save()?;
        }

        Ok(removed_count)
    }

    /// Parse various time input formats (input interpreted as local time)
    pub fn parse_time_input(input: &str) -> Result<DateTime<Utc>, BlockOverrideError> {
        let today = Local::now().date_naive();

        // Try parsing as a single number (0-23 hour)
        if let Ok(hour) = input.parse::<u32>() {
            if hour <= 23 {
                let local_time = today
                    .and_hms_opt(hour, 0, 0)
                    .ok_or(BlockOverrideError::TimeOutOfRange)?
                    .and_local_timezone(Local)
                    .single()
                    .ok_or(BlockOverrideError::TimeOutOfRange)?;

                // Check if this would be a future time (compare in local timezone)
                if local_time > Local::now() {
                    return Err(BlockOverrideError::FutureTime);
                }

                // Convert to UTC for storage
                return Ok(local_time.with_timezone(&Utc));
            } else {
                return Err(BlockOverrideError::HourOutOfRange);
            }
        }

        // Try parsing as HH:MM format
        if let Some((hour_str, minute_str)) = input.split_once(':') {
            let hour: u32 = hour_str
                .parse()
                .map_err(|_| BlockOverrideError::InvalidFormat)?;
            let minute: u32 = minute_str
                .parse()
                .map_err(|_| BlockOverrideError::InvalidFormat)?;

            if hour <= 23 && minute <= 59 {
                let local_time = today
                    .and_hms_opt(hour, 0, 0) // Floor to hour (ignore minutes)
                    .ok_or(BlockOverrideError::TimeOutOfRange)?
                    .and_local_timezone(Local)
                    .single()
                    .ok_or(BlockOverrideError::TimeOutOfRange)?;

                // Check if this would be a future time (compare in local timezone)
                if local_time > Local::now() {
                    return Err(BlockOverrideError::FutureTime);
                }

                // Convert to UTC for storage
                return Ok(local_time.with_timezone(&Utc));
            } else {
                return Err(BlockOverrideError::TimeOutOfRange);
            }
        }

        // Try parsing as ISO timestamp (interpreted as given timezone)
        match DateTime::parse_from_rfc3339(input) {
            Ok(dt) => {
                let local_time = dt.with_timezone(&Local);

                // Check if this would be a future time (compare in local timezone)
                if local_time > Local::now() {
                    return Err(BlockOverrideError::FutureTime);
                }

                // Convert to UTC and floor to hour
                Ok(floor_to_hour(dt.with_timezone(&Utc)))
            }
            Err(_) => Err(BlockOverrideError::InvalidFormat),
        }
    }

    /// Get the number of currently stored overrides
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Get all override dates (for debugging/display)
    pub fn get_all_dates(&self) -> Vec<String> {
        self.overrides.keys().cloned().collect()
    }

    /// Get the config file path (for debugging/display)
    pub fn get_config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

/// Floor a timestamp down to the nearest hour (set minutes, seconds, nanoseconds to 0)
pub fn floor_to_hour(timestamp: DateTime<Utc>) -> DateTime<Utc> {
    timestamp
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
}

impl Default for BlockOverrideManager {
    fn default() -> Self {
        // For the Default trait, we return an error-safe version
        // In practice, new() should be used which can handle errors
        Self::new().unwrap_or_else(|_| {
            // Fallback to current directory if home directory is not available
            Self::with_path(
                std::env::current_dir()
                    .unwrap()
                    .join("block_overrides.json"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_floor_to_hour() {
        let test_time = Utc.with_ymd_and_hms(2024, 8, 14, 14, 37, 42).unwrap();
        let floored = floor_to_hour(test_time);

        assert_eq!(floored.hour(), 14);
        assert_eq!(floored.minute(), 0);
        assert_eq!(floored.second(), 0);
        assert_eq!(floored.nanosecond(), 0);
    }

    #[test]
    fn test_parse_time_input_single_digit() {
        // Note: These tests might fail if run in certain time conditions
        // due to future time checking. Times are now interpreted as local time
        // and converted to UTC for storage.

        let result = BlockOverrideManager::parse_time_input("8");
        match result {
            Ok(time) => {
                // The result should be 8 AM local time converted to UTC
                // We can't assert exact UTC hour since it depends on local timezone
                // Just verify it's a valid time and minute is 0
                assert_eq!(time.minute(), 0);
                assert_eq!(time.second(), 0);
                // Verify it's today's date
                let today = Local::now().date_naive();
                assert_eq!(time.date_naive(), today);
            }
            Err(BlockOverrideError::FutureTime) => {
                // This is expected if the test runs after 8 AM local time on the same day
                // The logic is working correctly
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_parse_time_input_invalid_hour() {
        let result = BlockOverrideManager::parse_time_input("24");
        assert!(matches!(result, Err(BlockOverrideError::HourOutOfRange)));

        let result = BlockOverrideManager::parse_time_input("25");
        assert!(matches!(result, Err(BlockOverrideError::HourOutOfRange)));
    }

    #[test]
    fn test_parse_time_input_invalid_format() {
        let result = BlockOverrideManager::parse_time_input("abc");
        assert!(matches!(result, Err(BlockOverrideError::InvalidFormat)));
    }

    #[test]
    fn test_block_override_creation() {
        let start_time = Utc::now();
        let override_config = BlockOverride::new(
            start_time,
            "manual".to_string(),
            Some("Test override".to_string()),
        );

        assert_eq!(override_config.start_time, start_time);
        assert_eq!(override_config.source, "manual");
        assert_eq!(override_config.notes, Some("Test override".to_string()));
        assert!(override_config.created_at <= Utc::now());
    }
}
