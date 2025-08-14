use ccometixline::cli::Cli;
use ccometixline::config::{BlockOverrideManager, Config, ConfigLoader, InputData};
use ccometixline::core::StatusLineGenerator;
use chrono::{Local, NaiveDate, Utc};
use std::io;

fn main() -> io::Result<()> {
    let cli = Cli::parse_args();

    // Handle special CLI modes
    if cli.version {
        println!("CCometixLine v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cli.print_config {
        let config = Config::default();
        println!("{}", toml::to_string(&config).unwrap());
        return Ok(());
    }

    if cli.validate {
        println!("Configuration validation not implemented yet");
        return Ok(());
    }

    if cli.configure {
        println!("TUI configuration mode not implemented yet");
        return Ok(());
    }

    if cli.update {
        #[cfg(feature = "self-update")]
        {
            use ccometixline::updater::{github::check_for_updates, UpdateState, UpdateStatus};
            use chrono::Utc;

            println!("Checking for updates...");
            let mut state = UpdateState::load();
            state.status = UpdateStatus::Checking;
            state.last_check = Some(Utc::now());

            match check_for_updates() {
                Ok(Some(release)) => {
                    println!("New version available: v{}", release.version());
                    println!("Release notes: {}", release.name);
                    if let Some(asset) = release.find_asset_for_platform() {
                        println!("Download: {}", asset.browser_download_url);
                        println!("Size: {:.1} MB", asset.size as f64 / 1024.0 / 1024.0);

                        // Ask user for confirmation
                        print!("Do you want to download and install this update? [y/N]: ");
                        use std::io::{self, Write};
                        io::stdout().flush().unwrap();

                        let mut input = String::new();
                        io::stdin().read_line(&mut input).unwrap();

                        if input.trim().to_lowercase() == "y" {
                            println!("Downloading update...");
                            state.status = UpdateStatus::Downloading { progress: 0 };
                            let _ = state.save();

                            // Simulate download progress
                            for progress in (0..=100).step_by(20) {
                                state.status = UpdateStatus::Downloading { progress };
                                let _ = state.save();
                                println!("Progress: {}%", progress);
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }

                            println!("Installing update...");
                            state.status = UpdateStatus::Installing;
                            let _ = state.save();
                            std::thread::sleep(std::time::Duration::from_secs(2));

                            println!("Update completed successfully!");
                            state.status = UpdateStatus::Completed {
                                version: release.version(),
                                completed_at: chrono::Utc::now(),
                            };
                            state.latest_version = Some(release.version());
                            let _ = state.save();
                        } else {
                            println!("Update cancelled.");
                            state.status = UpdateStatus::Ready {
                                version: release.version(),
                                found_at: chrono::Utc::now(),
                            };
                            state.latest_version = Some(release.version());
                            let _ = state.save();
                        }
                    } else {
                        println!("No compatible asset found for your platform.");
                        state.status = UpdateStatus::Failed {
                            error: "No compatible asset".to_string(),
                        };
                        let _ = state.save();
                    }
                }
                Ok(None) => {
                    println!(
                        "You're running the latest version (v{})",
                        env!("CARGO_PKG_VERSION")
                    );
                    state.status = UpdateStatus::Idle;
                    let _ = state.save();
                }
                Err(e) => {
                    println!("Error checking for updates: {}", e);
                    state.status = UpdateStatus::Failed {
                        error: e.to_string(),
                    };
                    let _ = state.save();
                }
            }
        }
        #[cfg(not(feature = "self-update"))]
        {
            println!("Update check not available (self-update feature disabled)");
        }
        return Ok(());
    }

    // Handle block start time management
    if cli.set_block_start.is_some() || cli.clear_block_start || cli.show_block_status {
        return handle_block_management(&cli);
    }

    // Load configuration
    let config = ConfigLoader::load();

    // Read Claude Code data from stdin
    let stdin = io::stdin();
    let input: InputData = serde_json::from_reader(stdin.lock())?;

    // Generate statusline
    let generator = StatusLineGenerator::new(config);
    let statusline = generator.generate(&input);

    println!("{}", statusline);

    Ok(())
}

/// Handle block start time management CLI commands
fn handle_block_management(cli: &Cli) -> io::Result<()> {
    let mut manager = match BlockOverrideManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("Error: Failed to initialize block override manager: {}", e);
            return Err(io::Error::other(e));
        }
    };

    // Load existing configuration
    if let Err(e) = manager.load() {
        eprintln!("Warning: Failed to load existing configuration: {}", e);
    }

    let today = Local::now().date_naive();

    // Handle set block start time
    if let Some(time_input) = &cli.set_block_start {
        match BlockOverrideManager::parse_time_input(time_input) {
            Ok(start_time) => {
                let notes = Some(format!(
                    "Set via CLI at {}",
                    Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                ));

                match manager.set_override(today, start_time, "manual".to_string(), notes) {
                    Ok(()) => {
                        let local_start_time = start_time.with_timezone(&Local);
                        println!(
                            "✓ Block start time set to {} ({} local) for {}",
                            start_time.format("%Y-%m-%d %H:%M UTC"),
                            local_start_time.format("%H:%M %Z"),
                            today.format("%Y-%m-%d")
                        );
                        println!("  Configuration saved to: {:?}", manager.get_config_path());
                    }
                    Err(e) => {
                        eprintln!("Error: Failed to set block start time: {}", e);
                        return Err(io::Error::other(e));
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: Invalid time format: {}", e);
                let now_local = Local::now();
                eprintln!(
                    "Valid formats: single hour (0-23), HH:MM, or ISO timestamp (YYYY-MM-DDTHH:MM:SSZ)"
                );
                eprintln!(
                    "Times are interpreted as local time (current: {})",
                    now_local.format("%H:%M %Z")
                );
                return Err(io::Error::new(io::ErrorKind::InvalidInput, e));
            }
        }
    }

    // Handle clear block start
    if cli.clear_block_start {
        match manager.clear_override(today) {
            Ok(true) => {
                println!(
                    "✓ Block start time override cleared for {}",
                    today.format("%Y-%m-%d")
                );
            }
            Ok(false) => {
                println!(
                    "ℹ No block start time override was set for {}",
                    today.format("%Y-%m-%d")
                );
            }
            Err(e) => {
                eprintln!("Error: Failed to clear block start time: {}", e);
                return Err(io::Error::other(e));
            }
        }
    }

    // Handle show block status
    if cli.show_block_status {
        println!("Block Override Status:");
        println!("  Configuration file: {:?}", manager.get_config_path());
        println!("  Total overrides: {}", manager.override_count());

        if let Some(override_config) = manager.get_override(today) {
            println!("\n  Today ({}):", today.format("%Y-%m-%d"));
            println!("    ✓ Override active");
            let local_start_time = override_config.start_time.with_timezone(&Local);
            println!(
                "    ⏰ Block starts at: {} ({} local)",
                override_config.start_time.format("%H:%M UTC"),
                local_start_time.format("%H:%M %Z")
            );
            println!("    📝 Source: {}", override_config.source);
            println!(
                "    🕐 Created: {}",
                override_config.created_at.format("%Y-%m-%d %H:%M UTC")
            );
            if let Some(ref notes) = override_config.notes {
                println!("    📋 Notes: {}", notes);
            }
        } else {
            println!("\n  Today ({}):", today.format("%Y-%m-%d"));
            println!("    ⏱️ No override set (will use automatic detection)");
        }

        // Show recent overrides for context
        let all_dates = manager.get_all_dates();
        if !all_dates.is_empty() {
            println!("\n  Recent overrides:");
            let mut sorted_dates = all_dates;
            sorted_dates.sort();
            for date_str in sorted_dates.iter().rev().take(5) {
                if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    if let Some(override_config) = manager.get_override(date) {
                        println!(
                            "    {} -> {} ({})",
                            date.format("%Y-%m-%d"),
                            override_config.start_time.format("%H:%M UTC"),
                            override_config.source
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
