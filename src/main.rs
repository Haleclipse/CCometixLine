use ccometixline::cli::Cli;
use std::io::{self, IsTerminal, Read};

fn main() {
    let cli = Cli::parse_args();

    if cli.patch.is_some() {
        println!("ccline v2 no longer patches Claude Code; --patch was removed.");
        return;
    }

    if cli.config || io::stdin().is_terminal() {
        println!("ccline v2 is under development: the configuration TUI is not available yet.");
        return;
    }

    let mut input = String::new();
    let _ = io::stdin().read_to_string(&mut input);
    println!("\x1b[0mccline v2 (in development)\x1b[0m");
}
