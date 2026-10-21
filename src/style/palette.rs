use super::color::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Metric categories; each has its own color in a palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Identity,
    Workspace,
    Vcs,
    Context,
    Quota,
    Cost,
    Cache,
    Session,
    Timing,
    Activity,
    System,
    /// User commands; drawn in the accent color.
    Custom,
}

/// A named color slot in a palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Token {
    Ok,
    Warn,
    Crit,
    Accent,
    Muted,
    /// Default text; `None` means the terminal's foreground.
    Text,
    /// Background of the status line; `None` means the terminal's.
    Bg,
    /// Text drawn on a filled (colored) background.
    OnFill,
    Identity,
    Workspace,
    Vcs,
    Context,
    Quota,
    Cost,
    Cache,
    Session,
    Timing,
    Activity,
    System,
}

pub const TOKENS: [Token; 19] = [
    Token::Ok,
    Token::Warn,
    Token::Crit,
    Token::Accent,
    Token::Muted,
    Token::Text,
    Token::Bg,
    Token::OnFill,
    Token::Identity,
    Token::Workspace,
    Token::Vcs,
    Token::Context,
    Token::Quota,
    Token::Cost,
    Token::Cache,
    Token::Session,
    Token::Timing,
    Token::Activity,
    Token::System,
];

impl Token {
    pub fn parse(name: &str) -> Option<Token> {
        TOKENS.into_iter().find(|t| t.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Token::Ok => "ok",
            Token::Warn => "warn",
            Token::Crit => "crit",
            Token::Accent => "accent",
            Token::Muted => "muted",
            Token::Text => "text",
            Token::Bg => "bg",
            Token::OnFill => "on_fill",
            Token::Identity => "identity",
            Token::Workspace => "workspace",
            Token::Vcs => "vcs",
            Token::Context => "context",
            Token::Quota => "quota",
            Token::Cost => "cost",
            Token::Cache => "cache",
            Token::Session => "session",
            Token::Timing => "timing",
            Token::Activity => "activity",
            Token::System => "system",
        }
    }
}

impl From<Category> for Token {
    fn from(c: Category) -> Token {
        match c {
            Category::Identity => Token::Identity,
            Category::Workspace => Token::Workspace,
            Category::Vcs => Token::Vcs,
            Category::Context => Token::Context,
            Category::Quota => Token::Quota,
            Category::Cost => Token::Cost,
            Category::Cache => Token::Cache,
            Category::Session => Token::Session,
            Category::Timing => Token::Timing,
            Category::Activity => Token::Activity,
            Category::System => Token::System,
            Category::Custom => Token::Accent,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub name: String,
    pub light: bool,
    colors: BTreeMap<Token, Color>,
}

impl Palette {
    pub fn get(&self, token: Token) -> Option<Color> {
        self.colors.get(&token).copied()
    }

    /// Color for `token`, falling back to the terminal palette's.
    pub fn color(&self, token: Token) -> Option<Color> {
        self.get(token).or_else(|| terminal().get(token))
    }

    pub fn set(&mut self, token: Token, color: Color) {
        self.colors.insert(token, color);
    }

    /// Load a palette by name: a user file in `palettes/` wins over a built-in.
    pub fn load(name: &str) -> Option<Palette> {
        let path = crate::paths::home()
            .join("palettes")
            .join(format!("{name}.toml"));
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(file) = toml::from_str::<PaletteFile>(&text)
        {
            return Some(file.into_palette(name));
        }
        builtin(name)
    }
}

/// A user palette file: `light = false` and a `[colors]` table of token = color.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PaletteFile {
    light: bool,
    colors: BTreeMap<String, String>,
}

impl PaletteFile {
    fn into_palette(self, name: &str) -> Palette {
        let colors = self
            .colors
            .iter()
            .filter_map(|(k, v)| Some((Token::parse(k)?, Color::parse(v)?)))
            .collect();
        Palette {
            name: name.to_string(),
            light: self.light,
            colors,
        }
    }
}

pub const BUILTIN_NAMES: [&str; 9] = [
    "terminal",
    "catppuccin-mocha",
    "catppuccin-latte",
    "dracula",
    "nord",
    "tokyo-night",
    "rose-pine",
    "gruvbox",
    "anthropic-clay",
];

/// Token order used by the hex tables below.
const TABLE_ORDER: [Token; 18] = [
    Token::Ok,
    Token::Warn,
    Token::Crit,
    Token::Accent,
    Token::Muted,
    Token::Text,
    Token::Bg,
    Token::OnFill,
    Token::Identity,
    Token::Workspace,
    Token::Vcs,
    Token::Context,
    Token::Quota,
    Token::Cost,
    Token::Cache,
    Token::Session,
    Token::Timing,
    Token::Activity,
];

fn from_table(name: &str, light: bool, hex: [&str; 18], system: &str) -> Palette {
    let mut colors: BTreeMap<Token, Color> = TABLE_ORDER
        .into_iter()
        .zip(hex)
        .filter_map(|(t, h)| Some((t, Color::parse(h)?)))
        .collect();
    if let Some(c) = Color::parse(system) {
        colors.insert(Token::System, c);
    }
    Palette {
        name: name.to_string(),
        light,
        colors,
    }
}

/// ANSI 16 colors, so the line follows the terminal's own theme. Text and
/// background stay at the terminal defaults. Category colors match v1's
/// cometix theme.
pub fn terminal() -> Palette {
    use Color::Ansi16 as A;
    let colors = [
        (Token::Ok, A(2)),
        (Token::Warn, A(3)),
        (Token::Crit, A(1)),
        (Token::Accent, A(14)),
        (Token::Muted, A(8)),
        (Token::OnFill, A(0)),
        (Token::Identity, A(14)),
        (Token::Workspace, A(10)),
        (Token::Vcs, A(12)),
        (Token::Context, A(13)),
        (Token::Quota, A(6)),
        (Token::Cost, A(3)),
        (Token::Cache, A(4)),
        (Token::Session, A(2)),
        (Token::Timing, A(5)),
        (Token::Activity, A(11)),
        (Token::System, A(8)),
    ];
    Palette {
        name: "terminal".into(),
        light: false,
        colors: colors.into_iter().collect(),
    }
}

#[rustfmt::skip]
pub fn builtin(name: &str) -> Option<Palette> {
    //            ok         warn       crit       accent     muted      text       bg         on_fill    identity   workspace  vcs        context    quota      cost       cache      session    timing     activity
    Some(match name {
        "terminal" => terminal(),
        "catppuccin-mocha" => from_table(
            name,
            false,
            ["#a6e3a1", "#f9e2af", "#f38ba8", "#89b4fa", "#6c7086", "#cdd6f4", "#1e1e2e", "#11111b", "#b4befe", "#94e2d5", "#74c7ec", "#cba6f7", "#fab387", "#f9e2af", "#89dceb", "#f5c2e7", "#f2cdcd", "#eba0ac"],
            "#7f849c",
        ),
        "catppuccin-latte" => from_table(
            name,
            true,
            ["#40a02b", "#df8e1d", "#d20f39", "#1e66f5", "#9ca0b0", "#4c4f69", "#eff1f5", "#eff1f5", "#7287fd", "#179299", "#209fb5", "#8839ef", "#fe640b", "#df8e1d", "#04a5e5", "#ea76cb", "#dd7878", "#e64553"],
            "#8c8fa1",
        ),
        "dracula" => from_table(
            name,
            false,
            ["#50fa7b", "#ffb86c", "#ff5555", "#bd93f9", "#6272a4", "#f8f8f2", "#282a36", "#282a36", "#bd93f9", "#50fa7b", "#8be9fd", "#ff79c6", "#ffb86c", "#f1fa8c", "#8be9fd", "#ff79c6", "#bd93f9", "#ffb86c"],
            "#6272a4",
        ),
        "nord" => from_table(
            name,
            false,
            ["#a3be8c", "#ebcb8b", "#bf616a", "#88c0d0", "#4c566a", "#d8dee9", "#2e3440", "#2e3440", "#88c0d0", "#a3be8c", "#81a1c1", "#b48ead", "#d08770", "#ebcb8b", "#8fbcbb", "#5e81ac", "#b48ead", "#d08770"],
            "#616e88",
        ),
        "tokyo-night" => from_table(
            name,
            false,
            ["#9ece6a", "#e0af68", "#f7768e", "#7aa2f7", "#565f89", "#c0caf5", "#1a1b26", "#1a1b26", "#7aa2f7", "#9ece6a", "#7dcfff", "#bb9af7", "#ff9e64", "#e0af68", "#73daca", "#bb9af7", "#2ac3de", "#ff9e64"],
            "#565f89",
        ),
        "rose-pine" => from_table(
            name,
            false,
            ["#9ccfd8", "#f6c177", "#eb6f92", "#c4a7e7", "#6e6a86", "#e0def4", "#191724", "#191724", "#c4a7e7", "#9ccfd8", "#31748f", "#ebbcba", "#f6c177", "#f6c177", "#9ccfd8", "#ebbcba", "#c4a7e7", "#eb6f92"],
            "#908caa",
        ),
        "gruvbox" => from_table(
            name,
            false,
            ["#b8bb26", "#fabd2f", "#fb4934", "#83a598", "#928374", "#ebdbb2", "#282828", "#282828", "#83a598", "#b8bb26", "#8ec07c", "#d3869b", "#fe8019", "#fabd2f", "#8ec07c", "#d3869b", "#83a598", "#fe8019"],
            "#928374",
        ),
        "anthropic-clay" => from_table(
            name,
            false,
            ["#788c5d", "#d4a27f", "#bf4d43", "#d97757", "#87867f", "#f0eee6", "#1f1e1d", "#1f1e1d", "#d97757", "#788c5d", "#6a9bcc", "#cc785c", "#d4a27f", "#d4a27f", "#6a9bcc", "#c46686", "#87867f", "#cc785c"],
            "#87867f",
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_defines_every_token_except_terminal_defaults() {
        for name in BUILTIN_NAMES {
            let p = builtin(name).unwrap();
            for t in TOKENS {
                let optional = name == "terminal" && matches!(t, Token::Text | Token::Bg);
                assert!(optional || p.get(t).is_some(), "{name} lacks {t:?}");
            }
        }
    }

    #[test]
    fn token_names_round_trip() {
        for t in TOKENS {
            assert_eq!(Token::parse(t.name()), Some(t));
        }
    }

    #[test]
    fn user_palette_falls_back_per_token() {
        let p = PaletteFile {
            light: true,
            colors: [
                ("warn".into(), "#ff0000".into()),
                ("bogus".into(), "#000".into()),
            ]
            .into(),
        }
        .into_palette("mine");
        assert_eq!(p.color(Token::Warn), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(p.color(Token::Crit), Some(Color::Ansi16(1)));
        assert!(p.light);
    }
}
