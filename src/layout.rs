//! Fit a line into the available width by stepping items down from full to
//! compact to minimal and finally dropping them, lowest priority first.

use crate::render::span::{Line, Span, line_width};
use crate::style::skin::{ItemView, Side, Theme, compose};
use crate::style::{Category, Color, Piece, Severity};
use crate::widgets::Level;

/// An item with its rendering at each level (`None` where it has nothing).
#[derive(Clone, Debug)]
pub struct LaidItem {
    pub variants: [Option<Vec<Piece>>; 3],
    pub category: Category,
    pub severity: Severity,
    pub color: Option<Color>,
    pub fg: Option<Color>,
    /// 1 (dropped first) to 5 (kept longest).
    pub priority: u8,
}

#[derive(Clone, Debug, Default)]
pub struct LineSpec {
    pub left: Vec<LaidItem>,
    pub right: Vec<LaidItem>,
}

struct Slot<'a> {
    item: &'a LaidItem,
    side: Side,
    /// Position within its side.
    index: usize,
    order: usize,
    level: Option<Level>,
}

impl Slot<'_> {
    fn first_level(item: &LaidItem) -> Option<Level> {
        Level::ALL
            .into_iter()
            .find(|l| item.variants[l.index()].is_some())
    }

    /// Step down one level, or drop when no smaller level exists.
    fn step(&mut self) {
        let current = self.level.map(Level::index).unwrap_or(0);
        self.level = Level::ALL
            .into_iter()
            .skip(current + 1)
            .find(|l| self.item.variants[l.index()].is_some());
    }

    fn view(&self) -> Option<ItemView<'_>> {
        let pieces = self.item.variants[self.level?.index()].as_deref()?;
        Some(ItemView {
            pieces,
            category: self.item.category,
            severity: self.item.severity,
            color: self.item.color,
            fg: self.item.fg,
        })
    }
}

/// One step taken to make a line fit: an item moved down a level, or was
/// dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub side: Side,
    /// Position within its side of the `LineSpec`.
    pub index: usize,
    pub from: Level,
    /// `None` when the item was dropped.
    pub to: Option<Level>,
}

/// A laid-out line and how it got there.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub line: Line,
    /// Columns the line needs with every item at its fullest.
    pub natural: usize,
    /// Steps in the order they were taken; empty when the line fit.
    pub steps: Vec<Step>,
}

/// Lay out one line. With `budget = None` (width unknown) nothing is dropped
/// and right-side items simply follow the left ones.
pub fn fit(spec: &LineSpec, budget: Option<usize>, theme: &Theme) -> Line {
    plan(spec, budget, theme).line
}

/// Lay out one line like `fit`, recording each step down.
pub fn plan(spec: &LineSpec, budget: Option<usize>, theme: &Theme) -> Plan {
    let mut slots: Vec<Slot> = spec
        .left
        .iter()
        .enumerate()
        .map(|(i, item)| (i, item, Side::Left))
        .chain(
            spec.right
                .iter()
                .enumerate()
                .map(|(i, item)| (i, item, Side::Right)),
        )
        .enumerate()
        .map(|(order, (index, item, side))| Slot {
            item,
            side,
            index,
            order,
            level: Slot::first_level(item),
        })
        .collect();
    let natural = line_width(&assemble(&slots, Some(0), theme));
    let mut steps = Vec::new();

    loop {
        let line = assemble(&slots, budget, theme);
        let done = |line: Line, steps: Vec<Step>| Plan {
            line,
            natural,
            steps,
        };
        let Some(budget) = budget else {
            return done(line, steps);
        };
        if line_width(&line) <= budget {
            return done(line, steps);
        }
        // Lowest priority first; on ties the right side, then later items.
        let next = slots
            .iter_mut()
            .filter(|s| s.level.is_some())
            .min_by_key(|s| {
                (
                    s.item.priority,
                    s.side == Side::Left,
                    std::cmp::Reverse(s.order),
                )
            });
        match next {
            Some(slot) => {
                let from = slot.level.unwrap_or(Level::Full);
                slot.step();
                steps.push(Step {
                    side: slot.side,
                    index: slot.index,
                    from,
                    to: slot.level,
                });
            }
            None => return done(Line::new(), steps),
        }
    }
}

fn assemble(slots: &[Slot], budget: Option<usize>, theme: &Theme) -> Line {
    let views = |side: Side| -> Vec<ItemView> {
        slots
            .iter()
            .filter(|s| s.side == side)
            .filter_map(Slot::view)
            .collect()
    };
    let (left, right) = (views(Side::Left), views(Side::Right));
    let Some(budget) = budget else {
        let all: Vec<ItemView> = left.into_iter().chain(right).collect();
        return compose(&all, Side::Left, theme);
    };
    let mut line = compose(&left, Side::Left, theme);
    if right.is_empty() {
        return line;
    }
    let right_line = compose(&right, Side::Right, theme);
    let used = line_width(&line) + line_width(&right_line);
    let gap = budget.saturating_sub(used).max(1);
    line.push(Span::raw(" ".repeat(gap)));
    line.extend(right_line);
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::span::line_text;
    use crate::style::palette;
    use crate::style::{GlyphTier, Role, SkinKind};

    fn theme() -> Theme {
        Theme {
            palette: palette::terminal(),
            skin: SkinKind::Plain,
            glyphs: GlyphTier::Unicode,
            separator: Some(" | ".into()),
            fill: crate::style::FillStyle::Solid,
            edges: Default::default(),
        }
    }

    fn item(full: &str, compact: &str, minimal: &str, priority: u8) -> LaidItem {
        let v = |s: &str| (!s.is_empty()).then(|| vec![Piece::new(s, Role::Text)]);
        LaidItem {
            variants: [v(full), v(compact), v(minimal)],
            category: Category::Identity,
            severity: Severity::Normal,
            color: None,
            fg: None,
            priority,
        }
    }

    fn spec() -> LineSpec {
        LineSpec {
            left: vec![
                item("Opus 4.8", "Opus 4.8", "Opus", 5),
                item("ctx 62% · 124k", "62%", "62", 4),
                item("main ✓ ↑2", "main*", "main", 2),
            ],
            right: vec![item("$1.20", "$1.20", "$1", 3)],
        }
    }

    fn fit_text(budget: Option<usize>) -> String {
        line_text(&fit(&spec(), budget, &theme()))
    }

    fn gap(n: usize) -> String {
        " ".repeat(n)
    }

    #[test]
    fn right_aligns_when_it_fits() {
        let text = fit_text(Some(50));
        assert_eq!(
            text,
            format!("Opus 4.8 | ctx 62% · 124k | main ✓ ↑2{}$1.20", gap(8))
        );
        assert_eq!(crate::render::width::display_width(&text), 50);
    }

    #[test]
    fn steps_lowest_priority_down_first() {
        // git (priority 2) goes compact, then minimal, then is dropped; then
        // cost (3) does the same, all before context (4) changes.
        assert_eq!(
            fit_text(Some(40)),
            format!("Opus 4.8 | ctx 62% · 124k | main*{}$1.20", gap(2))
        );
        assert_eq!(
            fit_text(Some(38)),
            format!("Opus 4.8 | ctx 62% · 124k | main{}$1.20", gap(1))
        );
        assert_eq!(
            fit_text(Some(32)),
            format!("Opus 4.8 | ctx 62% · 124k{}$1.20", gap(2))
        );
        assert_eq!(
            fit_text(Some(29)),
            format!("Opus 4.8 | ctx 62% · 124k{}$1", gap(2))
        );
        assert_eq!(fit_text(Some(20)), "Opus 4.8 | 62%");
    }

    #[test]
    fn plan_records_each_step() {
        let s = spec();
        let p = plan(&s, Some(32), &theme());
        assert_eq!(p.natural, 43);
        let step = |side, index, from, to| Step {
            side,
            index,
            from,
            to,
        };
        assert_eq!(
            p.steps,
            vec![
                step(Side::Left, 2, Level::Full, Some(Level::Compact)),
                step(Side::Left, 2, Level::Compact, Some(Level::Minimal)),
                step(Side::Left, 2, Level::Minimal, None),
            ]
        );
        assert_eq!(line_text(&p.line), line_text(&fit(&s, Some(32), &theme())));
        assert!(plan(&s, Some(50), &theme()).steps.is_empty());
        assert!(plan(&s, None, &theme()).steps.is_empty());
    }

    #[test]
    fn unknown_width_keeps_everything_inline() {
        assert_eq!(
            fit_text(None),
            "Opus 4.8 | ctx 62% · 124k | main ✓ ↑2 | $1.20"
        );
    }

    #[test]
    fn empty_when_nothing_fits() {
        assert_eq!(fit_text(Some(2)), "");
    }

    #[test]
    fn skips_items_without_variants() {
        let mut s = spec();
        s.left.push(item("", "", "", 5));
        assert_eq!(
            line_text(&fit(&s, None, &theme())),
            "Opus 4.8 | ctx 62% · 124k | main ✓ ↑2 | $1.20"
        );
    }
}
