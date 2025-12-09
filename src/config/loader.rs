use super::types::Config;
use std::fs;
use std::path::{Path, PathBuf};

/// Result of config initialization
#[derive(Debug)]
pub enum InitResult {
    /// Config was created at the given path
    Created(PathBuf),
    /// Config already existed at the given path
    AlreadyExists(PathBuf),
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load() -> Config {
        Config::load().unwrap_or_else(|_| Config::default())
    }

    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Initialize themes directory and create built-in theme files
    pub fn init_themes() -> Result<(), Box<dyn std::error::Error>> {
        let themes_dir = Self::get_themes_path();

        // Create themes directory
        fs::create_dir_all(&themes_dir)?;

        let builtin_themes = [
            "cometix",
            "default",
            "minimal",
            "gruvbox",
            "nord",
            "powerline-dark",
            "powerline-light",
            "powerline-rose-pine",
            "powerline-tokyo-night",
        ];
        let mut created_any = false;

        for theme_name in &builtin_themes {
            let theme_path = themes_dir.join(format!("{}.toml", theme_name));

            if !theme_path.exists() {
                let theme_config = crate::ui::themes::ThemePresets::get_theme(theme_name);
                let content = toml::to_string_pretty(&theme_config)?;
                fs::write(&theme_path, content)?;
                println!("Created theme file: {}", theme_path.display());
                created_any = true;
            }
        }

        if !created_any {
            // println!("All built-in theme files already exist");
        }

        Ok(())
    }

    /// Get the themes directory path (~/.claude/ccline/themes/)
    pub fn get_themes_path() -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            home.join(".claude").join("ccline").join("themes")
        } else {
            PathBuf::from(".claude/ccline/themes")
        }
    }

    /// Ensure themes directory exists and has built-in themes (silent mode)
    pub fn ensure_themes_exist() {
        // Silently ensure themes exist without printing output
        let _ = Self::init_themes_silent();
    }

    /// Initialize themes directory and create built-in theme files (silent mode)
    fn init_themes_silent() -> Result<(), Box<dyn std::error::Error>> {
        let themes_dir = Self::get_themes_path();

        // Create themes directory
        fs::create_dir_all(&themes_dir)?;

        let builtin_themes = [
            "default",
            "minimal",
            "gruvbox",
            "nord",
            "cometix",
            "powerline-dark",
            "powerline-light",
            "powerline-rose-pine",
            "powerline-tokyo-night",
        ];

        for theme_name in &builtin_themes {
            let theme_path = themes_dir.join(format!("{}.toml", theme_name));

            if !theme_path.exists() {
                let theme_config = crate::ui::themes::ThemePresets::get_theme(theme_name);
                let content = toml::to_string_pretty(&theme_config)?;
                fs::write(&theme_path, content)?;
            }
        }

        Ok(())
    }
}

impl Config {
    /// Load configuration from default location
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        // Ensure themes directory exists and has built-in themes
        ConfigLoader::ensure_themes_exist();

        let config_path = Self::get_config_path();

        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to default location
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// Get the default config file path (~/.claude/ccline/config.toml)
    fn get_config_path() -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            home.join(".claude").join("ccline").join("config.toml")
        } else {
            PathBuf::from(".claude/ccline/config.toml")
        }
    }

    /// Initialize config directory and create default config
    pub fn init() -> Result<InitResult, Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        // Create directory
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Initialize themes directory and built-in themes
        ConfigLoader::init_themes()?;

        // Create default config if it doesn't exist
        if !config_path.exists() {
            let default_config = Config::default();
            default_config.save()?;
            Ok(InitResult::Created(config_path))
        } else {
            Ok(InitResult::AlreadyExists(config_path))
        }
    }

    /// Validate configuration
    pub fn check(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Basic validation
        if self.segments.is_empty() {
            return Err("No segments configured".into());
        }

        // Validate segment IDs are unique
        let mut seen_ids = std::collections::HashSet::new();
        for segment in &self.segments {
            if !seen_ids.insert(segment.id) {
                return Err(format!("Duplicate segment ID: {:?}", segment.id).into());
            }
        }

        Ok(())
    }

    /// Print configuration as TOML
    pub fn print(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        println!("{}", content);
        Ok(())
    }
}
