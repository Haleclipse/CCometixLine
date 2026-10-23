//! The preview bar shared by the editing pages: the config being edited,
//! rendered by the real pipeline against a sample session.

use super::convert;
use super::doc::Doc;
use super::ui;
use crate::config::{Config, ItemConfig, LineConfig};
use crate::env::AutoCompact;
use crate::git::GitInfo;
use crate::input::{Input, SubagentInput};
use crate::metrics::{Commands, GitSource};
use crate::models::Registry;
use crate::scenarios::{self, BASE, SCENARIOS};
use crate::state::StateView;
use crate::state::session::{Sample, SessionState};
use crate::statusline::{self, Frame, Report};
use crate::style::GlyphTier;
use crate::style::color::ColorDepth;
use chrono::FixedOffset;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use std::rc::Rc;

/// Most lines the bar shows.
pub const MAX_LINES: usize = 6;
const MIN_WIDTH: u16 = 20;

const DEPTHS: [Option<ColorDepth>; 5] = [
    None,
    Some(ColorDepth::TrueColor),
    Some(ColorDepth::Ansi256),
    Some(ColorDepth::Ansi16),
    Some(ColorDepth::None),
];
const TIERS: [Option<GlyphTier>; 4] = [
    None,
    Some(GlyphTier::Nerd),
    Some(GlyphTier::Unicode),
    Some(GlyphTier::Ascii),
];

/// Which configured lines the bar shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    /// The status line, under the prompt.
    #[default]
    Status,
    /// The line drawn for each background task.
    Subagent,
}

/// The newest real session, shown after the sample scenarios.
#[derive(Clone, Debug)]
pub struct Replay {
    pub input: Input,
    /// When it was last drawn, so timers read as they did then.
    pub now: i64,
    pub samples: Vec<Sample>,
    pub git: Option<GitInfo>,
    /// What the companion mod wrote for the session, if it ran there.
    pub bridge: Option<Rc<crate::state::bridge::Bridge>>,
}

impl Replay {
    /// The session's last payload, with git read once from its directory.
    pub fn from_session(s: &SessionState) -> Option<Replay> {
        let input = Input::parse(&s.last_input.as_ref()?.to_string()).ok()?;
        let git = input.current_dir().and_then(|d| {
            crate::git::status_within(
                std::path::Path::new(d),
                std::time::Duration::from_millis(800),
            )
        });
        let last = s.samples.last().map(|x| x.t).unwrap_or(0);
        let now = s.saved_at.max(last);
        let bridge = input
            .session_id
            .as_deref()
            .and_then(|id| crate::state::bridge::read(id, now * 1000))
            .map(Rc::new);
        Some(Replay {
            input,
            now,
            samples: s.samples.clone(),
            git,
            bridge,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Preview {
    /// Index into `SCENARIOS`; one past the end is the replay.
    pub scenario: usize,
    /// Columns to lay out in; `None` fits the bar.
    pub width: Option<u16>,
    /// Columns the bar has, from the last draw.
    pub fit: u16,
    depth: usize,
    tier: usize,
    replay: Option<Rc<Replay>>,
}

impl Default for Preview {
    fn default() -> Self {
        Preview {
            scenario: 0,
            width: None,
            fit: 96,
            depth: 0,
            tier: 0,
            replay: None,
        }
    }
}

/// A sample render's input: the session, or one background task of it.
struct Scene {
    input: Input,
    sub: Option<SubagentInput>,
    frame: Frame,
}

impl Scene {
    fn task(&self) -> Option<&crate::input::Task> {
        self.sub.as_ref()?.tasks.first()
    }
}

fn depth_name(d: ColorDepth) -> &'static str {
    match d {
        ColorDepth::TrueColor => "true",
        ColorDepth::Ansi256 => "256",
        ColorDepth::Ansi16 => "16",
        ColorDepth::None => "none",
    }
}

fn tier_name(t: GlyphTier) -> &'static str {
    match t {
        GlyphTier::Nerd => "nerd",
        GlyphTier::Unicode => "unicode",
        GlyphTier::Ascii => "ascii",
    }
}

impl Preview {
    pub fn select(&mut self, name: &str) {
        if let Some(i) = SCENARIOS.iter().position(|s| s.name == name) {
            self.scenario = i;
        }
    }

    pub fn next_scenario(&mut self, forward: bool) {
        let n = SCENARIOS.len() + usize::from(self.replay.is_some());
        self.scenario = (self.scenario + if forward { 1 } else { n - 1 }) % n;
    }

    pub fn set_replay(&mut self, replay: Option<Replay>) {
        self.replay = replay.map(Rc::new);
        self.scenario = self.scenario.min(SCENARIOS.len() - 1);
    }

    pub fn replaying(&self) -> bool {
        self.replay.is_some() && self.scenario >= SCENARIOS.len()
    }

    /// Switch between the replay and the first sample; false without one.
    pub fn toggle_replay(&mut self) -> bool {
        if self.replay.is_none() {
            return false;
        }
        self.scenario = if self.replaying() { 0 } else { SCENARIOS.len() };
        true
    }

    /// A copy showing the replay, if there is one.
    pub fn replayed(&self) -> Option<Preview> {
        let mut p = self.clone();
        p.replay.as_ref()?;
        p.scenario = SCENARIOS.len();
        Some(p)
    }

    pub fn resize(&mut self, delta: i32) {
        let fit = self.fit;
        let w = self.width.unwrap_or(fit) as i32 + delta;
        let w = w.clamp(MIN_WIDTH as i32, fit as i32) as u16;
        self.width = (w != fit).then_some(w);
    }

    pub fn cycle_depth(&mut self) {
        self.depth = (self.depth + 1) % DEPTHS.len();
    }

    pub fn cycle_glyphs(&mut self) {
        self.tier = (self.tier + 1) % TIERS.len();
    }

    pub fn depth(&self, doc: &Doc) -> ColorDepth {
        DEPTHS[self.depth].unwrap_or_else(|| doc.config.color_depth.resolve())
    }

    /// Columns the lines are laid out in.
    pub fn columns(&self) -> u16 {
        self.width.unwrap_or(self.fit).min(self.fit)
    }

    /// `config` with the bar's glyph override.
    fn effective(&self, config: &Config) -> Config {
        let mut config = config.clone();
        if let Some(t) = TIERS[self.tier] {
            config.glyphs = t;
        }
        config
    }

    /// The current scenario in `width` columns; for `Subagent`, its first
    /// background task.
    fn scene(&self, tz: FixedOffset, width: Option<usize>, target: Target) -> Scene {
        if let Some(r) = self.replay.as_ref().filter(|_| self.replaying())
            && target == Target::Status
        {
            return Scene {
                input: r.input.clone(),
                sub: None,
                frame: Frame {
                    now: r.now,
                    tz,
                    width,
                    git: GitSource::Fixed(r.git.clone()),
                    state: StateView {
                        samples: r.samples.clone(),
                        bridge: r.bridge.clone(),
                        ..StateView::default()
                    },
                    compact: AutoCompact::default(),
                    commands: Commands::Show,
                },
            };
        }
        // The replay has no background tasks; those use the first sample.
        let s = SCENARIOS.get(self.scenario).unwrap_or(&SCENARIOS[0]);
        let frame = Frame {
            now: BASE,
            tz,
            width,
            git: GitSource::Fixed(s.git()),
            state: s.state(BASE),
            compact: AutoCompact::default(),
            commands: Commands::Show,
        };
        let sub = (target == Target::Subagent).then(|| {
            let mut sub = scenarios::subagent(BASE);
            sub.columns = width;
            sub
        });
        let input = match &sub {
            Some(sub) => sub.tasks[0].input(sub),
            None => s.input(BASE),
        };
        Scene { input, sub, frame }
    }

    /// `config`'s `target` lines for the current scenario.
    pub fn render_lines(
        &self,
        config: &Config,
        doc: &Doc,
        registry: &Registry,
        tz: FixedOffset,
        target: Target,
    ) -> Vec<Line<'static>> {
        match target {
            Target::Status => self.render(config, doc, registry, tz, self.columns()),
            Target::Subagent => {
                let config = self.effective(config);
                let scene = self.scene(tz, Some(self.columns() as usize), target);
                let depth = self.depth(doc);
                let sub = scene.sub.expect("a subagent scene has tasks");
                statusline::subagent_lines(&sub, &config, registry, scene.frame)
                    .iter()
                    .take(MAX_LINES)
                    .map(|(_, l)| convert::line(l, depth))
                    .collect()
            }
        }
    }

    /// `config` rendered for the current scenario in `width` columns, with
    /// the bar's depth and glyph overrides; at most `MAX_LINES` lines.
    pub fn render(
        &self,
        config: &Config,
        doc: &Doc,
        registry: &Registry,
        tz: FixedOffset,
        width: u16,
    ) -> Vec<Line<'static>> {
        let config = self.effective(config);
        let scene = self.scene(tz, Some(width as usize), Target::Status);
        let depth = self.depth(doc);
        statusline::lines(&scene.input, &config, registry, scene.frame)
            .iter()
            .filter(|l| l.iter().any(|s| !s.text.trim().is_empty()))
            .take(MAX_LINES)
            .map(|l| convert::line(l, depth))
            .collect()
    }

    /// How `line` of the edited config lays out at the bar's width.
    pub fn report(
        &self,
        doc: &Doc,
        registry: &Registry,
        tz: FixedOffset,
        target: Target,
        line: &LineConfig,
    ) -> Report {
        let config = self.effective(&doc.config);
        let scene = self.scene(tz, Some(self.columns() as usize), target);
        statusline::report(
            &scene.input,
            scene.task(),
            &config,
            registry,
            &scene.frame,
            line,
        )
    }

    /// `item` drawn alone at each level for the current scenario; `None`
    /// when it has nothing to show.
    pub fn item_levels(
        &self,
        doc: &Doc,
        registry: &Registry,
        tz: FixedOffset,
        target: Target,
        item: &ItemConfig,
    ) -> Option<[Option<Line<'static>>; 3]> {
        let config = self.effective(&doc.config);
        let scene = self.scene(tz, None, target);
        let depth = self.depth(doc);
        let levels = statusline::item_levels(
            &scene.input,
            scene.task(),
            &config,
            registry,
            &scene.frame,
            item,
        )?;
        Some(levels.map(|l| l.map(|l| convert::line(&l, depth))))
    }

    /// The title on the bar's top rule, labelled when `room` allows.
    /// `showing` names what the bar shows when it isn't the edited config.
    /// The sample in view, or "your session" for the replay.
    pub fn scenario_name(&self) -> &'static str {
        SCENARIOS
            .get(self.scenario)
            .map_or("your session", |s| s.name)
    }

    pub fn title(
        &self,
        doc: &Doc,
        target: Target,
        showing: Option<&str>,
        room: u16,
    ) -> Line<'static> {
        let depth = match DEPTHS[self.depth] {
            Some(d) => depth_name(d).to_string(),
            None => format!("auto·{}", depth_name(self.depth(doc))),
        };
        let tier = match TIERS[self.tier] {
            Some(t) => tier_name(t).to_string(),
            None => tier_name(doc.config.glyphs).to_string(),
        };
        let width = match self.width {
            Some(w) => w.to_string(),
            None => format!("fit {}", self.fit),
        };
        let scenario = self.scenario_name().to_string();
        let values = [
            ("scenario", scenario),
            ("width", width),
            ("depth", depth),
            ("glyphs", tier),
        ];
        let heading = match (showing, target) {
            (Some(name), _) => format!(" Preview · {name} "),
            (None, Target::Status) => " Preview ".to_string(),
            (None, Target::Subagent) => " Preview · subagent ".to_string(),
        };
        let build = |labels: bool| {
            let mut spans = vec![Span::styled(heading.clone(), ui::title())];
            for (label, value) in &values {
                spans.push(Span::raw(if labels {
                    format!(" {label} ")
                } else {
                    " ".into()
                }));
                spans.push(ui::choice(value));
            }
            let keys = if self.replay.is_some() {
                "  [ ] - + { } R "
            } else {
                "  [ ] - + { } "
            };
            spans.push(Span::styled(keys, ui::muted()));
            Line::from(spans)
        };
        let full = build(true);
        if full.width() <= room as usize {
            full
        } else {
            build(false)
        }
    }

    /// `·········1·········2…` marking every tenth column up to the width.
    pub fn ruler(&self) -> Line<'static> {
        let w = self.columns() as usize;
        let text: String = (1..=w)
            .map(|c| {
                if c % 10 == 0 {
                    char::from_digit(((c / 10) % 10) as u32, 10).unwrap_or('·')
                } else {
                    '·'
                }
            })
            .collect();
        Line::styled(text, ui::muted())
    }

    pub fn area_rows(lines: usize) -> u16 {
        lines.clamp(1, MAX_LINES) as u16 + 2
    }
}

/// The rows the preview occupies within `area`: title rule, lines, ruler.
pub fn split(area: Rect, lines: usize) -> (Rect, Rect, Rect) {
    let n = lines.clamp(1, MAX_LINES) as u16;
    let title = Rect { height: 1, ..area };
    let body = Rect {
        y: area.y + 1,
        height: n,
        ..area
    };
    let ruler = Rect {
        y: area.y + 1 + n,
        height: 1,
        ..area
    };
    (title, body, ruler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_and_clamps() {
        let mut p = Preview::default();
        p.next_scenario(false);
        assert_eq!(p.scenario, SCENARIOS.len() - 1);
        p.next_scenario(true);
        assert_eq!(SCENARIOS[p.scenario].name, "normal");
        p.resize(-8);
        assert_eq!(p.width, Some(88));
        p.resize(100);
        assert_eq!(p.width, None);
        p.resize(-500);
        assert_eq!(p.width, Some(MIN_WIDTH));
        assert_eq!(p.ruler().spans[0].content.chars().count(), 20);
    }

    #[test]
    fn replays_the_last_session() {
        let mut p = Preview::default();
        assert!(!p.toggle_replay() && p.replayed().is_none());
        let session = SessionState {
            last_input: serde_json::from_str(
                r#"{"model": {"id": "claude-opus-4-8", "display_name": "Opus 4.8"}, "version": "2.1.305"}"#,
            )
            .ok(),
            saved_at: BASE,
            ..SessionState::default()
        };
        p.set_replay(Replay::from_session(&session));
        p.next_scenario(false);
        assert!(p.replaying());
        let (dir, paths) = crate::tui::doc::tests::temp_paths("replay");
        let doc = Doc::open(paths);
        let title: String = p
            .title(&doc, Target::Status, None, 200)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(title.contains("your session"), "{title}");
        assert!(p.toggle_replay() && p.scenario == 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
