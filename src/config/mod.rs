//! The configuration (`~/.claude/ccline/config.toml`) and loading it.

use crate::presets;
use crate::style::{BarGlyphs, ColorDepth, Edges, FillStyle, GlyphTier, SkinKind};
use crate::widgets::{Coloring, WidgetKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;

pub const VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepthSetting {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "truecolor")]
    TrueColor,
    #[serde(rename = "256")]
    Ansi256,
    #[serde(rename = "16")]
    Ansi16,
    #[serde(rename = "none")]
    None,
}

impl DepthSetting {
    pub fn resolve(self) -> ColorDepth {
        match self {
            DepthSetting::Auto => crate::env::color_depth(),
            DepthSetting::TrueColor => ColorDepth::TrueColor,
            DepthSetting::Ansi256 => ColorDepth::Ansi256,
            DepthSetting::Ansi16 => ColorDepth::Ansi16,
            DepthSetting::None => ColorDepth::None,
        }
    }
}

/// A size that is either `"auto"` or a number of columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dim(pub Option<u16>);

impl Serialize for Dim {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(n) => s.serialize_u16(n),
            None => s.serialize_str("auto"),
        }
    }
}

impl<'de> Deserialize<'de> for Dim {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Num(u16),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Num(n) => Ok(Dim(Some(n))),
            Raw::Text(t) if t == "auto" => Ok(Dim(None)),
            Raw::Text(t) => Err(serde::de::Error::custom(format!(
                "expected \"auto\" or a number, got {t:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    pub width: Dim,
    pub padding: Dim,
    pub reserve: u16,
}

/// One item on a line: a metric id, or a table of settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ItemSpec {
    Id(String),
    Item(Box<ItemConfig>),
}

impl ItemSpec {
    pub fn config(&self) -> Cow<'_, ItemConfig> {
        match self {
            ItemSpec::Id(id) => Cow::Owned(ItemConfig {
                metric: id.clone(),
                ..ItemConfig::default()
            }),
            ItemSpec::Item(c) => Cow::Borrowed(c.as_ref()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemConfig {
    pub metric: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<WidgetKind>,
    /// Bar glyph set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar: Option<BarGlyphs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cells: Option<u8>,
    /// Template for the full level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimal: Option<String>,
    /// Icon glyph; `""` for none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// A palette token name or a color (`#rrggbb`, `256:n`, `16:n`, name):
    /// the item's color, which filled skins use as the background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Text color, the same way: on the background in filled skins, next
    /// to the icon in the others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    /// Icon color, the same way; unset, the icon takes the item's color
    /// (its text color in filled skins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    /// Thresholds in percent, e.g. `[[50, "warn"], [80, "crit"]]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<(f64, String)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coloring: Option<Coloring>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    /// Metric-specific options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<toml::Table>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LineConfig {
    #[serde(default)]
    pub left: Vec<ItemSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub right: Vec<ItemSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Reuse a git status younger than this; 0 runs git on every render.
    pub cache_ms: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        GitConfig { cache_ms: 2_000 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    pub check: bool,
    pub interval_hours: u32,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        UpdateConfig {
            check: true,
            interval_hours: 24,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    /// The preset this config started from, for "modified" indicators.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    pub palette: String,
    pub skin: SkinKind,
    /// How pill and powerline segments are colored.
    pub fill: FillStyle,
    pub glyphs: GlyphTier,
    pub color_depth: DepthSetting,
    /// Separator for the plain skin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    /// Where powerline segments meet and how their run ends.
    pub powerline: Edges,
    pub layout: LayoutConfig,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub palette_overrides: BTreeMap<String, String>,
    pub git: GitConfig,
    pub update: UpdateConfig,
    /// The line drawn for each background task (`subagentStatusLine`); a
    /// built-in line when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent: Option<LineConfig>,
    #[serde(rename = "line")]
    pub lines: Vec<LineConfig>,
}

/// A config with no lines; the base that missing fields fall back to. The
/// config users start with is `presets::default_config()`.
impl Default for Config {
    fn default() -> Self {
        Config {
            version: VERSION,
            preset: None,
            palette: "terminal".into(),
            skin: SkinKind::Plain,
            fill: FillStyle::Solid,
            glyphs: GlyphTier::Nerd,
            color_depth: DepthSetting::Auto,
            separator: None,
            powerline: Edges::default(),
            layout: LayoutConfig::default(),
            palette_overrides: BTreeMap::new(),
            git: GitConfig::default(),
            update: UpdateConfig::default(),
            subagent: None,
            lines: Vec::new(),
        }
    }
}

/// What happened while loading.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadNote {
    Fine,
    /// No config file; using the default preset.
    Missing,
    /// The file couldn't be used; the default preset is used instead.
    Invalid(String),
}

impl Config {
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::paths::write_atomic(path, &self.to_toml())
    }

    /// Load the user's config.
    pub fn load() -> (Config, LoadNote) {
        load_from(&crate::paths::config_file())
    }
}

/// Load `path`. A file that can't be used gives the default preset, so the
/// status line never comes out blank; nothing is written.
pub fn load_from(path: &Path) -> (Config, LoadNote) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (presets::default_config(), LoadNote::Missing),
    };
    let table: toml::Table = match toml::from_str(&text) {
        Ok(t) => t,
        Err(e) => {
            return (
                presets::default_config(),
                LoadNote::Invalid(e.message().to_string()),
            );
        }
    };
    // Missing settings take their defaults; for the lines that is the
    // default preset's, as an empty status line is never what's meant.
    // `line = []`, written when every line was removed, stays empty.
    let has_lines = table.contains_key("line");
    match Config::deserialize(toml::Value::Table(table)) {
        Ok(mut config) => {
            if !has_lines {
                config.lines = presets::default_config().lines;
            }
            (config, LoadNote::Fine)
        }
        Err(e) => (
            presets::default_config(),
            LoadNote::Invalid(e.message().to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_the_removed_overflow_setting() {
        let c: Config =
            toml::from_str("version = 2\n[layout]\noverflow = \"shrink\"\nreserve = 2\n").unwrap();
        assert_eq!(c.layout.reserve, 2);
    }

    #[test]
    fn round_trips_through_toml() {
        let config = presets::get("dashboard").unwrap();
        let text = config.to_toml();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn parses_short_and_full_items() {
        let c: Config = toml::from_str(
            r#"
            version = 2
            [layout]
            width = 120
            padding = "auto"
            [[line]]
            left = ["model", { metric = "context", widget = "bar", bar = "eighths", levels = [[60, "warn"]] }]
            "#,
        )
        .unwrap();
        assert_eq!(c.layout.width, Dim(Some(120)));
        assert_eq!(c.layout.padding, Dim(None));
        let item = c.lines[0].left[1].config();
        assert_eq!(item.metric, "context");
        assert_eq!(item.bar, Some(BarGlyphs::Eighths));
        assert_eq!(
            item.levels.as_deref(),
            Some(&[(60.0, "warn".to_string())][..])
        );
        assert_eq!(c.lines[0].left[0].config().metric, "model");
    }

    #[test]
    fn falls_back_on_invalid_or_missing_files() {
        let dir = std::env::temp_dir().join(format!("ccline-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let (config, note) = load_from(&path);
        assert_eq!(note, LoadNote::Missing);
        assert!(!config.lines.is_empty());
        std::fs::write(&path, "version = 2\nskin = \"lasers\"\n[[line]]\n").unwrap();
        assert!(matches!(load_from(&path).1, LoadNote::Invalid(_)));

        // Without lines, the default preset's lines go with the rest.
        std::fs::write(&path, "palette = \"dracula\"\nstyle = { mode = \"x\" }\n").unwrap();
        let (config, note) = load_from(&path);
        assert_eq!(note, LoadNote::Fine);
        assert_eq!(config.palette, "dracula");
        assert_eq!(config.lines, presets::default_config().lines);

        // Every line removed and saved is still a config of the user's.
        let mut empty = presets::default_config();
        empty.lines.clear();
        empty.save(&path).unwrap();
        assert_eq!(load_from(&path), (empty, LoadNote::Fine));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
