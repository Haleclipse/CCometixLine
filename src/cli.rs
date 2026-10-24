use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "ccline")]
#[command(version, about = "High-performance Claude Code StatusLine")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Render task lines for `subagentStatusLine` (also detected from the
    /// input)
    #[arg(long)]
    pub subagent: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render built-in sample sessions (or a captured JSON file) with your config
    Preview(PreviewArgs),
    /// Check for a newer ccline now and show how to upgrade
    Update,
    /// The companion mod for Claude Code 2.1.287+: install, update, remove
    /// or show it
    Mod {
        #[command(subcommand)]
        action: Option<ModAction>,
    },
    /// Refresh the cached git status of a directory (started in the
    /// background by renders)
    #[command(hide = true)]
    RefreshGit { dir: std::path::PathBuf },
    /// Check for a newer ccline and record the answer (started in the
    /// background by renders)
    #[command(hide = true)]
    UpdateCheck,
    /// Run a `cmd` item's command and cache its output (started in the
    /// background by renders)
    #[command(hide = true)]
    RefreshCmd {
        #[arg(long)]
        cwd: Option<std::path::PathBuf>,
        #[arg(long)]
        timeout_ms: u64,
        #[arg(last = true)]
        run: String,
    },
}

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum ModAction {
    /// Show whether it is installed and current
    Status,
    /// Install it, or bring it up to date
    Install,
    /// Remove it
    Uninstall,
}

#[derive(Args, Debug)]
pub struct PreviewArgs {
    /// Scenario name, or a path to a status line JSON file
    #[arg(short, long, default_value = "normal")]
    pub scenario: String,

    /// Render every built-in scenario
    #[arg(long)]
    pub all: bool,

    /// List the built-in scenarios
    #[arg(long)]
    pub list: bool,

    /// Render the sample background tasks with your subagent line
    #[arg(long)]
    pub subagent: bool,

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
