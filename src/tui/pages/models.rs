//! 5 Models: the model registry. Ids nothing recognized, the user's rules
//! in match order with hits and shadowing, context modifiers, and a tester
//! that shows how any id resolves.

use super::{Cx, Env, Outcome, Page};
use crate::metrics::fmt::grouped;
use crate::models::builtin::{self, RULES};
use crate::models::{MatchKind, NameSource, Registry};
use crate::tui::dialogs::{ModifierDialog, RuleDialog};
use crate::tui::doc::Doc;
use crate::tui::ui::{self, TextInput};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState};

/// What Claude Code assumes for a model it doesn't know.
const CC_DEFAULT: u64 = 200_000;
const CC_1M: u64 = 1_000_000;
const ENV_VAR: &str = "CLAUDE_CODE_MAX_CONTEXT_TOKENS";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    Unrecognized,
    #[default]
    Rules,
    Modifiers,
    Tester,
}

#[derive(Default)]
pub struct Models {
    pane: Pane,
    unrec: usize,
    rule: usize,
    modifier: usize,
    builtins: bool,
    tester: TextInput,
}

struct Unrecognized {
    id: String,
    count: u64,
    last: i64,
    cc_window: Option<u64>,
}

/// Ids seen recently that nothing, including the rules being edited,
/// recognizes; most recent first.
fn unrecognized(env: &Env, registry: &Registry) -> Vec<Unrecognized> {
    let mut out: Vec<Unrecognized> = env
        .seen
        .iter()
        .filter(|(id, s)| registry.resolve(id, None, s.cc_window).source == NameSource::RawId)
        .map(|(id, s)| Unrecognized {
            id: id.clone(),
            count: s.count,
            last: s.last,
            cc_window: s.cc_window,
        })
        .collect();
    out.sort_by_key(|u| std::cmp::Reverse(u.last));
    out
}

/// Recently seen ids with Claude Code's window, most recent first.
fn seen_ids(env: &Env) -> Vec<(String, Option<u64>)> {
    let mut seen: Vec<_> = env.seen.iter().collect();
    seen.sort_by_key(|(_, s)| std::cmp::Reverse(s.last));
    seen.into_iter()
        .map(|(id, s)| (id.clone(), s.cc_window))
        .collect()
}

fn ago(now: i64, t: i64) -> String {
    match (now - t).max(0) {
        s if s < 60 => "just now".into(),
        s if s < 3_600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3_600),
        s => format!("{}d ago", s / 86_400),
    }
}

fn kind_name(k: MatchKind) -> &'static str {
    match k {
        MatchKind::Contains => "contains",
        MatchKind::Prefix => "prefix",
        MatchKind::Exact => "exact",
        MatchKind::Regex => "regex",
    }
}

/// The window Claude Code assumes for an id it can't size.
fn cc_assumes(id: &str) -> u64 {
    if id.to_lowercase().contains("[1m]") {
        CC_1M
    } else {
        CC_DEFAULT
    }
}

/// How the tester resolves `id`: the result line, and the environment
/// line that would make Claude Code use the real window, if it differs.
fn test_id(
    id: &str,
    registry: &Registry,
    rules: &[crate::models::Rule],
) -> (Line<'static>, Option<String>) {
    let assumed = cc_assumes(id);
    let r = registry.resolve(id, None, Some(assumed));
    let source = match r.source {
        NameSource::UserRule(i) => format!("rule #{} ({})", i + 1, kind_name(rules[i].match_kind)),
        NameSource::ClaudeCode => "Claude Code's name".into(),
        NameSource::Builtin if builtin::claude_model(id).is_some() => "Claude model".into(),
        NameSource::Builtin => "built-in rule".into(),
        NameSource::RawId => "not recognized".into(),
    };
    let window = r.window.unwrap_or(assumed);
    let line = Line::from(vec![
        Span::styled("→ ", ui::accent()),
        Span::raw(format!("{} · {}", r.name, grouped(window))),
        Span::styled(format!("    {source}"), ui::muted()),
    ]);
    // Claude Code sizes its own models; the variable is for the others.
    let fix = (builtin::claude_model(id).is_none() && window != assumed)
        .then(|| format!("{ENV_VAR}={window}"));
    (line, fix)
}

fn focused_panel(title: &str, focused: bool) -> Block<'static> {
    let b = ui::panel(title);
    if focused {
        b.border_style(ui::accent())
    } else {
        b
    }
}

fn marker(on: bool) -> Span<'static> {
    Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent())
}

impl Models {
    fn panes(&self, has_unrec: bool) -> Vec<Pane> {
        let mut p = vec![Pane::Rules, Pane::Modifiers, Pane::Tester];
        if has_unrec {
            p.insert(0, Pane::Unrecognized);
        }
        p
    }

    fn cycle(&mut self, forward: bool, has_unrec: bool) {
        let panes = self.panes(has_unrec);
        let i = panes.iter().position(|p| *p == self.pane).unwrap_or(0);
        let n = panes.len();
        self.pane = panes[(i + if forward { 1 } else { n - 1 }) % n];
    }

    fn render_unrecognized(&self, f: &mut Frame, area: Rect, cx: &Cx, list: &[Unrecognized]) {
        let focused = cx.focused && self.pane == Pane::Unrecognized;
        let block = focused_panel("Unrecognized ids", focused)
            .title_top(Line::styled(" from recent sessions ", ui::muted()).right_aligned());
        let lines: Vec<Line> = if list.is_empty() {
            vec![Line::styled(
                " · Every model id seen recently is recognized.",
                ui::muted(),
            )]
        } else {
            let rows = area.height.saturating_sub(2) as usize;
            let start = self.unrec.saturating_sub(rows.saturating_sub(1));
            list.iter()
                .enumerate()
                .skip(start)
                .take(rows)
                .map(|(i, u)| {
                    let on = focused && i == self.unrec;
                    let window = grouped(u.cc_window.unwrap_or(CC_DEFAULT));
                    Line::from(vec![
                        marker(on),
                        Span::styled("! ", Style::default().fg(ui::WARN)),
                        Span::styled(
                            format!("{:<28}", u.id),
                            if on { ui::accent() } else { Style::default() },
                        ),
                        Span::raw(format!(
                            "seen {:<5} {:<9}  CC assumes {window:<11}",
                            format!("{}×", u.count),
                            ago(cx.env.now, u.last)
                        )),
                        Span::styled("A", ui::accent()),
                        Span::raw(" add rule"),
                    ])
                })
                .collect()
        };
        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_rules(&self, f: &mut Frame, area: Rect, cx: &Cx, registry: &Registry) {
        let focused = cx.focused && self.pane == Pane::Rules;
        let rules = &cx.doc.models.rules;
        let seen = seen_ids(cx.env);
        let samples: Vec<&str> = seen.iter().map(|(id, _)| id.as_str()).collect();
        let shadowed = cx.doc.models.shadowed(&samples);
        let mut hits = vec![0u64; rules.len()];
        for (id, s) in &cx.env.seen {
            if let Some(i) = registry.matching_rule(id) {
                hits[i] += s.count;
            }
        }

        let mut rows: Vec<Row> = rules
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let warning = if r.regex_error().is_some() {
                    Span::styled("! bad regex", Style::default().fg(ui::BAD))
                } else if let Some(by) = shadowed[i] {
                    Span::styled(
                        format!("! #{} matches first", by + 1),
                        Style::default().fg(ui::WARN),
                    )
                } else {
                    Span::raw("")
                };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)),
                    Cell::from(kind_name(r.match_kind)),
                    Cell::from(r.pattern.clone()),
                    Cell::from(r.display_name.clone().unwrap_or_else(|| "—".into())),
                    Cell::from(
                        Line::from(r.context_limit.map(grouped).unwrap_or_else(|| "—".into()))
                            .right_aligned(),
                    ),
                    Cell::from(Line::from(hits[i].to_string()).right_aligned()),
                    Cell::from(warning),
                ])
            })
            .collect();
        if rules.is_empty() {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from(Span::styled(
                    "No rules yet. Press N to add one, or A for an id above.",
                    ui::muted(),
                )),
            ]));
        }
        if self.builtins {
            for b in RULES {
                rows.push(
                    Row::new(vec![
                        Cell::from("·"),
                        Cell::from("built-in"),
                        Cell::from(b.pattern),
                        Cell::from(b.name),
                        Cell::from(Line::from(grouped(b.window)).right_aligned()),
                        Cell::from(""),
                        Cell::from("read-only"),
                    ])
                    .style(ui::muted()),
                );
            }
            rows.push(
                Row::new(vec![
                    Cell::from("·"),
                    Cell::from("built-in"),
                    Cell::from(
                        "Claude ids in any provider's spelling (Bedrock, Vertex, OpenRouter…)",
                    ),
                ])
                .style(ui::muted()),
            );
        }

        let header = Row::new([
            "#",
            "match",
            "pattern",
            "display name",
            "context",
            "hits",
            "",
        ])
        .style(ui::muted());
        let widths = [
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(10),
            Constraint::Length(5),
            Constraint::Length(18),
        ];
        let hints = if focused {
            ui::hints(&[
                ("N", "new"),
                ("enter", "edit"),
                ("del", "delete"),
                ("alt+↑↓", "reorder"),
                ("B", "built-ins"),
            ])
        } else {
            Line::raw("")
        };
        let block = focused_panel("Rules", focused)
            .title_top(Line::styled(" first match wins ", ui::muted()).right_aligned())
            .title_bottom(hints);
        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .column_spacing(1)
            .row_highlight_style(ui::accent())
            .highlight_symbol(ui::SELECTED);
        let mut state = TableState::default()
            .with_selected((focused && !rules.is_empty()).then_some(self.rule));
        f.render_stateful_widget(table, area, &mut state);
    }

    fn render_modifiers(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let focused = cx.focused && self.pane == Pane::Modifiers;
        let describe = |pattern: &str, suffix: &str, window: Option<u64>| {
            let window = window
                .map(|w| format!(" · {}", grouped(w)))
                .unwrap_or_default();
            format!("{pattern}  →  \"{suffix}\"{window}")
        };
        let mut lines: Vec<Line> = cx
            .doc
            .models
            .context_modifiers
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let on = focused && i == self.modifier;
                Line::from(vec![
                    marker(on),
                    Span::styled(
                        describe(&m.pattern, &m.display_suffix, m.context_limit),
                        if on { ui::accent() } else { Style::default() },
                    ),
                ])
            })
            .collect();
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{}   built-in", describe("[1m]", " 1M", Some(CC_1M))),
                ui::muted(),
            ),
        ]));
        let hints = if focused {
            ui::hints(&[("N", "new"), ("enter", "edit"), ("del", "delete")])
        } else {
            Line::raw("")
        };
        f.render_widget(
            Paragraph::new(lines).block(focused_panel("Modifiers", focused).title_bottom(hints)),
            area,
        );
    }

    fn render_tester(&self, f: &mut Frame, area: Rect, cx: &Cx, registry: &Registry) {
        let focused = cx.focused && self.pane == Pane::Tester;
        let width = area.width.saturating_sub(8) as usize;
        let mut lines = vec![Line::from(
            [
                vec![
                    Span::styled("id ", ui::muted()),
                    Span::styled(ui::SELECTED, ui::accent()),
                ],
                self.tester
                    .line(width, focused, "type or paste a model id")
                    .spans,
            ]
            .concat(),
        )];
        let id = self.tester.text();
        let id = id.trim();
        if id.is_empty() {
            lines.push(Line::styled(
                "Shows how an id resolves, as you type.",
                ui::muted(),
            ));
        } else {
            let (result, fix) = test_id(id, registry, &cx.doc.models.rules);
            lines.push(result);
            if let Some(fix) = fix {
                lines.push(Line::styled(
                    format!(
                        "! Claude Code assumes {} for it. Set",
                        grouped(cc_assumes(id))
                    ),
                    Style::default().fg(ui::WARN),
                ));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(fix, ui::accent()),
                ]));
                lines.push(Line::styled(
                    "  so auto-compact uses the real window.",
                    ui::muted(),
                ));
            }
        }
        let hints = if focused {
            ui::hints(&[("enter", "copy the env line"), ("esc", "leave")])
        } else {
            Line::raw("")
        };
        f.render_widget(
            Paragraph::new(lines).block(focused_panel("Tester", focused).title_bottom(hints)),
            area,
        );
    }

    fn open_rule(&self, env: &Env, id: Option<&str>) -> Outcome {
        Outcome::Open(Box::new(RuleDialog::new(id, seen_ids(env))))
    }
}

impl Page for Models {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let registry = cx.doc.registry();
        let list = unrecognized(cx.env, &registry);
        if list.is_empty() && self.pane == Pane::Unrecognized {
            self.pane = Pane::Rules;
        }
        self.unrec = self.unrec.min(list.len().saturating_sub(1));
        self.rule = self.rule.min(cx.doc.models.rules.len().saturating_sub(1));
        self.modifier = self
            .modifier
            .min(cx.doc.models.context_modifiers.len().saturating_sub(1));

        let short = area.height < 18;
        let unrec_rows = list.len().clamp(1, if short { 1 } else { 3 }) as u16;
        let [top, mid, low] = Layout::vertical([
            Constraint::Length(unrec_rows + 2),
            Constraint::Fill(1),
            Constraint::Length(if short { 6 } else { 7 }),
        ])
        .areas(area);
        self.render_unrecognized(f, top, cx, &list);
        self.render_rules(f, mid, cx, &registry);
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)])
                .spacing(1)
                .areas(low);
        self.render_modifiers(f, left, cx);
        self.render_tester(f, right, cx, &registry);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome {
        let registry = doc.registry();
        let list = unrecognized(env, &registry);
        let moving = key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Tab => {
                self.cycle(true, !list.is_empty());
                return Outcome::Consumed;
            }
            KeyCode::BackTab => {
                self.cycle(false, !list.is_empty());
                return Outcome::Consumed;
            }
            _ => {}
        }

        if self.pane == Pane::Tester {
            return match key.code {
                KeyCode::Esc => {
                    self.pane = Pane::Rules;
                    Outcome::Consumed
                }
                KeyCode::Enter => {
                    let id = self.tester.text();
                    match test_id(id.trim(), &registry, &doc.models.rules).1 {
                        Some(fix) if !id.trim().is_empty() => Outcome::Copy(fix),
                        _ => Outcome::Toast(
                            "Nothing to copy: Claude Code already uses this window".into(),
                        ),
                    }
                }
                _ if self.tester.handle(key) => Outcome::Consumed,
                _ => Outcome::Ignored,
            };
        }

        if let KeyCode::Char('a' | 'A') = key.code {
            let pick = list.get(self.unrec).or(list.first());
            return match pick {
                Some(u) => self.open_rule(env, Some(&u.id)),
                None => Outcome::Toast("Every model id seen recently is recognized".into()),
            };
        }

        match self.pane {
            Pane::Unrecognized => match key.code {
                KeyCode::Up if self.unrec == 0 => return Outcome::Ignored,
                KeyCode::Up => self.unrec -= 1,
                KeyCode::Down => self.unrec = (self.unrec + 1).min(list.len().saturating_sub(1)),
                KeyCode::Enter => {
                    return self.open_rule(env, list.get(self.unrec).map(|u| u.id.as_str()));
                }
                _ => return Outcome::Ignored,
            },
            Pane::Rules => {
                let n = doc.models.rules.len();
                match key.code {
                    KeyCode::Up if moving && self.rule > 0 => {
                        let i = self.rule;
                        doc.edit(|_, m| m.rules.swap(i, i - 1));
                        self.rule -= 1;
                    }
                    KeyCode::Down if moving && self.rule + 1 < n => {
                        let i = self.rule;
                        doc.edit(|_, m| m.rules.swap(i, i + 1));
                        self.rule += 1;
                    }
                    KeyCode::Up if self.rule == 0 && !moving => return Outcome::Ignored,
                    KeyCode::Up => self.rule = self.rule.saturating_sub(1),
                    KeyCode::Down => self.rule = (self.rule + 1).min(n.saturating_sub(1)),
                    KeyCode::Char('n' | 'N') => return self.open_rule(env, None),
                    KeyCode::Char('b' | 'B') => self.builtins = !self.builtins,
                    KeyCode::Enter if n > 0 => {
                        let rule = &doc.models.rules[self.rule];
                        return Outcome::Open(Box::new(RuleDialog::edit(
                            self.rule,
                            rule,
                            seen_ids(env),
                        )));
                    }
                    KeyCode::Delete | KeyCode::Backspace if n > 0 => {
                        let i = self.rule;
                        doc.edit(|_, m| {
                            m.rules.remove(i);
                        });
                        return Outcome::Toast(format!("Deleted rule #{} (^Z undoes)", i + 1));
                    }
                    _ => return Outcome::Ignored,
                }
            }
            Pane::Modifiers => {
                let n = doc.models.context_modifiers.len();
                match key.code {
                    KeyCode::Up if self.modifier == 0 => return Outcome::Ignored,
                    KeyCode::Up => self.modifier -= 1,
                    KeyCode::Down => self.modifier = (self.modifier + 1).min(n.saturating_sub(1)),
                    KeyCode::Char('n' | 'N') => {
                        return Outcome::Open(Box::new(ModifierDialog::new(None, None)));
                    }
                    KeyCode::Enter if n > 0 => {
                        let m = &doc.models.context_modifiers[self.modifier];
                        return Outcome::Open(Box::new(ModifierDialog::new(
                            Some(self.modifier),
                            Some(m),
                        )));
                    }
                    KeyCode::Delete | KeyCode::Backspace if n > 0 => {
                        let i = self.modifier;
                        doc.edit(|_, m| {
                            m.context_modifiers.remove(i);
                        });
                        return Outcome::Toast("Deleted modifier (^Z undoes)".into());
                    }
                    _ => return Outcome::Ignored,
                }
            }
            Pane::Tester => unreachable!("handled above"),
        }
        Outcome::Consumed
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        match self.pane {
            Pane::Tester => vec![
                ("tab", "pane"),
                ("enter", "copy env line"),
                ("esc", "leave tester"),
            ],
            _ => vec![
                ("tab", "pane"),
                ("↑↓", "move"),
                ("enter", "edit"),
                ("A", "add from unrecognized"),
            ],
        }
    }

    fn file(&self) -> &'static str {
        "models.toml"
    }

    fn paste(&mut self, text: &str) -> bool {
        if self.pane != Pane::Tester {
            // Pasting an id anywhere on the page means testing it.
            self.pane = Pane::Tester;
            self.tester = TextInput::default();
        }
        self.tester.insert_str(text.trim());
        true
    }

    fn scenario(&self) -> Option<&'static str> {
        Some("third-party")
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{MatchKind, Rule};
    use crate::state::models_seen::Seen;
    use crate::tui::app::App;
    use crate::tui::app::tests::{ctrl, env, key, screen};
    use crate::tui::doc::Doc;
    use crate::tui::doc::tests::temp_paths;
    use crate::tui::pages::{Env, PageId};
    use ratatui::crossterm::event::KeyCode;

    fn seen(count: u64, ago: i64, source: &str) -> Seen {
        Seen {
            count,
            last: crate::scenarios::BASE - ago,
            cc_window: Some(200_000),
            source: source.into(),
        }
    }

    fn env_with_ids() -> Env {
        let mut e = env();
        e.seen
            .insert("kimi-k3-preview".into(), seen(12, 120, "raw"));
        e.seen
            .insert("deepseek-v4-chat".into(), seen(3, 90_000, "raw"));
        e.seen
            .insert("claude-opus-4-8".into(), seen(40, 30, "builtin"));
        e
    }

    fn typing(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn adds_a_rule_for_an_unrecognized_id_and_tests_it() {
        let (dir, paths) = temp_paths("models-add");
        let mut app = App::new(Doc::open(paths.clone()), env_with_ids());
        app.open_page(PageId::Models);
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("kimi-k3-preview") && s.contains("seen 12×") && s.contains("2m ago"),
            "{s}"
        );
        assert!(s.contains("deepseek-v4-chat") && s.contains("1d ago"));
        assert!(
            !s.contains("claude-opus-4-8  "),
            "Claude ids are recognized"
        );
        // The preview switched to the scenario where the registry matters.
        assert!(s.contains("‹third-party›"), "{s}");

        // A: a rule prefilled from the most recent unrecognized id.
        app.handle_key(key(KeyCode::Char('a')));
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("New rule") && s.contains("Kimi K3 Preview"),
            "{s}"
        );
        app.handle_key(key(KeyCode::Tab));
        typing(&mut app, "256k");
        assert!(screen(&mut app, 112, 33).contains("= 256,000 tokens"));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.doc.models.rules.len(), 1);
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("Added rule #1"), "{s}");
        assert!(!s.contains("seen 12×"), "no longer unrecognized: {s}");
        assert!(s.contains("models.toml *"));

        // Tester: resolves through the new rule and offers the env line.
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Tab));
        app.paste("kimi-k3-preview\n");
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("Kimi K3 Preview · 256,000") && s.contains("rule #1 (contains)"),
            "{s}"
        );
        assert!(s.contains("CLAUDE_CODE_MAX_CONTEXT_TOKENS=256000"), "{s}");
        // Typing in the tester doesn't switch pages or quit.
        typing(&mut app, "q1");
        assert!(!app.quit && app.page == PageId::Models);
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Backspace));
        app.handle_key(key(KeyCode::Enter));
        assert!(
            screen(&mut app, 112, 33).contains("Copied: CLAUDE_CODE_MAX_CONTEXT_TOKENS=256000")
        );

        app.handle_key(key(KeyCode::Esc));
        app.handle_key(ctrl('z'));
        assert!(app.doc.models.rules.is_empty());
        app.handle_key(ctrl('y'));
        app.handle_key(ctrl('s'));
        let saved = Doc::open(paths);
        assert_eq!(saved.models.rules[0].context_limit, Some(256_000));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn warns_about_rules_that_never_apply_and_reorders() {
        let (dir, paths) = temp_paths("models-shadow");
        let mut doc = Doc::open(paths);
        doc.edit(|_, m| {
            for (kind, pattern) in [
                (MatchKind::Contains, "kimi"),
                (MatchKind::Exact, "kimi-k2-0905"),
            ] {
                m.rules.push(Rule {
                    pattern: pattern.into(),
                    display_name: Some(pattern.to_uppercase()),
                    context_limit: Some(256_000),
                    match_kind: kind,
                });
            }
        });
        let mut app = App::new(doc, env());
        app.open_page(PageId::Models);
        assert!(screen(&mut app, 112, 33).contains("! #1 matches first"));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(ratatui::crossterm::event::KeyEvent::new(
            KeyCode::Up,
            ratatui::crossterm::event::KeyModifiers::ALT,
        ));
        assert_eq!(app.doc.models.rules[0].pattern, "kimi-k2-0905");
        assert!(!screen(&mut app, 112, 33).contains("matches first"));
        app.handle_key(key(KeyCode::Char('b')));
        assert!(screen(&mut app, 112, 33).contains("Kimi K2 Turbo"));
        app.handle_key(key(KeyCode::Delete));
        assert_eq!(app.doc.models.rules.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fits_a_small_terminal() {
        let (dir, paths) = temp_paths("models-small");
        let mut app = App::new(Doc::open(paths), env_with_ids());
        app.open_page(PageId::Models);
        let s = screen(&mut app, 80, 24);
        assert!(
            s.contains("Rules") && s.contains("Tester") && s.contains("Modifiers"),
            "{s}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
