//! Edits to the lines being composed, apart from drawing: where the cursor
//! points, and adding, removing and moving items and lines.

use crate::config::{Config, ItemConfig, ItemSpec, LineConfig};
use crate::presets;
use crate::style::skin::Side;
use crate::tui::preview::Target;
use std::borrow::Cow;

/// A place on the canvas: a line, and an item counted across its left side
/// then its right. `line` one past the last is the "new line" row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub pos: usize,
}

/// Where an item sits in the config.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loc {
    pub line: usize,
    pub side: Side,
    pub index: usize,
}

/// The lines `target` draws; the built-in subagent line when none is set.
pub fn lines(config: &Config, target: Target) -> Cow<'_, [LineConfig]> {
    match target {
        Target::Status => Cow::Borrowed(&config.lines),
        Target::Subagent => match &config.subagent {
            Some(line) => Cow::Borrowed(std::slice::from_ref(line)),
            None => Cow::Owned(vec![presets::default_subagent()]),
        },
    }
}

/// Apply `f` to `target`'s lines. The subagent line goes back to the
/// built-in one when it ends up the same.
pub fn edit_lines<R>(
    config: &mut Config,
    target: Target,
    f: impl FnOnce(&mut Vec<LineConfig>) -> R,
) -> R {
    match target {
        Target::Status => f(&mut config.lines),
        Target::Subagent => {
            let builtin = presets::default_subagent();
            let mut one = vec![config.subagent.take().unwrap_or_else(|| builtin.clone())];
            let out = f(&mut one);
            let line = one.into_iter().next().unwrap_or_default();
            config.subagent = (line != builtin).then_some(line);
            out
        }
    }
}

pub fn count(line: &LineConfig) -> usize {
    line.left.len() + line.right.len()
}

/// The item at `pos` on `line` (numbered `line_no`).
pub fn loc(line: &LineConfig, line_no: usize, pos: usize) -> Option<Loc> {
    let (side, index) = if pos < line.left.len() {
        (Side::Left, pos)
    } else if pos < count(line) {
        (Side::Right, pos - line.left.len())
    } else {
        return None;
    };
    Some(Loc {
        line: line_no,
        side,
        index,
    })
}

/// The item under `at`, if any.
pub fn item(lines: &[LineConfig], at: Cursor) -> Option<(Loc, &ItemSpec)> {
    let line = lines.get(at.line)?;
    let l = loc(line, at.line, at.pos)?;
    Some((l, &side(line, l.side)[l.index]))
}

pub fn side(line: &LineConfig, side: Side) -> &[ItemSpec] {
    match side {
        Side::Left => &line.left,
        Side::Right => &line.right,
    }
}

fn side_mut(line: &mut LineConfig, side: Side) -> &mut Vec<ItemSpec> {
    match side {
        Side::Left => &mut line.left,
        Side::Right => &mut line.right,
    }
}

/// Keep `at` on an existing item, or the new-line row when `grow`.
pub fn clamp(lines: &[LineConfig], at: Cursor, grow: bool) -> Cursor {
    let last = if grow {
        lines.len()
    } else {
        lines.len().saturating_sub(1)
    };
    let line = at.line.min(last);
    let pos = lines
        .get(line)
        .map(|l| at.pos.min(count(l).saturating_sub(1)))
        .unwrap_or(0);
    Cursor { line, pos }
}

/// Put `spec` after the item at `at`: at the end of the left side on an
/// empty line, on a new line from the new-line row. Returns where it went.
pub fn insert(lines: &mut Vec<LineConfig>, at: Cursor, spec: ItemSpec) -> Cursor {
    if at.line >= lines.len() {
        lines.push(LineConfig {
            left: vec![spec],
            right: Vec::new(),
        });
        return Cursor {
            line: lines.len() - 1,
            pos: 0,
        };
    }
    let line = &mut lines[at.line];
    match loc(line, at.line, at.pos) {
        Some(l) => {
            side_mut(line, l.side).insert(l.index + 1, spec);
            Cursor {
                pos: at.pos + 1,
                ..at
            }
        }
        None => {
            line.left.push(spec);
            Cursor {
                pos: line.left.len() - 1,
                ..at
            }
        }
    }
}

/// Remove the item at `at`; returns where the cursor goes.
pub fn remove(lines: &mut [LineConfig], at: Cursor) -> Option<Cursor> {
    let line = lines.get_mut(at.line)?;
    let l = loc(line, at.line, at.pos)?;
    side_mut(line, l.side).remove(l.index);
    Some(Cursor {
        pos: at.pos.min(count(line).saturating_sub(1)),
        ..at
    })
}

/// Move the item at `at` one place along its line. Past the end of the
/// left side it becomes the first on the right, and back again.
pub fn shift(lines: &mut [LineConfig], at: Cursor, forward: bool) -> Option<Cursor> {
    let line = lines.get_mut(at.line)?;
    let l = loc(line, at.line, at.pos)?;
    let last_left = l.side == Side::Left && l.index + 1 == line.left.len();
    match (l.side, forward) {
        (Side::Left, true) if last_left => {
            let spec = line.left.remove(l.index);
            line.right.insert(0, spec);
            Some(at)
        }
        (Side::Right, false) if l.index == 0 => {
            let spec = line.right.remove(0);
            line.left.push(spec);
            Some(at)
        }
        (_, true) => {
            let items = side_mut(line, l.side);
            if l.index + 1 >= items.len() {
                return None;
            }
            items.swap(l.index, l.index + 1);
            Some(Cursor {
                pos: at.pos + 1,
                ..at
            })
        }
        (_, false) => {
            if l.index == 0 {
                return None;
            }
            side_mut(line, l.side).swap(l.index - 1, l.index);
            Some(Cursor {
                pos: at.pos - 1,
                ..at
            })
        }
    }
}

/// Move the item at `at` to the end of its side on the line above or
/// below. Below the last line it starts a new one when `grow`.
pub fn move_line(
    lines: &mut Vec<LineConfig>,
    at: Cursor,
    down: bool,
    grow: bool,
) -> Option<Cursor> {
    let l = loc(lines.get(at.line)?, at.line, at.pos)?;
    let to = if down {
        at.line + 1
    } else {
        at.line.checked_sub(1)?
    };
    if to >= lines.len() {
        if !grow {
            return None;
        }
        lines.push(LineConfig::default());
    }
    let spec = side_mut(&mut lines[at.line], l.side).remove(l.index);
    let dest = &mut lines[to];
    side_mut(dest, l.side).push(spec);
    let pos = match l.side {
        Side::Left => dest.left.len() - 1,
        Side::Right => count(dest) - 1,
    };
    Some(Cursor { line: to, pos })
}

/// Whether any line has an item of `metric`.
pub fn has_metric(lines: &[LineConfig], metric: &str) -> bool {
    lines
        .iter()
        .flat_map(|l| l.left.iter().chain(&l.right))
        .any(|s| s.config().metric == metric)
}

/// Remove every item of `metric`; returns how many there were.
pub fn remove_metric(lines: &mut [LineConfig], metric: &str) -> usize {
    let mut removed = 0;
    for line in lines {
        for side in [&mut line.left, &mut line.right] {
            let before = side.len();
            side.retain(|s| s.config().metric != metric);
            removed += before - side.len();
        }
    }
    removed
}

/// Add an empty line after `line`; returns the cursor on it.
pub fn new_line(lines: &mut Vec<LineConfig>, line: usize) -> Cursor {
    let at = (line + 1).min(lines.len());
    lines.insert(at, LineConfig::default());
    Cursor { line: at, pos: 0 }
}

/// Apply `f` to the item's settings, keeping a bare metric id when none
/// are set.
pub fn edit_item(lines: &mut [LineConfig], at: Loc, f: impl FnOnce(&mut ItemConfig)) {
    let Some(spec) = lines
        .get_mut(at.line)
        .and_then(|l| side_mut(l, at.side).get_mut(at.index))
    else {
        return;
    };
    let mut config = spec.config().into_owned();
    f(&mut config);
    *spec = tidy(config);
}

fn tidy(mut config: ItemConfig) -> ItemSpec {
    if config.options.as_ref().is_some_and(|o| o.is_empty()) {
        config.options = None;
    }
    let bare = ItemConfig {
        metric: config.metric.clone(),
        ..ItemConfig::default()
    };
    if config == bare {
        ItemSpec::Id(config.metric)
    } else {
        ItemSpec::Item(Box::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(m: &str) -> ItemSpec {
        ItemSpec::Id(m.into())
    }

    fn line(left: &[&str], right: &[&str]) -> LineConfig {
        LineConfig {
            left: left.iter().map(|m| id(m)).collect(),
            right: right.iter().map(|m| id(m)).collect(),
        }
    }

    fn at(line: usize, pos: usize) -> Cursor {
        Cursor { line, pos }
    }

    #[test]
    fn inserts_after_the_cursor() {
        let mut lines = vec![line(&["model", "git"], &["cost"]), line(&[], &[])];
        assert_eq!(insert(&mut lines, at(0, 0), id("dir")), at(0, 1));
        assert_eq!(lines[0], line(&["model", "dir", "git"], &["cost"]));
        assert_eq!(insert(&mut lines, at(0, 3), id("cache")), at(0, 4));
        assert_eq!(lines[0], line(&["model", "dir", "git"], &["cost", "cache"]));
        assert_eq!(insert(&mut lines, at(1, 0), id("vim")), at(1, 0));
        assert_eq!(insert(&mut lines, at(2, 0), id("pr")), at(2, 0));
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn shifts_across_the_sides() {
        let mut lines = vec![line(&["a", "b"], &["c"])];
        assert_eq!(shift(&mut lines, at(0, 1), true), Some(at(0, 1)));
        assert_eq!(lines[0], line(&["a"], &["b", "c"]));
        assert_eq!(shift(&mut lines, at(0, 1), true), Some(at(0, 2)));
        assert_eq!(lines[0], line(&["a"], &["c", "b"]));
        assert_eq!(shift(&mut lines, at(0, 2), true), None);
        assert_eq!(shift(&mut lines, at(0, 1), false), Some(at(0, 1)));
        assert_eq!(lines[0], line(&["a", "c"], &["b"]));
        assert_eq!(shift(&mut lines, at(0, 0), false), None);
    }

    #[test]
    fn moves_between_lines_and_removes() {
        let mut lines = vec![line(&["a", "b"], &["c"]), line(&["d"], &[])];
        assert_eq!(move_line(&mut lines, at(0, 2), true, false), Some(at(1, 1)));
        assert_eq!(lines[1], line(&["d"], &["c"]));
        assert_eq!(move_line(&mut lines, at(1, 1), true, false), None);
        assert_eq!(move_line(&mut lines, at(1, 0), true, true), Some(at(2, 0)));
        assert_eq!(lines[2], line(&["d"], &[]));
        assert_eq!(move_line(&mut lines, at(0, 0), false, true), None);

        assert_eq!(remove(&mut lines, at(0, 1)), Some(at(0, 0)));
        assert_eq!(lines[0], line(&["a"], &[]));
        assert_eq!(remove(&mut lines, at(1, 0)), Some(at(1, 0)));
        assert_eq!(lines[1], line(&[], &[]));
        assert_eq!(new_line(&mut lines, 0), at(1, 0));
        assert_eq!(lines.len(), 4);
        assert_eq!(clamp(&lines, at(9, 9), false), at(3, 0));
        assert_eq!(clamp(&lines, at(9, 9), true), at(4, 0));
    }

    #[test]
    fn keeps_bare_ids_and_the_builtin_subagent_line() {
        let mut ls = vec![line(&["git"], &[])];
        let git = Loc {
            line: 0,
            side: Side::Left,
            index: 0,
        };
        edit_item(&mut ls, git, |c| c.priority = Some(1));
        assert!(matches!(&ls[0].left[0], ItemSpec::Item(c) if c.priority == Some(1)));
        edit_item(&mut ls, git, |c| {
            c.priority = None;
            c.options = Some(toml::Table::new());
        });
        assert_eq!(ls[0].left[0], id("git"));

        let mut two = vec![line(&["model", "git"], &["git"])];
        assert!(has_metric(&two, "git") && !has_metric(&two, "cost"));
        assert_eq!(remove_metric(&mut two, "git"), 2);
        assert_eq!(two[0], line(&["model"], &[]));

        let mut config = Config::default();
        assert_eq!(
            lines(&config, Target::Subagent)[0],
            presets::default_subagent()
        );
        edit_lines(&mut config, Target::Subagent, |l| {
            l[0].left.push(id("cost"))
        });
        assert!(config.subagent.is_some());
        edit_lines(&mut config, Target::Subagent, |l| l[0].left.pop());
        assert_eq!(config.subagent, None);
    }
}
