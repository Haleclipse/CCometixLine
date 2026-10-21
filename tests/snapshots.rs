//! Every preset against every built-in scenario at three widths, as plain
//! text. Run with `INSTA_UPDATE=always` (or `cargo insta review`) after an
//! intended rendering change.

use ccometixline::metrics::GitSource;
use ccometixline::models::Registry;
use ccometixline::render::ansi::{EmitOptions, emit};
use ccometixline::statusline::{Frame, lines};
use ccometixline::{presets, scenarios};
use chrono::FixedOffset;

const WIDTHS: [usize; 3] = [60, 100, 140];

fn render_preset(preset: &str) -> String {
    let config = presets::get(preset).unwrap();
    let registry = Registry::default();
    let mut out = String::new();
    for s in scenarios::SCENARIOS {
        for width in WIDTHS {
            let frame = Frame {
                now: scenarios::BASE,
                tz: FixedOffset::east_opt(0).unwrap(),
                width: Some(width),
                git: GitSource::Fixed(s.git()),
            };
            let text = emit(
                &lines(&s.input(scenarios::BASE), &config, &registry, frame),
                EmitOptions::plain(),
            );
            out.push_str(&format!("── {} @ {width}\n", s.name));
            for line in text.lines() {
                // Mark the right edge so alignment and trailing space are visible.
                out.push_str(line);
                out.push_str("⏎\n");
            }
        }
    }
    out
}

#[test]
fn presets_render_every_scenario() {
    for preset in presets::PRESETS {
        insta::assert_snapshot!(preset.name, render_preset(preset.name));
    }
}
