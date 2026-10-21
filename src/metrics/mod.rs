//! Metrics: named data sources. Each one reads the input (and later state or
//! the mod bridge) and produces a `Reading` — text fields plus optional
//! numbers for graphs — without knowing how it will be displayed.

mod code;
pub mod fmt;
mod identity;
mod usage;

use crate::env::AutoCompact;
use crate::git::GitInfo;
use crate::input::Input;
use crate::models::{Registry, Resolved};
use crate::state::StateView;
use crate::style::{Category, GlyphTier, IconKey, Role, Severity};
use crate::version::Version;
use crate::widgets::WidgetKind;
use chrono::FixedOffset;
use std::cell::OnceCell;
use std::path::Path;

/// Per-item metric options from the config (e.g. `dir`'s `style`).
pub type Options = toml::Table;

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub text: String,
    pub role: Option<Role>,
}

/// What a metric produced for this render.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Values for text templates, e.g. `pct` → `62`.
    pub fields: Vec<(&'static str, Field)>,
    /// Fill level for bars and gauges, 0..1 (may exceed 1).
    pub ratio: Option<f64>,
    /// Share of a time window already passed, 0..1, for pace bars.
    pub elapsed: Option<f64>,
    /// History for sparklines, oldest first.
    pub series: Option<Vec<f64>>,
    /// Severity decided by the metric itself; otherwise thresholds on `ratio`.
    pub severity: Option<Severity>,
    /// Replaces the metric's default icon.
    pub icon: Option<IconKey>,
    pub link: Option<String>,
}

impl Reading {
    /// Set a text field; empty values are left unset so optional template
    /// groups that use them disappear.
    pub fn set(self, name: &'static str, text: impl Into<String>) -> Self {
        self.set_with(name, text, None)
    }

    pub fn set_role(self, name: &'static str, text: impl Into<String>, role: Role) -> Self {
        self.set_with(name, text, Some(role))
    }

    fn set_with(mut self, name: &'static str, text: impl Into<String>, role: Option<Role>) -> Self {
        let text = text.into();
        if !text.is_empty() {
            self.fields.retain(|(n, _)| *n != name);
            self.fields.push((name, Field { text, role }));
        }
        self
    }

    pub fn get(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|(n, _)| *n == name).map(|(_, f)| f)
    }

    pub fn ratio(mut self, ratio: f64) -> Self {
        self.ratio = Some(ratio);
        self
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    pub fn link(mut self, link: Option<String>) -> Self {
        self.link = link;
        self
    }
}

/// Default thresholds on a ratio, in percent.
pub const LEVELS: &[(f64, Severity)] = &[(50.0, Severity::Warn), (80.0, Severity::Crit)];

pub struct MetricMeta {
    pub id: &'static str,
    pub category: Category,
    pub name: &'static str,
    pub summary: &'static str,
    pub icon: IconKey,
    pub widget: WidgetKind,
    /// Default text templates for the full, compact and minimal levels.
    pub templates: [&'static str; 3],
    /// Fields the templates may use.
    pub fields: &'static [&'static str],
    pub levels: &'static [(f64, Severity)],
    /// Oldest Claude Code that provides the data.
    pub min_cc: Option<Version>,
    pub needs_mod: bool,
    pub stateful: bool,
    pub priority: u8,
}

pub struct Metric {
    pub meta: MetricMeta,
    pub compute: fn(&Ctx, &Options) -> Option<Reading>,
}

/// Where git status comes from.
pub enum GitSource {
    /// Run git in the working directory, once per render.
    Live,
    /// Run git, reusing a cached result younger than this many milliseconds.
    Cached(u64),
    /// Fixed data, for previews and tests.
    Fixed(Option<GitInfo>),
}

/// Everything metrics may read during one render.
pub struct Ctx<'a> {
    pub input: &'a Input,
    pub registry: &'a Registry,
    /// Unix epoch seconds.
    pub now: i64,
    pub tz: FixedOffset,
    pub glyphs: GlyphTier,
    pub cc: Option<Version>,
    /// History and cross-session data; empty when state is unavailable.
    pub state: StateView,
    pub compact: AutoCompact,
    git: GitSource,
    git_cache: OnceCell<Option<GitInfo>>,
    model_cache: OnceCell<Option<Resolved>>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        input: &'a Input,
        registry: &'a Registry,
        now: i64,
        tz: FixedOffset,
        glyphs: GlyphTier,
    ) -> Self {
        Ctx {
            input,
            registry,
            now,
            tz,
            glyphs,
            cc: input.cc_version(),
            state: StateView::default(),
            compact: AutoCompact::default(),
            git: GitSource::Live,
            git_cache: OnceCell::new(),
            model_cache: OnceCell::new(),
        }
    }

    pub fn with_git(mut self, git: GitSource) -> Self {
        self.git = git;
        self
    }

    pub fn with_state(mut self, state: StateView) -> Self {
        self.state = state;
        self
    }

    pub fn with_compact(mut self, compact: AutoCompact) -> Self {
        self.compact = compact;
        self
    }

    pub fn git(&self) -> Option<&GitInfo> {
        self.git_cache
            .get_or_init(|| {
                let dir = || self.input.current_dir().map(Path::new);
                match &self.git {
                    GitSource::Fixed(info) => info.clone(),
                    GitSource::Live => crate::git::status(dir()?),
                    GitSource::Cached(ttl) => crate::state::git_cache::status(dir()?, *ttl),
                }
            })
            .as_ref()
    }

    /// The model as the registry resolves it.
    pub fn model(&self) -> Option<&Resolved> {
        self.model_cache
            .get_or_init(|| {
                let model = self.input.model.as_ref()?;
                let id = model.id.as_deref().or(model.display_name.as_deref())?;
                let window = self
                    .input
                    .context_window
                    .as_ref()
                    .and_then(|c| c.context_window_size);
                Some(
                    self.registry
                        .resolve(id, model.display_name.as_deref(), window),
                )
            })
            .as_ref()
    }

    /// Whether the running Claude Code is at least `min`; unknown versions
    /// are assumed new enough and left to field presence.
    pub fn supports(&self, min: Version) -> bool {
        self.cc.is_none_or(|v| v >= min)
    }
}

pub fn catalog() -> &'static [Metric] {
    static CATALOG: std::sync::OnceLock<Vec<Metric>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut all = Vec::new();
        all.extend(identity::metrics());
        all.extend(usage::metrics());
        all.extend(code::metrics());
        all
    })
}

pub fn find(id: &str) -> Option<&'static Metric> {
    catalog().iter().find(|m| m.meta.id == id)
}

/// Compute `metric` if the running Claude Code supports it.
pub fn read(metric: &Metric, ctx: &Ctx, opts: &Options) -> Option<Reading> {
    if metric.meta.min_cc.is_some_and(|min| !ctx.supports(min)) {
        return None;
    }
    (metric.compute)(ctx, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_templates_use_declared_fields() {
        let mut seen = std::collections::HashSet::new();
        for m in catalog() {
            assert!(seen.insert(m.meta.id), "duplicate {}", m.meta.id);
            for t in m.meta.templates {
                let mut rest = t;
                while let Some(start) = rest.find('{') {
                    let end = rest[start..].find('}').expect("closed field") + start;
                    let name = &rest[start + 1..end];
                    assert!(
                        m.meta.fields.contains(&name),
                        "{}: {name} not declared",
                        m.meta.id
                    );
                    rest = &rest[end + 1..];
                }
            }
        }
    }

    #[test]
    fn gates_on_claude_code_version() {
        let input = Input::parse(r#"{"version": "2.1.200", "prompt_cache": {"warm": true, "caching_observed": true, "ttl": "5m", "expires_at": 9999999999}}"#).unwrap();
        let reg = Registry::default();
        let ctx = Ctx::new(
            &input,
            &reg,
            0,
            FixedOffset::east_opt(0).unwrap(),
            GlyphTier::Unicode,
        );
        let cache = find("cache").unwrap();
        assert!(read(cache, &ctx, &Options::new()).is_none());
    }
}
