use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Main config structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub style: StyleConfig,
    pub segments: Vec<SegmentConfig>,
    pub theme: String,
}

// Default implementation moved to ui/themes/presets.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    pub mode: StyleMode,
    pub separator: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleMode {
    Plain,
    NerdFont,
    Powerline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentConfig {
    pub id: SegmentId,
    pub enabled: bool,
    pub icon: IconConfig,
    pub colors: ColorConfig,
    pub styles: TextStyleConfig,
    pub options: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconConfig {
    pub plain: String,
    pub nerd_font: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorConfig {
    pub icon: Option<AnsiColor>,
    pub text: Option<AnsiColor>,
    pub background: Option<AnsiColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextStyleConfig {
    pub text_bold: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnsiColor {
    Color16 { c16: u8 },
    Color256 { c256: u8 },
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentId {
    Model,
    Directory,
    Git,
    ContextWindow,
    Usage,
    Credits,
    Cost,
    Session,
    OutputStyle,
    Update,
}

// Legacy compatibility structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SegmentsConfig {
    pub directory: bool,
    pub git: bool,
    pub model: bool,
    // pub usage: bool,
}

// Data structures compatible with existing main.rs
#[derive(Deserialize)]
pub struct Model {
    pub id: String,
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct Workspace {
    pub current_dir: String,
}

#[derive(Deserialize)]
pub struct Cost {
    pub total_cost_usd: Option<f64>,
    pub total_duration_ms: Option<u64>,
    pub total_api_duration_ms: Option<u64>,
    pub total_lines_added: Option<u32>,
    pub total_lines_removed: Option<u32>,
}

#[derive(Deserialize)]
pub struct OutputStyle {
    pub name: String,
}

/// Effort level reported by Claude Code for the current turn
/// (e.g. "low", "medium", "high", "xhigh", "max").
#[derive(Deserialize)]
pub struct Effort {
    pub level: Option<String>,
}

/// Reset timestamp as reported by Claude Code: epoch seconds (integer or
/// fractional) or an RFC 3339 string. Any other shape is treated as absent so
/// this optional field can never fail the whole statusline input.
#[derive(Debug, Clone)]
pub struct ResetsAt(String);

impl ResetsAt {
    fn from_json(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::String(s) => chrono::DateTime::parse_from_rfc3339(s.trim())
                .ok()
                .map(|dt| Self(dt.to_rfc3339())),
            serde_json::Value::Number(n) => {
                let secs = n
                    .as_i64()
                    .or_else(|| n.as_f64().map(|f| f.trunc() as i64))?;
                chrono::DateTime::from_timestamp(secs, 0).map(|dt| Self(dt.to_rfc3339()))
            }
            _ => None,
        }
    }

    /// Normalized RFC 3339 string so downstream code handles one format
    pub fn to_rfc3339(&self) -> String {
        self.0.clone()
    }
}

/// Lenient deserializer: unrecognised values become `None` instead of an error
fn deserialize_resets_at<'de, D>(deserializer: D) -> Result<Option<ResetsAt>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.as_ref().and_then(ResetsAt::from_json))
}

/// Rate-limit window as reported by Claude Code in the statusline input
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitWindow {
    #[serde(default)]
    pub used_percentage: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_resets_at")]
    pub resets_at: Option<ResetsAt>,
}

/// Rate limits reported by Claude Code (five-hour session, seven-day week)
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimits {
    #[serde(default)]
    pub five_hour: Option<RateLimitWindow>,
    #[serde(default)]
    pub seven_day: Option<RateLimitWindow>,
}

/// Context window info reported by Claude Code for the current session.
/// `context_window_size` is the authoritative limit for the active model
/// (e.g. 1,000,000 for models with a native 1M window), so it takes priority
/// over any limit derived from the model ID.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ContextWindow {
    #[serde(default)]
    pub context_window_size: Option<u32>,
    #[serde(default)]
    pub total_input_tokens: Option<u32>,
    #[serde(default)]
    pub total_output_tokens: Option<u32>,
    #[serde(default)]
    pub used_percentage: Option<f64>,
}

#[derive(Deserialize)]
pub struct InputData {
    pub model: Model,
    pub workspace: Workspace,
    pub transcript_path: String,
    pub cost: Option<Cost>,
    pub output_style: Option<OutputStyle>,
    #[serde(default)]
    pub effort: Option<Effort>,
    #[serde(default)]
    pub rate_limits: Option<RateLimits>,
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
}

// OpenAI-style nested token details
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: Option<u32>,
    #[serde(default)]
    pub audio_tokens: Option<u32>,
}

// Raw usage data from different LLM providers (flexible parsing)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RawUsage {
    // Anthropic-style input tokens
    #[serde(default)]
    pub input_tokens: Option<u32>,

    // OpenAI-style input tokens (separate field to handle both formats)
    #[serde(default)]
    pub prompt_tokens: Option<u32>,

    // Anthropic-style output tokens
    #[serde(default)]
    pub output_tokens: Option<u32>,

    // OpenAI-style output tokens (separate field to handle both formats)
    #[serde(default)]
    pub completion_tokens: Option<u32>,

    // Total tokens (some providers only provide this)
    #[serde(default)]
    pub total_tokens: Option<u32>,

    // Anthropic-style cache fields
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,

    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,

    // OpenAI-style cache fields (separate fields to handle both formats)
    #[serde(default)]
    pub cache_creation_prompt_tokens: Option<u32>,

    #[serde(default)]
    pub cache_read_prompt_tokens: Option<u32>,

    #[serde(default)]
    pub cached_tokens: Option<u32>,

    // OpenAI-style nested details
    #[serde(default)]
    pub prompt_tokens_details: Option<PromptTokensDetails>,

    // Completion token details (OpenAI)
    #[serde(default)]
    pub completion_tokens_details: Option<HashMap<String, u32>>,

    // Catch unknown fields for future compatibility and debugging
    #[serde(flatten, skip_serializing)]
    pub extra: HashMap<String, serde_json::Value>,
}

// Normalized internal representation after processing
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct NormalizedUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,

    // Metadata for debugging and analysis
    pub calculation_source: String,
    pub raw_data_available: Vec<String>,
}

impl NormalizedUsage {
    /// Get tokens that count toward context window
    /// This includes all tokens that consume context window space
    /// Output tokens from this turn will become input tokens in the next turn
    pub fn context_tokens(&self) -> u32 {
        self.input_tokens
            + self.cache_creation_input_tokens
            + self.cache_read_input_tokens
            + self.output_tokens
    }

    /// Get total tokens for cost calculation
    /// Priority: use total_tokens if available, otherwise sum all components
    pub fn total_for_cost(&self) -> u32 {
        if self.total_tokens > 0 {
            self.total_tokens
        } else {
            self.input_tokens
                + self.output_tokens
                + self.cache_creation_input_tokens
                + self.cache_read_input_tokens
        }
    }

    /// Get the most appropriate token count for general display
    /// For OpenAI format: use total_tokens directly
    /// For Anthropic format: use context_tokens (input + cache)
    pub fn display_tokens(&self) -> u32 {
        // For Claude/Anthropic format: prefer input-related tokens for context window display
        let context = self.context_tokens();
        if context > 0 {
            return context;
        }

        // For OpenAI format: use total_tokens when no input breakdown available
        if self.total_tokens > 0 {
            return self.total_tokens;
        }

        // Fallback to any available tokens
        self.input_tokens.max(self.output_tokens)
    }
}

impl Config {
    /// Check if current config matches the specified theme preset
    pub fn matches_theme(&self, theme_name: &str) -> bool {
        let theme_preset = crate::ui::themes::ThemePresets::get_theme(theme_name);

        // Compare style config
        if self.style.mode != theme_preset.style.mode
            || self.style.separator != theme_preset.style.separator
        {
            return false;
        }

        // Compare segments count and order
        if self.segments.len() != theme_preset.segments.len() {
            return false;
        }

        // Compare each segment config
        for (current, preset) in self.segments.iter().zip(theme_preset.segments.iter()) {
            if !self.segment_matches(current, preset) {
                return false;
            }
        }

        true
    }

    /// Check if current config has been modified from the selected theme
    pub fn is_modified_from_theme(&self) -> bool {
        !self.matches_theme(&self.theme)
    }

    /// Compare two segment configs for equality
    fn segment_matches(&self, current: &SegmentConfig, preset: &SegmentConfig) -> bool {
        current.id == preset.id
            && current.enabled == preset.enabled
            && current.icon.plain == preset.icon.plain
            && current.icon.nerd_font == preset.icon.nerd_font
            && self.color_matches(&current.colors.icon, &preset.colors.icon)
            && self.color_matches(&current.colors.text, &preset.colors.text)
            && self.color_matches(&current.colors.background, &preset.colors.background)
            && current.styles.text_bold == preset.styles.text_bold
            && current.options == preset.options
    }

    /// Compare two optional colors for equality
    fn color_matches(&self, current: &Option<AnsiColor>, preset: &Option<AnsiColor>) -> bool {
        match (current, preset) {
            (None, None) => true,
            (Some(c1), Some(c2)) => c1 == c2,
            _ => false,
        }
    }
}

impl PartialEq for AnsiColor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnsiColor::Color16 { c16: a }, AnsiColor::Color16 { c16: b }) => a == b,
            (AnsiColor::Color256 { c256: a }, AnsiColor::Color256 { c256: b }) => a == b,
            (
                AnsiColor::Rgb {
                    r: r1,
                    g: g1,
                    b: b1,
                },
                AnsiColor::Rgb {
                    r: r2,
                    g: g2,
                    b: b2,
                },
            ) => r1 == r2 && g1 == g2 && b1 == b2,
            _ => false,
        }
    }
}

impl RawUsage {
    /// Convert raw usage data to normalized format with intelligent token inference
    pub fn normalize(self) -> NormalizedUsage {
        let mut result = NormalizedUsage::default();
        let mut sources = Vec::new();

        // Collect available raw data fields and merge tokens with Anthropic priority
        let mut available_fields = Vec::new();

        // Merge input tokens (priority: input_tokens > prompt_tokens)
        let input = self.input_tokens.or(self.prompt_tokens).unwrap_or(0);
        if input > 0 {
            available_fields.push("input_tokens".to_string());
        }

        // Merge output tokens (priority: output_tokens > completion_tokens)
        let output = self.output_tokens.or(self.completion_tokens).unwrap_or(0);
        if output > 0 {
            available_fields.push("output_tokens".to_string());
        }

        let total = self.total_tokens.unwrap_or(0);
        if total > 0 {
            available_fields.push("total_tokens".to_string());
        }

        // Merge cache creation tokens (priority: Anthropic > OpenAI)
        let cache_creation = self
            .cache_creation_input_tokens
            .or(self.cache_creation_prompt_tokens)
            .unwrap_or(0);
        if cache_creation > 0 {
            available_fields.push("cache_creation".to_string());
        }

        // Merge cache read tokens (priority: Anthropic > OpenAI > nested format)
        let cache_read = self
            .cache_read_input_tokens
            .or(self.cache_read_prompt_tokens)
            .or(self.cached_tokens)
            .or_else(|| {
                // Fallback to OpenAI nested format
                self.prompt_tokens_details
                    .as_ref()
                    .and_then(|d| d.cached_tokens)
            })
            .unwrap_or(0);
        if cache_read > 0 {
            available_fields.push("cache_read".to_string());
        }

        result.raw_data_available = available_fields;

        // Use merged cache values (already calculated above with Anthropic priority)

        // Token calculation logic - prioritize total_tokens for OpenAI format
        let total_value = if total > 0 {
            sources.push("total_tokens_direct".to_string());
            total
        } else if input > 0 || output > 0 || cache_read > 0 || cache_creation > 0 {
            let calculated = input + output + cache_read + cache_creation;
            sources.push("total_from_components".to_string());
            calculated
        } else {
            0
        };

        // Assignment
        result.input_tokens = input;
        result.output_tokens = output;
        result.total_tokens = total_value;
        result.cache_creation_input_tokens = cache_creation;
        result.cache_read_input_tokens = cache_read;
        result.calculation_source = sources.join("+");

        result
    }
}

// Legacy alias for backward compatibility
pub type Usage = RawUsage;

#[derive(Deserialize)]
pub struct Message {
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct TranscriptEntry {
    pub r#type: Option<String>,
    pub message: Option<Message>,
    #[serde(rename = "leafUuid")]
    pub leaf_uuid: Option<String>,
    pub uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    pub summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(json: &str) -> RateLimitWindow {
        serde_json::from_str(json).expect("window should deserialize")
    }

    #[test]
    fn resets_at_accepts_integer_and_fractional_epoch() {
        let w = window(r#"{"resets_at": 1758750000}"#);
        assert_eq!(
            w.resets_at.map(|r| r.to_rfc3339()).as_deref(),
            Some("2025-09-24T21:40:00+00:00")
        );
        let w = window(r#"{"resets_at": 1758750000.5}"#);
        assert_eq!(
            w.resets_at.map(|r| r.to_rfc3339()).as_deref(),
            Some("2025-09-24T21:40:00+00:00")
        );
    }

    #[test]
    fn resets_at_normalizes_rfc3339_and_rejects_other_strings() {
        let w = window(r#"{"resets_at": "2025-09-24T23:40:00+02:00"}"#);
        assert_eq!(
            w.resets_at.map(|r| r.to_rfc3339()).as_deref(),
            Some("2025-09-24T23:40:00+02:00")
        );
        let w = window(r#"{"resets_at": "tomorrow"}"#);
        assert!(w.resets_at.is_none());
        let w = window(r#"{"resets_at": ""}"#);
        assert!(w.resets_at.is_none());
        let w = window(r#"{"resets_at": true}"#);
        assert!(w.resets_at.is_none());
    }

    #[test]
    fn rate_limits_accept_a_single_window() {
        let limits: RateLimits =
            serde_json::from_str(r#"{"seven_day": {"used_percentage": 17}}"#).unwrap();
        assert!(limits.five_hour.is_none());
        assert_eq!(limits.seven_day.and_then(|w| w.used_percentage), Some(17.0));
        let limits: RateLimits = serde_json::from_str("{}").unwrap();
        assert!(limits.five_hour.is_none() && limits.seven_day.is_none());
    }
}
