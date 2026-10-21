//! Built-in model knowledge: names for Claude model ids in any provider's
//! spelling, and windows for a few third-party models (kept from v1).

use regex::Regex;
use std::sync::OnceLock;

pub struct BuiltinRule {
    pub pattern: &'static str,
    pub name: &'static str,
    pub window: u64,
}

/// Substring rules, checked in order (more specific patterns first).
pub const RULES: &[BuiltinRule] = &[
    BuiltinRule {
        pattern: "kimi-k2-turbo",
        name: "Kimi K2 Turbo",
        window: 128_000,
    },
    BuiltinRule {
        pattern: "kimi-k2",
        name: "Kimi K2",
        window: 128_000,
    },
    BuiltinRule {
        pattern: "glm-4.5",
        name: "GLM-4.5",
        window: 128_000,
    },
    BuiltinRule {
        pattern: "qwen3-coder",
        name: "Qwen Coder",
        window: 256_000,
    },
];

pub fn rule_for(id_lower: &str) -> Option<&'static BuiltinRule> {
    RULES.iter().find(|r| id_lower.contains(r.pattern))
}

/// A Claude model's family and version, e.g. `opus` 4.8.
#[derive(Clone, Debug, PartialEq)]
pub struct ClaudeModel {
    /// Lowercase: `opus`, `sonnet`, `haiku`, `fable` or `mythos`.
    pub family: String,
    pub major: u32,
    pub minor: Option<u32>,
}

/// Parse a Claude model id in any provider's spelling, e.g.
/// `claude-opus-4-8`, `claude-3-5-sonnet-20241022`,
/// `us.anthropic.claude-sonnet-4-5-20250929-v1:0`, `claude-sonnet-5@20250929`
/// or `anthropic/claude-opus-4.1`.
pub fn claude_model(id: &str) -> Option<ClaudeModel> {
    static FAMILY: OnceLock<Regex> = OnceLock::new();
    let re = FAMILY.get_or_init(|| {
        let kw = "opus|sonnet|haiku|fable|mythos";
        Regex::new(&format!(
            r"(?i)(?:(?P<pre_major>\d{{1,2}})(?:[.-](?P<pre_minor>\d{{1,2}}))?-(?P<pre_kw>{kw})|(?P<post_kw>{kw})-(?P<post_major>\d{{1,2}})(?:[.-](?P<post_minor>\d{{1,2}}))?)(?:-\d{{3,}}|-[a-z]|\[|@|:|$)"
        ))
        .expect("family regex compiles")
    });
    let caps = re.captures(id)?;
    let family = caps.name("post_kw").or_else(|| caps.name("pre_kw"))?;
    let major = caps.name("post_major").or_else(|| caps.name("pre_major"))?;
    let minor = caps.name("post_minor").or_else(|| caps.name("pre_minor"));
    Some(ClaudeModel {
        family: family.as_str().to_ascii_lowercase(),
        major: major.as_str().parse().ok()?,
        minor: minor.and_then(|m| m.as_str().parse().ok()),
    })
}

/// Display name of a Claude model id, e.g. `Opus 4.8`.
pub fn claude_name(id: &str) -> Option<String> {
    let m = claude_model(id)?;
    let mut chars = m.family.chars();
    let family: String = chars
        .next()
        .map(|c| c.to_ascii_uppercase())
        .into_iter()
        .chain(chars)
        .collect();
    Some(match m.minor {
        Some(minor) => format!("{family} {}.{minor}", m.major),
        None => format!("{family} {}", m.major),
    })
}

/// List price of uncached input tokens in USD per million, for estimates.
/// Cache writes cost 1.25× this with the 5-minute TTL and 2× with 1 hour.
pub fn input_price(id: &str) -> Option<f64> {
    let m = claude_model(id)?;
    let version = (m.major, m.minor.unwrap_or(0));
    match m.family.as_str() {
        "opus" if version >= (4, 5) => Some(5.0),
        "opus" => Some(15.0),
        "sonnet" => Some(3.0),
        "haiku" if version >= (4, 0) => Some(1.0),
        "haiku" if version >= (3, 5) => Some(0.8),
        "haiku" => Some(0.25),
        "fable" => Some(10.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_claude_ids_across_providers() {
        let cases = [
            ("claude-opus-4-8", "Opus 4.8"),
            ("claude-opus-5-5[1m]", "Opus 5.5"),
            ("claude-3-5-sonnet-20241022", "Sonnet 3.5"),
            ("claude-3-opus-20240229", "Opus 3"),
            ("claude-opus-4-20250514", "Opus 4"),
            ("claude-sonnet-4-5-20250929", "Sonnet 4.5"),
            ("us.anthropic.claude-sonnet-4-5-20250929-v1:0", "Sonnet 4.5"),
            ("claude-sonnet-5@20250929", "Sonnet 5"),
            ("anthropic/claude-opus-4.1", "Opus 4.1"),
            ("claude-haiku-4-5", "Haiku 4.5"),
            ("claude-fable-5", "Fable 5"),
            ("claude-sonnet-4-thinking", "Sonnet 4"),
        ];
        for (id, want) in cases {
            assert_eq!(claude_name(id).as_deref(), Some(want), "{id}");
        }
    }

    #[test]
    fn prices_by_family_and_version() {
        let cases = [
            ("claude-opus-4-8", Some(5.0)),
            ("claude-opus-4-1-20250805", Some(15.0)),
            ("claude-3-opus-20240229", Some(15.0)),
            ("claude-sonnet-5", Some(3.0)),
            ("claude-haiku-4-5", Some(1.0)),
            ("claude-3-5-haiku-20241022", Some(0.8)),
            ("claude-3-haiku-20240307", Some(0.25)),
            ("kimi-k2-turbo-preview", None),
        ];
        for (id, want) in cases {
            assert_eq!(input_price(id), want, "{id}");
        }
    }

    #[test]
    fn ignores_non_claude_ids() {
        assert_eq!(claude_name("kimi-k2-0905-preview"), None);
        assert_eq!(claude_name("gpt-5"), None);
    }

    #[test]
    fn prefers_specific_builtin_rules() {
        assert_eq!(
            rule_for("kimi-k2-turbo-preview").unwrap().name,
            "Kimi K2 Turbo"
        );
        assert_eq!(rule_for("moonshot/kimi-k2").unwrap().name, "Kimi K2");
        assert!(rule_for("deepseek-v4").is_none());
    }
}
