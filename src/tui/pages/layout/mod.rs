//! 3 Layout: the composer. Metrics from the library go onto the canvas's
//! lines; the inspector edits the selected item. `t` switches between the
//! status line and the line drawn for each background task.

mod canvas;
mod edit;
mod inspector;
mod library;

use super::{Cx, Env, Outcome, Page};
use crate::config::{ItemConfig, ItemSpec, LineConfig};
use crate::metrics::{self, Metric, MetricMeta};
use crate::statusline::Report;
use crate::tui::doc::Doc;
use crate::tui::preview::Target;
use crate::tui::ui::{self, TextInput};
use edit::{Cursor, Loc};
use inspector::{Change, Field};
use library::{Avail, Library};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

/// Narrower than this, one pane shows at a time.
const THREE_PANES: u16 = 100;
const LIBRARY_WIDTH: u16 = 27;
const INSPECTOR_WIDTH: u16 = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pane {
    Library,
    #[default]
    Canvas,
    Inspector,
}

const PANES: [Pane; 3] = [Pane::Library, Pane::Canvas, Pane::Inspector];

impl Pane {
    fn title(self) -> &'static str {
        match self {
            Pane::Library => "Library",
            Pane::Canvas => "Canvas",
            Pane::Inspector => "Inspector",
        }
    }
}

#[derive(Default)]
pub struct LayoutPage {
    pane: Pane,
    target: Target,
    lib: Library,
    cursor: Cursor,
    /// Row in the inspector.
    field: usize,
    /// The inspector field being typed in.
    editing: Option<TextInput>,
}

fn metric_ids(lines: &[LineConfig]) -> Vec<String> {
    lines
        .iter()
        .flat_map(|l| l.left.iter().chain(&l.right))
        .map(|s| s.config().metric.clone())
        .collect()
}

impl LayoutPage {
    fn grow(&self) -> bool {
        self.target == Target::Status
    }

    fn block(&self, pane: Pane, cx: &Cx) -> Block<'static> {
        let b = ui::panel(pane.title());
        if cx.focused && self.pane == pane {
            b.border_style(ui::accent())
        } else {
            b
        }
    }

    /// The selected item, its settings and its metric.
    fn selected(&self, lines: &[LineConfig]) -> Option<(Loc, ItemConfig, &'static MetricMeta)> {
        let (loc, spec) = edit::item(lines, self.cursor)?;
        let config = spec.config().into_owned();
        let meta = &metrics::find(&config.metric)?.meta;
        Some((loc, config, meta))
    }

    /// Change the lines through `f`, which returns where the cursor goes.
    fn apply(
        &mut self,
        doc: &mut Doc,
        f: impl FnOnce(&mut Vec<LineConfig>, Cursor) -> Option<Cursor>,
    ) -> bool {
        let (target, at) = (self.target, self.cursor);
        let mut moved = None;
        doc.edit(|c, _| moved = edit::edit_lines(c, target, |lines| f(lines, at)));
        if let Some(c) = moved {
            self.cursor = c;
        }
        moved.is_some()
    }

    fn edit_item(&self, doc: &mut Doc, loc: Loc, f: impl FnOnce(&mut ItemConfig)) {
        let target = self.target;
        doc.edit(|c, _| edit::edit_lines(c, target, |lines| edit::edit_item(lines, loc, f)));
    }

    fn render_inspector(&self, f: &mut Frame, area: Rect, cx: &Cx, lines: &[LineConfig]) {
        let block = self.block(Pane::Inspector, cx);
        let inner = block.inner(area);
        f.render_widget(block, area);
        let width = inner.width as usize;
        let Some((_, cfg, meta)) = self.selected(lines) else {
            let text = Paragraph::new(vec![
                Line::styled("Pick an item on the canvas", ui::muted()),
                Line::styled("to see its settings here.", ui::muted()),
            ]);
            f.render_widget(text, inner);
            return;
        };
        let palette = crate::statusline::theme(&cx.doc.config).palette;
        let look = inspector::Look {
            tier: cx.doc.config.glyphs,
            palette: &palette,
            depth: cx.preview.depth(cx.doc),
        };
        let mut text = vec![
            Line::from(vec![
                Span::styled(meta.id, ui::strong()),
                Span::styled(format!("  {}", meta.name), ui::muted()),
            ]),
            Line::styled(meta.summary, ui::muted()),
            Line::raw(""),
        ];
        let fields = inspector::fields(&cfg, meta);
        let row = self.field.min(fields.len() - 1);
        let on = cx.focused && self.pane == Pane::Inspector;
        for (i, field) in fields.iter().enumerate() {
            text.push(inspector::row(
                *field,
                &cfg,
                meta,
                &look,
                on && i == row,
                self.editing.as_ref(),
                width,
            ));
        }
        if let Field::Opt(i) = fields[row]
            && on
        {
            text.push(Line::styled(
                format!("  {}", meta.options[i].summary),
                ui::muted(),
            ));
        }

        text.push(Line::raw(""));
        let registry = cx.doc.registry();
        match cx
            .preview
            .item_levels(cx.doc, &registry, cx.env.tz, self.target, &cfg)
        {
            Some(levels) => {
                text.push(Line::styled("Shows", ui::muted()));
                for (name, line) in ["full", "compact", "minimal"].iter().zip(levels) {
                    let mut spans = vec![Span::styled(
                        format!("  {name:<w$}", w = inspector::LABEL),
                        ui::muted(),
                    )];
                    match line {
                        Some(l) => spans.extend(l.spans),
                        None => spans.push(Span::styled("nothing", ui::muted())),
                    }
                    text.push(Line::from(spans));
                }
            }
            None => text.push(Line::styled(
                "Nothing to show in this scenario",
                ui::muted(),
            )),
        }
        let mut notes = Vec::new();
        if let Some(v) = meta.min_cc {
            notes.push(format!("needs CC {v}"));
        }
        if meta.stateful {
            notes.push("keeps history".into());
        }
        if !notes.is_empty() {
            text.push(Line::styled(notes.join(" · "), ui::muted()));
        }
        f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
    }

    fn handle_library(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome {
        if key.code == KeyCode::Esc && !self.lib.typing {
            self.pane = Pane::Canvas;
            return Outcome::Consumed;
        }
        match self.lib.handle(key) {
            library::Key::Add(m) => self.add(m, doc, env),
            library::Key::Consumed => Outcome::Consumed,
            library::Key::Ignored => Outcome::Ignored,
        }
    }

    fn add(&mut self, m: &'static Metric, doc: &mut Doc, env: &Env) -> Outcome {
        let id = m.meta.id;
        match library::avail(&m.meta, self.target, env.cc_version()) {
            Avail::Needs(v) => return Outcome::Toast(format!("{id} needs Claude Code {v}")),
            Avail::TaskOnly => {
                return Outcome::Toast(format!(
                    "{id} describes a background task; t edits the subagent line"
                ));
            }
            Avail::Yes => {}
        }
        self.apply(doc, |lines, at| {
            Some(edit::insert(lines, at, ItemSpec::Id(id.to_string())))
        });
        // A command shows nothing until it has a command line.
        if m.meta.options.first().is_some_and(|o| o.key == "run") {
            self.pane = Pane::Inspector;
            self.field = 0;
            self.editing = Some(TextInput::default());
            return Outcome::Toast("Type the command to run, then enter".into());
        }
        Outcome::Toast(match self.target {
            Target::Status => format!("Added {id} to line {}", self.cursor.line + 1),
            Target::Subagent => format!("Added {id} to the subagent line"),
        })
    }

    fn handle_canvas(&mut self, key: KeyEvent, doc: &mut Doc) -> Outcome {
        let lines = edit::lines(&doc.config, self.target).into_owned();
        let grow = self.grow();
        let moving = key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT);
        let last = if grow {
            lines.len()
        } else {
            lines.len().saturating_sub(1)
        };
        let on_item = edit::item(&lines, self.cursor).is_some();
        match key.code {
            KeyCode::Up if moving => {
                self.apply(doc, |l, at| edit::move_line(l, at, false, grow));
            }
            KeyCode::Down if moving => {
                self.apply(doc, |l, at| edit::move_line(l, at, true, grow));
            }
            KeyCode::Left if moving => {
                self.apply(doc, |l, at| edit::shift(l, at, false));
            }
            KeyCode::Right if moving => {
                self.apply(doc, |l, at| edit::shift(l, at, true));
            }
            KeyCode::Up if self.cursor.line == 0 => return Outcome::Ignored,
            KeyCode::Up => self.cursor.line -= 1,
            KeyCode::Down => self.cursor.line = (self.cursor.line + 1).min(last),
            KeyCode::Left => self.cursor.pos = self.cursor.pos.saturating_sub(1),
            KeyCode::Right => self.cursor.pos += 1,
            KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('x') if on_item => {
                self.apply(doc, |l, at| edit::remove(l, at));
            }
            // On an empty line, x removes the line itself.
            KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('x') | KeyCode::Char('D')
                if grow && self.cursor.line < lines.len() =>
            {
                let n = self.cursor.line;
                self.apply(doc, |l, at| {
                    l.remove(n);
                    Some(at)
                });
                return Outcome::Toast(format!("Removed line {}", n + 1));
            }
            KeyCode::Char('n') if grow => {
                let n = self.cursor.line;
                self.apply(doc, |l, _| Some(edit::new_line(l, n)));
            }
            KeyCode::Enter if self.cursor.line >= lines.len() && grow => {
                let n = lines.len();
                self.apply(doc, |l, _| Some(edit::new_line(l, n)));
            }
            KeyCode::Enter if on_item => {
                self.pane = Pane::Inspector;
                self.field = 0;
            }
            KeyCode::Enter | KeyCode::Char('a') => self.pane = Pane::Library,
            _ => return Outcome::Ignored,
        }
        let lines = edit::lines(&doc.config, self.target);
        self.cursor = edit::clamp(&lines, self.cursor, grow);
        Outcome::Consumed
    }

    fn handle_inspector(&mut self, key: KeyEvent, doc: &mut Doc) -> Outcome {
        let lines = edit::lines(&doc.config, self.target).into_owned();
        let Some((loc, cfg, meta)) = self.selected(&lines) else {
            if key.code == KeyCode::Esc {
                self.pane = Pane::Canvas;
                return Outcome::Consumed;
            }
            return Outcome::Ignored;
        };
        let fields = inspector::fields(&cfg, meta);
        self.field = self.field.min(fields.len() - 1);
        let field = fields[self.field];
        let tier = doc.config.glyphs;
        match key.code {
            KeyCode::Up if self.field == 0 => return Outcome::Ignored,
            KeyCode::Up => self.field -= 1,
            KeyCode::Down => self.field = (self.field + 1).min(fields.len() - 1),
            KeyCode::Esc => self.pane = Pane::Canvas,
            KeyCode::Left | KeyCode::Right if inspector::change(field, meta) == Change::Cycle => {
                let forward = key.code == KeyCode::Right;
                self.edit_item(doc, loc, |c| inspector::cycle(field, c, meta, forward));
            }
            KeyCode::Enter | KeyCode::Char(' ') => match inspector::change(field, meta) {
                Change::Cycle => {
                    self.edit_item(doc, loc, |c| inspector::cycle(field, c, meta, true));
                }
                Change::Text => {
                    self.editing = Some(TextInput::new(&inspector::text(field, &cfg, meta, tier)));
                }
                Change::Dialog => {
                    return Outcome::Toast("The levels editor arrives in the next step".into());
                }
            },
            KeyCode::Delete | KeyCode::Backspace => {
                self.edit_item(doc, loc, |c| inspector::reset(field, c, meta));
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Consumed
    }

    /// Keys while typing in an inspector field.
    fn handle_typing(&mut self, key: KeyEvent, doc: &mut Doc) -> Outcome {
        let Some(input) = self.editing.as_mut() else {
            return Outcome::Ignored;
        };
        match key.code {
            KeyCode::Esc => self.editing = None,
            KeyCode::Enter => {
                let text = input.text();
                let lines = edit::lines(&doc.config, self.target).into_owned();
                let Some((loc, cfg, meta)) = self.selected(&lines) else {
                    self.editing = None;
                    return Outcome::Consumed;
                };
                let fields = inspector::fields(&cfg, meta);
                let field = fields[self.field.min(fields.len() - 1)];
                let tier = doc.config.glyphs;
                let mut error = None;
                self.edit_item(doc, loc, |c| {
                    if let Err(e) = inspector::set_text(field, c, meta, &text, tier) {
                        error = Some(e);
                    }
                });
                if let Some(e) = error {
                    return Outcome::Toast(e);
                }
                self.editing = None;
            }
            _ => {
                input.handle(key);
            }
        }
        Outcome::Consumed
    }
}

impl Page for LayoutPage {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let lines = edit::lines(&cx.doc.config, self.target).into_owned();
        self.cursor = edit::clamp(&lines, self.cursor, self.grow());
        let registry = cx.doc.registry();
        let reports: Vec<Report> = lines
            .iter()
            .map(|l| {
                cx.preview
                    .report(cx.doc, &registry, cx.env.tz, self.target, l)
            })
            .collect();

        let (lib_area, canvas_area, insp_area) = if area.width < THREE_PANES {
            // One pane at a time, with a row naming all three.
            let [tabs, body] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
            let mut spans = vec![Span::raw(" ")];
            for p in PANES {
                let style = if p == self.pane {
                    ui::tab_current()
                } else {
                    ui::muted()
                };
                spans.push(Span::styled(format!(" {} ", p.title()), style));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(" tab next pane", ui::muted()));
            f.render_widget(Paragraph::new(Line::from(spans)), tabs);
            let none = Rect::default();
            match self.pane {
                Pane::Library => (body, none, none),
                Pane::Canvas => (none, body, none),
                Pane::Inspector => (none, none, body),
            }
        } else {
            let [l, c, i] = Layout::horizontal([
                Constraint::Length(LIBRARY_WIDTH),
                Constraint::Fill(1),
                Constraint::Length(INSPECTOR_WIDTH),
            ])
            .spacing(1)
            .areas(area);
            (l, c, i)
        };

        if !lib_area.is_empty() {
            let used = metric_ids(&lines);
            let cc = cx.env.cc_version();
            let target = self.target;
            let block = self.block(Pane::Library, cx);
            let focused = cx.focused && self.pane == Pane::Library;
            self.lib.render(
                f,
                lib_area,
                block,
                focused,
                &|meta| library::avail(meta, target, cc),
                &used,
            );
        }
        if !canvas_area.is_empty() {
            let target_name = match self.target {
                Target::Status => "status line",
                Target::Subagent => "subagent line",
            };
            let block = self.block(Pane::Canvas, cx).title_top(
                Line::from(vec![
                    Span::styled(" t ", ui::muted()),
                    ui::choice(target_name),
                    Span::raw(" "),
                ])
                .right_aligned(),
            );
            let view = canvas::View {
                lines: &lines,
                reports: &reports,
                cursor: self.cursor,
                focused: cx.focused && self.pane == Pane::Canvas,
                target: self.target,
                columns: cx.preview.columns(),
            };
            canvas::render(f, canvas_area, block, &view);
        }
        if !insp_area.is_empty() {
            self.render_inspector(f, insp_area, cx, &lines);
        }
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome {
        if self.editing.is_some() {
            return self.handle_typing(key, doc);
        }
        if self.lib.typing {
            return self.handle_library(key, doc, env);
        }
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let i = PANES.iter().position(|p| *p == self.pane).unwrap_or(0);
                let step = if key.code == KeyCode::Tab { 1 } else { 2 };
                self.pane = PANES[(i + step) % 3];
                return Outcome::Consumed;
            }
            KeyCode::Char('t') => {
                self.target = match self.target {
                    Target::Status => Target::Subagent,
                    Target::Subagent => Target::Status,
                };
                self.cursor = Cursor::default();
                return Outcome::Consumed;
            }
            _ => {}
        }
        match self.pane {
            Pane::Library => self.handle_library(key, doc, env),
            Pane::Canvas => self.handle_canvas(key, doc),
            Pane::Inspector => self.handle_inspector(key, doc),
        }
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        if self.editing.is_some() {
            return vec![("enter", "set"), ("esc", "cancel")];
        }
        match self.pane {
            Pane::Library => vec![
                ("tab", "pane"),
                ("enter", "add"),
                ("←→", "fold"),
                ("/", "search"),
                ("t", "switch line"),
            ],
            Pane::Canvas if self.target == Target::Subagent => vec![
                ("tab", "pane"),
                ("alt+arrows", "move"),
                ("x", "remove"),
                ("t", "status line"),
                ("a", "add"),
            ],
            Pane::Canvas => vec![
                ("tab", "pane"),
                ("alt+arrows", "move"),
                ("x", "remove"),
                ("n", "new line"),
                ("t", "subagent line"),
                ("a", "add"),
            ],
            Pane::Inspector => vec![
                ("tab", "pane"),
                ("←→", "change"),
                ("enter", "edit"),
                ("del", "default"),
            ],
        }
    }

    fn paste(&mut self, text: &str) -> bool {
        if let Some(input) = self.editing.as_mut() {
            input.insert_str(text);
            return true;
        }
        self.lib.paste(text)
    }

    fn target(&self) -> Target {
        self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use crate::tui::app::tests::{ctrl, env, key, screen};
    use crate::tui::doc::tests::temp_paths;
    use crate::tui::pages::PageId;

    fn alt(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    fn ids(line: &LineConfig) -> (Vec<String>, Vec<String>) {
        let ids = |s: &[ItemSpec]| s.iter().map(|i| i.config().metric.clone()).collect();
        (ids(&line.left), ids(&line.right))
    }

    #[test]
    fn composes_lines() {
        let (dir, paths) = temp_paths("layout");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Layout);
        let s = screen(&mut app, 112, 33);
        for text in [
            "Library",
            "Canvas",
            "Inspector",
            "Line 1",
            "Fits in",
            "▸ Identity",
        ] {
            assert!(s.contains(text), "{text}: {s}");
        }
        let (left, right) = ids(&app.doc.config.lines[0]);

        // Library → Identity → effort, added after the first item.
        app.handle_key(key(KeyCode::BackTab));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        let mut want = left.clone();
        want.insert(1, "effort".into());
        assert_eq!(ids(&app.doc.config.lines[0]).0, want);

        // Canvas: move it to the end of the left side, then over to the right.
        app.handle_key(key(KeyCode::Tab));
        for _ in 0..left.len() {
            app.handle_key(alt(KeyCode::Right));
        }
        let (l, r) = ids(&app.doc.config.lines[0]);
        assert_eq!(l, left);
        assert_eq!(r[0], "effort");
        assert_eq!(r[1..], right[..]);

        // Inspector: widget text → bar on the selected effort.
        app.handle_key(key(KeyCode::Enter));
        let s = screen(&mut app, 112, 33);
        assert!(s.contains("Reasoning effort level"), "{s}");
        app.handle_key(key(KeyCode::Right));
        assert!(matches!(
            &app.doc.config.lines[0].right[0],
            ItemSpec::Item(c) if c.widget == Some(crate::widgets::WidgetKind::Bar)
        ));
        app.handle_key(key(KeyCode::Delete));
        assert_eq!(
            app.doc.config.lines[0].right[0],
            ItemSpec::Id("effort".into())
        );

        // Remove it, then undo the removal.
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(ids(&app.doc.config.lines[0]), (left.clone(), right.clone()));
        app.handle_key(ctrl('z'));
        assert_eq!(ids(&app.doc.config.lines[0]).1[0], "effort");

        // A new line below; the "+ new line" row adds one too.
        let n = app.doc.config.lines.len();
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.doc.config.lines.len(), n + 1);
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(app.doc.config.lines.len(), n);

        // Task metrics only go on the subagent line.
        app.handle_key(key(KeyCode::Char('t')));
        assert!(screen(&mut app, 112, 33).contains("Preview · subagent"));
        app.handle_key(key(KeyCode::Char('x')));
        assert!(app.doc.config.subagent.is_some());
        app.handle_key(ctrl('z'));
        assert!(app.doc.config.subagent.is_none());

        // One pane at a time below 100 columns.
        let s = screen(&mut app, 90, 30);
        assert!(
            s.contains("tab next pane") && !s.contains("╭ Library"),
            "{s}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_command_asks_for_its_command_line() {
        let (dir, paths) = temp_paths("layout-cmd");
        let mut app = App::new(Doc::open(paths), env());
        app.open_page(PageId::Layout);
        app.handle_key(key(KeyCode::BackTab));
        for k in "/cmd".chars() {
            app.handle_key(key(KeyCode::Char(k)));
        }
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Enter));
        for k in "uptime".chars() {
            app.handle_key(key(KeyCode::Char(k)));
        }
        app.handle_key(key(KeyCode::Enter));
        let line = &app.doc.config.lines[0];
        let cmd = line
            .left
            .iter()
            .find(|s| s.config().metric == "cmd")
            .unwrap();
        let run = cmd
            .config()
            .options
            .as_ref()
            .and_then(|o| o.get("run").cloned());
        assert_eq!(run, Some(toml::Value::String("uptime".into())));
        assert!(screen(&mut app, 112, 33).contains("cmd uptime"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
