use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Update status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    /// New version available
    Ready {
        version: String,
        found_at: DateTime<Utc>,
    },
    Failed {
        error: String,
    },
}

/// Update state persistence structure
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateState {
    pub status: UpdateStatus,
    pub last_check: Option<DateTime<Utc>>,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_pid: Option<u32>,
}

impl UpdateState {
    /// Get status bar display text
    pub fn status_text(&self) -> Option<String> {
        match &self.status {
            UpdateStatus::Ready { version, .. } => Some(format!("\u{f06b0} v{}", version)),
            _ => None,
        }
    }

    /// Load update state from config directory and trigger auto-check if needed
    pub fn load() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccline");

        let state_file = config_dir.join(".update_state.json");

        let mut state = if let Ok(content) = std::fs::read_to_string(&state_file) {
            serde_json::from_str::<UpdateState>(&content).unwrap_or_else(|_| UpdateState {
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            })
        } else {
            UpdateState {
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            }
        };

        if state.should_check_update() {
            let should_start_check = if let Some(pid) = state.update_pid {
                !Self::is_process_running(pid)
            } else {
                true
            };

            if should_start_check {
                state.update_pid = Some(std::process::id());
                state.last_check = Some(Utc::now());
                let _ = state.save();

                match registry::check_for_updates() {
                    Ok(Some(version)) => {
                        state.status = UpdateStatus::Ready {
                            version: version.clone(),
                            found_at: Utc::now(),
                        };
                        state.latest_version = Some(version);
                    }
                    Ok(None) => {
                        state.status = UpdateStatus::Idle;
                    }
                    Err(_) => {
                        state.status = UpdateStatus::Idle;
                    }
                }

                state.update_pid = None;
                let _ = state.save();
            }
        }

        state
    }

    fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("ps").arg("-p").arg(pid.to_string()).output() {
                output.status.success()
            } else {
                false
            }
        }

        #[cfg(windows)]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("tasklist")
                .arg("/FI")
                .arg(&format!("PID eq {}", pid))
                .output()
            {
                String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
            } else {
                false
            }
        }

        #[cfg(not(any(unix, windows)))]
        false
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let config_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccline");

        std::fs::create_dir_all(&config_dir)?;
        let state_file = config_dir.join(".update_state.json");

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&state_file, content)?;

        Ok(())
    }

    /// Check interval: 1 hour
    fn should_check_update(&self) -> bool {
        if matches!(self.status, UpdateStatus::Checking) {
            return false;
        }

        if let Some(last_check) = self.last_check {
            Utc::now().signed_duration_since(last_check).num_hours() >= 1
        } else {
            true
        }
    }
}

/// npm registry version check
mod registry {
    /// Check @cometix/ccline latest version from npm registry
    pub fn check_for_updates() -> Result<Option<String>, Box<dyn std::error::Error>> {
        let url = "https://registry.npmjs.org/@cometix/ccline/latest";

        let response = ureq::get(url).header("Accept", "application/json").call()?;

        let data: serde_json::Value = response.into_body().read_json()?;
        let latest = data["version"].as_str().ok_or("Missing version field")?;

        let current = env!("CARGO_PKG_VERSION");
        let current_ver = semver::Version::parse(current)?;
        let latest_ver = semver::Version::parse(latest)?;

        if latest_ver > current_ver {
            Ok(Some(latest.to_string()))
        } else {
            Ok(None)
        }
    }
}
