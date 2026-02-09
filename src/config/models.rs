use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    #[serde(rename = "models")]
    pub model_entries: Vec<ModelEntry>,
    #[serde(default)]
    pub context_modifiers: Vec<ContextModifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub pattern: String,
    pub display_name: String,
    pub context_limit: u32,
}

/// Context modifier that overrides context limits and appends a suffix to display names.
/// For example, `[1m]` in a model ID indicates 1M context window, regardless of the base model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextModifier {
    pub pattern: String,
    pub display_suffix: String,
    pub context_limit: u32,
}

/// Built-in Claude model family definition (internal, not serialized).
/// Uses regex with named capture groups to auto-extract version numbers from model IDs.
///
/// Handles both naming conventions:
///   - `claude-{variant}-{major}[-{minor}]-{date}` (e.g., `claude-opus-4-6-20250901`)
///   - `claude-{major}[-{minor}]-{variant}-{date}` (e.g., `claude-4-opus-20250514`)
struct BuiltinModelFamily {
    regex: Regex,
    display_prefix: String,
    context_limit: u32,
}

impl BuiltinModelFamily {
    /// Create a new model family with auto-generated regex pattern.
    ///
    /// The regex matches version numbers (1-2 digits) adjacent to the keyword,
    /// followed by a boundary that signals "version numbers have ended":
    ///   - `-\d{3,}` : date suffix (e.g., `-20250514`)
    ///   - `-[a-z]`  : text qualifier (e.g., `-thinking`, `-preview`, `-latest`)
    ///   - `\[`      : context modifier (e.g., `[1m]`)
    ///   - `$`       : end of string
    ///
    /// The boundary is consumed but only named capture groups are used for version extraction.
    /// Rust's `regex` crate does not support lookahead, so the NFA engine's natural
    /// backtracking prevents date digits from being captured as minor version numbers.
    fn new(keyword: &str, display_prefix: &str, context_limit: u32) -> Self {
        let pattern = format!(
            r"(?:(?P<pre_major>\d{{1,2}})(?:-(?P<pre_minor>\d{{1,2}}))?-{kw}|{kw}-(?P<post_major>\d{{1,2}})(?:-(?P<post_minor>\d{{1,2}}))?)(?:-\d{{3,}}|-[a-z]|\[|$)",
            kw = keyword
        );
        Self {
            regex: Regex::new(&pattern).expect("built-in family regex should compile"),
            display_prefix: display_prefix.to_string(),
            context_limit,
        }
    }

    /// Try to match a model ID (already lowercased) and extract a formatted display name.
    /// Returns `None` if the model ID doesn't match this family.
    fn match_model(&self, model_id_lower: &str) -> Option<String> {
        let caps = self.regex.captures(model_id_lower)?;

        let major = caps
            .name("post_major")
            .or_else(|| caps.name("pre_major"))
            .map(|m| m.as_str())?;

        let minor = caps
            .name("post_minor")
            .or_else(|| caps.name("pre_minor"))
            .map(|m| m.as_str());

        let version = match minor {
            Some(m) => format!("{}.{}", major, m),
            None => major.to_string(),
        };

        Some(format!("{} {}", self.display_prefix, version))
    }
}

/// Lazily-initialized built-in Claude model families.
/// Regex compilation happens only once per process, regardless of how many times
/// `get_display_name()` or `get_context_limit()` are called.
static BUILTIN_FAMILIES: OnceLock<Vec<BuiltinModelFamily>> = OnceLock::new();

impl ModelConfig {
    /// Get built-in Claude model families (compiled once, cached via OnceLock).
    fn builtin_families() -> &'static [BuiltinModelFamily] {
        BUILTIN_FAMILIES.get_or_init(|| {
            vec![
                BuiltinModelFamily::new("sonnet", "Sonnet", 200_000),
                BuiltinModelFamily::new("opus", "Opus", 200_000),
                BuiltinModelFamily::new("haiku", "Haiku", 200_000),
            ]
        })
    }

    /// Try to match a model ID against built-in Claude model families.
    /// Returns `(display_name, context_limit)` if matched.
    fn match_builtin_family(model_id: &str) -> Option<(String, u32)> {
        let model_lower = model_id.to_lowercase();
        for family in Self::builtin_families() {
            if let Some(name) = family.match_model(&model_lower) {
                return Some((name, family.context_limit));
            }
        }
        None
    }

    /// Load model configuration from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: ModelConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load model configuration with fallback locations
    pub fn load() -> Self {
        let mut model_config = Self::default();

        // First, try to create default models.toml if it doesn't exist
        if let Some(home_dir) = dirs::home_dir() {
            let user_models_path = home_dir.join(".claude").join("ccline").join("models.toml");
            if !user_models_path.exists() {
                let _ = Self::create_default_file(&user_models_path);
            }
        }

        // Try loading from user config directory first, then local
        let config_paths = [
            dirs::home_dir().map(|d| d.join(".claude").join("ccline").join("models.toml")),
            Some(Path::new("models.toml").to_path_buf()),
        ];

        for path in config_paths.iter().flatten() {
            if path.exists() {
                if let Ok(config) = Self::load_from_file(path) {
                    // Prepend external models to built-in ones for priority
                    let mut merged_entries = config.model_entries;
                    merged_entries.extend(model_config.model_entries);
                    model_config.model_entries = merged_entries;

                    // Prepend external modifiers to built-in ones for priority
                    let mut merged_modifiers = config.context_modifiers;
                    merged_modifiers.extend(model_config.context_modifiers);
                    model_config.context_modifiers = merged_modifiers;

                    return model_config;
                }
            }
        }

        // Fallback to default configuration if no file found
        model_config
    }

    /// Resolve a model ID in a single pass through all matching layers.
    /// Returns `(display_name, context_limit, modifier_suffix)`.
    ///
    /// Matching priority for display_name:
    ///   1. User/built-in model entries (simple substring match)
    ///   2. Built-in Claude model families (regex with version extraction)
    ///   3. None (caller should use upstream fallback)
    ///
    /// Matching priority for context_limit:
    ///   1. Context modifiers (e.g., `[1m]` → 1M) — highest priority
    ///   2. Model entries / built-in families (from whichever matched display_name)
    ///   3. Default (200k)
    fn resolve(&self, model_id: &str) -> (Option<String>, u32, Option<String>) {
        let model_lower = model_id.to_lowercase();

        // Phase 1: Find base display name and its context_limit
        let (base_name, base_limit) = self
            .model_entries
            .iter()
            .find(|e| model_lower.contains(&e.pattern.to_lowercase()))
            .map(|e| (Some(e.display_name.clone()), Some(e.context_limit)))
            .unwrap_or_else(|| {
                Self::match_builtin_family(model_id)
                    .map(|(name, limit)| (Some(name), Some(limit)))
                    .unwrap_or((None, None))
            });

        // Phase 2: Find matching context modifier (independent of model identity)
        let modifier = self
            .context_modifiers
            .iter()
            .find(|m| model_lower.contains(&m.pattern.to_lowercase()));

        // Compose display name with modifier suffix
        let display_name = match (&base_name, modifier) {
            (Some(name), Some(m)) => Some(format!("{}{}", name, m.display_suffix)),
            (Some(name), None) => Some(name.clone()),
            (None, _) => None,
        };

        // Context limit: modifier overrides base
        let context_limit = modifier
            .map(|m| m.context_limit)
            .or(base_limit)
            .unwrap_or(200_000);

        let modifier_suffix = modifier.map(|m| m.display_suffix.clone());

        (display_name, context_limit, modifier_suffix)
    }

    /// Get context limit for a model based on ID pattern matching.
    /// Priority: context modifiers > model entries > built-in families > default (200k).
    pub fn get_context_limit(&self, model_id: &str) -> u32 {
        let (_, limit, _) = self.resolve(model_id);
        limit
    }

    /// Try to get context limit for a model, returns None if no match found.
    /// Returns `Some(limit)` if any layer matched (modifier, entry, or builtin family).
    pub fn try_get_context_limit(&self, model_id: &str) -> Option<u32> {
        let (display_name, limit, modifier_suffix) = self.resolve(model_id);
        if display_name.is_some() || modifier_suffix.is_some() {
            Some(limit)
        } else {
            None
        }
    }

    /// Get display name for a model using layered matching.
    /// Composes base name with any matching context modifier suffix.
    /// Returns None if nothing matches (caller should use upstream fallback display_name).
    pub fn get_display_name(&self, model_id: &str) -> Option<String> {
        let (display_name, _, _) = self.resolve(model_id);
        display_name
    }

    /// Get the display suffix from any matching context modifier.
    /// Used to append modifier info (e.g., " 1M") to upstream fallback display names
    /// when the model itself is not recognized by our config.
    pub fn get_display_suffix(&self, model_id: &str) -> Option<String> {
        let (_, _, suffix) = self.resolve(model_id);
        suffix
    }

    /// Create default model configuration file with minimal template
    pub fn create_default_file<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn std::error::Error>> {
        // Create a minimal template config (not the full fallback config)
        let template_config = Self {
            model_entries: vec![],
            context_modifiers: vec![],
        };

        let toml_content = toml::to_string_pretty(&template_config)?;

        // Add comments and examples to the template
        let template_content = format!(
            "# CCometixLine Model Configuration\n\
             # This file defines model display names and context limits for different LLM models\n\
             # File location: ~/.claude/ccline/models.toml\n\
             #\n\
             # Claude models are automatically recognized (Sonnet, Opus, Haiku) with\n\
             # version extraction. You only need to add entries here for overrides or\n\
             # third-party models.\n\
             \n\
             {}\n\
             \n\
             # Model configurations (simple substring matching)\n\
             # Each [[models]] section defines a model pattern and its properties\n\
             # These take priority over built-in Claude model recognition\n\
             \n\
             # Example:\n\
             # [[models]]\n\
             # pattern = \"my-model\"\n\
             # display_name = \"My Model\"\n\
             # context_limit = 128000\n\
             \n\
             # Context modifiers override context limits and append suffix to display names\n\
             # They are matched independently, enabling composition:\n\
             #   model \"Opus 4\" + modifier \" 1M\" = \"Opus 4 1M\"\n\
             \n\
             # Example:\n\
             # [[context_modifiers]]\n\
             # pattern = \"[1m]\"\n\
             # display_suffix = \" 1M\"\n\
             # context_limit = 1000000\n",
            toml_content.trim()
        );

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, template_content)?;
        Ok(())
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            // Only third-party models need explicit entries.
            // Claude models (Sonnet, Opus, Haiku) are handled by built-in regex families.
            model_entries: vec![
                ModelEntry {
                    pattern: "glm-4.5".to_string(),
                    display_name: "GLM-4.5".to_string(),
                    context_limit: 128_000,
                },
                ModelEntry {
                    pattern: "kimi-k2-turbo".to_string(),
                    display_name: "Kimi K2 Turbo".to_string(),
                    context_limit: 128_000,
                },
                ModelEntry {
                    pattern: "kimi-k2".to_string(),
                    display_name: "Kimi K2".to_string(),
                    context_limit: 128_000,
                },
                ModelEntry {
                    pattern: "qwen3-coder".to_string(),
                    display_name: "Qwen Coder".to_string(),
                    context_limit: 256_000,
                },
            ],
            context_modifiers: vec![ContextModifier {
                pattern: "[1m]".to_string(),
                display_suffix: " 1M".to_string(),
                context_limit: 1_000_000,
            }],
        }
    }
}
