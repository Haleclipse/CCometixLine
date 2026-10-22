//! H · First-run setup: what's there, the font, a preset, and pointing
//! Claude Code at ccline. Finishing saves the config.

use super::{Action, Dialog, Reply};
use crate::cc_settings::{self, Mark, Settings};
use crate::config::LoadNote;
use crate::presets::{self, PRESETS, Parts};
use crate::style::glyphs::{self, GlyphTier, IconKey};
use crate::tui::doc::Doc;
use crate::tui::pages::Cx;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::path::PathBuf;

const WIDTH: u16 = 76;
const STEPS: [&str; 4] = ["Detect", "Fonts", "Preset", "Connect"];

pub struct Setup {
    step: usize,
    preset: usize,
    /// Claude Code's settings file.
    settings: PathBuf,
}

impl Setup {
    pub fn new(settings: PathBuf) -> Self {
        Setup {
            step: 0,
            preset: PRESETS
                .iter()
                .position(|p| p.name == presets::DEFAULT)
                .unwrap_or(0),
            settings,
        }
    }

    fn progress(&self) -> Line<'static> {
        let mut spans = vec![Span::raw(" ")];
        for (i, name) in STEPS.iter().enumerate() {
            let (dot, style) = if i <= self.step {
                ("●", ui::accent())
            } else {
                ("○", ui::muted())
            };
            spans.push(Span::styled(format!("{dot} {name}   "), style));
        }
        Line::from(spans)
    }

    /// The settings with `statusLine` pointing at ccline, if that changes
    /// anything.
    fn connection(&self) -> Result<Option<(Settings, Settings)>, String> {
        let base = Settings::load(&self.settings)?;
        let mut line = base.status_line().unwrap_or_default();
        if cc_settings::is_ccline(&line.command) {
            return Ok(None);
        }
        line.command = cc_settings::suggested_command();
        let mut draft = base.clone();
        draft.set_status_line(Some(&line));
        Ok(Some((base, draft)))
    }

    fn detect(&self, cx: &Cx) -> Vec<Line<'static>> {
        let row = |label: &str, value: String| {
            Line::from(vec![
                Span::styled(format!("  {label:<14}"), ui::muted()),
                Span::raw(value),
            ])
        };
        let cc = match cx.env.cc_version() {
            Some(v) => format!("{v}, from your last session"),
            None => "not seen yet; fine for a first run".into(),
        };
        let status = match Settings::load(&self.settings).map(|s| s.status_line()) {
            Ok(Some(l)) if cc_settings::is_ccline(&l.command) => "runs ccline".to_string(),
            Ok(Some(l)) => format!("runs {}", crate::render::width::truncate(&l.command, 40)),
            Ok(None) => "not set".into(),
            Err(e) => e,
        };
        let config = match &cx.doc.note {
            LoadNote::Missing => "none yet".to_string(),
            LoadNote::V1 => "a v1 config; kept as config.v1.toml when saved".into(),
            LoadNote::Invalid(e) => format!("unreadable ({e}); replaced when saved"),
            LoadNote::Fine | LoadNote::Migrated => "a v2 config".into(),
        };
        let depth = format!("{:?}", crate::env::color_depth()).to_lowercase();
        vec![
            Line::raw(" ccline found:"),
            Line::raw(""),
            row("Claude Code", cc),
            row("statusLine", status),
            row("config", config),
            row("colors", depth),
        ]
    }

    fn fonts(&self) -> Vec<Line<'static>> {
        let sample = |tier: GlyphTier| -> String {
            [
                IconKey::Model,
                IconKey::Dir,
                IconKey::Git,
                IconKey::Context,
                IconKey::Cost,
            ]
            .iter()
            .map(|k| glyphs::icon(*k, tier))
            .collect::<Vec<_>>()
            .join("  ")
        };
        vec![
            Line::raw(" Do these show as icons, not boxes or question marks?"),
            Line::raw(""),
            Line::from(vec![
                Span::styled("   Nerd      ", ui::muted()),
                Span::styled(
                    format!("{}    \u{e0b6}\u{e0b4} \u{e0b0}", sample(GlyphTier::Nerd)),
                    ui::strong(),
                ),
            ]),
            Line::from(vec![
                Span::styled("   Unicode   ", ui::muted()),
                Span::styled(sample(GlyphTier::Unicode), ui::strong()),
            ]),
        ]
    }

    fn presets(&self, cx: &Cx) -> Vec<Line<'static>> {
        let registry = cx.doc.registry();
        let mut lines = vec![
            Line::raw(" Pick a starting point; everything can be changed later."),
            Line::raw(""),
        ];
        for (i, p) in PRESETS.iter().enumerate() {
            let on = i == self.preset;
            lines.push(Line::from(ui::clip(
                vec![
                    Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
                    Span::styled(
                        format!("{:<14}", p.name),
                        if on { ui::accent() } else { Style::default() },
                    ),
                    Span::styled(p.summary.to_string(), ui::muted()),
                ],
                WIDTH as usize - 3,
            )));
        }
        lines.push(Line::raw(""));
        if let Some(config) = presets::get(PRESETS[self.preset].name) {
            let mut config = config;
            config.glyphs = cx.doc.config.glyphs;
            let width = WIDTH - 4;
            for l in cx
                .preview
                .render(&config, cx.doc, &registry, cx.env.tz, width)
                .into_iter()
                .take(3)
            {
                let mut spans = vec![Span::raw("  ")];
                spans.extend(l.spans);
                lines.push(Line::from(spans));
            }
        }
        lines
    }

    fn connect(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        match self.connection() {
            Err(e) => {
                lines.push(Line::styled(
                    format!(" Can't change settings.json: {e}"),
                    Style::default().fg(ui::BAD),
                ));
            }
            Ok(None) => {
                lines.push(Line::styled(
                    " Claude Code already runs ccline.",
                    Style::default().fg(ui::OK),
                ));
            }
            Ok(Some((base, draft))) => {
                lines.push(Line::raw(" Point Claude Code's status line at ccline:"));
                lines.push(Line::raw(""));
                for (mark, line) in draft.diff(&base).into_iter().take(10) {
                    let (sign, style) = match mark {
                        Mark::Same => ("  ", Style::default()),
                        Mark::Added => ("+ ", Style::default().fg(ui::OK)),
                        Mark::Removed => ("- ", Style::default().fg(ui::BAD)),
                    };
                    lines.push(Line::from(ui::clip(
                        vec![Span::styled(format!("  {sign}{line}"), style)],
                        WIDTH as usize - 2,
                    )));
                }
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    if base.exists() {
                        " The old file is kept as settings.json.ccline-bak."
                    } else {
                        " There is no settings.json yet; this creates it."
                    },
                    ui::muted(),
                ));
            }
        }
        lines
    }

    fn hints(&self) -> Line<'static> {
        match self.step {
            0 => ui::hints(&[("enter", "next"), ("esc", "skip setup")]),
            1 => ui::hints(&[
                ("N", "Nerd Font"),
                ("U", "Unicode only"),
                ("A", "ASCII only"),
                ("esc", "skip setup"),
            ]),
            2 => ui::hints(&[("↑↓", "choose"), ("enter", "use it"), ("esc", "skip setup")]),
            _ => match self.connection() {
                Ok(Some(_)) => ui::hints(&[
                    ("Y", "apply and finish"),
                    ("S", "skip this"),
                    ("esc", "close"),
                ]),
                _ => ui::hints(&[("enter", "finish"), ("esc", "close")]),
            },
        }
    }

    /// Save the config and close.
    fn finish(&self) -> Reply {
        Reply::Then(Action::Save {
            force: false,
            then_quit: false,
        })
    }
}

impl Dialog for Setup {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let body = match self.step {
            0 => self.detect(cx),
            1 => self.fonts(),
            2 => self.presets(cx),
            _ => self.connect(),
        };
        let height = body.len() as u16 + 6;
        let rect = ui::popup(f, area, WIDTH, height);
        let mut text = vec![self.progress(), Line::raw("")];
        text.extend(body);
        text.push(Line::raw(""));
        text.push(self.hints());
        let title = format!(
            "Setup · step {} of {} · {}",
            self.step + 1,
            STEPS.len(),
            STEPS[self.step].to_lowercase()
        );
        f.render_widget(Paragraph::new(text).block(ui::panel(&title)), rect);
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        if key.code == KeyCode::Esc {
            return Reply::Close;
        }
        match (self.step, key.code) {
            (0, KeyCode::Enter) => self.step = 1,
            (1, KeyCode::Char(c)) => {
                let tier = match c.to_ascii_lowercase() {
                    'n' => GlyphTier::Nerd,
                    'u' => GlyphTier::Unicode,
                    'a' => GlyphTier::Ascii,
                    _ => return Reply::Stay,
                };
                doc.edit(|c, _| c.glyphs = tier);
                self.step = 2;
            }
            (2, KeyCode::Up) => self.preset = self.preset.saturating_sub(1),
            (2, KeyCode::Down) => self.preset = (self.preset + 1).min(PRESETS.len() - 1),
            (2, KeyCode::Enter) => {
                if let Some(preset) = presets::get(PRESETS[self.preset].name) {
                    // The font answer stands over the preset's glyphs.
                    doc.edit(|c, _| {
                        let glyphs = c.glyphs;
                        *c = presets::apply(c, &preset, Parts::All);
                        c.glyphs = glyphs;
                    });
                }
                self.step = 3;
            }
            (3, KeyCode::Char('y' | 'Y')) => {
                if let Ok(Some((base, draft))) = self.connection()
                    && let Err(e) = draft.save(&base)
                {
                    return Reply::Then(Action::Error(format!(
                        "Couldn't write settings.json: {e}"
                    )));
                }
                return self.finish();
            }
            (3, KeyCode::Char('s' | 'S') | KeyCode::Enter) => return self.finish(),
            _ => {}
        }
        Reply::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::doc::tests::temp_paths;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn walks_through_and_connects() {
        let (dir, paths) = temp_paths("setup");
        let settings = dir.join("settings.json");
        let mut doc = Doc::open(paths);
        let mut s = Setup::new(settings.clone());
        s.handle(key(KeyCode::Enter), &mut doc);
        s.handle(key(KeyCode::Char('u')), &mut doc);
        assert_eq!(doc.config.glyphs, GlyphTier::Unicode);
        s.handle(key(KeyCode::Down), &mut doc);
        s.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(doc.config.preset.as_deref(), Some(PRESETS[s.preset].name));
        assert_eq!(doc.config.glyphs, GlyphTier::Unicode);
        assert!(matches!(
            s.handle(key(KeyCode::Char('y')), &mut doc),
            Reply::Then(Action::Save { .. })
        ));
        let line = Settings::load(&settings).unwrap().status_line().unwrap();
        assert!(cc_settings::is_ccline(&line.command));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
