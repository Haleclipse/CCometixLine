use atty;
use ccometixline::cli::Cli;
use ccometixline::config::{Config, InputData};
use ccometixline::core::{collect_all_segments, StatusLineGenerator};
use ccometixline::ui::run_intro;
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_args();

    // Handle configuration commands
    if cli.init {
        Config::init()?;
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

    if cli.intro {
        return run_intro();
    }

    if atty::is(atty::Stream::Stdin) {
        println!("CCometixLine is running in interactive mode. Use --help for options or -i/--intro to run the intro application.");
        Ok(())
    } else {
        // Load configuration
        let mut config = Config::load().unwrap_or_else(|_| Config::default());

        // Apply theme override if provided
        if let Some(theme) = cli.theme {
            config = ccometixline::ui::themes::ThemePresets::get_theme(&theme);
        }

        // Read Claude Code data from stdin
        let stdin = io::stdin();
        let input: InputData = serde_json::from_reader(stdin.lock())?;

        // Collect segment data
        let segments_data = collect_all_segments(&config, &input);

        // Render statusline
        let generator = StatusLineGenerator::new(config);
        let statusline = generator.generate(segments_data);

        println!("{}", statusline);
        Ok(())
    }
}
