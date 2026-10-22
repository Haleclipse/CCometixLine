//! 6 Claude Code: the status line and agent panel settings in Claude
//! Code's `settings.json` (edited as a draft, written after a diff), the
//! update check, and what the companion mod will add.

use super::{Cx, Env, Outcome, Page, PageId};
use crate::cc_settings::{self, Settings, StatusLine};
use crate::state::update;
use crate::tui::dialogs::{SettingsDiff, shown_path};
use crate::tui::doc::Doc;
use crate::tui::ui;
use crate::version::Version;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    #[default]
    StatusLine,
    Agents,
    Updates,
}

const PANES: [Pane; 3] = [Pane::StatusLine, Pane::Agents, Pane::Updates];
/// Rows per pane; the status line's last row applies the draft.
const ROWS: [usize; 3] = [5, 2, 3];
const REFRESH: [Option<u64>; 7] = [
    None,
    Some(1),
    Some(2),
    Some(5),
    Some(10),
    Some(30),
    Some(60),
];
const INTERVALS: [u32; 5] = [6, 12, 24, 48, 168];
const UPGRADE: &str = "npm update -g @cometix/ccline";
/// Oldest Claude Code with mods.
const MODS: Version = Version {
    major: 2,
    minor: 1,
    patch: 287,
};
/// Label column per pane.
fn label_width(pane: Pane) -> usize {
    match pane {
        Pane::StatusLine => 17,
        Pane::Agents => 16,
        Pane::Updates => 13,
    }
}

#[derive(Default)]
pub struct ClaudePage {
    pane: Pane,
    rows: [usize; 3],
    /// The file as read, or why it can't be used.
    base: Option<Result<Settings, String>>,
    /// Changes not yet written.
    draft: Option<Settings>,
}

fn step<T: PartialEq + Copy>(all: &[T], current: T, forward: bool) -> T {
    let i = all.iter().position(|v| *v == current).unwrap_or(0);
    let n = all.len();
    all[(i + if forward { 1 } else { n - 1 }) % n]
}

fn on_off(on: bool) -> Span<'static> {
    ui::choice(if on { "on" } else { "off" })
}

impl ClaudePage {
    /// Read the file again if it changed; drop a draft it now matches.
    fn sync(&mut self, path: &Path) {
        let fresh = matches!(&self.base, Some(Ok(s)) if !s.stale() && s.path == path);
        if fresh {
            return;
        }
        let loaded = Settings::load(path);
        if let (Some(d), Ok(new)) = (&self.draft, &loaded)
            && !d.differs(new)
        {
            self.draft = None;
        }
        self.base = Some(loaded);
    }

    fn base(&self) -> Option<&Settings> {
        self.base.as_ref()?.as_ref().ok()
    }

    /// The settings as edited so far.
    fn current(&self) -> Option<&Settings> {
        self.draft.as_ref().or(self.base())
    }

    fn pending(&self) -> bool {
        matches!((&self.draft, self.base()), (Some(d), Some(b)) if d.differs(b))
    }

    /// Edit the draft, starting one from the file.
    fn edit(&mut self, f: impl FnOnce(&mut Settings)) {
        if self.draft.is_none() {
            self.draft = self.base().cloned();
        }
        if let Some(d) = self.draft.as_mut() {
            f(d);
        }
    }

    /// Change the status line; there must be one first.
    fn edit_line(&mut self, f: impl FnOnce(&mut StatusLine)) -> Outcome {
        let Some(mut line) = self.current().and_then(Settings::status_line) else {
            return Outcome::Toast("Set the command first".into());
        };
        f(&mut line);
        self.edit(|d| d.set_status_line(Some(&line)));
        Outcome::Consumed
    }

    fn review(&self) -> Outcome {
        match (&self.draft, self.base()) {
            (Some(d), Some(b)) if d.differs(b) => Outcome::Open(Box::new(SettingsDiff {
                base: b.clone(),
                draft: d.clone(),
            })),
            _ => Outcome::Toast("No changes to apply".into()),
        }
    }

    fn block(&self, title: String, pane: Option<Pane>, cx: &Cx) -> Block<'static> {
        let b = ui::panel(&title);
        if cx.focused && pane == Some(self.pane) {
            b.border_style(ui::accent())
        } else {
            b
        }
    }

    /// A selectable row in `pane`, its value cut to fit `width`.
    fn row(
        &self,
        pane: Pane,
        i: usize,
        label: &str,
        value: Vec<Span<'static>>,
        (cx, width): (&Cx, usize),
    ) -> Line<'static> {
        let on = cx.focused && self.pane == pane && self.rows[pane as usize] == i;
        let w = label_width(pane);
        let mut spans = vec![
            Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
            Span::styled(
                format!("{label:<w$}"),
                if on { ui::accent() } else { Style::default() },
            ),
        ];
        spans.extend(ui::clip(value, width.saturating_sub(2 + w)));
        Line::from(spans)
    }

    /// A row that only shows something, aligned with `pane`'s rows.
    fn info(pane: Pane, label: &str, value: Span<'static>, width: usize) -> Line<'static> {
        let w = label_width(pane);
        let mut spans = vec![Span::raw(format!("  {label:<w$}"))];
        spans.extend(ui::clip(vec![value], width.saturating_sub(2 + w)));
        Line::from(spans)
    }

    fn render_status_line(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let title = format!("statusLine · {}", shown_path(&cx.env.settings));
        let block = self.block(title, Some(Pane::StatusLine), cx);
        let width = block.inner(area).width as usize;
        let at = (cx, width);
        let lines = match (&self.base, self.current()) {
            (Some(Err(e)), _) => vec![
                Line::styled(format!("Can't edit it: {e}"), Style::default().fg(ui::BAD)),
                Line::styled("Fix the file by hand, then come back.", ui::muted()),
            ],
            (_, None) => vec![Line::styled("Reading…", ui::muted())],
            (_, Some(s)) => {
                let line = s.status_line();
                let room = width.saturating_sub(2 + label_width(Pane::StatusLine) + 2);
                let command = match &line {
                    Some(l) if cc_settings::is_ccline(&l.command) => {
                        let mut v = ui::clip(vec![Span::raw(l.command.clone())], room);
                        v.push(Span::styled(" ✓", Style::default().fg(ui::OK)));
                        v
                    }
                    Some(l) => vec![
                        Span::raw(crate::render::width::truncate(
                            &l.command,
                            room.saturating_sub(11),
                        )),
                        Span::styled("  not ccline", Style::default().fg(ui::WARN)),
                    ],
                    None => vec![Span::styled("not set", Style::default().fg(ui::WARN))],
                };
                let l = line.clone().unwrap_or_default();
                let has = line.is_some();
                let value = |v: Vec<Span<'static>>| {
                    if has {
                        v
                    } else {
                        vec![Span::styled("—", ui::muted())]
                    }
                };
                let refresh = match l.refresh_interval {
                    Some(n) => vec![ui::choice(&format!("{n}s"))],
                    None => vec![
                        ui::choice("not set"),
                        Span::styled("  2s keeps timers live", ui::muted()),
                    ],
                };
                let pane = Pane::StatusLine;
                let mut out = vec![
                    self.row(pane, 0, "command", command, at),
                    self.row(
                        pane,
                        1,
                        "padding",
                        value(vec![ui::choice(&l.padding.unwrap_or(0).to_string())]),
                        at,
                    ),
                    self.row(pane, 2, "refreshInterval", value(refresh), at),
                    self.row(
                        pane,
                        3,
                        "hide vim mode",
                        value(vec![on_off(l.hide_vim_mode_indicator.unwrap_or(false))]),
                        at,
                    ),
                    Line::raw(""),
                ];
                let apply = if self.pending() {
                    Span::styled("review and apply changes…", Style::default().fg(ui::WARN))
                } else {
                    Span::styled("no changes", ui::muted())
                };
                let on = cx.focused && self.pane == pane && self.rows[0] == 4;
                out.push(Line::from(vec![
                    Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
                    apply,
                ]));
                if line
                    .as_ref()
                    .is_none_or(|l| !cc_settings::is_ccline(&l.command))
                {
                    out.push(Line::raw(""));
                    out.push(Line::from(ui::clip(
                        vec![
                            Span::styled("enter", ui::accent()),
                            Span::styled(" on command uses ", ui::muted()),
                            Span::raw(cc_settings::suggested_command()),
                        ],
                        width,
                    )));
                }
                out
            }
        };
        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_agents(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let pane = Pane::Agents;
        let block = self.block("Agent panel · subagentStatusLine".into(), Some(pane), cx);
        let width = block.inner(area).width as usize;
        let enabled = self
            .current()
            .and_then(Settings::subagent_command)
            .is_some();
        let line = cx
            .doc
            .config
            .subagent
            .clone()
            .unwrap_or_else(crate::presets::default_subagent);
        let ids = |s: &[crate::config::ItemSpec]| -> String {
            s.iter()
                .map(|i| i.config().metric.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut summary = ids(&line.left);
        if !line.right.is_empty() {
            summary = format!("{summary} ⇥ {}", ids(&line.right));
        }
        let text = vec![
            self.row(pane, 0, "enabled", vec![on_off(enabled)], (cx, width)),
            self.row(
                pane,
                1,
                "each task shows",
                vec![Span::raw(summary)],
                (cx, width),
            ),
            Line::raw(""),
            Line::styled(
                "One row per background agent, every 5 seconds. Enter on the row above edits it in Layout (t switches lines).",
                ui::muted(),
            ),
        ];
        f.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
            area,
        );
    }

    fn render_updates(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let pane = Pane::Updates;
        let block = self.block("Updates".into(), Some(pane), cx);
        let width = block.inner(area).width as usize;
        let u = &cx.doc.config.update;
        let latest = match &cx.env.update.latest {
            Some(v) if update::is_newer(v, update::CURRENT) => {
                Span::styled(format!("{v}  ↑ available"), Style::default().fg(ui::WARN))
            }
            Some(v) => Span::raw(format!("{v}  up to date")),
            None => Span::styled("not checked yet", ui::muted()),
        };
        let text = vec![
            self.row(pane, 0, "check", vec![on_off(u.check)], (cx, width)),
            self.row(
                pane,
                1,
                "every",
                vec![ui::choice(&format!("{}h", u.interval_hours))],
                (cx, width),
            ),
            self.row(pane, 2, "upgrade", vec![Span::raw(UPGRADE)], (cx, width)),
            Self::info(pane, "current", Span::raw(update::CURRENT), width),
            Self::info(pane, "latest", latest, width),
            Line::raw(""),
            Line::styled(
                "Checked in the background; never slows the status line. Enter on upgrade copies the command.",
                ui::muted(),
            ),
        ];
        f.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
            area,
        );
    }

    fn render_mod(&self, f: &mut Frame, area: Rect, cx: &Cx) {
        let block = self.block("Companion mod".into(), None, cx);
        let width = block.inner(area).width as usize;
        let needs = match cx.env.cc_version() {
            Some(v) if v >= MODS => Span::styled(
                format!("Claude Code ≥ {MODS}  ✓"),
                Style::default().fg(ui::OK),
            ),
            Some(v) => Span::styled(
                format!("Claude Code ≥ {MODS}, running {v}"),
                Style::default().fg(ui::WARN),
            ),
            None => Span::raw(format!("Claude Code ≥ {MODS}")),
        };
        let pane = Pane::Updates;
        let text = vec![
            Self::info(
                pane,
                "status",
                Span::styled("coming in a later release", ui::muted()),
                width,
            ),
            Self::info(pane, "needs", needs, width),
            Self::info(pane, "adds", Span::raw("context breakdown"), width),
            Self::info(pane, "", Span::raw("live tools and agents"), width),
            Self::info(pane, "", Span::raw("time to first token"), width),
            Self::info(pane, "", Span::raw("a /ccline pane in Claude Code"), width),
        ];
        f.render_widget(Paragraph::new(text).block(block), area);
    }

    fn handle_status_line(&mut self, key: KeyEvent) -> Outcome {
        let forward = matches!(
            key.code,
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ')
        );
        match (self.rows[0], key.code) {
            (0, KeyCode::Enter) => {
                let mut line = self
                    .current()
                    .and_then(Settings::status_line)
                    .unwrap_or_default();
                line.command = cc_settings::suggested_command();
                self.edit(|d| d.set_status_line(Some(&line)));
                Outcome::Consumed
            }
            (1, KeyCode::Left | KeyCode::Right) => self.edit_line(|l| {
                let p = l.padding.unwrap_or(0);
                l.padding = Some(if forward {
                    (p + 1).min(10)
                } else {
                    p.saturating_sub(1)
                });
            }),
            (2, KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ')) => {
                self.edit_line(|l| l.refresh_interval = step(&REFRESH, l.refresh_interval, forward))
            }
            (3, KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ')) => self
                .edit_line(|l| {
                    let on = !l.hide_vim_mode_indicator.unwrap_or(false);
                    l.hide_vim_mode_indicator = on.then_some(true);
                }),
            (1..=3, KeyCode::Delete | KeyCode::Backspace) => {
                let row = self.rows[0];
                self.edit_line(|l| match row {
                    1 => l.padding = None,
                    2 => l.refresh_interval = None,
                    _ => l.hide_vim_mode_indicator = None,
                })
            }
            (4, KeyCode::Enter) => self.review(),
            _ => Outcome::Ignored,
        }
    }

    fn handle_agents(&mut self, key: KeyEvent) -> Outcome {
        match (self.rows[1], key.code) {
            (0, KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) => {
                let on = self
                    .current()
                    .and_then(Settings::subagent_command)
                    .is_some();
                let command = self
                    .current()
                    .and_then(Settings::status_line)
                    .map(|l| l.command)
                    .filter(|c| cc_settings::is_ccline(c))
                    .unwrap_or_else(cc_settings::suggested_command);
                self.edit(|d| d.set_subagent((!on).then_some(command.as_str())));
                Outcome::Toast("Changed the draft; apply it from the statusLine panel (a)".into())
            }
            (1, KeyCode::Enter) => Outcome::Goto(PageId::Layout),
            _ => Outcome::Ignored,
        }
    }

    fn handle_updates(&mut self, key: KeyEvent, doc: &mut Doc) -> Outcome {
        let forward = !matches!(key.code, KeyCode::Left);
        match (self.rows[2], key.code) {
            (0, KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) => {
                doc.edit(|c, _| c.update.check = !c.update.check);
                Outcome::Consumed
            }
            (1, KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) => {
                doc.edit(|c, _| {
                    c.update.interval_hours = step(&INTERVALS, c.update.interval_hours, forward)
                });
                Outcome::Consumed
            }
            (2, KeyCode::Enter) => Outcome::Copy(UPGRADE.into()),
            _ => Outcome::Ignored,
        }
    }
}

impl Page for ClaudePage {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        self.sync(&cx.env.settings);
        let [top, bottom] =
            Layout::vertical([Constraint::Length(10), Constraint::Fill(1)]).areas(area);
        let [status, module] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .spacing(1)
                .areas(top);
        let [agents, updates] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .spacing(1)
                .areas(bottom);
        self.render_status_line(f, status, cx);
        self.render_mod(f, module, cx);
        self.render_agents(f, agents, cx);
        self.render_updates(f, updates, cx);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome {
        self.sync(&env.settings);
        let p = self.pane as usize;
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let step = if key.code == KeyCode::Tab { 1 } else { 2 };
                self.pane = PANES[(p + step) % 3];
                return Outcome::Consumed;
            }
            KeyCode::Up if self.rows[p] == 0 => return Outcome::Ignored,
            KeyCode::Up => {
                self.rows[p] -= 1;
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                self.rows[p] = (self.rows[p] + 1).min(ROWS[p] - 1);
                return Outcome::Consumed;
            }
            KeyCode::Char('a') => return self.review(),
            _ => {}
        }
        if self.pane != Pane::Updates && self.base().is_none() {
            return Outcome::Ignored;
        }
        match self.pane {
            Pane::StatusLine => self.handle_status_line(key),
            Pane::Agents => self.handle_agents(key),
            Pane::Updates => self.handle_updates(key, doc),
        }
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("tab", "pane"),
            ("←→", "change"),
            ("enter", "set"),
            ("a", "apply to settings.json"),
        ]
    }

    fn preview(&self) -> bool {
        false
    }

    fn file(&self) -> &'static str {
        "settings.json"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use crate::tui::app::tests::{env, key, screen};
    use crate::tui::doc::tests::temp_paths;

    #[test]
    fn edits_a_draft_and_applies_it_after_the_diff() {
        let (dir, paths) = temp_paths("claude");
        let settings = dir.join("settings.json");
        std::fs::write(
            &settings,
            "{\n  \"statusLine\": {\"type\": \"command\", \"command\": \"bash x.sh\"}\n}\n",
        )
        .unwrap();
        let mut app = App::new(
            Doc::open(paths),
            Env {
                settings: settings.clone(),
                ..env()
            },
        );
        app.open_page(PageId::Claude);
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("not ccline") && s.contains("Companion mod"),
            "{s}"
        );

        // Point the command at ccline, set a refresh interval.
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Right));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("review and apply") && s.contains("‹2s›"), "{s}");
        assert!(
            std::fs::read_to_string(&settings)
                .unwrap()
                .contains("bash x.sh")
        );

        // The diff, then the write and its backup.
        app.handle_key(key(KeyCode::Char('a')));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("+   \"refreshInterval\": 2"), "{s}");
        app.handle_key(key(KeyCode::Char('y')));
        let written = Settings::load(&settings).unwrap();
        let line = written.status_line().unwrap();
        assert!(cc_settings::is_ccline(&line.command));
        assert_eq!(line.refresh_interval, Some(2));
        assert!(dir.join("settings.json.ccline-bak").exists());
        assert!(screen(&mut app, 112, 33).contains("no changes"));

        // The agent panel uses the same command.
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Char('y')));
        let written = Settings::load(&settings).unwrap();
        assert_eq!(written.subagent_command(), Some(line.command));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
