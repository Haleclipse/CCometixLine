//! 1 Home: health of the setup and the ways in.

use super::{Cx, Env, Outcome, Page, PageId};
use crate::config::LoadNote;
use crate::tui::doc::Doc;
use crate::tui::ui;
use crate::version::MIN_SUPPORTED;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

enum Go {
    Page(PageId),
    Quit,
}

const START: [(&str, &str, Go); 5] = [
    ("Open composer", "3", Go::Page(PageId::Layout)),
    ("Browse presets", "2", Go::Page(PageId::Presets)),
    ("Model registry", "5", Go::Page(PageId::Models)),
    ("Claude Code integration", "6", Go::Page(PageId::Claude)),
    ("Quit", "q", Go::Quit),
];

#[derive(Default)]
pub struct Home {
    selected: usize,
}

fn check(mark: char, text: String) -> Line<'static> {
    let style = match mark {
        '✓' => Style::default().fg(ui::OK),
        '!' => Style::default().fg(ui::WARN),
        _ => ui::muted(),
    };
    Line::from(vec![
        Span::styled(format!(" {mark} "), style),
        Span::raw(text),
    ])
}

fn health(cx: &Cx) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    out.push(match cx.env.cc_version() {
        Some(v) if v >= MIN_SUPPORTED => {
            check('✓', format!("Claude Code {v}   (needs {MIN_SUPPORTED})"))
        }
        Some(v) => check(
            '!',
            format!("Claude Code {v} is older than {MIN_SUPPORTED}"),
        ),
        None => check('·', "Claude Code version not seen yet".into()),
    });
    out.push(match &cx.doc.note {
        LoadNote::Fine => check('✓', "config.toml valid".into()),
        LoadNote::Missing => check('·', "no config.toml yet: using the classic preset".into()),
        LoadNote::Migrated => check('·', "v1 config moved to config.v1.toml".into()),
        LoadNote::Invalid(e) => check('!', format!("config.toml invalid: {e}")),
    });
    out.push(match &cx.doc.models_error {
        Some(e) => check('!', format!("models.toml unreadable: {e}")),
        None => check(
            '✓',
            format!("models.toml: {} rules", cx.doc.models.rules.len()),
        ),
    });
    out
}

impl Page for Home {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let [head, panels, _] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(9),
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
            Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
                .spacing(1)
                .areas(panels);
        f.render_widget(
            Paragraph::new(health(cx))
                .wrap(Wrap { trim: false })
                .block(ui::panel("Health")),
            left,
        );

        let start: Vec<Line> = START
            .iter()
            .enumerate()
            .map(|(i, (label, key, _))| {
                let marker = if i == self.selected { "> " } else { "  " };
                let style = if i == self.selected {
                    ui::accent()
                } else {
                    Style::default()
                };
                let width = right.width.saturating_sub(6) as usize;
                let text = format!("{marker}{label:<w$}", w = width.saturating_sub(4));
                Line::from(vec![
                    Span::styled(text, style),
                    Span::styled(format!("{key:>2}"), ui::muted()),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(start).block(ui::panel("Start")), right);
    }

    fn handle(&mut self, key: KeyEvent, _: &mut Doc, _: &Env) -> Outcome {
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.checked_sub(1).unwrap_or(START.len() - 1);
                Outcome::Consumed
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1) % START.len();
                Outcome::Consumed
            }
            KeyCode::Enter => match START[self.selected].2 {
                Go::Page(p) => Outcome::Goto(p),
                Go::Quit => Outcome::Quit,
            },
            _ => Outcome::Ignored,
        }
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("↑↓", "move"), ("enter", "open"), ("1-6", "pages")]
    }
}
