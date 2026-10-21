use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "ccline")]
#[command(version, about = "High-performance Claude Code StatusLine")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Override the preset, legacy v1 theme or palette for this render
    #[arg(short = 't', long = "theme", global = true)]
    pub theme: Option<String>,

    /// Removed in v2; kept so existing invocations exit cleanly
    #[arg(long = "patch", hide = true)]
    pub patch: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render built-in sample sessions (or a captured JSON file) with your config
    Preview(PreviewArgs),
    /// Check for a newer ccline now and show how to upgrade
    Update,
    /// Refresh the cached git status of a directory (started in the
    /// background by renders)
    #[command(hide = true)]
    RefreshGit { dir: std::path::PathBuf },
    /// Check for a newer ccline and record the answer (started in the
    /// background by renders)
    #[command(hide = true)]
    UpdateCheck,
}

#[derive(Args, Debug)]
pub struct PreviewArgs {
    /// Scenario name, or a path to a status line JSON file
    #[arg(short, long, default_value = "normal")]
    pub scenario: String,

    /// Render every built-in scenario
    #[arg(long)]
    pub all: bool,

    /// List the built-in scenarios and presets
    #[arg(long)]
    pub list: bool,

    /// Columns available (default: $COLUMNS, else 100)
    #[arg(short, long)]
    pub width: Option<usize>,

    /// Color depth: auto, truecolor, 256, 16 or none
    #[arg(long, default_value = "auto")]
    pub color: String,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
