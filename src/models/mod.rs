//! The model registry: turns model ids Claude Code can't name or size into a
//! readable name and the real context window.

pub mod builtin;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    #[default]
    Contains,
    Prefix,
    Exact,
    Regex,
}

impl MatchKind {
    fn is_contains(&self) -> bool {
        *self == MatchKind::Contains
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub pattern: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_limit: Option<u64>,
    #[serde(
        rename = "match",
        default,
        skip_serializing_if = "MatchKind::is_contains"
    )]
    pub match_kind: MatchKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub pattern: String,
    #[serde(default)]
    pub display_suffix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_limit: Option<u64>,
}

/// Contents of `models.toml`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelsFile {
    #[serde(rename = "models", default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub context_modifiers: Vec<Modifier>,
}

/// Where the display name came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameSource {
    /// Index into the user's rules.
    UserRule(usize),
    ClaudeCode,
    Builtin,
    RawId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub name: String,
    pub source: NameSource,
    /// The context window to measure against, if known.
    pub window: Option<u64>,
    /// True when `window` differs from what Claude Code reported.
    pub window_corrected: bool,
}

struct CompiledRule {
    rule: Rule,
    regex: Option<Regex>,
}

pub struct Registry {
    rules: Vec<CompiledRule>,
    modifiers: Vec<Modifier>,
    /// Why `models.toml` could not be used, if it couldn't.
    pub load_error: Option<String>,
}

fn builtin_modifiers() -> Vec<Modifier> {
    vec![Modifier {
        pattern: "[1m]".into(),
        display_suffix: " 1M".into(),
        context_limit: Some(1_000_000),
    }]
}

impl Default for Registry {
    fn default() -> Self {
        Registry::from_file(ModelsFile::default())
    }
}

impl Rule {
    /// Whether this rule matches `id`, ignoring case. A regex that doesn't
    /// compile matches nothing (see `regex_error`).
    pub fn matches(&self, id: &str) -> bool {
        let (id, pattern) = (id.to_lowercase(), self.pattern.to_lowercase());
        match self.match_kind {
            MatchKind::Contains => id.contains(&pattern),
            MatchKind::Prefix => id.starts_with(&pattern),
            MatchKind::Exact => id == pattern,
            MatchKind::Regex => {
                Regex::new(&format!("(?i){}", self.pattern)).is_ok_and(|re| re.is_match(&id))
            }
        }
    }

    pub fn regex_error(&self) -> Option<String> {
        if self.match_kind != MatchKind::Regex {
            return None;
        }
        Regex::new(&format!("(?i){}", self.pattern))
            .err()
            .map(|e| e.to_string())
    }
}

impl ModelsFile {
    /// For each rule, the earlier rule that takes every id it would match,
    /// so it never applies. Judged on the rule's own pattern (for literal
    /// patterns) and on `samples` it matches, such as ids seen recently.
    pub fn shadowed(&self, samples: &[&str]) -> Vec<Option<usize>> {
        (0..self.rules.len())
            .map(|j| {
                let rule = &self.rules[j];
                let mut ids: Vec<&str> = samples
                    .iter()
                    .copied()
                    .filter(|id| rule.matches(id))
                    .collect();
                if rule.match_kind != MatchKind::Regex {
                    ids.push(&rule.pattern);
                }
                let first = |id: &str| (0..j).find(|&i| self.rules[i].matches(id));
                let by: Vec<Option<usize>> = ids.iter().map(|id| first(id)).collect();
                if by.is_empty() || by.iter().any(Option::is_none) {
                    None
                } else {
                    by.into_iter().flatten().min()
                }
            })
            .collect()
    }

    /// Read `path`; a missing file is an empty registry.
    pub fn load(path: &std::path::Path) -> Result<ModelsFile, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ModelsFile::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Write `path` atomically. Comments in a hand-edited file are not kept.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        crate::paths::write_atomic(path, &text)
    }
}

impl Registry {
    /// Load `models.toml`; a missing file is fine, a broken one falls back to
    /// the built-ins and records why in `load_error`.
    pub fn load() -> Self {
        match ModelsFile::load(&crate::paths::models_file()) {
            Ok(file) => Registry::from_file(file),
            Err(e) => Registry {
                load_error: Some(e),
                ..Registry::default()
            },
        }
    }

    pub fn from_file(file: ModelsFile) -> Self {
        let rules = file
            .rules
            .into_iter()
            .map(|rule| {
                let regex = (rule.match_kind == MatchKind::Regex)
                    .then(|| Regex::new(&format!("(?i){}", rule.pattern)).ok())
                    .flatten();
                CompiledRule { rule, regex }
            })
            .collect();
        let mut modifiers = file.context_modifiers;
        modifiers.extend(builtin_modifiers());
        Registry {
            rules,
            modifiers,
            load_error: None,
        }
    }

    pub fn resolve(&self, id: &str, cc_name: Option<&str>, cc_window: Option<u64>) -> Resolved {
        let id_lower = id.to_lowercase();
        let user = self
            .rules
            .iter()
            .enumerate()
            .find(|(_, r)| rule_matches(r, &id_lower));
        let modifier = self
            .modifiers
            .iter()
            .find(|m| id_lower.contains(&m.pattern.to_lowercase()));
        let builtin_rule = builtin::rule_for(&id_lower);

        let cc_base = cc_name
            .map(strip_context_suffix)
            .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(id));

        let (base, source) =
            if let Some((i, r)) = user.filter(|(_, r)| r.rule.display_name.is_some()) {
                (
                    r.rule.display_name.clone().unwrap_or_default(),
                    NameSource::UserRule(i),
                )
            } else if let Some(name) = cc_base {
                (name, NameSource::ClaudeCode)
            } else if let Some(name) = builtin::claude_name(id) {
                (name, NameSource::Builtin)
            } else if let Some(r) = builtin_rule {
                (r.name.to_string(), NameSource::Builtin)
            } else {
                (id.to_string(), NameSource::RawId)
            };

        let name = match modifier {
            Some(m)
                if !m.display_suffix.trim().is_empty()
                    && !base.ends_with(m.display_suffix.trim()) =>
            {
                format!("{base}{}", m.display_suffix)
            }
            _ => base,
        };

        let window = user
            .and_then(|(_, r)| r.rule.context_limit)
            .or(modifier.and_then(|m| m.context_limit))
            .or(builtin_rule.map(|r| r.window))
            .or(cc_window);

        Resolved {
            name,
            source,
            window,
            window_corrected: window.is_some() && window != cc_window,
        }
    }

    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter().map(|r| &r.rule)
    }

    /// Index of the first user rule that matches `id`.
    pub fn matching_rule(&self, id: &str) -> Option<usize> {
        let id_lower = id.to_lowercase();
        self.rules.iter().position(|r| rule_matches(r, &id_lower))
    }
}

/// Parse a context window written as `256000`, `256,000`, `256k`, `1m` or
/// `1.5M`.
pub fn parse_window(text: &str) -> Option<u64> {
    let t = text.trim().replace(['_', ','], "").to_lowercase();
    let (number, scale) = match t.strip_suffix('k') {
        Some(n) => (n, 1_000.0),
        None => match t.strip_suffix('m') {
            Some(n) => (n, 1_000_000.0),
            None => (t.as_str(), 1.0),
        },
    };
    let value = number.trim().parse::<f64>().ok()? * scale;
    (value >= 1.0 && value.is_finite()).then(|| value.round() as u64)
}

fn rule_matches(rule: &CompiledRule, id_lower: &str) -> bool {
    let pattern = rule.rule.pattern.to_lowercase();
    match rule.rule.match_kind {
        MatchKind::Contains => id_lower.contains(&pattern),
        MatchKind::Prefix => id_lower.starts_with(&pattern),
        MatchKind::Exact => id_lower == pattern,
        MatchKind::Regex => rule.regex.as_ref().is_some_and(|re| re.is_match(id_lower)),
    }
}

/// `Opus 5.5 (1M context)` → `Opus 5.5`; the registry's modifiers add the
/// window back in the house style.
fn strip_context_suffix(name: &str) -> String {
    static SUFFIX: OnceLock<Regex> = OnceLock::new();
    let re = SUFFIX.get_or_init(|| {
        Regex::new(r"(?i)\s*\([^)]*\bcontext\b[^)]*\)").expect("suffix regex compiles")
    });
    re.replace_all(name, "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(toml_text: &str) -> Registry {
        Registry::from_file(toml::from_str(toml_text).unwrap())
    }

    #[test]
    fn reads_rules_and_modifiers() {
        let r = registry(
            r#"
            [[models]]
            pattern = "kimi-k2-0905-preview"
            display_name = "Kimi K2 0905"
            context_limit = 256000

            [[context_modifiers]]
            pattern = "[1m]"
            display_suffix = " 1M"
            context_limit = 1000000
            "#,
        );
        let res = r.resolve(
            "kimi-k2-0905-preview",
            Some("kimi-k2-0905-preview"),
            Some(200_000),
        );
        assert_eq!(res.name, "Kimi K2 0905");
        assert_eq!(res.source, NameSource::UserRule(0));
        assert_eq!(res.window, Some(256_000));
        assert!(res.window_corrected);
    }

    fn rule(kind: MatchKind, pattern: &str) -> Rule {
        Rule {
            pattern: pattern.into(),
            display_name: None,
            context_limit: None,
            match_kind: kind,
        }
    }

    #[test]
    fn parses_windows() {
        assert_eq!(parse_window("256k"), Some(256_000));
        assert_eq!(parse_window("1m"), Some(1_000_000));
        assert_eq!(parse_window("1.5M"), Some(1_500_000));
        assert_eq!(parse_window(" 262,144 "), Some(262_144));
        assert_eq!(parse_window("128_000"), Some(128_000));
        assert_eq!(parse_window(""), None);
        assert_eq!(parse_window("big"), None);
        assert_eq!(parse_window("0"), None);
    }

    #[test]
    fn finds_rules_that_never_apply() {
        let file = ModelsFile {
            rules: vec![
                rule(MatchKind::Contains, "kimi-k2"),
                rule(MatchKind::Exact, "kimi-k2-0905"),
                rule(MatchKind::Prefix, "k3"),
                rule(MatchKind::Regex, "^k3-turbo"),
                rule(MatchKind::Regex, "^qwen"),
                rule(MatchKind::Regex, "(unclosed"),
            ],
            context_modifiers: Vec::new(),
        };
        let shadowed = file.shadowed(&["k3-turbo-preview", "kimi-k2-turbo"]);
        assert_eq!(shadowed, vec![None, Some(0), None, Some(2), None, None]);
        assert!(file.rules[3].matches("K3-Turbo-x"));
        assert!(file.rules[5].regex_error().is_some());
        assert!(!file.rules[5].matches("(unclosed"));
        let reg = Registry::from_file(file);
        assert_eq!(reg.matching_rule("kimi-k2-0905"), Some(0));
        assert_eq!(reg.matching_rule("gpt-5"), None);
    }

    #[test]
    fn saves_only_what_is_set() {
        let dir = std::env::temp_dir().join(format!("ccline-models-{}", std::process::id()));
        let path = dir.join("models.toml");
        assert_eq!(ModelsFile::load(&path), Ok(ModelsFile::default()));
        let file = ModelsFile {
            rules: vec![Rule {
                pattern: "kimi-k2-0905".into(),
                display_name: Some("Kimi K2 0905".into()),
                context_limit: Some(256_000),
                match_kind: MatchKind::Contains,
            }],
            context_modifiers: Vec::new(),
        };
        file.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("[[models]]") && !text.contains("match"),
            "{text}"
        );
        assert_eq!(ModelsFile::load(&path), Ok(file));
        std::fs::write(&path, "[[models]]\npattern = 3\n").unwrap();
        assert!(ModelsFile::load(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_claude_code_names_in_house_style() {
        let r = Registry::default();
        let res = r.resolve(
            "claude-opus-5-5[1m]",
            Some("Opus 5.5 (1M context)"),
            Some(1_000_000),
        );
        assert_eq!(res.name, "Opus 5.5 1M");
        assert_eq!(res.source, NameSource::ClaudeCode);
        assert_eq!(res.window, Some(1_000_000));
        assert!(!res.window_corrected);
    }

    #[test]
    fn falls_back_to_builtins_then_raw_id() {
        let r = Registry::default();
        let bedrock = r.resolve(
            "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
            None,
            Some(200_000),
        );
        assert_eq!(bedrock.name, "Sonnet 4.5");
        assert_eq!(bedrock.source, NameSource::Builtin);

        let qwen = r.resolve("qwen3-coder-plus", Some("qwen3-coder-plus"), Some(200_000));
        assert_eq!(
            (qwen.name.as_str(), qwen.window),
            ("Qwen Coder", Some(256_000))
        );
        assert!(qwen.window_corrected);

        let unknown = r.resolve("deepseek-v4-chat", Some("deepseek-v4-chat"), Some(200_000));
        assert_eq!(unknown.source, NameSource::RawId);
        assert_eq!(unknown.window, Some(200_000));
        assert!(!unknown.window_corrected);
    }

    #[test]
    fn supports_match_kinds() {
        let r = registry(
            r#"
            [[models]]
            pattern = "^k3-turbo"
            match = "regex"
            display_name = "Kimi K3 Turbo"
            [[models]]
            pattern = "k3"
            match = "prefix"
            context_limit = 128000
            [[models]]
            pattern = "exact-id"
            match = "exact"
            display_name = "Exact"
            "#,
        );
        assert_eq!(r.resolve("K3-Turbo-0", None, None).name, "Kimi K3 Turbo");
        let k3 = r.resolve("k3-preview", Some("k3-preview"), Some(200_000));
        assert_eq!((k3.name.as_str(), k3.window), ("k3-preview", Some(128_000)));
        assert_eq!(r.resolve("exact-id", None, None).name, "Exact");
        assert_eq!(
            r.resolve("exact-id-2", None, None).source,
            NameSource::RawId
        );
    }

    #[test]
    fn writes_minimal_toml() {
        let file = ModelsFile {
            rules: vec![Rule {
                pattern: "x".into(),
                display_name: Some("X".into()),
                context_limit: None,
                match_kind: MatchKind::Contains,
            }],
            context_modifiers: vec![],
        };
        let text = toml::to_string(&file).unwrap();
        assert!(!text.contains("match"));
        assert!(!text.contains("context_limit"));
        assert_eq!(toml::from_str::<ModelsFile>(&text).unwrap(), file);
    }
}
