use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ccline")]
#[command(version, about = "High-performance Claude Code StatusLine")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Override the preset, legacy v1 theme or palette for this render
    #[arg(short = 't', long = "theme")]
    pub theme: Option<String>,

    /// Removed in v2; kept so existing invocations exit cleanly
    #[arg(long = "patch", hide = true)]
    pub patch: Option<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
