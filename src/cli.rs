use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ccline", bin_name = "ccline")]
#[command(version, about = "High-performance Claude Code StatusLine")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Set theme
    #[arg(short = 't', long = "theme")]
    pub theme: Option<String>,

    /// Patch Claude Code cli.js to disable context warnings
    #[arg(long = "patch")]
    pub patch: Option<String>,

    /// List segment options with their current and default values
    #[arg(long = "options")]
    pub options: bool,

    /// Set a segment option and save the config (repeatable), e.g. usage.reset_format=countdown
    #[arg(long = "set", value_name = "SEGMENT.KEY=VALUE")]
    pub set: Vec<String>,

    /// Remove a segment option so it falls back to its default (repeatable), e.g. usage.reset_format
    #[arg(long = "unset", value_name = "SEGMENT.KEY")]
    pub unset: Vec<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
