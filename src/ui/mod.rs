pub mod app;
pub mod components;
pub mod events;
pub mod layout;
pub mod main_menu;
pub mod themes;

pub use app::App;
pub use main_menu::{MainMenu, MenuResult};

pub fn run_configurator() -> Result<(), Box<dyn std::error::Error>> {
    App::run()
}
