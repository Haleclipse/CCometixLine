//! 1 Home: the health of the setup, each problem with the key that fixes
//! it, the ways in, and the status line as it looks with the last real
//! session.

use super::{Cx, Env, Outcome, Page, PageId};
use crate::cc_settings::{self, Settings};
use crate::config::LoadNote;
use crate::state::update;
use crate::tui::dialogs::SettingsDiff;
use crate::tui::doc::Doc;
use crate::tui::ui;
use crate::version::MIN_SUPPORTED;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

enum Go {
    Page(PageId),
    Setup,
    Quit,
}

const START: [(&str, Go); 6] = [
    ("Open composer", Go::Page(PageId::Layout)),
    ("Browse presets", Go::Page(PageId::Presets)),
    ("Model registry", Go::Page(PageId::Models)),
    ("Claude Code integration", Go::Page(PageId::Claude)),
    ("First-run setup", Go::Setup),
    ("Quit", Go::Quit),
];
const UPGRADE: &str = "npm update -g @cometix/ccline";
/// Recommended `refreshInterval`, in seconds.
const REFRESH: u64 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Ok,
    Warn,
    Info,
}

struct Check {
    mark: Mark,
    text: String,
    /// The key that fixes it, and what it does.
    fix: Option<(&'static str, &'static str)>,
}

fn check(mark: Mark, text: impl Into<String>) -> Check {
    Check {
        mark,
        text: text.into(),
        fix: None,
    }
}

impl Check {
    fn fix(mut self, key: &'static str, what: &'static str) -> Check {
        self.fix = Some((key, what));
        self
    }
}

#[derive(Default)]
pub struct Home {
    selected: usize,
}

fn checks(cx: &Cx) -> Vec<Check> {
    let mut out = Vec::new();
    if crate::paths::DEV_BUILD {
        out.push(check(
            Mark::Info,
            format!(
                "development build: files in {}",
                crate::paths::home().display()
            ),
        ));
    }
    out.push(match cx.env.cc_version() {
        Some(v) if v >= MIN_SUPPORTED => check(Mark::Ok, format!("Claude Code {v}")),
        Some(v) => check(
            Mark::Warn,
            format!("Claude Code {v} is older than {MIN_SUPPORTED}"),
        ),
        None => check(Mark::Info, "Claude Code version not seen yet"),
    });
    match Settings::load(&cx.env.settings) {
        Err(e) => out.push(check(Mark::Warn, e)),
        Ok(s) => match s.status_line() {
            Some(l) if cc_settings::is_ccline(&l.command) => {
                out.push(check(Mark::Ok, "statusLine runs ccline"));
                if l.refresh_interval.is_none() {
                    out.push(check(Mark::Info, "refreshInterval not set").fix("i", "set 2s"));
                }
            }
            Some(_) => out
                .push(check(Mark::Warn, "statusLine runs another command").fix("s", "use ccline")),
            None => out.push(check(Mark::Warn, "statusLine not set").fix("s", "connect")),
        },
    }
    out.push(match &cx.doc.note {
        LoadNote::Fine => check(Mark::Ok, "config.toml valid"),
        LoadNote::Missing => check(Mark::Info, "no config.toml yet: the default preset"),
        LoadNote::Migrated => check(Mark::Info, "v1 config moved to config.v1.toml"),
        LoadNote::V1 => check(
            Mark::Warn,
            "config.toml is v1: ^S replaces it, keeping config.v1.toml",
        ),
        LoadNote::Invalid(e) => check(Mark::Warn, format!("config.toml invalid: {e}")),
    });
    out.push(match &cx.doc.models_error {
        Some(e) => check(Mark::Warn, format!("models.toml unreadable: {e}")),
        None => {
            let unknown = super::models::unrecognized(cx.env, &cx.doc.registry()).len();
            if unknown > 0 {
                check(
                    Mark::Info,
                    format!(
                        "{unknown} unrecognized model id{}",
                        if unknown == 1 { "" } else { "s" }
                    ),
                )
                .fix("m", "review")
            } else {
                check(
                    Mark::Ok,
                    format!("models.toml: {} rules", cx.doc.models.rules.len()),
                )
            }
        }
    });
    if let Some(v) = &cx.env.update.latest
        && update::is_newer(v, update::CURRENT)
    {
        out.push(check(Mark::Info, format!("ccline {v} available")).fix("u", "copy upgrade"));
    }
    out
}

fn health_line(c: &Check, width: usize) -> Line<'static> {
    let (sign, style) = match c.mark {
        Mark::Ok => ("✓", Style::default().fg(ui::OK)),
        Mark::Warn => ("!", Style::default().fg(ui::WARN)),
        Mark::Info => ("·", ui::muted()),
    };
    let fix = c
        .fix
        .map(|(key, what)| {
            vec![
                Span::styled(key.to_string(), ui::accent()),
                Span::styled(format!(" {what}"), ui::muted()),
            ]
        })
        .unwrap_or_default();
    let fix_w: usize = fix.iter().map(Span::width).sum();
    let room = width.saturating_sub(3 + fix_w + 2);
    let text = crate::render::width::truncate(&c.text, room);
    let gap = width.saturating_sub(3 + crate::render::width::display_width(&text) + fix_w + 1);
    let mut spans = vec![
        Span::styled(format!(" {sign} "), style),
        Span::raw(text),
        Span::raw(" ".repeat(gap)),
    ];
    spans.extend(fix);
    Line::from(spans)
}

/// Point `statusLine` at ccline, or give it a refresh interval, after
/// showing the change.
fn change_settings(env: &Env, refresh: bool) -> Outcome {
    let base = match Settings::load(&env.settings) {
        Ok(s) => s,
        Err(e) => return Outcome::Toast(e),
    };
    let mut line = base.status_line().unwrap_or_default();
    if refresh {
        line.refresh_interval = Some(REFRESH);
    } else {
        line.command = cc_settings::suggested_command();
    }
    let mut draft = base.clone();
    draft.set_status_line(Some(&line));
    if !draft.differs(&base) {
        return Outcome::Toast("Already set".into());
    }
    Outcome::Open(Box::new(SettingsDiff { base, draft }))
}

impl Home {
    /// The status line for the last session (else a sample) and its
    /// panel's title.
    fn now(cx: &Cx, width: u16) -> (Vec<Line<'static>>, String) {
        let registry = cx.doc.registry();
        let (preview, title) = match cx.preview.replayed() {
            Some(p) => (p, "replay of your last session"),
            None => (cx.preview.clone(), "no session yet: a sample session"),
        };
        let lines = preview.render(&cx.doc.config, cx.doc, &registry, cx.env.tz, width);
        (lines, title.to_string())
    }

    fn render_now(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>, title: String) {
        let width = area.width.saturating_sub(2);
        let block = ui::panel("Your status line now").title_top(
            Line::styled(format!(" {title} · {width} cols "), ui::muted()).right_aligned(),
        );
        f.render_widget(Paragraph::new(lines).block(block), area);
    }
}

impl Page for Home {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let checks = checks(cx);
        let rows = (checks.len().max(START.len()) + 2) as u16;
        let (lines, title) = Self::now(cx, area.width.saturating_sub(2));
        let [head, panels, _, now, _] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(rows),
            Constraint::Length(1),
            Constraint::Length(lines.len().max(1) as u16 + 2),
            Constraint::Fill(1),
        ])
        .areas(area);
        f.render_widget(
            Paragraph::new(Line::styled(
                "  ccline — compose your Claude Code status line",
                ui::strong(),
            )),
            head,
        );
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                .spacing(1)
                .areas(panels);
        let width = left.width.saturating_sub(2) as usize;
        let health: Vec<Line> = checks.iter().map(|c| health_line(c, width)).collect();
        f.render_widget(Paragraph::new(health).block(ui::panel("Health")), left);

        let start: Vec<Line> = START
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                let on = cx.focused && i == self.selected;
                let marker = if on { ui::SELECTED } else { "  " };
                let style = if on { ui::accent() } else { Style::default() };
                Line::from(vec![
                    Span::styled(marker, ui::accent()),
                    Span::styled(label.to_string(), style),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(start).block(ui::panel("Start")), right);
        Self::render_now(f, now, lines, title);
    }

    fn handle(&mut self, key: KeyEvent, _: &mut Doc, env: &Env) -> Outcome {
        match key.code {
            // Up from the first entry goes to the tab row.
            KeyCode::Up if self.selected == 0 => Outcome::Ignored,
            KeyCode::Up => {
                self.selected -= 1;
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(START.len() - 1);
                Outcome::Consumed
            }
            KeyCode::Enter => match START[self.selected].1 {
                Go::Page(p) => Outcome::Goto(p),
                Go::Setup => Outcome::Open(Box::new(crate::tui::dialogs::Setup::new(
                    env.settings.clone(),
                ))),
                Go::Quit => Outcome::Quit,
            },
            KeyCode::Char('s') => change_settings(env, false),
            KeyCode::Char('i') => change_settings(env, true),
            KeyCode::Char('m') => Outcome::Goto(PageId::Models),
            KeyCode::Char('u') => Outcome::Copy(UPGRADE.into()),
            _ => Outcome::Ignored,
        }
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("↑↓", "move"), ("enter", "open"), ("letters", "fix")]
    }

    fn preview(&self) -> bool {
        false
    }

    fn shortcuts(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use crate::tui::app::tests::{env, key, screen};
    use crate::tui::doc::tests::temp_paths;

    #[test]
    fn health_offers_to_connect_the_status_line() {
        let (dir, paths) = temp_paths("home");
        let settings = dir.join("settings.json");
        let mut app = App::new(
            Doc::open(paths),
            Env {
                settings: settings.clone(),
                ..env()
            },
        );
        let s = screen(&mut app, 112, 33);
        for text in [
            "Health",
            "statusLine not set",
            "s connect",
            "Your status line now",
            "a sample session",
        ] {
            assert!(s.contains(text), "{text}: {s}");
        }
        // Fix keys work from the tab row too.
        app.handle_key(key(KeyCode::Char('s')));
        assert!(screen(&mut app, 112, 33).contains("Change "));
        app.handle_key(key(KeyCode::Char('y')));
        let s = screen(&mut app, 112, 33);
        assert!(
            s.contains("statusLine runs ccline") && s.contains("i set 2s"),
            "{s}"
        );
        app.handle_key(key(KeyCode::Char('i')));
        app.handle_key(key(KeyCode::Char('y')));
        let line = Settings::load(&settings).unwrap().status_line().unwrap();
        assert_eq!(line.refresh_interval, Some(REFRESH));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
