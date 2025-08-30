use serde_json::{json, Value};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub struct ClaudeConfig;

impl ClaudeConfig {
    /// Get Claude Code settings.json path
    pub fn get_settings_path() -> Option<PathBuf> {
        let home = dirs::home_dir()?;

        #[cfg(target_os = "windows")]
        let settings_path = home
            .join("AppData")
            .join("Roaming")
            .join("Claude")
            .join("settings.json");

        #[cfg(not(target_os = "windows"))]
        let settings_path = home.join(".config").join("claude").join("settings.json");

        Some(settings_path)
    }

    /// Check if Claude Code statusLine is already configured
    pub fn has_statusline_config() -> bool {
        if let Some(settings_path) = Self::get_settings_path() {
            if settings_path.exists() {
                if let Ok(content) = fs::read_to_string(&settings_path) {
                    if let Ok(settings) = serde_json::from_str::<Value>(&content) {
                        return settings.get("statusLine").is_some();
                    }
                }
            }
        }
        false
    }

    /// Get the appropriate ccline command path
    fn get_ccline_command() -> String {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        #[cfg(target_os = "windows")]
        let command = format!("%USERPROFILE%\\.claude\\ccline\\ccline.exe");

        #[cfg(not(target_os = "windows"))]
        let command = format!("{}/.claude/ccline/ccline", home.display());

        command
    }

    /// Prompt user for overwrite confirmation
    pub fn prompt_overwrite() -> io::Result<bool> {
        print!("Claude Code statusLine is already configured. Overwrite? (y/N): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_lowercase() == "y" || input.trim().to_lowercase() == "yes")
    }

    /// Configure Claude Code statusLine
    pub fn configure_statusline(force: bool) -> Result<(), Box<dyn std::error::Error>> {
        // Check if already configured and prompt if needed
        if !force && Self::has_statusline_config()
            && !Self::prompt_overwrite()? {
                println!("Configuration cancelled.");
                return Ok(());
            }

        let settings_path =
            Self::get_settings_path().ok_or("Could not determine Claude Code settings path")?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read existing settings or create empty object
        let mut settings = if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)?;
            serde_json::from_str::<Value>(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        // Add or update statusLine configuration
        let statusline_config = json!({
            "type": "command",
            "command": Self::get_ccline_command(),
            "padding": 0
        });

        settings["statusLine"] = statusline_config;

        // Write back to file
        let formatted = serde_json::to_string_pretty(&settings)?;
        fs::write(&settings_path, formatted)?;

        println!("✅ Claude Code statusLine configured successfully!");
        println!("Settings saved to: {}", settings_path.display());

        Ok(())
    }
}
