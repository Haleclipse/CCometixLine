use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

pub fn get_step_title(current_step: usize) -> &'static str {
    match current_step {
        0 => " Welcome ",
        1 => " Nerd Font Test ",
        2 => " Automatic Configuration ",
        _ => " Intro ",
    }
}

pub fn get_step_content(current_step: usize) -> Text<'static> {
    match current_step {
        0 => Text::from(vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Welcome to "),
                Span::styled(
                    "CCometixLine",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("! 🚀"),
            ]),
            Line::from(""),
            Line::from("A high-performance statusline tool for Claude Code,"),
            Line::from("built with Rust for maximum speed and reliability."),
            Line::from(""),
            Line::from("Key features:"),
            Line::from("• Real-time Git branch and status information"),
            Line::from("• Current working directory and file details"),
            Line::from("• Multiple themes and customizable configuration"),
            Line::from("• Seamless integration with Claude Code"),
            Line::from(""),
            Line::from("Press Enter or → to continue!"),
        ]),

        1 => Text::from(vec![
            Line::from("Nerd Font Display Test"),
            Line::from(""),
            Line::from("Can you see these icons clearly and distinctly?"),
            Line::from(""),
            Line::from(vec![
                Span::styled("\u{e26d}", Style::default().fg(Color::Yellow)),
                Span::raw(" ← Should be a Haleclipse"),
            ]),
            Line::from(vec![
                Span::styled("\u{f024b}", Style::default().fg(Color::Magenta)),
                Span::raw(" ← Should be a folder icon"),
            ]),
            Line::from(vec![
                Span::styled("\u{f02a2}", Style::default().fg(Color::Blue)),
                Span::raw(" ← Should be a git branch icon"),
            ]),
            Line::from(""),
            Line::from("Powerline separators:"),
            Line::from(vec![
                Span::styled("\u{e0b0}", Style::default().fg(Color::Cyan)),
                Span::raw(" ← Should be angular separators"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "If you see boxes (□) or question marks (?)",
                Style::default().fg(Color::Red),
            )]),
            Line::from(vec![Span::styled(
                "instead of distinct icons, we recommend installing",
                Style::default().fg(Color::Red),
            )]),
            Line::from(vec![
                Span::styled(
                    "Maple Mono",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" for the best experience.", Style::default().fg(Color::Red)),
            ]),
        ]),

        2 => Text::from(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Automatic Claude Code Configuration",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from("CCometixLine can automatically configure Claude Code for you!"),
            Line::from(""),
            Line::from("This will:"),
            Line::from("• Detect your Claude Code settings file"),
            Line::from("• Add statusLine configuration automatically"),
            Line::from("• Set the correct path for ccline"),
            Line::from("• Handle platform differences (Windows/Linux/macOS)"),
            Line::from(""),
            Line::from("If you already have a statusLine configured,"),
            Line::from("we'll ask before overwriting it."),
            Line::from(""),
            Line::from(vec![
                Span::styled("Would you like to automatically configure Claude Code?", 
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" - Yes, configure automatically"),
            ]),
            Line::from(vec![
                Span::styled("N", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" - No, I'll configure manually"),
            ]),
            Line::from(vec![
                Span::styled("S", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" - Skip and start configurator"),
            ]),
        ]),

        _ => Text::from(""),
    }
}