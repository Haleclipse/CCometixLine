//! The canvas: each line's items as chips, left side then right, with what
//! the layout gives up when the preview is too narrow.

use super::edit::{self, Cursor};
use crate::config::{ItemSpec, LineConfig};
use crate::statusline::Report;
use crate::style::skin::Side;
use crate::tui::preview::Target;
use crate::tui::ui;
use crate::widgets::Level;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

/// What the layout did with an item at the preview's width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    Full,
    Smaller,
    Dropped,
    /// Nothing to show in this scenario.
    Empty,
}

fn fate(report: &Report, side: Side, index: usize) -> Fate {
    if report.empty.contains(&(side, index)) {
        return Fate::Empty;
    }
    match report
        .steps
        .iter()
        .rev()
        .find(|s| s.side == side && s.index == index)
    {
        None => Fate::Full,
        Some(s) if s.to.is_none() => Fate::Dropped,
        Some(_) => Fate::Smaller,
    }
}

/// The chip text: the metric id, and a `cmd`'s command.
pub fn label(spec: &ItemSpec) -> String {
    let c = spec.config();
    let run = c
        .options
        .as_ref()
        .and_then(|o| o.get("run"))
        .and_then(|v| v.as_str());
    match run {
        Some(run) if c.metric == "cmd" => {
            format!("cmd {}", crate::render::width::truncate(run.trim(), 14))
        }
        _ => c.metric.clone(),
    }
}

fn level_name(l: Option<Level>) -> &'static str {
    match l {
        Some(Level::Full) => "full",
        Some(Level::Compact) => "compact",
        Some(Level::Minimal) => "minimal",
        None => "dropped",
    }
}

pub struct View<'a> {
    pub lines: &'a [LineConfig],
    pub reports: &'a [Report],
    pub cursor: Cursor,
    pub focused: bool,
    pub target: Target,
    pub columns: u16,
}

/// Rows for one line: its heading, then its chips wrapped to `width`.
fn line_rows(view: &View, n: usize, width: usize) -> Vec<Line<'static>> {
    let line = &view.lines[n];
    let report = view.reports.get(n);
    let here = view.cursor.line == n;
    let mut rows = Vec::new();

    let heading = match view.target {
        Target::Status => format!("Line {}", n + 1),
        Target::Subagent => "Each task".to_string(),
    };
    let mut head = vec![Span::styled(heading.clone(), ui::strong())];
    if let Some(r) = report.filter(|_| edit::count(line) > 0) {
        let cols = view.columns as usize;
        let color = if r.natural <= cols {
            ui::OK
        } else if r.steps.iter().any(|s| s.to.is_none()) {
            ui::BAD
        } else {
            ui::WARN
        };
        let meter = format!("{}/{cols}", r.natural);
        let gap = width.saturating_sub(heading.chars().count() + meter.len());
        head.push(Span::raw(" ".repeat(gap)));
        head.push(Span::styled(meter, Style::default().fg(color)));
    }
    rows.push(Line::from(head));

    let marker = if here && view.focused {
        Span::styled(ui::SELECTED, ui::accent())
    } else {
        Span::raw("  ")
    };
    if edit::count(line) == 0 {
        rows.push(Line::from(vec![
            marker,
            Span::styled("empty · add from the library", ui::muted()),
        ]));
        return rows;
    }

    let mut chips: Vec<Span<'static>> = Vec::new();
    let all = line
        .left
        .iter()
        .enumerate()
        .map(|(i, s)| (Side::Left, i, s))
        .chain(
            line.right
                .iter()
                .enumerate()
                .map(|(i, s)| (Side::Right, i, s)),
        );
    for (pos, (side, index, spec)) in all.enumerate() {
        if side == Side::Right && index == 0 {
            chips.push(Span::styled(" ⇥", ui::muted()));
        }
        let style = if here && pos == view.cursor.pos {
            if view.focused {
                ui::tab_on()
            } else {
                ui::tab_current()
            }
        } else {
            match report.map(|r| fate(r, side, index)) {
                Some(Fate::Smaller) => Style::default().fg(ui::WARN),
                Some(Fate::Dropped) => Style::default()
                    .fg(ui::BAD)
                    .add_modifier(Modifier::CROSSED_OUT),
                Some(Fate::Empty) => ui::muted(),
                _ => Style::default(),
            }
        };
        chips.push(Span::styled(format!(" {} ", label(spec)), style));
    }
    if line.right.is_empty() {
        chips.push(Span::styled(" ⇥", ui::muted()));
    }

    // Wrap the chips; continuation rows keep the marker's indent.
    let room = width.saturating_sub(2).max(1);
    let mut row = vec![marker];
    let mut used = 0;
    for chip in chips {
        let w = chip.width();
        if used > 0 && used + w > room {
            rows.push(Line::from(std::mem::replace(
                &mut row,
                vec![Span::raw("  ")],
            )));
            used = 0;
        }
        used += w;
        row.push(chip);
    }
    rows.push(Line::from(row));
    rows
}

/// What gives way at the preview's width, merging an item's steps in a row.
fn narrow_rows(view: &View) -> Vec<Line<'static>> {
    let cols = view.columns;
    let mut merged: Vec<(usize, String, Level, Option<Level>)> = Vec::new();
    for (n, report) in view.reports.iter().enumerate() {
        let mut last: Option<(Side, usize)> = None;
        for step in &report.steps {
            let key = (step.side, step.index);
            if last == Some(key)
                && let Some(m) = merged.last_mut()
            {
                m.3 = step.to;
                continue;
            }
            last = Some(key);
            let spec = &edit::side(&view.lines[n], step.side)[step.index];
            merged.push((n, label(spec), step.from, step.to));
        }
    }
    if merged.is_empty() {
        return vec![
            Line::styled(format!("Fits in {cols} columns."), ui::muted()),
            Line::from(vec![
                Span::styled("-", ui::accent()),
                Span::styled(" narrows the preview", ui::muted()),
            ]),
        ];
    }
    let mut rows = vec![Line::styled(
        format!("Too narrow at {cols} columns:"),
        ui::muted(),
    )];
    for (i, (n, label, from, to)) in merged.into_iter().enumerate() {
        let line = match view.target {
            Target::Status => format!("L{} ", n + 1),
            Target::Subagent => String::new(),
        };
        let change = match to {
            None => Span::styled("dropped", Style::default().fg(ui::BAD)),
            Some(_) => Span::styled(
                format!("{} → {}", level_name(Some(from)), level_name(to)),
                Style::default().fg(ui::WARN),
            ),
        };
        rows.push(Line::from(vec![
            Span::styled(format!("{:>2} {line}", i + 1), ui::muted()),
            Span::raw(format!("{label:<14} ")),
            change,
        ]));
    }
    rows
}

pub fn render(f: &mut Frame, area: Rect, block: Block<'static>, view: &View) {
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;

    let mut rows: Vec<Line> = Vec::new();
    let mut cursor_row = 0;
    for n in 0..view.lines.len() {
        if n == view.cursor.line {
            cursor_row = rows.len();
        }
        rows.extend(line_rows(view, n, width));
        rows.push(Line::raw(""));
    }
    if view.target == Target::Status {
        let on = view.cursor.line >= view.lines.len();
        if on {
            cursor_row = rows.len();
        }
        let (marker, style) = if on && view.focused {
            (Span::styled(ui::SELECTED, ui::accent()), ui::accent())
        } else {
            (Span::raw("  "), ui::muted())
        };
        rows.push(Line::from(vec![marker, Span::styled("+ new line", style)]));
        rows.push(Line::raw(""));
    }
    rows.extend(narrow_rows(view));

    // Scroll only as far as needed to show the cursor's line.
    let height = inner.height as usize;
    let skip = (cursor_row + 3).saturating_sub(height);
    let shown: Vec<Line> = rows.into_iter().skip(skip).collect();
    f.render_widget(Paragraph::new(shown), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ItemConfig;
    use crate::layout::Step;

    #[test]
    fn labels_and_fates() {
        let mut opts = toml::Table::new();
        opts.insert("run".into(), "uptime | awk '{print $3}'".into());
        let cmd = ItemSpec::Item(ItemConfig {
            metric: "cmd".into(),
            options: Some(opts),
            ..ItemConfig::default()
        });
        assert_eq!(label(&cmd), "cmd uptime | awk …");
        assert_eq!(label(&ItemSpec::Id("git".into())), "git");

        let report = Report {
            steps: vec![
                Step {
                    side: Side::Left,
                    index: 1,
                    from: Level::Full,
                    to: Some(Level::Compact),
                },
                Step {
                    side: Side::Right,
                    index: 0,
                    from: Level::Minimal,
                    to: None,
                },
            ],
            empty: vec![(Side::Left, 2)],
            ..Report::default()
        };
        assert_eq!(fate(&report, Side::Left, 0), Fate::Full);
        assert_eq!(fate(&report, Side::Left, 1), Fate::Smaller);
        assert_eq!(fate(&report, Side::Left, 2), Fate::Empty);
        assert_eq!(fate(&report, Side::Right, 0), Fate::Dropped);
    }
}
