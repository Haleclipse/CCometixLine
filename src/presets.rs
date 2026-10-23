//! Presets: layouts with a look. Applying one copies it into the user's
//! config. Built-ins live here; the user's own are files in `presets/`,
//! and a file named like a built-in replaces it.

use crate::config::{Config, DepthSetting, LineConfig};
use crate::style::{Edges, FillStyle, GlyphTier, SkinKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Preset {
    pub name: &'static str,
    pub summary: &'static str,
    /// Style and data axes for filtering, as on statuslin.es: `minimal`,
    /// `multi-line`, `powerline`, `quota`, `cache`, `context`.
    pub tags: &'static [&'static str],
    toml: &'static str,
}

pub const DEFAULT: &str = "classic";

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "classic",
        summary: "The v1 look: model, directory, git and context on one line",
        tags: &[],
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
right = ["update"]
"#,
    },
    Preset {
        name: "minimal",
        summary: "Model, context and cost, no icons",
        tags: &["minimal"],
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
        tags: &["multi-line", "quota", "context"],
        toml: r#"
palette = "catppuccin-mocha"
skin = "pill"
glyphs = "nerd"

[[line]]
left = ["model", "dir", "git"]
right = ["update", "cost"]

[[line]]
left = [{ metric = "context", bar = "eighths", cells = 10 }, "context.trend", { metric = "quota.5h", cells = 8 }]
right = ["cache"]

[[line]]
left = ["lines", "pr", "worktree"]
right = ["duration"]
"#,
    },
    Preset {
        name: "budget",
        summary: "Rate limits with pace and run-out, spend rates, compaction and cache",
        tags: &["multi-line", "quota"],
        toml: r#"
palette = "tokyo-night"
skin = "plain"
glyphs = "nerd"

[[line]]
left = [{ metric = "quota.5h", cells = 10 }, { metric = "quota.7d", cells = 10 }, "quota.eta"]
right = ["cost", "cost.rate", "cost.today"]

[[line]]
left = [{ metric = "context", bar = "blocks" }, "context.to_compact", "cache"]
right = ["api_share"]
"#,
    },
    Preset {
        name: "cache-aware",
        summary: "Prompt cache countdown, hit ratio and misses under the usual line",
        tags: &["multi-line", "cache"],
        toml: r#"
palette = "rose-pine"
skin = "plain"
glyphs = "nerd"
separator = " · "

[[line]]
left = ["model", "dir", "git", { metric = "context", widget = "text" }]
right = ["update", "cost"]

[[line]]
left = [
  { metric = "cache", widget = "bar", bar = "dots", cells = 8, format = "[warm {left}][{cold}]", priority = 4 },
  { metric = "cache", icon = "", format = "[{hit}% hit][ · {misses}× miss][ · last: {cause}]", compact = "[{hit}% hit][ · {misses}× miss]", minimal = "[{hit}%]" },
]
right = ["cost.rate"]
"#,
    },
    Preset {
        name: "focus",
        summary: "A wide context gauge, its growth per turn and the room left before compaction",
        tags: &["multi-line", "context"],
        toml: r#"
palette = "terminal"
skin = "plain"
glyphs = "unicode"

[[line]]
left = [{ metric = "context", bar = "eighths", cells = 20, coloring = "gradient", priority = 5 }]
right = [{ metric = "model", priority = 2 }]

[[line]]
left = [{ metric = "context.trend", cells = 12 }, { metric = "context.to_compact", icon = "", priority = 4 }]
"#,
    },
    Preset {
        name: "zen",
        summary: "Plain ASCII without color, for any terminal",
        tags: &["minimal"],
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
        summary: "Powerline segments with a quota gauge, in v1's Tokyo Night colors",
        tags: &["powerline", "quota"],
        toml: r#"
palette = "powerline-tokyo-night"
skin = "powerline"
glyphs = "nerd"

[[line]]
left = ["model", "dir", "git", { metric = "context", widget = "text" }]
right = [{ metric = "quota.5h", widget = "gauge" }, "cost"]
"#,
    },
];

/// The line drawn for each background task when the config has no
/// `[subagent]` section: what it is doing on the left, kept longest; its
/// context, running time, model and token trend on the right.
const SUBAGENT: &str = r#"
left = [{ metric = "task.label", icon = "", priority = 4 }]
right = [
  { metric = "task.trend", priority = 1 },
  { metric = "context", widget = "text", format = "{pct}% · {tokens}", compact = "{tokens}", priority = 3 },
  { metric = "model", compact = "{family}", priority = 2 },
  { metric = "task.elapsed", priority = 3 },
]
"#;

pub fn default_subagent() -> LineConfig {
    toml::from_str(SUBAGENT).expect("built-in subagent line parses")
}

/// The preset `name` as a full config, or `None` if there is none.
pub fn get(name: &str) -> Option<Config> {
    let preset = PRESETS.iter().find(|p| p.name == name)?;
    let mut config: Config = toml::from_str(preset.toml).expect("built-in presets parse");
    config.preset = Some(preset.name.to_string());
    Some(config)
}

/// What applying a preset replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parts {
    /// Lines and look.
    All,
    /// The `[[line]]` layout only.
    Layout,
    /// Skin, fill, palette and its overrides, glyphs, color depth and
    /// separator; the old palette's overrides go with it.
    Style,
}

/// `config` with `preset` applied. Settings about the user's setup rather
/// than the look (width, padding, git, updates, the subagent line) stay.
pub fn apply(config: &Config, preset: &Config, parts: Parts) -> Config {
    let mut out = config.clone();
    if parts != Parts::Style {
        out.lines = preset.lines.clone();
    }
    if parts != Parts::Layout {
        out.palette = preset.palette.clone();
        out.skin = preset.skin;
        out.fill = preset.fill;
        out.glyphs = preset.glyphs;
        out.color_depth = preset.color_depth;
        out.separator = preset.separator.clone();
        out.powerline = preset.powerline;
        out.palette_overrides = preset.palette_overrides.clone();
    }
    if parts == Parts::All {
        out.preset = preset.preset.clone();
    }
    out
}

/// Whether `config` still looks exactly like `preset`.
pub fn matches(config: &Config, preset: &Config) -> bool {
    apply(config, preset, Parts::All) == *config
}

/// The config used when the user has none.
pub fn default_config() -> Config {
    get(DEFAULT).expect("default preset exists")
}

/// Longest name a preset of the user's may have, in characters.
pub const NAME_MAX: usize = 32;

/// A typed name as a preset's file name: lowercase letters and digits,
/// with dashes between words.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for ch in text.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
        } else if matches!(ch, ' ' | '-' | '_' | '.') && !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Why `name` (a slug) can't name a new preset, if it can't.
pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("Give it a name.".into());
    }
    if name.chars().count() > NAME_MAX {
        return Some(format!("Keep the name to {NAME_MAX} characters."));
    }
    if PRESETS.iter().any(|p| p.name == name) {
        return Some(format!("{name} is a built-in preset; pick another name."));
    }
    None
}

/// Where a preset comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    Builtin,
    /// A file in the user's `presets/`.
    Mine {
        path: PathBuf,
        modified: Option<SystemTime>,
        /// Named like a built-in, which it replaces.
        replaces: bool,
    },
}

/// A preset as the gallery lists it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub summary: String,
    pub tags: Vec<&'static str>,
    pub origin: Origin,
    /// The preset as a full config, or why its file can't be used.
    pub config: Result<Config, String>,
}

impl Entry {
    pub fn is_mine(&self) -> bool {
        matches!(self.origin, Origin::Mine { .. })
    }
}

/// What a preset file holds: the parts `apply` copies, and a summary. The
/// fields keep `Config`'s order, so a preset reads like a `config.toml`.
#[derive(Serialize)]
struct PresetFile<'a> {
    version: u32,
    #[serde(skip_serializing_if = "str::is_empty")]
    summary: &'a str,
    palette: &'a str,
    skin: SkinKind,
    fill: FillStyle,
    glyphs: GlyphTier,
    color_depth: DepthSetting,
    #[serde(skip_serializing_if = "Option::is_none")]
    separator: Option<&'a str>,
    powerline: Edges,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    palette_overrides: &'a BTreeMap<String, String>,
    #[serde(rename = "line")]
    lines: &'a [LineConfig],
}

const FILE_HEADER: &str = "# A ccline preset: the status line's layout and look. Applying it leaves\n\
                           # width, git, update and subagent line settings as they are.\n\n";

fn file_text(summary: &str, c: &Config) -> String {
    let file = PresetFile {
        version: crate::config::VERSION,
        summary,
        palette: &c.palette,
        skin: c.skin,
        fill: c.fill,
        glyphs: c.glyphs,
        color_depth: c.color_depth,
        separator: c.separator.as_deref(),
        powerline: c.powerline,
        palette_overrides: &c.palette_overrides,
        lines: &c.lines,
    };
    format!(
        "{FILE_HEADER}{}",
        toml::to_string_pretty(&file).unwrap_or_default()
    )
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Head {
    summary: String,
}

/// A preset file's summary and config, the config named `name`.
fn read_file(path: &Path, name: &str) -> Result<(String, Config), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let table: toml::Table = toml::from_str(&text).map_err(|e| e.message().to_string())?;
    let head = Head::deserialize(toml::Value::Table(table.clone())).unwrap_or_default();
    let mut config =
        Config::deserialize(toml::Value::Table(table)).map_err(|e| e.message().to_string())?;
    // Applying a preset without lines would leave the status line empty.
    if config.lines.is_empty() {
        return Err("it has no [[line]]".into());
    }
    config.preset = Some(name.to_string());
    Ok((head.summary, config))
}

/// Filter tags for a preset of the user's, read from what it shows.
fn tags_of(c: &Config) -> Vec<&'static str> {
    let ids: Vec<String> = c
        .lines
        .iter()
        .flat_map(|l| l.left.iter().chain(&l.right))
        .map(|i| i.config().metric.clone())
        .collect();
    let mut tags = vec!["mine"];
    if c.lines.len() > 1 {
        tags.push("multi-line");
    }
    if c.skin == SkinKind::Powerline {
        tags.push("powerline");
    }
    if ids.iter().any(|m| m.starts_with("quota.")) {
        tags.push("quota");
    }
    if ids.iter().any(|m| m == "cache" || m.starts_with("cache.")) {
        tags.push("cache");
    }
    tags
}

/// The presets on offer, the user's first, and the changes made to theirs.
/// Every change is written at once: the files are the library.
pub struct Library {
    dir: PathBuf,
    entries: Vec<Entry>,
    /// Bumped whenever the entries are read again, so views can follow.
    generation: u64,
    /// The preset the last change made, renamed or replaced.
    last: Option<String>,
}

impl Library {
    pub fn load(dir: PathBuf) -> Library {
        let mut lib = Library {
            dir,
            entries: Vec::new(),
            generation: 0,
            last: None,
        };
        lib.refresh();
        lib
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn last(&self) -> Option<&str> {
        self.last.as_deref()
    }

    /// Read the directory again, after changes made elsewhere.
    pub fn reload(&mut self) {
        self.last = None;
        self.refresh();
    }

    fn refresh(&mut self) {
        let mut mine = Vec::new();
        for e in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("toml") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let name = name.to_string();
            let (summary, config) = match read_file(&path, &name) {
                Ok((s, c)) => (s, Ok(c)),
                Err(e) => (String::new(), Err(e)),
            };
            mine.push(Entry {
                tags: config
                    .as_ref()
                    .map(tags_of)
                    .unwrap_or_else(|_| vec!["mine"]),
                origin: Origin::Mine {
                    modified: e.metadata().and_then(|m| m.modified()).ok(),
                    replaces: PRESETS.iter().any(|p| p.name == name),
                    path,
                },
                name,
                summary,
                config,
            });
        }
        mine.sort_by(|a, b| a.name.cmp(&b.name));
        let builtin: Vec<Entry> = PRESETS
            .iter()
            .filter(|p| !mine.iter().any(|m| m.name == p.name))
            .map(|p| Entry {
                name: p.name.to_string(),
                summary: p.summary.to_string(),
                tags: p.tags.to_vec(),
                origin: Origin::Builtin,
                config: Ok(get(p.name).expect("listed presets exist")),
            })
            .collect();
        self.entries = mine;
        self.entries.extend(builtin);
        self.generation += 1;
    }

    fn mine(&self, name: &str) -> Result<&Entry, String> {
        self.get(name)
            .filter(|e| e.is_mine())
            .ok_or_else(|| format!("{name} isn't one of your presets"))
    }

    fn taken(&self, name: &str) -> Result<(), String> {
        match self.get(name) {
            Some(e) if e.is_mine() => Err(format!("You already have {name}; W on it replaces it.")),
            _ => Ok(()),
        }
    }

    fn write(&self, path: &Path, summary: &str, config: &Config) -> Result<(), String> {
        crate::paths::write_atomic(path, &file_text(summary, config))
            .map_err(|e| format!("couldn't write {}: {e}", path.display()))
    }

    fn changed(&mut self, last: Option<&str>) {
        self.last = last.map(str::to_string);
        self.refresh();
    }

    /// Save `config`'s layout and look as a new preset.
    pub fn create(&mut self, name: &str, summary: &str, config: &Config) -> Result<(), String> {
        if let Some(problem) = name_problem(name) {
            return Err(problem);
        }
        self.taken(name)?;
        self.write(&self.dir.join(format!("{name}.toml")), summary, config)?;
        self.changed(Some(name));
        Ok(())
    }

    /// Replace one of the user's presets with `config`, keeping its summary.
    pub fn replace(&mut self, name: &str, config: &Config) -> Result<(), String> {
        let entry = self.mine(name)?;
        let Origin::Mine { path, .. } = &entry.origin else {
            unreachable!("mine() returns the user's presets")
        };
        self.write(path, &entry.summary, config)?;
        self.changed(Some(name));
        Ok(())
    }

    /// Rename one of the user's presets and set its summary.
    pub fn rename(&mut self, old: &str, new: &str, summary: &str) -> Result<(), String> {
        let entry = self.mine(old)?.clone();
        let Origin::Mine { path, .. } = &entry.origin else {
            unreachable!("mine() returns the user's presets")
        };
        let to = if new == old {
            path.clone()
        } else {
            if let Some(problem) = name_problem(new) {
                return Err(problem);
            }
            self.taken(new)?;
            self.dir.join(format!("{new}.toml"))
        };
        if summary == entry.summary {
            if to != *path {
                std::fs::rename(path, &to).map_err(|e| format!("couldn't rename {old}: {e}"))?;
            }
        } else {
            let config = entry
                .config
                .as_ref()
                .map_err(|e| format!("can't read {old}: {e}"))?;
            self.write(&to, summary, config)?;
            if to != *path {
                std::fs::remove_file(path).map_err(|e| format!("couldn't remove {old}: {e}"))?;
            }
        }
        self.changed(Some(new));
        Ok(())
    }

    /// Delete one of the user's presets.
    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let entry = self.mine(name)?;
        let Origin::Mine { path, .. } = &entry.origin else {
            unreachable!("mine() returns the user's presets")
        };
        std::fs::remove_file(path).map_err(|e| format!("couldn't delete {name}: {e}"))?;
        self.changed(None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics;

    #[test]
    fn applies_parts_and_keeps_the_setup() {
        let mut mine = get("classic").unwrap();
        mine.layout.reserve = 4;
        mine.git.cache_ms = 0;
        mine.palette_overrides
            .insert("warn".into(), "#ff0000".into());
        let dash = get("dashboard").unwrap();

        let all = apply(&mine, &dash, Parts::All);
        assert_eq!((all.lines.len(), all.skin), (3, dash.skin));
        assert_eq!(all.preset.as_deref(), Some("dashboard"));
        assert_eq!((all.layout.reserve, all.git.cache_ms), (4, 0));
        assert!(all.palette_overrides.is_empty());
        assert!(matches(&all, &dash));

        let layout = apply(&mine, &dash, Parts::Layout);
        assert_eq!(
            (&layout.lines, &layout.palette),
            (&dash.lines, &mine.palette)
        );
        assert_eq!(layout.preset.as_deref(), Some("classic"));

        let style = apply(&mine, &dash, Parts::Style);
        assert_eq!((&style.lines, &style.palette), (&mine.lines, &dash.palette));
        assert!(!matches(&style, &dash) && !matches(&mine, &dash));
    }

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
        let sub = default_subagent();
        for item in sub.left.iter().chain(&sub.right) {
            assert!(metrics::find(&item.config().metric).is_some());
        }
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ccline-presets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn names(lib: &Library) -> Vec<(&str, bool)> {
        lib.entries()
            .iter()
            .map(|e| (e.name.as_str(), e.is_mine()))
            .collect()
    }

    #[test]
    fn names_presets_like_files() {
        assert_eq!(slug("  Work Laptop "), "work-laptop");
        assert_eq!(slug("night_owl.v2"), "night-owl-v2");
        assert_eq!(slug("--a  --  b--"), "a-b");
        assert_eq!(slug("工作 布局"), "工作-布局");
        assert_eq!(slug("!!!"), "");
        assert!(name_problem("").is_some());
        assert!(name_problem("classic").unwrap().contains("built-in"));
        assert!(name_problem(&"x".repeat(NAME_MAX + 1)).is_some());
        assert_eq!(name_problem("work"), None);
    }

    #[test]
    fn saves_the_layout_and_look_and_reads_them_back() {
        let dir = temp("save");
        let mut lib = Library::load(dir.clone());
        assert!(lib.entries().iter().all(|e| !e.is_mine()));
        assert_eq!(lib.entries().len(), PRESETS.len());

        let mut mine = get("dashboard").unwrap();
        mine.palette = "nord".into();
        mine.palette_overrides
            .insert("warn".into(), "#ff8800".into());
        mine.layout.reserve = 6;
        mine.git.cache_ms = 0;
        mine.subagent = Some(default_subagent());
        lib.create("work", "Quota and cache", &mine).unwrap();
        assert_eq!(lib.last(), Some("work"));

        let text = std::fs::read_to_string(dir.join("work.toml")).unwrap();
        assert!(text.starts_with("# A ccline preset"), "{text}");
        assert!(text.contains("summary = \"Quota and cache\""), "{text}");
        for setup in ["[layout]", "[git]", "[update]", "[subagent]", "preset ="] {
            assert!(!text.contains(setup), "{setup} stays out: {text}");
        }

        let e = lib.get("work").unwrap();
        assert!(e.is_mine());
        assert_eq!(e.summary, "Quota and cache");
        assert_eq!(e.tags, ["mine", "multi-line", "quota", "cache"]);
        let back = e.config.as_ref().unwrap();
        assert_eq!(back.preset.as_deref(), Some("work"));
        // Applying it reproduces the look and layout it was saved from.
        let mut want = apply(&default_config(), &mine, Parts::All);
        want.preset = Some("work".into());
        assert_eq!(apply(&default_config(), back, Parts::All), want);
        assert_eq!(names(&lib)[0], ("work", true), "the user's come first");

        assert!(
            lib.create("work", "", &mine)
                .unwrap_err()
                .contains("already")
        );
        assert!(
            lib.create("classic", "", &mine)
                .unwrap_err()
                .contains("built-in")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaces_renames_and_deletes() {
        let dir = temp("edit");
        let mut lib = Library::load(dir.clone());
        lib.create("work", "Mine", &get("classic").unwrap())
            .unwrap();
        lib.replace("work", &get("zen").unwrap()).unwrap();
        let e = lib.get("work").unwrap();
        assert_eq!(e.summary, "Mine", "replacing keeps the summary");
        assert_eq!(e.config.as_ref().unwrap().glyphs, GlyphTier::Ascii);

        lib.rename("work", "home", "Mine").unwrap();
        assert!(lib.get("work").is_none() && !dir.join("work.toml").exists());
        assert_eq!(lib.last(), Some("home"));
        lib.rename("home", "home", "At home").unwrap();
        assert_eq!(lib.get("home").unwrap().summary, "At home");
        lib.rename("home", "office", "Office").unwrap();
        assert_eq!(lib.get("office").unwrap().summary, "Office");
        assert!(!dir.join("home.toml").exists());

        lib.create("other", "", &get("zen").unwrap()).unwrap();
        assert!(lib.rename("office", "other", "").is_err());
        assert!(
            lib.rename("classic", "x", "").is_err(),
            "built-ins don't change"
        );
        assert!(lib.replace("classic", &get("zen").unwrap()).is_err());
        assert!(lib.delete("minimal").is_err());

        lib.delete("office").unwrap();
        assert!(lib.get("office").is_none() && !dir.join("office.toml").exists());
        assert_eq!(lib.last(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lists_hand_written_files_and_their_problems() {
        let dir = temp("files");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.toml"), "palette = [").unwrap();
        std::fs::write(dir.join("look-only.toml"), "palette = \"nord\"\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a preset").unwrap();
        // Named like a built-in: it replaces it, as palette files do.
        std::fs::write(
            dir.join("minimal.toml"),
            "summary = \"My minimal\"\nskin = \"powerline\"\n[[line]]\nleft = [\"model\"]\n",
        )
        .unwrap();
        let lib = Library::load(dir.clone());
        let mine: Vec<_> = names(&lib).into_iter().filter(|(_, m)| *m).collect();
        assert_eq!(
            mine,
            [("broken", true), ("look-only", true), ("minimal", true)]
        );
        assert_eq!(lib.entries().len(), PRESETS.len() + 2);
        assert!(lib.get("broken").unwrap().config.is_err());
        assert!(
            lib.get("look-only")
                .unwrap()
                .config
                .as_ref()
                .unwrap_err()
                .contains("[[line]]")
        );
        let min = lib.get("minimal").unwrap();
        assert!(matches!(min.origin, Origin::Mine { replaces: true, .. }));
        assert_eq!(min.tags, ["mine", "powerline"]);
        assert_eq!(min.config.as_ref().unwrap().skin, SkinKind::Powerline);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
