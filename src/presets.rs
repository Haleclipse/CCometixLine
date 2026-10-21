//! Built-in presets: layouts with a look. Applying one copies it into the
//! user's config.

use crate::config::Config;

pub struct Preset {
    pub name: &'static str,
    pub summary: &'static str,
    toml: &'static str,
}

pub const DEFAULT: &str = "classic";

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "classic",
        summary: "The v1 look: model, directory, git and context on one line",
        toml: r#"
palette = "terminal"
skin = "plain"
glyphs = "nerd"
separator = " | "

[[line]]
left = [
  "model",
  "dir",
  "git",
  { metric = "context", widget = "text", format = "{pct}%[ · {tokens} tokens]" },
]
"#,
    },
    Preset {
        name: "minimal",
        summary: "Model, context and cost, no icons",
        toml: r#"
palette = "terminal"
skin = "plain"
glyphs = "unicode"
separator = " · "

[[line]]
left = [
  { metric = "model", icon = "" },
  { metric = "context", widget = "text", icon = "", format = "{pct}%" },
  { metric = "cost", icon = "" },
]
"#,
    },
    Preset {
        name: "dashboard",
        summary: "Three lines: identity, meters, code activity",
        toml: r#"
palette = "catppuccin-mocha"
skin = "pill"
glyphs = "nerd"

[[line]]
left = ["model", "dir", "git"]
right = ["cost"]

[[line]]
left = [{ metric = "context", bar = "eighths", cells = 10 }, { metric = "quota.5h", cells = 8 }]
right = ["cache"]

[[line]]
left = ["lines", "pr", "worktree"]
right = ["duration"]
"#,
    },
    Preset {
        name: "budget",
        summary: "Rate limits with pace, cost and cache",
        toml: r#"
palette = "tokyo-night"
skin = "plain"
glyphs = "nerd"

[[line]]
left = [{ metric = "quota.5h", cells = 10 }, { metric = "quota.7d", cells = 10 }]
right = ["cost"]

[[line]]
left = [{ metric = "context", bar = "blocks" }, "cache"]
right = ["api_share"]
"#,
    },
    Preset {
        name: "focus",
        summary: "One wide context gauge",
        toml: r#"
palette = "terminal"
skin = "plain"
glyphs = "unicode"

[[line]]
left = [{ metric = "context", bar = "eighths", cells = 20, coloring = "gradient", priority = 5 }]
right = [{ metric = "model", priority = 2 }]
"#,
    },
    Preset {
        name: "zen",
        summary: "Plain ASCII without color, for any terminal",
        toml: r#"
palette = "terminal"
skin = "plain"
glyphs = "ascii"
color_depth = "none"
separator = " | "

[[line]]
left = [
  { metric = "model", compact = "{family}" },
  { metric = "context", widget = "text", format = "ctx {pct}%" },
  "cost",
]
"#,
    },
    Preset {
        name: "powerline",
        summary: "Powerline segments with a quota gauge",
        toml: r#"
palette = "tokyo-night"
skin = "powerline"
glyphs = "nerd"

[[line]]
left = ["model", "dir", "git", { metric = "context", widget = "text" }]
right = [{ metric = "quota.5h", widget = "gauge" }, "cost"]
"#,
    },
];

/// The preset `name` as a full config, or `None` if there is none.
pub fn get(name: &str) -> Option<Config> {
    let preset = PRESETS.iter().find(|p| p.name == name)?;
    let mut config: Config = toml::from_str(preset.toml).expect("built-in presets parse");
    config.preset = Some(preset.name.to_string());
    Some(config)
}

/// The config used when the user has none.
pub fn default_config() -> Config {
    get(DEFAULT).expect("default preset exists")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics;

    #[test]
    fn every_preset_parses_and_uses_known_metrics() {
        for p in PRESETS {
            let c = get(p.name).unwrap();
            assert!(!c.lines.is_empty(), "{}", p.name);
            for line in &c.lines {
                for item in line.left.iter().chain(&line.right) {
                    let id = item.config().metric.clone();
                    assert!(
                        metrics::find(&id).is_some(),
                        "{}: unknown metric {id}",
                        p.name
                    );
                }
            }
            assert!(
                crate::style::palette::builtin(&c.palette).is_some(),
                "{}",
                p.name
            );
        }
    }
}
