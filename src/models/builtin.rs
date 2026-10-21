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

/// Name a Claude model id, e.g. `claude-opus-4-8`, `claude-3-5-sonnet-20241022`,
/// `us.anthropic.claude-sonnet-4-5-20250929-v1:0`, `claude-sonnet-5@20250929`
/// or `anthropic/claude-opus-4.1`.
pub fn claude_name(id: &str) -> Option<String> {
    static FAMILY: OnceLock<Regex> = OnceLock::new();
    let re = FAMILY.get_or_init(|| {
        let kw = "opus|sonnet|haiku|fable|mythos";
        Regex::new(&format!(
            r"(?i)(?:(?P<pre_major>\d{{1,2}})(?:[.-](?P<pre_minor>\d{{1,2}}))?-(?P<pre_kw>{kw})|(?P<post_kw>{kw})-(?P<post_major>\d{{1,2}})(?:[.-](?P<post_minor>\d{{1,2}}))?)(?:-\d{{3,}}|-[a-z]|\[|@|:|$)"
        ))
        .expect("family regex compiles")
    });
    let caps = re.captures(id)?;
    let family = caps
        .name("post_kw")
        .or_else(|| caps.name("pre_kw"))?
        .as_str();
    let major = caps
        .name("post_major")
        .or_else(|| caps.name("pre_major"))?
        .as_str();
    let minor = caps.name("post_minor").or_else(|| caps.name("pre_minor"));
    let mut chars = family.chars();
    let family: String = chars
        .next()
        .map(|c| c.to_ascii_uppercase())
        .into_iter()
        .chain(chars.map(|c| c.to_ascii_lowercase()))
        .collect();
    Some(match minor {
        Some(m) => format!("{family} {major}.{}", m.as_str()),
        None => format!("{family} {major}"),
    })
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
