use super::types::{Config, SegmentsConfig};

pub const DEFAULT_CONFIG: Config = Config {
    theme: String::new(), // Set to "dark" at runtime
    segments: SegmentsConfig {
        directory: true,
        git: true,
        model: true,
        usage: true,
        cost: true,
        burn_rate: true,
    },
};

impl Default for Config {
    fn default() -> Self {
        let cost_features_enabled = std::env::var("CCLINE_DISABLE_COST").is_err();
        Config {
            theme: "dark".to_string(),
            segments: SegmentsConfig {
                directory: true,
                git: true,
                model: true,
                usage: true,
                cost: cost_features_enabled,
                burn_rate: cost_features_enabled,
            },
        }
    }
}
