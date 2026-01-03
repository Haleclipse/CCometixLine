#[cfg(feature = "tui")]
pub mod alias_editor;
#[cfg(feature = "tui")]
pub mod app;
#[cfg(feature = "tui")]
pub mod components;
#[cfg(feature = "tui")]
pub mod events;
#[cfg(feature = "tui")]
pub mod layout;
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
pub fn run_alias_editor() -> Result<(), Box<dyn std::error::Error>> {
    alias_editor::AliasEditorApp::run()
}

#[cfg(not(feature = "tui"))]
pub fn run_configurator() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("TUI feature is not enabled. Please install with --features tui");
    std::process::exit(1);
}

#[cfg(not(feature = "tui"))]
pub fn run_alias_editor() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("TUI feature is not enabled. Please install with --features tui");
    std::process::exit(1);
}
