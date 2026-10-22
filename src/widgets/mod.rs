//! Widgets turn a reading into pieces at one of three widths.

pub mod template;

use crate::metrics::Reading;
use crate::style::glyphs::{self, BarGlyphs, GlyphTier};
use crate::style::{Piece, Role, Severity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetKind {
    #[default]
    Text,
    Bar,
    Gauge,
    Sparkline,
    Pace,
}

/// How much room an item takes; layout steps down through these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Full,
    Compact,
    Minimal,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Full, Level::Compact, Level::Minimal];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Coloring {
    /// Color the whole graph by threshold.
    #[default]
    Steps,
    /// Color each cell by its position along ok → warn → crit.
    Gradient,
}

/// A widget's resolved settings for one item.
#[derive(Clone, Debug)]
pub struct Props {
    pub kind: WidgetKind,
    pub bar: BarGlyphs,
    pub cells: usize,
    pub templates: [String; 3],
    /// Icon glyph; empty for none.
    pub icon: String,
    pub levels: Vec<(f64, Severity)>,
    pub coloring: Coloring,
}

/// Severity from the reading itself, else from thresholds on its ratio.
pub fn severity(reading: &Reading, levels: &[(f64, Severity)]) -> Severity {
    if let Some(s) = reading.severity {
        return s;
    }
    let Some(ratio) = reading.ratio else {
        return Severity::Normal;
    };
    let pct = ratio * 100.0;
    levels
        .iter()
        .filter(|(at, _)| pct >= *at)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, s)| *s)
        .unwrap_or(Severity::Normal)
}

/// Render `reading` at `level`; `None` when nothing would be visible.
pub fn render(
    reading: &Reading,
    props: &Props,
    level: Level,
    tier: GlyphTier,
) -> Option<Vec<Piece>> {
    let text = template::render(&props.templates[level.index()], reading);
    let sev = severity(reading, &props.levels);
    let icon = (!props.icon.is_empty()).then(|| Piece::new(props.icon.clone(), Role::Icon));

    let graph: Option<Vec<Piece>> = match (props.kind, level) {
        (WidgetKind::Bar, Level::Full) => reading.ratio.map(|r| bar(r, props, sev, tier)),
        (WidgetKind::Gauge, Level::Full | Level::Compact) => reading
            .ratio
            .and_then(|r| glyphs::gauge(r, tier))
            .map(|g| vec![Piece::new(g, Role::Fill(sev))]),
        (WidgetKind::Sparkline, Level::Full | Level::Compact) => reading.series.as_ref().map(|s| {
            let n = if level == Level::Full {
                props.cells
            } else {
                props.cells / 2
            };
            spark(s, n.max(2), tier)
        }),
        (WidgetKind::Pace, Level::Full) if tier != GlyphTier::Ascii => reading
            .ratio
            .zip(reading.elapsed)
            .map(|(used, elapsed)| pace(used, elapsed, props.cells)),
        _ => None,
    };

    // A gauge replaces the icon; other graphs sit between icon and text.
    let mut out = Vec::new();
    match (&graph, props.kind) {
        (Some(g), WidgetKind::Gauge) => out.extend(g.iter().cloned()),
        (Some(g), _) => {
            out.extend(icon.clone());
            if !out.is_empty() {
                out.push(Piece::new(" ", Role::Text));
            }
            out.extend(g.iter().cloned());
        }
        (None, _) if level != Level::Minimal => out.extend(icon.clone()),
        (None, _) => {}
    }

    let pace_text = props.kind == WidgetKind::Pace && level != Level::Minimal;
    let mut tail = if pace_text {
        // Pace shows how far ahead of or behind the clock usage is.
        let arrow = reading
            .get("arrow")
            .map(|f| Piece::new(f.text.clone(), f.role.unwrap_or(Role::Text)));
        let mut t = text.clone();
        if let Some(a) = arrow {
            t.push(Piece::new(" ", Role::Text));
            t.push(a);
        }
        t
    } else {
        text
    };
    if tail.is_empty() && graph.is_none() {
        return None;
    }
    if let Some(link) = &reading.link {
        for p in &mut tail {
            p.link = Some(link.clone());
        }
    }
    if !out.is_empty() && !tail.is_empty() {
        out.push(Piece::new(" ", Role::Text));
    }
    out.append(&mut tail);
    Some(template::merge(out))
}

fn bar(ratio: f64, props: &Props, sev: Severity, tier: GlyphTier) -> Vec<Piece> {
    let set = props.bar.for_tier(tier);
    let cells = props.cells.max(1);
    let filled = ratio.clamp(0.0, 1.0) * cells as f64;
    let (open, close) = set.frame();
    // Solid bars get a track behind them in filled skins.
    let mark = |p: Piece| if set.solid() { p.inset() } else { p };
    let mut out = Vec::new();
    if !open.is_empty() {
        out.push(mark(Piece::new(open, Role::Muted)));
    }
    for i in 0..cells {
        let fill_role = match props.coloring {
            Coloring::Steps => Role::Fill(sev),
            Coloring::Gradient => Role::Gradient((i as f32 + 0.5) / cells as f32),
        };
        let amount = (filled - i as f64).clamp(0.0, 1.0);
        let piece = if amount >= 1.0 {
            Piece::new(set.full(), fill_role)
        } else if amount > 0.0
            && let Some(partials) = set.partials()
        {
            let idx = ((amount * 8.0).round() as usize).clamp(1, 8);
            if idx == 8 {
                Piece::new(set.full(), fill_role)
            } else {
                Piece::new(partials[idx - 1], fill_role)
            }
        } else {
            Piece::new(set.empty(), Role::Empty)
        };
        out.push(mark(piece));
    }
    if !close.is_empty() {
        out.push(mark(Piece::new(close, Role::Muted)));
    }
    out
}

fn spark(series: &[f64], n: usize, tier: GlyphTier) -> Vec<Piece> {
    let levels = glyphs::spark_levels(tier);
    let tail = &series[series.len().saturating_sub(n)..];
    let max = tail.iter().copied().fold(f64::MIN, f64::max);
    let min = tail.iter().copied().fold(f64::MAX, f64::min);
    let span = (max - min).max(f64::EPSILON);
    let text: String = tail
        .iter()
        .map(|v| {
            let i = (((v - min) / span) * (levels.len() - 1) as f64).round() as usize;
            levels[i.min(levels.len() - 1)]
        })
        .collect();
    vec![Piece::new(text, Role::Fill(Severity::Normal))]
}

fn pace(used: f64, elapsed: f64, cells: usize) -> Vec<Piece> {
    let cells = cells.max(1);
    let filled = |r: f64| (r.clamp(0.0, 1.0) * cells as f64).round() as usize;
    let (u, e) = (filled(used), filled(elapsed));
    (0..cells)
        .map(|i| {
            Piece::new(
                "▀",
                Role::Pace {
                    used: i < u,
                    elapsed: i < e,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(kind: WidgetKind) -> Props {
        Props {
            kind,
            bar: BarGlyphs::Shapes,
            cells: 10,
            templates: [
                "{pct}%[ · {tokens}]".into(),
                "{pct}%".into(),
                "{pct}%".into(),
            ],
            icon: "◔".into(),
            levels: crate::metrics::LEVELS.to_vec(),
            coloring: Coloring::Steps,
        }
    }

    fn text(pieces: &[Piece]) -> String {
        pieces.iter().map(|p| p.text.as_str()).collect()
    }

    fn reading() -> Reading {
        Reading::default()
            .set("pct", "62")
            .set("tokens", "124k")
            .ratio(0.62)
    }

    #[test]
    fn text_widget_levels() {
        let p = props(WidgetKind::Text);
        let r = reading();
        let t = |l| text(&render(&r, &p, l, GlyphTier::Unicode).unwrap());
        assert_eq!(t(Level::Full), "◔ 62% · 124k");
        assert_eq!(t(Level::Compact), "◔ 62%");
        assert_eq!(t(Level::Minimal), "62%");
    }

    #[test]
    fn bar_widget_draws_cells() {
        let p = props(WidgetKind::Bar);
        let r = reading();
        assert_eq!(
            text(&render(&r, &p, Level::Full, GlyphTier::Unicode).unwrap()),
            "◔ ▰▰▰▰▰▰▱▱▱▱ 62% · 124k"
        );
        // In the ASCII tier icons are empty.
        let ascii = Props {
            icon: String::new(),
            ..props(WidgetKind::Bar)
        };
        assert_eq!(
            text(&render(&r, &ascii, Level::Full, GlyphTier::Ascii).unwrap()),
            "[======----] 62% · 124k"
        );
        let eighths = Props {
            bar: BarGlyphs::Eighths,
            ..props(WidgetKind::Bar)
        };
        assert_eq!(
            text(&render(&r, &eighths, Level::Full, GlyphTier::Unicode).unwrap()),
            "◔ ▕██████▎   ▏ 62% · 124k"
        );
    }

    #[test]
    fn severity_comes_from_thresholds_or_reading() {
        assert_eq!(
            severity(&Reading::default().ratio(0.3), crate::metrics::LEVELS),
            Severity::Normal
        );
        assert_eq!(
            severity(&Reading::default().ratio(0.62), crate::metrics::LEVELS),
            Severity::Warn
        );
        assert_eq!(
            severity(&Reading::default().ratio(0.91), crate::metrics::LEVELS),
            Severity::Crit
        );
        let own = Reading::default().ratio(0.9).severity(Severity::Good);
        assert_eq!(severity(&own, crate::metrics::LEVELS), Severity::Good);
    }

    #[test]
    fn gauge_replaces_icon_and_falls_back_in_ascii() {
        let p = props(WidgetKind::Gauge);
        let r = reading();
        assert_eq!(
            text(&render(&r, &p, Level::Full, GlyphTier::Unicode).unwrap()),
            "◕ 62% · 124k"
        );
        assert_eq!(
            text(&render(&r, &p, Level::Full, GlyphTier::Ascii).unwrap()),
            "◔ 62% · 124k"
        );
    }

    #[test]
    fn pace_draws_dual_cells_and_arrow() {
        let p = Props {
            templates: ["{pct}%".into(), "{pct}%".into(), "{pct}%".into()],
            cells: 4,
            ..props(WidgetKind::Pace)
        };
        let mut r = Reading::default()
            .set("pct", "75")
            .set_role("arrow", "⇡25%", Role::Bad)
            .ratio(0.75);
        r.elapsed = Some(0.5);
        let pieces = render(&r, &p, Level::Full, GlyphTier::Unicode).unwrap();
        assert_eq!(text(&pieces), "◔ ▀▀▀▀ 75% ⇡25%");
        // Neighbouring cells with the same role are merged, so expand by char.
        let cells: Vec<Role> = pieces
            .iter()
            .flat_map(|p| p.text.chars().filter(|c| *c == '▀').map(move |_| p.role))
            .collect();
        assert_eq!(cells.len(), 4);
        for cell in &cells[..2] {
            assert_eq!(
                *cell,
                Role::Pace {
                    used: true,
                    elapsed: true
                }
            );
        }
        assert_eq!(
            cells[2],
            Role::Pace {
                used: true,
                elapsed: false
            }
        );
        assert_eq!(
            cells[3],
            Role::Pace {
                used: false,
                elapsed: false
            }
        );
        assert_eq!(
            text(&render(&r, &p, Level::Minimal, GlyphTier::Unicode).unwrap()),
            "75%"
        );
    }

    #[test]
    fn sparkline_scales_to_range() {
        let p = Props {
            templates: ["+{d}".into(), "".into(), "{d}".into()],
            cells: 8,
            ..props(WidgetKind::Sparkline)
        };
        let mut r = Reading::default().set("d", "12k");
        r.series = Some(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        assert_eq!(
            text(&render(&r, &p, Level::Full, GlyphTier::Unicode).unwrap()),
            "◔ ▁▂▃▄▅▆▇█ +12k"
        );
        assert_eq!(
            text(&render(&r, &p, Level::Compact, GlyphTier::Unicode).unwrap()),
            "◔ ▁▃▆█"
        );
    }

    #[test]
    fn hidden_when_nothing_to_show() {
        let p = props(WidgetKind::Text);
        assert!(render(&Reading::default(), &p, Level::Full, GlyphTier::Unicode).is_none());
    }

    #[test]
    fn links_cover_text_but_not_icon() {
        let p = Props {
            templates: ["{n}".into(), "{n}".into(), "{n}".into()],
            ..props(WidgetKind::Text)
        };
        let r = Reading::default()
            .set("n", "#1")
            .link(Some("https://x/1".into()));
        let pieces = render(&r, &p, Level::Full, GlyphTier::Unicode).unwrap();
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].link, None);
        assert_eq!(pieces[1], Piece::new(" ", Role::Text));
        assert_eq!(pieces[2].text, "#1");
        assert_eq!(pieces[2].link.as_deref(), Some("https://x/1"));
    }
}
