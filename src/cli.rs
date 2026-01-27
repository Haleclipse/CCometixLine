use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ccline")]
#[command(version, about = "High-performance Claude Code StatusLine")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Set theme
    #[arg(short = 't', long = "theme")]
    pub theme: Option<String>,

    /// Print current configuration
    #[arg(long = "print")]
    pub print: bool,

    /// Initialize config file
    #[arg(long = "init")]
    pub init: bool,

    /// Check configuration
    #[arg(long = "check")]
    pub check: bool,

    /// Check for updates
    #[arg(short = 'u', long = "update")]
    pub update: bool,

    /// Patch Claude Code cli.js to disable context warnings
    #[arg(long = "patch")]
    pub patch: Option<String>,

    /// Enable multiline output with activity tracking (agents/tools lines)
    #[arg(short = 'm', long = "multiline")]
    pub multiline: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
