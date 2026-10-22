//! The inspector: the selected item's settings, each shown with its value
//! and changed by cycling, typing or a dialog.

use crate::config::ItemConfig;
use crate::metrics::{MetricMeta, OptKind, OptSpec};
use crate::style::color::ColorDepth;
use crate::style::glyphs::{self, BarGlyphs, GlyphTier};
use crate::style::palette::{Palette, TOKENS, Token};
use crate::style::{Color, Severity};
use crate::tui::ui::{self, TextInput};
use crate::widgets::{Coloring, Level, WidgetKind};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The metric's own option, by index into its `options`.
    Opt(usize),
    Widget,
    Bar,
    Cells,
    Template(Level),
    Icon,
    Color,
    Levels,
    Coloring,
    Priority,
}

/// What Enter does on a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Step to the next value.
    Cycle,
    /// Start typing.
    Text,
    /// Open that field's dialog.
    Widget,
    Icon,
    Color,
    Levels,
}

const WIDGETS: [WidgetKind; 5] = [
    WidgetKind::Text,
    WidgetKind::Bar,
    WidgetKind::Gauge,
    WidgetKind::Sparkline,
    WidgetKind::Pace,
];
const BARS: [BarGlyphs; 6] = [
    BarGlyphs::Shapes,
    BarGlyphs::Eighths,
    BarGlyphs::Blocks,
    BarGlyphs::Dots,
    BarGlyphs::Braille,
    BarGlyphs::Ascii,
];
const COLORINGS: [Coloring; 2] = [Coloring::Steps, Coloring::Gradient];
const CELLS: (u8, u8, u8) = (2, 10, 40);
/// Label column width.
pub const LABEL: usize = 11;

fn lower<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}").to_lowercase()
}

fn step<T: PartialEq + Copy>(all: &[T], current: T, forward: bool) -> T {
    let i = all.iter().position(|v| *v == current).unwrap_or(0);
    let n = all.len();
    all[(i + if forward { 1 } else { n - 1 }) % n]
}

/// `None` when `value` is the default, so the config only keeps changes.
fn unless_default<T: PartialEq>(value: T, default: T) -> Option<T> {
    (value != default).then_some(value)
}

pub fn sev_name(s: Severity) -> &'static str {
    match s {
        Severity::Normal => "normal",
        Severity::Good => "ok",
        Severity::Warn => "warn",
        Severity::Crit => "crit",
    }
}

pub fn fields(cfg: &ItemConfig, meta: &MetricMeta) -> Vec<Field> {
    let kind = cfg.widget.unwrap_or(meta.widget);
    let mut f: Vec<Field> = (0..meta.options.len()).map(Field::Opt).collect();
    f.push(Field::Widget);
    if kind == WidgetKind::Bar {
        f.push(Field::Bar);
    }
    if matches!(
        kind,
        WidgetKind::Bar | WidgetKind::Sparkline | WidgetKind::Pace
    ) {
        f.push(Field::Cells);
    }
    f.extend(Level::ALL.map(Field::Template));
    f.push(Field::Icon);
    f.push(Field::Color);
    if !meta.levels.is_empty() || cfg.levels.is_some() {
        f.push(Field::Levels);
    }
    if kind == WidgetKind::Bar {
        f.push(Field::Coloring);
    }
    f.push(Field::Priority);
    f
}

pub fn label(field: Field, meta: &MetricMeta) -> String {
    match field {
        Field::Opt(i) => meta.options[i].key.to_string(),
        Field::Widget => "Widget".into(),
        Field::Bar => "Bar".into(),
        Field::Cells => "Cells".into(),
        Field::Template(Level::Full) => "Full".into(),
        Field::Template(Level::Compact) => "Compact".into(),
        Field::Template(Level::Minimal) => "Minimal".into(),
        Field::Icon => "Icon".into(),
        Field::Color => "Color".into(),
        Field::Levels => "Levels".into(),
        Field::Coloring => "Coloring".into(),
        Field::Priority => "Priority".into(),
    }
}

pub fn change(field: Field, meta: &MetricMeta) -> Change {
    match field {
        Field::Opt(i) => match meta.options[i].kind {
            OptKind::Text | OptKind::Number => Change::Text,
            OptKind::Flag | OptKind::Choice(_) => Change::Cycle,
        },
        Field::Template(_) => Change::Text,
        Field::Widget => Change::Widget,
        Field::Icon => Change::Icon,
        Field::Color => Change::Color,
        Field::Levels => Change::Levels,
        Field::Bar | Field::Cells | Field::Coloring | Field::Priority => Change::Cycle,
    }
}

/// Whether ←→ step through the field's values.
pub fn cycles(field: Field, meta: &MetricMeta) -> bool {
    match field {
        Field::Opt(i) => matches!(meta.options[i].kind, OptKind::Flag | OptKind::Choice(_)),
        Field::Widget
        | Field::Bar
        | Field::Cells
        | Field::Color
        | Field::Coloring
        | Field::Priority => true,
        Field::Template(_) | Field::Icon | Field::Levels => false,
    }
}

fn template_mut(cfg: &mut ItemConfig, level: Level) -> &mut Option<String> {
    match level {
        Level::Full => &mut cfg.format,
        Level::Compact => &mut cfg.compact,
        Level::Minimal => &mut cfg.minimal,
    }
}

fn template(cfg: &ItemConfig, level: Level) -> Option<&String> {
    match level {
        Level::Full => cfg.format.as_ref(),
        Level::Compact => cfg.compact.as_ref(),
        Level::Minimal => cfg.minimal.as_ref(),
    }
}

fn opt<'a>(cfg: &'a ItemConfig, spec: &OptSpec) -> Option<&'a toml::Value> {
    cfg.options.as_ref()?.get(spec.key)
}

fn set_opt(cfg: &mut ItemConfig, spec: &OptSpec, value: Option<toml::Value>) {
    let table = cfg.options.get_or_insert_with(toml::Table::new);
    match value {
        Some(v) => {
            table.insert(spec.key.to_string(), v);
        }
        None => {
            table.remove(spec.key);
        }
    }
}

fn opt_text(cfg: &ItemConfig, spec: &OptSpec) -> String {
    match opt(cfg, spec) {
        Some(toml::Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => spec.default.to_string(),
    }
}

/// The token or color an item is drawn in, by name.
fn color_name(cfg: &ItemConfig) -> Option<&str> {
    cfg.color.as_deref()
}

/// Whether the item sets this field itself.
pub fn is_set(field: Field, cfg: &ItemConfig, meta: &MetricMeta) -> bool {
    match field {
        Field::Opt(i) => opt(cfg, &meta.options[i]).is_some(),
        Field::Widget => cfg.widget.is_some(),
        Field::Bar => cfg.bar.is_some(),
        Field::Cells => cfg.cells.is_some(),
        Field::Template(l) => template(cfg, l).is_some(),
        Field::Icon => cfg.icon.is_some(),
        Field::Color => cfg.color.is_some(),
        Field::Levels => cfg.levels.is_some(),
        Field::Coloring => cfg.coloring.is_some(),
        Field::Priority => cfg.priority.is_some(),
    }
}

/// Step a cycling field to its next or previous value.
pub fn cycle(field: Field, cfg: &mut ItemConfig, meta: &MetricMeta, forward: bool) {
    match field {
        Field::Opt(i) => {
            let spec = &meta.options[i];
            match spec.kind {
                OptKind::Flag => {
                    let on = opt(cfg, spec).and_then(|v| v.as_bool()).unwrap_or(false);
                    set_opt(cfg, spec, (!on).then_some(toml::Value::Boolean(true)));
                }
                OptKind::Choice(values) => {
                    let current = opt_text(cfg, spec);
                    let next = step(values, current.as_str(), forward);
                    set_opt(
                        cfg,
                        spec,
                        (next != spec.default).then(|| toml::Value::String(next.into())),
                    );
                }
                OptKind::Text | OptKind::Number => {}
            }
        }
        Field::Widget => {
            let next = step(&WIDGETS, cfg.widget.unwrap_or(meta.widget), forward);
            cfg.widget = unless_default(next, meta.widget);
        }
        Field::Bar => {
            let next = step(&BARS, cfg.bar.unwrap_or_default(), forward);
            cfg.bar = unless_default(next, BarGlyphs::default());
        }
        Field::Cells => {
            let (min, default, max) = CELLS;
            let n = cfg.cells.unwrap_or(default);
            let next = if forward {
                n.saturating_add(1).min(max)
            } else {
                n.saturating_sub(1).max(min)
            };
            cfg.cells = unless_default(next, default);
        }
        Field::Coloring => {
            let next = step(&COLORINGS, cfg.coloring.unwrap_or_default(), forward);
            cfg.coloring = unless_default(next, Coloring::default());
        }
        Field::Priority => {
            let p = cfg.priority.unwrap_or(meta.priority);
            let next = if forward {
                (p + 1).min(5)
            } else {
                p.saturating_sub(1).max(1)
            };
            cfg.priority = unless_default(next, meta.priority);
        }
        Field::Color => {
            // No color (the category's), then each palette token.
            let names: Vec<Option<&str>> = std::iter::once(None)
                .chain(TOKENS.iter().map(|t| Some(t.name())))
                .collect();
            let current = color_name(cfg).filter(|c| Token::parse(c).is_some());
            let next = step(&names, current, forward);
            cfg.color = next.map(str::to_string);
        }
        Field::Template(_) | Field::Icon | Field::Levels => {}
    }
}

/// What a text field starts out with when editing begins.
pub fn text(field: Field, cfg: &ItemConfig, meta: &MetricMeta, tier: GlyphTier) -> String {
    match field {
        Field::Opt(i) => {
            let spec = &meta.options[i];
            opt(cfg, spec)
                .map(|_| opt_text(cfg, spec))
                .unwrap_or_default()
        }
        Field::Template(l) => template(cfg, l)
            .cloned()
            .unwrap_or_else(|| meta.templates[l.index()].to_string()),
        Field::Icon => cfg
            .icon
            .clone()
            .unwrap_or_else(|| glyphs::icon(meta.icon, tier).to_string()),
        _ => String::new(),
    }
}

/// Store typed text; an error says what's wrong with it.
pub fn set_text(
    field: Field,
    cfg: &mut ItemConfig,
    meta: &MetricMeta,
    text: &str,
    tier: GlyphTier,
) -> Result<(), String> {
    match field {
        Field::Opt(i) => {
            let spec = &meta.options[i];
            let text = text.trim();
            let value = match spec.kind {
                _ if text.is_empty() => None,
                OptKind::Number => Some(toml::Value::Integer(
                    text.parse::<u32>()
                        .map_err(|_| format!("{} takes a whole number", spec.key))?
                        .into(),
                )),
                _ => Some(toml::Value::String(text.to_string())),
            };
            set_opt(cfg, spec, value);
        }
        Field::Template(l) => {
            *template_mut(cfg, l) =
                unless_default(text.to_string(), meta.templates[l.index()].into());
        }
        Field::Icon => {
            cfg.icon = unless_default(
                text.trim().to_string(),
                glyphs::icon(meta.icon, tier).into(),
            );
        }
        _ => {}
    }
    Ok(())
}

/// Go back to the metric's default.
pub fn reset(field: Field, cfg: &mut ItemConfig, meta: &MetricMeta) {
    match field {
        Field::Opt(i) => set_opt(cfg, &meta.options[i], None),
        Field::Widget => cfg.widget = None,
        Field::Bar => cfg.bar = None,
        Field::Cells => cfg.cells = None,
        Field::Template(l) => *template_mut(cfg, l) = None,
        Field::Icon => cfg.icon = None,
        Field::Color => cfg.color = None,
        Field::Levels => cfg.levels = None,
        Field::Coloring => cfg.coloring = None,
        Field::Priority => cfg.priority = None,
    }
}

/// `50 warn · 80 crit`
pub fn levels_text(cfg: &ItemConfig, meta: &MetricMeta) -> String {
    let all: Vec<String> = match &cfg.levels {
        Some(l) => l.iter().map(|(at, s)| format!("{at} {s}")).collect(),
        None => meta
            .levels
            .iter()
            .map(|(at, s)| format!("{at} {}", sev_name(*s)))
            .collect(),
    };
    if all.is_empty() {
        "none".into()
    } else {
        all.join(" · ")
    }
}

/// Everything the value column needs besides the item.
pub struct Look<'a> {
    pub tier: GlyphTier,
    pub palette: &'a Palette,
    pub depth: ColorDepth,
}

/// The value column for `field`.
pub fn value(field: Field, cfg: &ItemConfig, meta: &MetricMeta, look: &Look) -> Vec<Span<'static>> {
    let plain = |s: String| vec![Span::raw(s)];
    let muted = |s: &str| vec![Span::styled(s.to_string(), ui::muted())];
    match field {
        Field::Opt(i) => {
            let spec = &meta.options[i];
            match spec.kind {
                OptKind::Flag => {
                    let on = opt(cfg, spec).and_then(|v| v.as_bool()).unwrap_or(false);
                    vec![ui::choice(if on { "on" } else { "off" })]
                }
                OptKind::Choice(_) => vec![ui::choice(&opt_text(cfg, spec))],
                OptKind::Text | OptKind::Number => match opt(cfg, spec) {
                    Some(_) => plain(opt_text(cfg, spec)),
                    None if spec.default.is_empty() => muted("none"),
                    None => muted(spec.default),
                },
            }
        }
        Field::Widget => vec![ui::choice(&lower(cfg.widget.unwrap_or(meta.widget)))],
        Field::Bar => vec![ui::choice(&lower(cfg.bar.unwrap_or_default()))],
        Field::Cells => vec![ui::choice(&cfg.cells.unwrap_or(CELLS.1).to_string())],
        Field::Template(l) => match template(cfg, l) {
            Some(t) if t.is_empty() => muted("hidden"),
            Some(t) => plain(t.clone()),
            None => muted(meta.templates[l.index()]),
        },
        Field::Icon => match &cfg.icon {
            Some(i) if i.is_empty() => muted("none"),
            Some(i) => plain(i.clone()),
            None => {
                let glyph = glyphs::icon(meta.icon, look.tier);
                let mut v = plain(format!("{glyph} "));
                v.extend(muted("default"));
                v
            }
        },
        Field::Color => match color_name(cfg) {
            None => muted("category"),
            Some(name) => {
                let color = match Token::parse(name) {
                    Some(t) => look.palette.color(t),
                    None => Color::parse(name),
                };
                vec![ui::swatch(color, look.depth), Span::raw(format!(" {name}"))]
            }
        },
        Field::Levels => {
            let text = levels_text(cfg, meta);
            let mut v = plain(text);
            if cfg.coloring == Some(Coloring::Gradient) {
                v.extend(muted(" gradient"));
            }
            v
        }
        Field::Coloring => vec![ui::choice(&lower(cfg.coloring.unwrap_or_default()))],
        Field::Priority => {
            let p = cfg.priority.unwrap_or(meta.priority).clamp(1, 5) as usize;
            vec![
                Span::styled("●".repeat(p), ui::accent()),
                Span::styled("○".repeat(5 - p), ui::muted()),
                Span::styled(
                    if p == 1 {
                        "  first to go".to_string()
                    } else if p == 5 {
                        "  kept longest".to_string()
                    } else {
                        String::new()
                    },
                    ui::muted(),
                ),
            ]
        }
    }
}

/// One row: marker, label, then the value or the field being typed in.
pub fn row(
    field: Field,
    cfg: &ItemConfig,
    meta: &MetricMeta,
    look: &Look,
    on: bool,
    editing: Option<&TextInput>,
    width: usize,
) -> Line<'static> {
    let marker = Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent());
    let style = if on { ui::accent() } else { Style::default() };
    let mut spans = vec![
        marker,
        Span::styled(format!("{:<LABEL$}", label(field, meta)), style),
    ];
    match editing {
        Some(input) if on => {
            let room = width.saturating_sub(2 + LABEL + 1);
            spans.extend(input.line(room, true, "").spans);
        }
        _ => {
            let room = width.saturating_sub(2 + LABEL + 2);
            spans.extend(ui::clip(value(field, cfg, meta, look), room));
            if is_set(field, cfg, meta) {
                spans.push(Span::styled(" *", Style::default().fg(ui::WARN)));
            }
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics;

    fn meta(id: &str) -> &'static MetricMeta {
        &metrics::find(id).unwrap().meta
    }

    fn bare(id: &str) -> ItemConfig {
        ItemConfig {
            metric: id.into(),
            ..ItemConfig::default()
        }
    }

    #[test]
    fn lists_fields_for_the_widget() {
        let ctx = meta("context");
        let f = fields(&bare("context"), ctx);
        assert!(f.contains(&Field::Bar) && f.contains(&Field::Coloring));
        let mut text = bare("context");
        text.widget = Some(WidgetKind::Text);
        let f = fields(&text, ctx);
        assert!(!f.contains(&Field::Bar) && !f.contains(&Field::Cells));
        assert_eq!(fields(&bare("cmd"), meta("cmd"))[0], Field::Opt(0));
        assert!(!fields(&bare("cmd"), meta("cmd")).contains(&Field::Levels));
    }

    #[test]
    fn cycles_back_to_defaults() {
        let git = meta("git");
        let mut c = bare("git");
        cycle(Field::Widget, &mut c, git, true);
        assert_eq!(c.widget, Some(WidgetKind::Bar));
        cycle(Field::Widget, &mut c, git, false);
        assert_eq!(c.widget, None);
        cycle(Field::Priority, &mut c, git, true);
        assert_eq!(c.priority, Some(5));
        cycle(Field::Priority, &mut c, git, true);
        assert_eq!(c.priority, Some(5));
        cycle(Field::Opt(0), &mut c, git, true);
        assert_eq!(opt(&c, &git.options[0]), Some(&toml::Value::Boolean(true)));
        cycle(Field::Opt(0), &mut c, git, true);
        assert_eq!(opt(&c, &git.options[0]), None);
        cycle(Field::Color, &mut c, git, true);
        assert_eq!(c.color.as_deref(), Some("ok"));
        cycle(Field::Color, &mut c, git, false);
        assert_eq!(c.color, None);

        let dir = meta("dir");
        let mut d = bare("dir");
        cycle(Field::Opt(0), &mut d, dir, false);
        assert_eq!(opt_text(&d, &dir.options[0]), "full");
    }

    #[test]
    fn sets_text_fields() {
        let cmd = meta("cmd");
        let mut c = bare("cmd");
        set_text(Field::Opt(0), &mut c, cmd, " uptime ", GlyphTier::Nerd).unwrap();
        assert_eq!(text(Field::Opt(0), &c, cmd, GlyphTier::Nerd), "uptime");
        assert!(set_text(Field::Opt(1), &mut c, cmd, "soon", GlyphTier::Nerd).is_err());
        set_text(Field::Opt(1), &mut c, cmd, "30", GlyphTier::Nerd).unwrap();
        assert_eq!(opt(&c, &cmd.options[1]), Some(&toml::Value::Integer(30)));

        let git = meta("git");
        let mut g = bare("git");
        let default = text(Field::Template(Level::Full), &g, git, GlyphTier::Nerd);
        set_text(
            Field::Template(Level::Full),
            &mut g,
            git,
            &default,
            GlyphTier::Nerd,
        )
        .unwrap();
        assert_eq!(g.format, None);
        set_text(
            Field::Template(Level::Minimal),
            &mut g,
            git,
            "",
            GlyphTier::Nerd,
        )
        .unwrap();
        assert_eq!(g.minimal.as_deref(), Some(""));
        let glyph = text(Field::Icon, &g, git, GlyphTier::Unicode);
        set_text(Field::Icon, &mut g, git, &glyph, GlyphTier::Unicode).unwrap();
        assert_eq!(g.icon, None);
        set_text(Field::Icon, &mut g, git, "", GlyphTier::Unicode).unwrap();
        assert_eq!(g.icon.as_deref(), Some(""));
        reset(Field::Icon, &mut g, git);
        assert_eq!(g.icon, None);
        assert_eq!(levels_text(&g, git), "50 warn · 80 crit");
    }
}
