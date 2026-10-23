//! The composer's dialogs, each for one item: A · its widget, C · its icon
//! and D · its levels. Each draws the item as it would look with the
//! choice under the cursor.

use super::edit::{self, Loc};
use super::inspector::sev_name;
use crate::config::ItemConfig;
use crate::metrics::MetricMeta;
use crate::style::Severity;
use crate::style::glyphs::{self, BarGlyphs, GlyphTier, IconKey};
use crate::style::palette::Token;
use crate::style::skin;
use crate::tui::convert;
use crate::tui::dialogs::{Action, Dialog, Reply};
use crate::tui::doc::Doc;
use crate::tui::nerd;
use crate::tui::pages::Cx;
use crate::tui::preview::Target;
use crate::tui::ui::{self, TextInput};
use crate::widgets::{Coloring, WidgetKind};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// The item a dialog edits.
#[derive(Clone, Copy)]
pub struct ItemRef {
    pub target: Target,
    pub loc: Loc,
    pub meta: &'static MetricMeta,
}

impl ItemRef {
    fn current(&self, doc: &Doc) -> ItemConfig {
        let lines = edit::lines(&doc.config, self.target);
        lines
            .get(self.loc.line)
            .and_then(|l| edit::side(l, self.loc.side).get(self.loc.index))
            .map(|s| s.config().into_owned())
            .unwrap_or_default()
    }

    fn apply(&self, doc: &mut Doc, f: impl FnOnce(&mut ItemConfig)) {
        let (target, loc) = (self.target, self.loc);
        doc.edit(|c, _| edit::edit_lines(c, target, |lines| edit::edit_item(lines, loc, f)));
    }

    /// `item` drawn at its fullest for the preview's scenario.
    fn sample(&self, cx: &Cx, item: &ItemConfig) -> Option<Line<'static>> {
        let registry = cx.doc.registry();
        let levels = cx
            .preview
            .item_levels(cx.doc, &registry, cx.env.tz, self.target, item)?;
        levels.into_iter().flatten().next()
    }

    fn title(&self, what: &str) -> String {
        format!("{what} · {}", self.meta.id)
    }
}

fn text_of(line: &Option<Line>) -> String {
    line.as_ref()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .unwrap_or_default()
}

/// `Preview  <the item>` for the bottom of a dialog.
fn sample_row(sample: Option<Line<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(" Preview   ", ui::muted())];
    match sample {
        Some(l) => spans.extend(l.spans),
        None => spans.push(Span::styled("nothing in this scenario", ui::muted())),
    }
    Line::from(spans)
}

const WIDGETS: [(WidgetKind, Option<BarGlyphs>, &str); 10] = [
    (WidgetKind::Text, None, "text"),
    (WidgetKind::Bar, Some(BarGlyphs::Shapes), "bar · shapes"),
    (WidgetKind::Bar, Some(BarGlyphs::Eighths), "bar · eighths"),
    (WidgetKind::Bar, Some(BarGlyphs::Blocks), "bar · blocks"),
    (WidgetKind::Bar, Some(BarGlyphs::Dots), "bar · dots"),
    (WidgetKind::Bar, Some(BarGlyphs::Braille), "bar · braille"),
    (WidgetKind::Bar, Some(BarGlyphs::Ascii), "bar · ascii"),
    (WidgetKind::Gauge, None, "gauge"),
    (WidgetKind::Sparkline, None, "sparkline"),
    (WidgetKind::Pace, None, "pace"),
];

/// A · The widget, each drawn live.
pub struct WidgetDialog {
    item: ItemRef,
    row: usize,
}

impl WidgetDialog {
    pub fn new(item: ItemRef, current: &ItemConfig) -> Self {
        let kind = current.widget.unwrap_or(item.meta.widget);
        let bar = current.bar.unwrap_or_default();
        let row = WIDGETS
            .iter()
            .position(|(k, b, _)| *k == kind && (b.is_none() || *b == Some(bar)))
            .unwrap_or(0);
        WidgetDialog { item, row }
    }

    fn with(current: &ItemConfig, choice: usize) -> ItemConfig {
        let (kind, bar, _) = WIDGETS[choice];
        let mut c = current.clone();
        c.widget = Some(kind);
        if bar.is_some() {
            c.bar = bar;
        }
        c
    }
}

impl Dialog for WidgetDialog {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let rect = ui::popup(f, area, 68, WIDGETS.len() as u16 + 4);
        let current = self.item.current(cx.doc);
        let plain = text_of(&self.item.sample(cx, &Self::with(&current, 0)));
        let mut text = Vec::new();
        for (i, (_, _, label)) in WIDGETS.iter().enumerate() {
            let on = i == self.row;
            let sample = self.item.sample(cx, &Self::with(&current, i));
            // Widgets that draw the same as text have no data to graph.
            let graphless = i > 0 && text_of(&sample) == plain;
            let style = match (on, graphless) {
                (true, _) => ui::accent(),
                (false, true) => ui::muted(),
                (false, false) => Style::default(),
            };
            let mut spans = vec![
                Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
                Span::styled(format!("{label:<15}"), style),
            ];
            if graphless {
                spans.push(Span::styled("no graph for this metric", ui::muted()));
            } else if let Some(l) = sample {
                spans.extend(l.spans);
            }
            text.push(Line::from(spans));
        }
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("↑↓", "choose"),
            ("enter", "apply"),
            ("esc", "cancel"),
        ]));
        f.render_widget(
            Paragraph::new(text).block(ui::panel(&self.item.title("Widget"))),
            rect,
        );
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        match key.code {
            KeyCode::Esc => return Reply::Close,
            KeyCode::Up => self.row = self.row.saturating_sub(1),
            KeyCode::Down => self.row = (self.row + 1).min(WIDGETS.len() - 1),
            KeyCode::Enter => {
                let (kind, bar, label) = WIDGETS[self.row];
                let default = self.item.meta.widget;
                self.item.apply(doc, |c| {
                    c.widget = (kind != default).then_some(kind);
                    if let Some(b) = bar {
                        c.bar = (b != BarGlyphs::default()).then_some(b);
                    }
                });
                return Reply::Then(Action::Toast(format!("Widget: {label}")));
            }
            _ => {}
        }
        Reply::Stay
    }
}

/// Unicode symbols offered besides the icons.
const SYMBOLS: &str = "◔◑◕●○◆◇■□▲△▶★☆✓✗⚑⚡♦⌘⏱⌛◷↑↓↻⇄⑂±∑∂€£¥";
const ICON_COLS: usize = 4;
/// Columns for an icon's name in the grid.
const ICON_NAME: usize = 12;
/// The browser's grid: icons per row, three columns each, and rows shown.
const BROWSE_COLS: usize = 23;
const BROWSE_ROWS: usize = 10;

/// The "All Nerd Fonts" tab: every Nerd Fonts icon, found by name.
struct Browser {
    query: TextInput,
    /// Positions in `nerd::all()`, best match first.
    found: Vec<usize>,
    at: usize,
    /// The first row in view.
    top: usize,
}

impl Browser {
    fn new() -> Browser {
        Browser {
            query: TextInput::default(),
            found: nerd::search(""),
            at: 0,
            top: 0,
        }
    }

    fn refresh(&mut self) {
        self.found = nerd::search(&self.query.text());
        self.at = 0;
        self.top = 0;
    }

    fn focused(&self) -> Option<&'static nerd::Glyph> {
        self.found.get(self.at).map(|&i| &nerd::all()[i])
    }

    /// Move by `by` icons, keeping the cursor's row in view. Past the end
    /// it stops at the last icon; past the start, at the first for a page,
    /// else where it is.
    fn step(&mut self, by: isize) {
        let Some(last) = self.found.len().checked_sub(1) else {
            return;
        };
        self.at = match self.at.checked_add_signed(by) {
            Some(n) => n.min(last),
            None if by < -(BROWSE_COLS as isize) => 0,
            None => self.at,
        };
        let row = self.at / BROWSE_COLS;
        if row < self.top {
            self.top = row;
        } else if row >= self.top + BROWSE_ROWS {
            self.top = row + 1 - BROWSE_ROWS;
        }
    }
}

/// `10617` as `10,617`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// C · The icon: the metric's own, none, any icon or symbol, or typed.
pub struct IconDialog {
    item: ItemRef,
    tier: GlyphTier,
    at: usize,
    custom: Option<TextInput>,
    /// The "All Nerd Fonts" tab, while it is open.
    browse: Option<Browser>,
}

impl IconDialog {
    pub fn new(item: ItemRef, tier: GlyphTier) -> Self {
        let tier = if tier == GlyphTier::Ascii {
            GlyphTier::Unicode
        } else {
            tier
        };
        IconDialog {
            item,
            tier,
            at: 0,
            custom: None,
            browse: None,
        }
    }

    /// `(glyph, name)`; the first is the metric's own, the second none.
    fn entries(&self) -> Vec<(String, String)> {
        let mut out = vec![
            (
                glyphs::icon(self.item.meta.icon, self.tier).to_string(),
                "default".to_string(),
            ),
            (String::new(), "none".to_string()),
        ];
        out.extend(
            IconKey::ALL
                .iter()
                .map(|k| (glyphs::icon(*k, self.tier).to_string(), k.name())),
        );
        if self.tier == GlyphTier::Unicode {
            out.extend(SYMBOLS.chars().map(|c| (c.to_string(), String::new())));
        }
        out
    }

    /// The icon setting the entry stands for.
    fn icon(
        &self,
        at: usize,
        entries: &[(String, String)],
        config_tier: GlyphTier,
    ) -> Option<String> {
        match at {
            0 => None,
            1 => Some(String::new()),
            _ => self.pick(entries[at].0.clone(), config_tier),
        }
    }

    /// The setting for `glyph`: none when it is the metric's own.
    fn pick(&self, glyph: String, config_tier: GlyphTier) -> Option<String> {
        (glyph != glyphs::icon(self.item.meta.icon, config_tier)).then_some(glyph)
    }

    fn candidate(&self, doc: &Doc) -> Option<String> {
        match (&self.custom, &self.browse) {
            (Some(input), _) => Some(input.text().trim().to_string()),
            (None, Some(b)) => match b.focused() {
                Some(g) => self.pick(g.glyph.to_string(), doc.config.glyphs),
                None => self.item.current(doc).icon,
            },
            (None, None) => self.icon(self.at, &self.entries(), doc.config.glyphs),
        }
    }

    /// The tabs: the two glyph sets, then every Nerd Fonts icon.
    fn tabs(&self) -> Vec<Span<'static>> {
        let on = if self.browse.is_some() {
            2
        } else if self.tier == GlyphTier::Nerd {
            0
        } else {
            1
        };
        let mut spans = vec![Span::raw(" ")];
        for (i, label) in ["Nerd", "Unicode", "All Nerd Fonts"]
            .into_iter()
            .enumerate()
        {
            let style = if i == on {
                ui::tab_current()
            } else {
                Style::default()
            };
            spans.push(Span::styled(format!(" {label} "), style));
            spans.push(Span::raw(" "));
        }
        spans
    }

    fn switch_tier(&mut self, tier: GlyphTier) {
        self.browse = None;
        self.tier = tier;
        self.at = self.at.min(self.entries().len() - 1);
    }

    fn render_browse(&self, b: &Browser, f: &mut Frame, area: Rect, cx: &Cx) {
        let rect = ui::popup(f, area, 74, BROWSE_ROWS as u16 + 11);
        let all = nerd::all();
        let mut head = self.tabs();
        head.push(Span::styled(
            format!("  {} of {}", thousands(b.found.len()), thousands(all.len())),
            ui::muted(),
        ));
        let prefixes: Vec<String> = nerd::classes()[..4]
            .iter()
            .map(|c| format!("{c}-"))
            .collect();
        let placeholder = format!(
            "a name, {} …, a codepoint, or paste an icon",
            prefixes.join(" ")
        );
        let mut search = vec![Span::styled(" / ", ui::accent())];
        search.extend(b.query.line(60, true, &placeholder).spans);
        let mut text = vec![
            Line::from(head),
            Line::raw(""),
            Line::from(search),
            Line::raw(""),
        ];
        for r in b.top..b.top + BROWSE_ROWS {
            let mut spans = vec![Span::raw(" ")];
            for c in 0..BROWSE_COLS {
                let i = r * BROWSE_COLS + c;
                let Some(&g) = b.found.get(i) else {
                    break;
                };
                let style = if i == b.at {
                    ui::tab_on()
                } else {
                    Style::default()
                };
                spans.push(Span::styled(format!(" {} ", all[g].glyph), style));
            }
            text.push(Line::from(spans));
        }
        text.push(Line::raw(""));
        text.push(match b.focused() {
            Some(g) => {
                let mut spans = vec![
                    Span::styled(ui::SELECTED, ui::accent()),
                    Span::raw(format!("{}  ", g.glyph)),
                    Span::styled(g.names[0], ui::accent()),
                    Span::styled(format!("  U+{:X}", g.glyph as u32), ui::muted()),
                ];
                if g.names.len() > 1 {
                    spans.push(Span::styled(
                        format!("  also {}", g.names[1..].join(" ")),
                        ui::muted(),
                    ));
                }
                if cx.doc.config.glyphs != GlyphTier::Nerd {
                    spans.push(Span::styled(
                        "  needs a Nerd Font",
                        Style::default().fg(ui::WARN),
                    ));
                }
                Line::from(ui::clip(spans, 72))
            }
            None => Line::styled(" No icon matches", ui::muted()),
        });
        let mut item = self.item.current(cx.doc);
        item.icon = self.candidate(cx.doc);
        text.push(sample_row(self.item.sample(cx, &item)));
        text.push(Line::raw(""));
        let esc = if b.query.text().is_empty() {
            "cancel"
        } else {
            "clear"
        };
        text.push(ui::hints(&[
            ("type", "search"),
            ("arrows", "move"),
            ("enter", "pick"),
            ("tab", "glyphs"),
            ("esc", esc),
        ]));
        let title = format!(
            "{} · Nerd Fonts {}",
            self.item.title("Icon"),
            nerd::version()
        );
        f.render_widget(Paragraph::new(text).block(ui::panel(&title)), rect);
    }

    /// Keys on the "All Nerd Fonts" tab.
    fn handle_browse(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        let Some(b) = self.browse.as_mut() else {
            return Reply::Stay;
        };
        let (row, page) = (BROWSE_COLS as isize, (BROWSE_COLS * BROWSE_ROWS) as isize);
        match key.code {
            KeyCode::Esc if !b.query.text().is_empty() => {
                b.query = TextInput::default();
                b.refresh();
            }
            KeyCode::Esc => return Reply::Close,
            KeyCode::Tab => self.switch_tier(GlyphTier::Nerd),
            KeyCode::BackTab => self.switch_tier(GlyphTier::Unicode),
            KeyCode::Left => b.step(-1),
            KeyCode::Right => b.step(1),
            KeyCode::Up => b.step(-row),
            KeyCode::Down => b.step(row),
            KeyCode::PageUp => b.step(-page),
            KeyCode::PageDown => b.step(page),
            KeyCode::Enter => {
                if let Some(g) = b.focused() {
                    let icon = self.pick(g.glyph.to_string(), doc.config.glyphs);
                    self.item.apply(doc, |c| c.icon = icon);
                    return Reply::Then(Action::Toast(format!("Icon: {}", g.names[0])));
                }
            }
            _ => {
                let before = b.query.text();
                b.query.handle(key);
                if b.query.text() != before {
                    b.refresh();
                }
            }
        }
        Reply::Stay
    }
}

impl Dialog for IconDialog {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        if let Some(b) = &self.browse {
            return self.render_browse(b, f, area, cx);
        }
        let entries = self.entries();
        let rows = entries.len().div_ceil(ICON_COLS);
        let rect = ui::popup(f, area, 74, rows as u16 + 8);
        let mut text = vec![Line::from(self.tabs()), Line::raw("")];
        for r in 0..rows {
            let mut spans = vec![Span::raw(" ")];
            for c in 0..ICON_COLS {
                let i = r * ICON_COLS + c;
                let Some((glyph, name)) = entries.get(i) else {
                    break;
                };
                let on = self.custom.is_none() && i == self.at;
                spans.push(Span::styled(
                    if on { ui::SELECTED } else { "  " },
                    ui::accent(),
                ));
                let glyph_w = ratatui::text::Span::raw(glyph.as_str()).width();
                spans.push(Span::raw(format!(
                    "{glyph}{}",
                    " ".repeat(2usize.saturating_sub(glyph_w))
                )));
                spans.push(Span::styled(
                    format!(
                        " {:<w$}",
                        crate::render::width::truncate(name, ICON_NAME),
                        w = ICON_NAME
                    ),
                    if on { ui::accent() } else { ui::muted() },
                ));
            }
            text.push(Line::from(spans));
        }
        text.push(Line::raw(""));
        let custom = match &self.custom {
            Some(input) => {
                let mut spans = vec![Span::styled(" Custom    ", ui::accent())];
                spans.extend(input.line(20, true, "type any character").spans);
                Line::from(spans)
            }
            None => Line::from(vec![
                Span::styled(" Custom    ", ui::muted()),
                Span::styled("c", ui::accent()),
                Span::styled(" type your own", ui::muted()),
            ]),
        };
        text.push(custom);
        let mut item = self.item.current(cx.doc);
        item.icon = self.candidate(cx.doc);
        text.push(sample_row(self.item.sample(cx, &item)));
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("arrows", "move"),
            ("tab", "glyphs"),
            ("enter", "pick"),
            ("esc", "cancel"),
        ]));
        f.render_widget(
            Paragraph::new(text).block(ui::panel(&self.item.title("Icon"))),
            rect,
        );
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        if let Some(input) = self.custom.as_mut() {
            match key.code {
                KeyCode::Esc => self.custom = None,
                KeyCode::Enter => {
                    let icon = Some(input.text().trim().to_string());
                    self.item.apply(doc, |c| c.icon = icon);
                    return Reply::Then(Action::Toast("Icon set".into()));
                }
                _ => {
                    input.handle(key);
                }
            }
            return Reply::Stay;
        }
        if self.browse.is_some() {
            return self.handle_browse(key, doc);
        }
        let n = self.entries().len();
        match key.code {
            KeyCode::Esc => return Reply::Close,
            KeyCode::Tab => match self.tier {
                GlyphTier::Nerd => self.switch_tier(GlyphTier::Unicode),
                _ => self.browse = Some(Browser::new()),
            },
            KeyCode::BackTab => match self.tier {
                GlyphTier::Nerd => self.browse = Some(Browser::new()),
                _ => self.switch_tier(GlyphTier::Nerd),
            },
            KeyCode::Char('c') => self.custom = Some(TextInput::default()),
            KeyCode::Left => self.at = self.at.saturating_sub(1),
            KeyCode::Right => self.at = (self.at + 1).min(n - 1),
            KeyCode::Up => self.at = self.at.checked_sub(ICON_COLS).unwrap_or(self.at),
            KeyCode::Down => self.at = (self.at + ICON_COLS).min(n - 1),
            KeyCode::Enter => {
                let icon = self.icon(self.at, &self.entries(), doc.config.glyphs);
                self.item.apply(doc, |c| c.icon = icon);
                return Reply::Then(Action::Toast("Icon set".into()));
            }
            _ => {}
        }
        Reply::Stay
    }

    fn paste(&mut self, text: &str) -> bool {
        match self.browse.as_mut() {
            Some(b) => {
                b.query.insert_str(text.trim());
                b.refresh();
            }
            None => self.custom = Some(TextInput::new(text.trim())),
        }
        true
    }
}

const SEVERITIES: [Severity; 4] = [
    Severity::Good,
    Severity::Warn,
    Severity::Crit,
    Severity::Normal,
];
/// Cells in the 0–100 scale.
const SCALE: usize = 50;

fn parse_sev(name: &str) -> Severity {
    SEVERITIES
        .iter()
        .copied()
        .find(|s| sev_name(*s) == name || (name == "good" && *s == Severity::Good))
        .unwrap_or(Severity::Normal)
}

fn sev_token(s: Severity) -> Option<Token> {
    match s {
        Severity::Good => Some(Token::Ok),
        Severity::Warn => Some(Token::Warn),
        Severity::Crit => Some(Token::Crit),
        Severity::Normal => None,
    }
}

/// D · Levels: thresholds in percent, and stepped or gradient coloring.
pub struct LevelsDialog {
    item: ItemRef,
    steps: Vec<(u8, Severity)>,
    gradient: bool,
    /// 0 coloring, then the steps, then "add".
    row: usize,
}

impl LevelsDialog {
    pub fn new(item: ItemRef, current: &ItemConfig) -> Self {
        let steps = match &current.levels {
            Some(l) => l
                .iter()
                .map(|(at, s)| (at.clamp(0.0, 100.0) as u8, parse_sev(s)))
                .collect(),
            None => item
                .meta
                .levels
                .iter()
                .map(|(at, s)| (*at as u8, *s))
                .collect(),
        };
        LevelsDialog {
            gradient: current.coloring == Some(Coloring::Gradient),
            item,
            steps,
            row: 1,
        }
    }

    fn levels(&self) -> Vec<(f64, String)> {
        let mut steps = self.steps.clone();
        steps.sort_by_key(|(at, _)| *at);
        steps
            .into_iter()
            .map(|(at, s)| (at as f64, sev_name(s).to_string()))
            .collect()
    }

    /// The settings to store: nothing where they match the metric's.
    fn settings(&self) -> (Option<Vec<(f64, String)>>, Option<Coloring>) {
        let levels = self.levels();
        let default: Vec<(f64, String)> = self
            .item
            .meta
            .levels
            .iter()
            .map(|(at, s)| (*at, sev_name(*s).to_string()))
            .collect();
        (
            (levels != default).then_some(levels),
            self.gradient.then_some(Coloring::Gradient),
        )
    }

    fn severity_at(&self, pct: f64) -> Severity {
        self.steps
            .iter()
            .filter(|(at, _)| pct >= *at as f64)
            .max_by_key(|(at, _)| *at)
            .map(|(_, s)| *s)
            .unwrap_or(Severity::Normal)
    }

    fn scale(&self, cx: &Cx) -> Vec<Line<'static>> {
        let theme = crate::statusline::theme(&cx.doc.config);
        let depth = cx.preview.depth(cx.doc);
        let paint = |c: Option<crate::style::Color>| {
            c.and_then(|c| convert::color(c, depth))
                .map(|c| Style::default().fg(c))
                .unwrap_or_default()
        };
        let mut cells = vec![Span::raw("   ")];
        for i in 0..SCALE {
            let t = (i as f32 + 0.5) / SCALE as f32;
            let color = if self.gradient {
                skin::gradient(&theme, t)
            } else {
                sev_token(self.severity_at(t as f64 * 100.0))
                    .and_then(|tok| theme.palette.color(tok))
            };
            cells.push(Span::styled("━", paint(color)));
        }
        // Tick labels under each threshold.
        let mut ticks = vec![' '; SCALE + 4];
        let mut put = |pos: usize, label: String| {
            for (k, ch) in label.chars().enumerate() {
                if let Some(slot) = ticks.get_mut(pos + k) {
                    *slot = ch;
                }
            }
        };
        put(0, "0".into());
        for (at, _) in &self.steps {
            put(*at as usize * SCALE / 100, at.to_string());
        }
        put(SCALE - 2, "100".into());
        vec![
            Line::from(cells),
            Line::styled(
                format!("   {}", ticks.iter().collect::<String>()),
                ui::muted(),
            ),
        ]
    }
}

impl Dialog for LevelsDialog {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx) {
        let rect = ui::popup(f, area, 74, self.steps.len() as u16 + 12);
        let row = |i: usize, label: String, value: Vec<Span<'static>>| {
            let on = i == self.row;
            let mut spans = vec![
                Span::styled(if on { ui::SELECTED } else { "  " }, ui::accent()),
                Span::styled(
                    format!("{label:<11}"),
                    if on { ui::accent() } else { Style::default() },
                ),
            ];
            spans.extend(value);
            Line::from(spans)
        };
        let coloring = if self.gradient { "gradient" } else { "steps" };
        let mut text = vec![
            row(
                0,
                "Coloring".into(),
                vec![
                    ui::choice(coloring),
                    Span::styled("   steps · gradient", ui::muted()),
                ],
            ),
            Line::raw(""),
        ];
        text.extend(self.scale(cx));
        text.push(Line::raw(""));
        for (i, (at, sev)) in self.steps.iter().enumerate() {
            text.push(row(
                i + 1,
                format!("Step {}", i + 1),
                vec![
                    Span::raw(format!("from {at:>3}%   ")),
                    ui::choice(sev_name(*sev)),
                ],
            ));
        }
        let add = self.steps.len() + 1;
        text.push(row(add, "+ add step".into(), Vec::new()));
        text.push(Line::raw(""));
        let mut item = self.item.current(cx.doc);
        (item.levels, item.coloring) = self.settings();
        text.push(sample_row(self.item.sample(cx, &item)));
        text.push(Line::raw(""));
        text.push(ui::hints(&[
            ("←→", "±5"),
            ("space", "color"),
            ("del", "remove"),
            ("r", "reset"),
            ("enter", "apply"),
            ("esc", "cancel"),
        ]));
        f.render_widget(
            Paragraph::new(text).block(ui::panel(&self.item.title("Levels"))),
            rect,
        );
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc) -> Reply {
        let add = self.steps.len() + 1;
        let fine = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Esc => return Reply::Close,
            KeyCode::Up => self.row = self.row.saturating_sub(1),
            KeyCode::Down => self.row = (self.row + 1).min(add),
            KeyCode::Left | KeyCode::Right if self.row == 0 => self.gradient = !self.gradient,
            KeyCode::Left | KeyCode::Right if (1..add).contains(&self.row) => {
                let delta: i16 = if fine { 1 } else { 5 };
                let delta = if key.code == KeyCode::Left {
                    -delta
                } else {
                    delta
                };
                let at = &mut self.steps[self.row - 1].0;
                *at = (*at as i16 + delta).clamp(0, 100) as u8;
            }
            KeyCode::Char(' ') if (1..add).contains(&self.row) => {
                let sev = &mut self.steps[self.row - 1].1;
                let i = SEVERITIES.iter().position(|s| s == sev).unwrap_or(0);
                *sev = SEVERITIES[(i + 1) % SEVERITIES.len()];
            }
            KeyCode::Char(' ') if self.row == 0 => self.gradient = !self.gradient,
            KeyCode::Delete | KeyCode::Backspace if (1..add).contains(&self.row) => {
                self.steps.remove(self.row - 1);
                self.row = self.row.min(self.steps.len() + 1);
            }
            KeyCode::Enter if self.row == add => {
                let last = self.steps.iter().map(|(at, _)| *at).max().unwrap_or(40);
                self.steps.push(((last + 10).min(100), Severity::Crit));
                self.row = self.steps.len();
            }
            KeyCode::Char('r') => {
                let meta = self.item.meta;
                self.steps = meta.levels.iter().map(|(at, s)| (*at as u8, *s)).collect();
                self.gradient = false;
                self.row = self.row.min(self.steps.len() + 1);
            }
            KeyCode::Enter => {
                let (levels, coloring) = self.settings();
                self.item.apply(doc, |c| {
                    c.levels = levels;
                    c.coloring = coloring;
                });
                return Reply::Then(Action::Toast(format!(
                    "Levels: {}",
                    super::inspector::levels_text(&self.item.current(doc), self.item.meta)
                )));
            }
            _ => {}
        }
        Reply::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ItemSpec;
    use crate::metrics;
    use crate::style::skin::Side;
    use crate::tui::doc::tests::temp_paths;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn browses_every_nerd_font_icon() {
        let (dir, paths) = temp_paths("layout-nerd");
        let mut doc = Doc::open(paths);
        doc.edit(|c, _| {
            c.lines = vec![crate::config::LineConfig {
                left: vec![ItemSpec::Id("dir".into())],
                right: Vec::new(),
            }]
        });
        let item = ItemRef {
            target: Target::Status,
            loc: Loc {
                line: 0,
                side: Side::Left,
                index: 0,
            },
            meta: &metrics::find("dir").unwrap().meta,
        };
        let typed = |d: &mut IconDialog, doc: &mut Doc, text: &str| {
            for c in text.chars() {
                d.handle(key(KeyCode::Char(c)), doc);
            }
        };

        // Nerd, Unicode, then every icon; typing searches.
        let mut d = IconDialog::new(item, GlyphTier::Nerd);
        d.handle(key(KeyCode::Tab), &mut doc);
        assert!(d.browse.is_none() && d.tier == GlyphTier::Unicode);
        d.handle(key(KeyCode::Tab), &mut doc);
        assert_eq!(d.browse.as_ref().unwrap().found.len(), nerd::all().len());
        typed(&mut d, &mut doc, "folder open");
        let first = d.browse.as_ref().unwrap().focused().unwrap();
        assert_eq!(first.names[0], "md-folder_open");
        d.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(item.current(&doc).icon.as_deref(), Some("\u{f0770}"));

        // A pasted icon finds its name; the metric's own leaves the setting.
        let mut d = IconDialog::new(item, GlyphTier::Nerd);
        d.handle(key(KeyCode::BackTab), &mut doc);
        d.paste("\u{f024b}");
        assert_eq!(
            d.browse.as_ref().unwrap().focused().unwrap().names[0],
            "md-folder"
        );
        d.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(item.current(&doc).icon, None);

        // Esc clears the search first, then closes.
        let mut d = IconDialog::new(item, GlyphTier::Nerd);
        d.handle(key(KeyCode::BackTab), &mut doc);
        typed(&mut d, &mut doc, "git");
        d.handle(key(KeyCode::Down), &mut doc);
        assert_eq!(d.browse.as_ref().unwrap().at, BROWSE_COLS);
        assert!(matches!(d.handle(key(KeyCode::Esc), &mut doc), Reply::Stay));
        assert!(d.browse.as_ref().unwrap().query.text().is_empty());
        assert!(matches!(
            d.handle(key(KeyCode::Esc), &mut doc),
            Reply::Close
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn widget_icon_and_levels_write_the_item() {
        let (dir, paths) = temp_paths("layout-dialogs");
        let mut doc = Doc::open(paths);
        doc.edit(|c, _| {
            c.lines = vec![crate::config::LineConfig {
                left: vec![ItemSpec::Id("context".into())],
                right: Vec::new(),
            }]
        });
        let item = ItemRef {
            target: Target::Status,
            loc: Loc {
                line: 0,
                side: Side::Left,
                index: 0,
            },
            meta: &metrics::find("context").unwrap().meta,
        };

        let mut w = WidgetDialog::new(item, &ItemConfig::default());
        assert_eq!(w.row, 1, "context draws a bar of shapes by default");
        w.handle(key(KeyCode::Down), &mut doc);
        w.handle(key(KeyCode::Enter), &mut doc);
        let c = item.current(&doc);
        assert_eq!((c.widget, c.bar), (None, Some(BarGlyphs::Eighths)));

        let mut icon = IconDialog::new(item, GlyphTier::Unicode);
        icon.handle(key(KeyCode::Right), &mut doc);
        icon.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(item.current(&doc).icon.as_deref(), Some(""));
        let mut icon = IconDialog::new(item, GlyphTier::Unicode);
        icon.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(item.current(&doc).icon, None);

        let mut levels = LevelsDialog::new(item, &ItemConfig::default());
        assert_eq!(
            levels.steps,
            vec![(50, Severity::Warn), (80, Severity::Crit)]
        );
        levels.handle(key(KeyCode::Right), &mut doc);
        levels.handle(key(KeyCode::Up), &mut doc);
        levels.handle(key(KeyCode::Right), &mut doc);
        levels.handle(key(KeyCode::Enter), &mut doc);
        let c = item.current(&doc);
        assert_eq!(c.coloring, Some(Coloring::Gradient));
        assert_eq!(
            c.levels,
            Some(vec![(55.0, "warn".to_string()), (80.0, "crit".to_string())])
        );

        // Back to the metric's levels stores nothing.
        let mut levels = LevelsDialog::new(item, &c);
        levels.handle(key(KeyCode::Char('r')), &mut doc);
        levels.handle(key(KeyCode::Enter), &mut doc);
        assert_eq!(
            item.current(&doc),
            ItemConfig {
                metric: "context".into(),
                bar: Some(BarGlyphs::Eighths),
                ..ItemConfig::default()
            }
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
