use crate::config::AnsiColor;
use ratatui::style::Color;

/// Serializes an AnsiColor to a JSON string for metadata storage
pub fn serialize_ansi_color_to_json(color: &AnsiColor) -> String {
    match color {
        AnsiColor::Color256 { c256 } => {
            serde_json::json!({"c256": c256}).to_string()
        }
        AnsiColor::Color16 { c16 } => {
            serde_json::json!({"c16": c16}).to_string()
        }
        AnsiColor::Rgb { r, g, b } => {
            serde_json::json!({"r": r, "g": g, "b": b}).to_string()
        }
    }
}

/// Converts a 16-color ANSI code to a ratatui Color
/// This is used throughout the TUI for consistent color rendering
pub fn c16_to_ratatui_color(c16: u8) -> Color {
    match c16 {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::Gray,
        _ => Color::White, // Default fallback
    }
}

/// Converts an AnsiColor to a ratatui Color
/// Handles all three color formats: c16, c256, and RGB
pub fn ansi_color_to_ratatui(color: &AnsiColor) -> Color {
    match color {
        AnsiColor::Color16 { c16 } => c16_to_ratatui_color(*c16),
        AnsiColor::Color256 { c256 } => Color::Indexed(*c256),
        AnsiColor::Rgb { r, g, b } => Color::Rgb(*r, *g, *b),
    }
}
