//! Registry of per-segment options and helpers to edit them from the CLI.

use super::{Config, SegmentConfig, SegmentId};
use serde_json::Value;

/// Description of an option a segment understands
pub struct OptionSpec {
    pub key: &'static str,
    pub default: &'static str,
    pub description: &'static str,
    /// Allowed values when the option is an enumeration (empty: free-form)
    pub choices: &'static [&'static str],
}

impl OptionSpec {
    pub fn is_bool(&self) -> bool {
        self.default == "true" || self.default == "false"
    }
}

/// Options each segment reads from its `[segments.options]` table
pub fn known_options(id: SegmentId) -> &'static [OptionSpec] {
    match id {
        SegmentId::Model => &[OptionSpec {
            key: "show_effort",
            default: "true",
            description: "Append the effort level reported by Claude Code (e.g. \"· high\")",
            choices: &[],
        }],
        SegmentId::Usage => &[
            OptionSpec {
                key: "reset_format",
                default: "time",
                description: "\"time\" (03:20 / Thu 00h) or \"countdown\" (4h 52m / 6d 3h)",
                choices: &["time", "countdown"],
            },
            OptionSpec {
                key: "show_weekly",
                default: "true",
                description: "Show the weekly usage block",
                choices: &[],
            },
            OptionSpec {
                key: "cache_duration",
                default: "300",
                description: "Seconds to cache usage API responses",
                choices: &[],
            },
            OptionSpec {
                key: "timeout",
                default: "2",
                description: "Usage API request timeout in seconds",
                choices: &[],
            },
            OptionSpec {
                key: "api_base_url",
                default: "https://api.anthropic.com",
                description: "Usage API base URL (override for proxies)",
                choices: &[],
            },
        ],
        SegmentId::Credits => &[OptionSpec {
            key: "show_percent",
            default: "true",
            description: "Append the used percentage of the monthly credit limit",
            choices: &[],
        }],
        SegmentId::Git => &[OptionSpec {
            key: "show_sha",
            default: "false",
            description: "Show the short commit SHA next to the branch",
            choices: &[],
        }],
        _ => &[],
    }
}

/// Option keys of a segment in display order: known options first, then any
/// custom keys present in the config (sorted).
pub fn option_keys(segment: &SegmentConfig) -> Vec<String> {
    let specs = known_options(segment.id);
    let mut keys: Vec<String> = specs.iter().map(|o| o.key.to_string()).collect();
    let mut extra: Vec<String> = segment
        .options
        .keys()
        .filter(|k| !specs.iter().any(|o| o.key == k.as_str()))
        .cloned()
        .collect();
    extra.sort();
    keys.extend(extra);
    keys
}

/// Current value of an option as text, and whether it comes from the default
pub fn current_value(segment: &SegmentConfig, key: &str) -> (String, bool) {
    match segment.options.get(key) {
        Some(v) => (display_value(v), false),
        None => (
            known_options(segment.id)
                .iter()
                .find(|o| o.key == key)
                .map(|o| o.default.to_string())
                .unwrap_or_default(),
            true,
        ),
    }
}

/// What pressing Enter on an option does in the TUI
pub enum OptionEdit {
    /// Value was switched in place (bool toggle or next choice); holds the new value
    Cycled(String),
    /// Free-form value: open a text input pre-filled with this value
    Prompt(String),
}

/// Toggle a bool, advance an enumeration (`delta` = +1 next / -1 previous),
/// or ask for free-form input
pub fn cycle_option(segment: &mut SegmentConfig, key: &str, delta: i32) -> OptionEdit {
    let (current, _) = current_value(segment, key);
    let spec = known_options(segment.id).iter().find(|o| o.key == key);
    let next = match spec {
        Some(spec) if spec.is_bool() => {
            Some(if current == "true" { "false" } else { "true" }.to_string())
        }
        Some(spec) if !spec.choices.is_empty() => {
            let len = spec.choices.len() as i32;
            let idx = spec.choices.iter().position(|c| *c == current).unwrap_or(0) as i32;
            Some(spec.choices[((idx + delta).rem_euclid(len)) as usize].to_string())
        }
        _ => match current.as_str() {
            "true" => Some("false".to_string()),
            "false" => Some("true".to_string()),
            _ => None,
        },
    };
    match next {
        Some(value) => {
            segment.options.insert(key.to_string(), parse_value(&value));
            OptionEdit::Cycled(value)
        }
        None => OptionEdit::Prompt(current),
    }
}

/// All segment ids, in display order
pub const ALL_SEGMENTS: [SegmentId; 10] = [
    SegmentId::Model,
    SegmentId::Directory,
    SegmentId::Git,
    SegmentId::ContextWindow,
    SegmentId::Usage,
    SegmentId::Credits,
    SegmentId::Cost,
    SegmentId::Session,
    SegmentId::OutputStyle,
    SegmentId::Update,
];

/// Segment id as written in config.toml (snake_case)
pub fn segment_name(id: SegmentId) -> String {
    serde_json::to_value(id)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

pub fn parse_segment_id(name: &str) -> Option<SegmentId> {
    serde_json::from_value(Value::String(name.trim().to_lowercase())).ok()
}

/// Parse a CLI value: booleans and numbers become typed, anything else stays a string
pub fn parse_value(raw: &str) -> Value {
    let raw = raw.trim();
    match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => raw
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| raw.parse::<f64>().map(Value::from))
            .unwrap_or_else(|_| Value::String(raw.to_string())),
    }
}

/// Render a stored option value as the user would type it
pub fn display_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Parse "segment.key" into its parts
fn split_target(target: &str) -> Result<(SegmentId, String), String> {
    let (segment, key) = target
        .split_once('.')
        .ok_or_else(|| format!("expected SEGMENT.KEY, got \"{}\"", target))?;
    let id = parse_segment_id(segment).ok_or_else(|| {
        format!(
            "unknown segment \"{}\" (expected one of: {})",
            segment,
            ALL_SEGMENTS.map(segment_name).join(", ")
        )
    })?;
    let key = key.trim();
    if key.is_empty() {
        return Err(format!("missing option key in \"{}\"", target));
    }
    Ok((id, key.to_string()))
}

/// Apply `SEGMENT.KEY=VALUE`. Returns a human-readable summary of the change.
pub fn set_option(config: &mut Config, assignment: &str) -> Result<String, String> {
    let (target, raw_value) = assignment
        .split_once('=')
        .ok_or_else(|| format!("expected SEGMENT.KEY=VALUE, got \"{}\"", assignment))?;
    let (id, key) = split_target(target)?;
    let value = parse_value(raw_value);

    let segment = config
        .segments
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("segment \"{}\" is not in the config", segment_name(id)))?;
    segment.options.insert(key.clone(), value.clone());

    let known = known_options(id).iter().any(|o| o.key == key);
    let warning = if known {
        ""
    } else {
        " (warning: not a known option for this segment)"
    };
    Ok(format!(
        "{}.{} = {}{}",
        segment_name(id),
        key,
        display_value(&value),
        warning
    ))
}

/// Remove `SEGMENT.KEY` so the segment falls back to its default
pub fn unset_option(config: &mut Config, target: &str) -> Result<String, String> {
    let (id, key) = split_target(target)?;
    let segment = config
        .segments
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("segment \"{}\" is not in the config", segment_name(id)))?;
    match segment.options.remove(&key) {
        Some(_) => Ok(format!("{}.{} reset to default", segment_name(id), key)),
        None => Ok(format!("{}.{} was not set", segment_name(id), key)),
    }
}

/// Human-readable listing of every option, with current and default values
pub fn render_options(config: &Config) -> String {
    let mut out = String::new();
    for id in ALL_SEGMENTS {
        let specs = known_options(id);
        let segment = config.segments.iter().find(|s| s.id == id);
        let extra: Vec<(&String, &Value)> = segment
            .map(|s| {
                s.options
                    .iter()
                    .filter(|(k, _)| !specs.iter().any(|o| o.key == k.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        if specs.is_empty() && extra.is_empty() {
            continue;
        }
        let state = match segment {
            Some(s) if s.enabled => "enabled",
            Some(_) => "disabled",
            None => "not in config",
        };
        out.push_str(&format!("[{}] ({})\n", segment_name(id), state));
        for spec in specs {
            let current = segment
                .and_then(|s| s.options.get(spec.key))
                .map(display_value);
            let value = match &current {
                Some(v) => format!("{} (default: {})", v, spec.default),
                None => format!("{} (default)", spec.default),
            };
            out.push_str(&format!(
                "  {:<16} {:<40} {}\n",
                spec.key, value, spec.description
            ));
        }
        for (key, value) in extra {
            out.push_str(&format!(
                "  {:<16} {:<40} (custom)\n",
                key,
                display_value(value)
            ));
        }
    }
    out.push_str("\nChange with: ccline --set SEGMENT.KEY=VALUE   (e.g. --set usage.reset_format=countdown)\n");
    out.push_str("Reset with:  ccline --unset SEGMENT.KEY\n");
    out
}
