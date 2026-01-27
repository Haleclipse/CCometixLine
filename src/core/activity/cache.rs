//! Configuration counts caching with TTL support
//!
//! This module provides caching functionality for config counts to avoid
//! repeatedly scanning the filesystem. The cache is stored in a JSON file
//! at `~/.claude/ccline/.config-cache.json` with a configurable TTL.

use super::config_counter::count_configs;
use super::types::ConfigCounts;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default cache TTL in seconds
pub const DEFAULT_TTL_SECS: u64 = 60;

/// Cache file name
const CACHE_FILE_NAME: &str = ".config-cache.json";

/// Cache entry structure stored in the cache file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Cached configuration counts
    pub counts: ConfigCounts,
    /// Unix timestamp when the cache was created (seconds since epoch)
    pub timestamp_secs: u64,
    /// Current working directory used when counting (for cache invalidation)
    pub cwd: Option<String>,
}

impl CacheEntry {
    /// Create a new cache entry with the current timestamp
    pub fn new(counts: ConfigCounts, cwd: Option<String>) -> Self {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            counts,
            timestamp_secs,
            cwd,
        }
    }

    /// Check if the cache entry is still valid
    ///
    /// A cache entry is valid if:
    /// 1. It hasn't expired (current time - timestamp < ttl)
    /// 2. The cwd matches the requested cwd
    pub fn is_valid(&self, cwd: Option<&str>, ttl_secs: u64) -> bool {
        // Check cwd match
        let cwd_matches = match (&self.cwd, cwd) {
            (Some(cached_cwd), Some(requested_cwd)) => cached_cwd == requested_cwd,
            (None, None) => true,
            _ => false,
        };

        if !cwd_matches {
            return false;
        }

        // Check TTL
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let age = current_time.saturating_sub(self.timestamp_secs);
        age < ttl_secs
    }
}

/// Get the cache file path
///
/// Returns `~/.claude/ccline/.config-cache.json`
fn get_cache_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("ccline").join(CACHE_FILE_NAME))
}

/// Read the cache entry from the cache file
fn read_cache() -> Option<CacheEntry> {
    let cache_path = get_cache_path()?;

    if !cache_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&cache_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write a cache entry to the cache file
fn write_cache(entry: &CacheEntry) -> bool {
    let cache_path = match get_cache_path() {
        Some(p) => p,
        None => return false,
    };

    // Ensure parent directory exists
    if let Some(parent) = cache_path.parent() {
        if let Err(_) = fs::create_dir_all(parent) {
            return false;
        }
    }

    // Serialize and write
    let content = match serde_json::to_string_pretty(entry) {
        Ok(c) => c,
        Err(_) => return false,
    };

    fs::write(&cache_path, content).is_ok()
}

/// Get configuration counts with caching support
///
/// This function checks the cache first and returns cached counts if valid.
/// If the cache is invalid or missing, it calls `count_configs()` to get
/// fresh counts, saves them to the cache, and returns them.
///
/// # Arguments
///
/// * `cwd` - Optional current working directory for project scope scanning
/// * `ttl_secs` - Optional TTL in seconds (defaults to 60 seconds)
///
/// # Returns
///
/// A `ConfigCounts` struct with the configuration counts
///
/// # Cache Behavior
///
/// - Cache file location: `~/.claude/ccline/.config-cache.json`
/// - Cache is invalidated if:
///   - TTL has expired
///   - The cwd doesn't match the cached cwd
///   - The cache file is missing or corrupted
/// - On any I/O error, fresh counts are returned (fail-safe)
///
/// # Example
///
/// ```ignore
/// use ccometixline::core::activity::cache::get_config_counts_cached;
///
/// // Get counts with default TTL (60 seconds)
/// let counts = get_config_counts_cached(Some("/path/to/project"), None);
///
/// // Get counts with custom TTL (30 seconds)
/// let counts = get_config_counts_cached(Some("/path/to/project"), Some(30));
/// ```
pub fn get_config_counts_cached(cwd: Option<&str>, ttl_secs: Option<u64>) -> ConfigCounts {
    let ttl = ttl_secs.unwrap_or(DEFAULT_TTL_SECS);

    // Try to read from cache
    if let Some(entry) = read_cache() {
        if entry.is_valid(cwd, ttl) {
            return entry.counts;
        }
    }

    // Cache miss or invalid - get fresh counts
    let counts = count_configs(cwd);

    // Save to cache (ignore errors - caching is best-effort)
    let entry = CacheEntry::new(counts.clone(), cwd.map(String::from));
    let _ = write_cache(&entry);

    counts
}

/// Invalidate the cache by deleting the cache file
///
/// This function removes the cache file, forcing the next call to
/// `get_config_counts_cached()` to fetch fresh counts.
///
/// # Returns
///
/// `true` if the cache was successfully invalidated (or didn't exist),
/// `false` if there was an error deleting the file.
///
/// # Example
///
/// ```ignore
/// use ccometixline::core::activity::cache::invalidate_cache;
///
/// // Force refresh on next call
/// invalidate_cache();
/// ```
pub fn invalidate_cache() -> bool {
    let cache_path = match get_cache_path() {
        Some(p) => p,
        None => return true, // No cache path means nothing to invalidate
    };

    if !cache_path.exists() {
        return true; // Nothing to delete
    }

    fs::remove_file(&cache_path).is_ok()
}

/// Get the cache file path (for testing/debugging)
///
/// Returns the full path to the cache file, or None if the home directory
/// cannot be determined.
pub fn cache_file_path() -> Option<PathBuf> {
    get_cache_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to create a temporary cache directory structure
    fn setup_temp_cache_dir() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp dir")
    }

    /// Helper to create a file with content
    fn create_file(path: &PathBuf, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        let mut file = File::create(path).expect("Failed to create file");
        file.write_all(content.as_bytes())
            .expect("Failed to write file");
    }

    #[test]
    fn test_cache_entry_new() {
        let counts = ConfigCounts {
            claude_md_count: 1,
            rules_count: 2,
            mcp_count: 3,
            hooks_count: 4,
        };

        let entry = CacheEntry::new(counts.clone(), Some("/test/path".to_string()));

        assert_eq!(entry.counts.claude_md_count, 1);
        assert_eq!(entry.counts.rules_count, 2);
        assert_eq!(entry.counts.mcp_count, 3);
        assert_eq!(entry.counts.hooks_count, 4);
        assert_eq!(entry.cwd, Some("/test/path".to_string()));
        assert!(entry.timestamp_secs > 0);
    }

    #[test]
    fn test_cache_entry_is_valid_matching_cwd() {
        let counts = ConfigCounts::default();
        let entry = CacheEntry::new(counts, Some("/test/path".to_string()));

        // Same cwd should be valid
        assert!(entry.is_valid(Some("/test/path"), DEFAULT_TTL_SECS));

        // Different cwd should be invalid
        assert!(!entry.is_valid(Some("/other/path"), DEFAULT_TTL_SECS));

        // None vs Some should be invalid
        assert!(!entry.is_valid(None, DEFAULT_TTL_SECS));
    }

    #[test]
    fn test_cache_entry_is_valid_none_cwd() {
        let counts = ConfigCounts::default();
        let entry = CacheEntry::new(counts, None);

        // None cwd should match None
        assert!(entry.is_valid(None, DEFAULT_TTL_SECS));

        // None should not match Some
        assert!(!entry.is_valid(Some("/test/path"), DEFAULT_TTL_SECS));
    }

    #[test]
    fn test_cache_entry_is_valid_expired() {
        let counts = ConfigCounts::default();
        let mut entry = CacheEntry::new(counts, None);

        // Set timestamp to 100 seconds ago
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        entry.timestamp_secs = current_time.saturating_sub(100);

        // Should be invalid with 60 second TTL
        assert!(!entry.is_valid(None, 60));

        // Should be valid with 200 second TTL
        assert!(entry.is_valid(None, 200));
    }

    #[test]
    fn test_cache_entry_serialization() {
        let counts = ConfigCounts {
            claude_md_count: 1,
            rules_count: 2,
            mcp_count: 3,
            hooks_count: 4,
        };
        let entry = CacheEntry::new(counts, Some("/test".to_string()));

        // Serialize
        let json = serde_json::to_string(&entry).expect("Failed to serialize");

        // Deserialize
        let deserialized: CacheEntry =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.counts.claude_md_count, 1);
        assert_eq!(deserialized.counts.rules_count, 2);
        assert_eq!(deserialized.counts.mcp_count, 3);
        assert_eq!(deserialized.counts.hooks_count, 4);
        assert_eq!(deserialized.cwd, Some("/test".to_string()));
        assert_eq!(deserialized.timestamp_secs, entry.timestamp_secs);
    }

    #[test]
    fn test_cache_entry_json_format() {
        let counts = ConfigCounts {
            claude_md_count: 2,
            rules_count: 5,
            mcp_count: 3,
            hooks_count: 1,
        };
        let mut entry = CacheEntry::new(counts, Some("/project".to_string()));
        entry.timestamp_secs = 1700000000; // Fixed timestamp for testing

        let json = serde_json::to_string_pretty(&entry).expect("Failed to serialize");

        // Verify JSON structure
        assert!(json.contains("\"claude_md_count\": 2"));
        assert!(json.contains("\"rules_count\": 5"));
        assert!(json.contains("\"mcp_count\": 3"));
        assert!(json.contains("\"hooks_count\": 1"));
        assert!(json.contains("\"timestamp_secs\": 1700000000"));
        assert!(json.contains("\"cwd\": \"/project\""));
    }

    #[test]
    fn test_get_cache_path() {
        let path = get_cache_path();

        // Should return Some on most systems
        if let Some(p) = path {
            assert!(p.ends_with(".config-cache.json"));
            assert!(p.to_string_lossy().contains(".claude"));
            assert!(p.to_string_lossy().contains("ccline"));
        }
    }

    #[test]
    fn test_invalidate_cache_nonexistent() {
        // Invalidating a non-existent cache should succeed
        // (This test may affect the actual cache file, so we just verify it doesn't panic)
        let result = invalidate_cache();
        // Result should be true (either deleted or didn't exist)
        assert!(result);
    }

    #[test]
    fn test_get_config_counts_cached_returns_valid_counts() {
        // This test verifies the function returns valid counts
        // Note: This may use the actual cache or create a new one
        let counts = get_config_counts_cached(None, Some(1)); // 1 second TTL

        // Verify we get a valid ConfigCounts struct
        // Values depend on actual system configuration
        assert!(counts.claude_md_count <= 100); // Sanity check
        assert!(counts.rules_count <= 1000);
        assert!(counts.mcp_count <= 100);
        assert!(counts.hooks_count <= 100);
    }

    #[test]
    fn test_default_ttl_value() {
        assert_eq!(DEFAULT_TTL_SECS, 60);
    }

    #[test]
    fn test_cache_file_path_function() {
        let path = cache_file_path();

        // Should match get_cache_path
        assert_eq!(path, get_cache_path());
    }

    // Integration test for cache read/write cycle
    // Note: This test uses the actual cache location
    #[test]
    fn test_cache_integration() {
        // First, invalidate any existing cache
        invalidate_cache();

        // Get counts (should create cache)
        let counts1 = get_config_counts_cached(None, Some(60));

        // Get counts again (should use cache)
        let counts2 = get_config_counts_cached(None, Some(60));

        // Both should return the same values
        assert_eq!(counts1.claude_md_count, counts2.claude_md_count);
        assert_eq!(counts1.rules_count, counts2.rules_count);
        assert_eq!(counts1.mcp_count, counts2.mcp_count);
        assert_eq!(counts1.hooks_count, counts2.hooks_count);

        // Clean up
        invalidate_cache();
    }

    #[test]
    fn test_cache_cwd_change_invalidates() {
        // Invalidate any existing cache
        invalidate_cache();

        // Get counts with cwd1
        let _counts1 = get_config_counts_cached(Some("/path1"), Some(60));

        // Read the cache directly
        let cached = read_cache();
        assert!(cached.is_some());

        let entry = cached.unwrap();
        assert_eq!(entry.cwd, Some("/path1".to_string()));

        // The cache should be invalid for a different cwd
        assert!(!entry.is_valid(Some("/path2"), 60));

        // Clean up
        invalidate_cache();
    }
}
