use ccometixline::cli::Cli;
use ccometixline::config::{Config, InputData};
use ccometixline::core::{collect_all_segments, MultilineConfig, MultilineRenderer, StatusLineGenerator};
use std::io::{self, IsTerminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_args();

    // Handle configuration commands
    if cli.init {
        use ccometixline::config::InitResult;
        match Config::init()? {
            InitResult::Created(path) => println!("Created config at {}", path.display()),
            InitResult::AlreadyExists(path) => {
                println!("Config already exists at {}", path.display())
            }
        }
        return Ok(());
    }

    if cli.print {
        let mut config = Config::load().unwrap_or_else(|_| Config::default());

        // Apply theme override if provided
        if let Some(theme) = cli.theme {
            config = ccometixline::ui::themes::ThemePresets::get_theme(&theme);
        }

        config.print()?;
        return Ok(());
    }

    if cli.check {
        let config = Config::load()?;
        config.check()?;
        println!("✓ Configuration valid");
        return Ok(());
    }

    if cli.config {
        #[cfg(feature = "tui")]
        {
            ccometixline::ui::run_configurator()?;
        }
        #[cfg(not(feature = "tui"))]
        {
            eprintln!("TUI feature is not enabled. Please install with --features tui");
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.update {
        #[cfg(feature = "self-update")]
        {
            println!("Update feature not implemented in new architecture yet");
        }
        #[cfg(not(feature = "self-update"))]
        {
            println!("Update check not available (self-update feature disabled)");
        }
        return Ok(());
    }

    // Handle Claude Code patcher
    if let Some(claude_path) = cli.patch {
        use ccometixline::utils::ClaudeCodePatcher;

        println!("🔧 Claude Code Context Warning Disabler");
        println!("Target file: {}", claude_path);

        // Create backup in same directory
        let backup_path = format!("{}.backup", claude_path);
        std::fs::copy(&claude_path, &backup_path)?;
        println!("📦 Created backup: {}", backup_path);

        // Load and patch
        let mut patcher = ClaudeCodePatcher::new(&claude_path)?;

        println!("\n🔄 Applying patches...");
        let results = patcher.apply_all_patches();
        patcher.save()?;

        ClaudeCodePatcher::print_summary(&results);
        println!("💡 To restore warnings, replace your cli.js with the backup file:");
        println!("   cp {} {}", backup_path, claude_path);

        return Ok(());
    }

    // Load configuration
    let mut config = Config::load().unwrap_or_else(|_| Config::default());

    // Apply theme override if provided
    if let Some(theme) = cli.theme {
        config = ccometixline::ui::themes::ThemePresets::get_theme(&theme);
    }

    // Check if stdin has data
    if io::stdin().is_terminal() {
        // No input data available, show main menu
        #[cfg(feature = "tui")]
        {
            use ccometixline::ui::{MainMenu, MenuResult};

            if let Some(result) = MainMenu::run()? {
                match result {
                    MenuResult::LaunchConfigurator => {
                        ccometixline::ui::run_configurator()?;
                    }
                    MenuResult::InitConfig | MenuResult::CheckConfig => {
                        // These are now handled internally by the menu
                        // and should not be returned, but handle gracefully
                    }
                    MenuResult::Exit => {
                        // Exit gracefully
                    }
                }
            }
        }
        #[cfg(not(feature = "tui"))]
        {
            eprintln!("No input data provided and TUI feature is not enabled.");
            eprintln!("Usage: echo '{{...}}' | ccline");
            eprintln!("   or: ccline --help");
        }
        return Ok(());
    }

    // Read Claude Code data from stdin
    let stdin = io::stdin();
    let input: InputData = serde_json::from_reader(stdin.lock())?;

    // Check if multiline mode is enabled
    if cli.multiline {
        // Use multiline renderer with activity tracking
        let multiline_config = MultilineConfig::default();
        let renderer = MultilineRenderer::new(config, multiline_config);
        renderer.render_and_print(&input);
    } else {
        // Use standard single-line output
        // Collect segment data
        let segments_data = collect_all_segments(&config, &input);

        // Render statusline
        let generator = StatusLineGenerator::new(config);
        let statusline = generator.generate(segments_data);

        println!("{}", statusline);
    }

    Ok(())
}
