#[cfg(feature = "tui")]
pub mod app;
#[cfg(feature = "tui")]
pub mod components;
#[cfg(feature = "tui")]
pub mod events;
#[cfg(feature = "tui")]
pub mod intro;
#[cfg(feature = "tui")]
pub mod main_menu;
#[cfg(feature = "tui")]
pub mod themes;

#[cfg(feature = "tui")]
pub use app::App;
#[cfg(feature = "tui")]
pub use main_menu::{MainMenu, MenuResult};

#[cfg(feature = "tui")]
pub fn run_configurator() -> Result<(), Box<dyn std::error::Error>> {
    App::run()
}

#[cfg(feature = "tui")]
pub fn run_intro() -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut intro_app = intro::IntroApp::new();

    // Main loop
    let result = loop {
        terminal.draw(|f| intro_app.render(f))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Esc => {
                    if intro_app.is_showing_overwrite_prompt() {
                        intro_app.handle_overwrite_response(false);
                    } else {
                        intro_app.skip_intro();
                        break Ok(());
                    }
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if intro_app.is_showing_overwrite_prompt() {
                        intro_app.handle_overwrite_response(true);
                        if intro_app.should_continue() {
                            break Ok(());
                        }
                    } else if intro_app.is_awaiting_config_choice() {
                        intro_app.handle_config_choice('y');
                        if intro_app.should_continue() {
                            break Ok(());
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    if intro_app.is_showing_overwrite_prompt() {
                        intro_app.handle_overwrite_response(false);
                        if intro_app.should_continue() {
                            break Ok(());
                        }
                    } else if intro_app.is_awaiting_config_choice() {
                        intro_app.handle_config_choice('n');
                        if intro_app.should_continue() {
                            break Ok(());
                        }
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    if intro_app.is_awaiting_config_choice() {
                        intro_app.handle_config_choice('s');
                        if intro_app.should_continue() {
                            break Ok(());
                        }
                    }
                }
                KeyCode::Enter | KeyCode::Right => {
                    if !intro_app.is_showing_overwrite_prompt()
                        && !intro_app.is_awaiting_config_choice()
                    {
                        intro_app.next_step();
                        if intro_app.should_continue() {
                            break Ok(());
                        }
                    }
                }
                KeyCode::Left => {
                    if !intro_app.is_showing_overwrite_prompt() {
                        intro_app.prev_step();
                    }
                }
                _ => {}
            }

            if intro_app.should_exit() {
                break Ok(());
            }
        }
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

#[cfg(not(feature = "tui"))]
pub fn run_configurator() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("TUI feature is not enabled. Please install with --features tui");
    std::process::exit(1);
}

#[cfg(not(feature = "tui"))]
pub fn run_intro() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("TUI feature is not enabled. Please install with --features tui");
    std::process::exit(1);
}
