//! The model registry: turns model ids Claude Code can't name or size into a
//! readable name and the real context window.
//!
//! `models.toml` keeps the v1 format; v2 only adds optional fields.

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

impl Registry {
    /// Load `models.toml`; a missing file is fine, a broken one falls back to
    /// the built-ins and records why in `load_error`.
    pub fn load() -> Self {
        let path = crate::paths::models_file();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return Registry::default(),
        };
        match toml::from_str::<ModelsFile>(&text) {
            Ok(file) => Registry::from_file(file),
            Err(e) => Registry {
                load_error: Some(format!("{}: {e}", path.display())),
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
    fn reads_v1_files() {
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
