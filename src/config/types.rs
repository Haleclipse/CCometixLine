use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub theme: String,
    pub segments: SegmentsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SegmentsConfig {
    pub directory: bool,
    pub git: bool,
    pub model: bool,
    pub usage: bool,
    #[serde(default = "default_true")]
    pub cost: bool,
    #[serde(default = "default_true")]
    pub burn_rate: bool,
}

fn default_true() -> bool {
    true
}

// Data structures compatible with existing main.rs
#[derive(Deserialize)]
pub struct Model {
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct Workspace {
    pub current_dir: String,
}

#[derive(Deserialize)]
pub struct InputData {
    pub model: Model,
    pub workspace: Workspace,
    pub transcript_path: String,
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
    // Common input token naming variants
    #[serde(default, alias = "prompt_tokens")]
    pub input_tokens: Option<u32>,

    // Common output token naming variants
    #[serde(default, alias = "completion_tokens")]
    pub output_tokens: Option<u32>,

    // Total tokens (some providers only provide this)
    #[serde(default)]
    pub total_tokens: Option<u32>,

    // Anthropic-style cache fields
    #[serde(default, alias = "cache_creation_prompt_tokens")]
    pub cache_creation_input_tokens: Option<u32>,

    #[serde(default, alias = "cache_read_prompt_tokens")]
    pub cache_read_input_tokens: Option<u32>,

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

impl RawUsage {
    /// Convert raw usage data to normalized format with intelligent token inference
    pub fn normalize(self) -> NormalizedUsage {
        let mut result = NormalizedUsage::default();
        let mut sources = Vec::new();

        // Collect available raw data fields
        let mut available_fields = Vec::new();
        if self.input_tokens.is_some() {
            available_fields.push("input_tokens".to_string());
        }
        if self.output_tokens.is_some() {
            available_fields.push("output_tokens".to_string());
        }
        if self.total_tokens.is_some() {
            available_fields.push("total_tokens".to_string());
        }
        if self.cache_creation_input_tokens.is_some() {
            available_fields.push("cache_creation".to_string());
        }
        if self.cache_read_input_tokens.is_some() {
            available_fields.push("cache_read".to_string());
        }

        result.raw_data_available = available_fields;

        // Extract directly available values
        let input = self.input_tokens.unwrap_or(0);
        let output = self.output_tokens.unwrap_or(0);
        let total = self.total_tokens.unwrap_or(0);

        // Handle cache tokens with fallback to OpenAI nested format
        let cache_read = self
            .cache_read_input_tokens
            .or_else(|| {
                self.prompt_tokens_details
                    .as_ref()
                    .and_then(|d| d.cached_tokens)
            })
            .unwrap_or(0);

        let cache_creation = self.cache_creation_input_tokens.unwrap_or(0);

        // Token calculation logic - prioritize total_tokens for OpenAI format
        let final_total = if total > 0 {
            sources.push("total_tokens_direct".to_string());
            total
        } else if input > 0 || output > 0 || cache_read > 0 || cache_creation > 0 {
            let calculated = input + output + cache_read + cache_creation;
            sources.push("total_from_components".to_string());
            calculated
        } else {
            0
        };

        // Final assignment
        result.input_tokens = input;
        result.output_tokens = output;
        result.total_tokens = final_total;
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
    #[serde(default)]
    pub id: Option<String>,
    pub usage: Option<Usage>,
    pub model: Option<String>,
}

#[derive(Deserialize)]
pub struct TranscriptEntry {
    pub r#type: Option<String>,
    pub message: Option<Message>,
    #[serde(default, alias = "requestId")]
    pub request_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}
