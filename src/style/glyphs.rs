//! Icons and graph glyphs for each glyph tier.

use serde::{Deserialize, Serialize};

/// Which glyphs the terminal font can show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlyphTier {
    /// Nerd Font icons (private use area).
    #[default]
    Nerd,
    /// Standard Unicode symbols, no special font.
    Unicode,
    /// ASCII only.
    Ascii,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconKey {
    Model,
    Effort,
    Thinking,
    Fast,
    OutputStyle,
    Vim,
    Agent,
    SessionName,
    Duration,
    ApiShare,
    Context,
    Trend,
    Compact,
    Quota,
    Eta,
    Spend,
    Cost,
    Rate,
    Today,
    Cache,
    Dir,
    Git,
    Repo,
    Pr,
    Worktree,
    Lines,
    Version,
    Update,
    Command,
}

impl IconKey {
    pub const ALL: [IconKey; 29] = [
        IconKey::Model,
        IconKey::Effort,
        IconKey::Thinking,
        IconKey::Fast,
        IconKey::OutputStyle,
        IconKey::Vim,
        IconKey::Agent,
        IconKey::SessionName,
        IconKey::Duration,
        IconKey::ApiShare,
        IconKey::Context,
        IconKey::Trend,
        IconKey::Compact,
        IconKey::Quota,
        IconKey::Eta,
        IconKey::Spend,
        IconKey::Cost,
        IconKey::Rate,
        IconKey::Today,
        IconKey::Cache,
        IconKey::Dir,
        IconKey::Git,
        IconKey::Repo,
        IconKey::Pr,
        IconKey::Worktree,
        IconKey::Lines,
        IconKey::Version,
        IconKey::Update,
        IconKey::Command,
    ];

    /// A short lowercase name, for pickers.
    pub fn name(self) -> String {
        let debug = format!("{self:?}");
        let mut out = String::new();
        for (i, c) in debug.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                out.push(' ');
            }
            out.extend(c.to_lowercase());
        }
        out
    }
}

/// The icon for `key`; empty when the tier has none.
pub fn icon(key: IconKey, tier: GlyphTier) -> &'static str {
    let (nerd, unicode) = match key {
        IconKey::Model => ("\u{e26d}", "◆"),
        IconKey::Effort => ("\u{f0e7}", "ϟ"),
        IconKey::Thinking => ("\u{f0eb}", "✻"),
        IconKey::Fast => ("\u{f135}", "»"),
        IconKey::OutputStyle => ("\u{f12f5}", "◎"),
        IconKey::Vim => ("\u{e62b}", "v"),
        IconKey::Agent => ("\u{f007}", "@"),
        IconKey::SessionName => ("\u{f02b}", "#"),
        IconKey::Duration => ("\u{f19bb}", "◷"),
        IconKey::ApiShare => ("\u{f017}", "⧗"),
        IconKey::Context => ("\u{f49b}", "◔"),
        IconKey::Trend => ("\u{f201}", "∿"),
        IconKey::Compact => ("\u{f066}", "⇲"),
        IconKey::Quota => ("\u{f0a9e}", "◷"),
        IconKey::Eta => ("\u{f252}", "⇥"),
        IconKey::Spend => ("\u{f155}", "$"),
        IconKey::Cost => ("\u{eec1}", "$"),
        IconKey::Rate => ("\u{f0e4}", "∂"),
        IconKey::Today => ("\u{f073}", "∑"),
        IconKey::Cache => ("\u{f1c0}", "≈"),
        IconKey::Dir => ("\u{f024b}", "▣"),
        IconKey::Git => ("\u{f02a2}", "⎇"),
        IconKey::Repo => ("\u{f1d3}", "▤"),
        IconKey::Pr => ("\u{f407}", "⇄"),
        IconKey::Worktree => ("\u{f1bb}", "⑂"),
        IconKey::Lines => ("±", "±"),
        IconKey::Version => ("\u{f05a}", "v"),
        IconKey::Update => ("\u{f06b0}", "↑"),
        IconKey::Command => ("\u{f120}", "›"),
    };
    match tier {
        GlyphTier::Nerd => nerd,
        GlyphTier::Unicode => unicode,
        GlyphTier::Ascii => "",
    }
}

/// Glyph set for bar graphs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarGlyphs {
    /// `█░`
    Blocks,
    /// `█▉▊▋▌▍▎▏`, sub-cell precision
    Eighths,
    /// `▰▱`
    #[default]
    Shapes,
    /// `●○`
    Dots,
    /// `⣿⣀`
    Braille,
    /// `[==--]`
    Ascii,
}

impl BarGlyphs {
    /// The set to use at `tier`; ASCII tier forces ASCII bars.
    pub fn for_tier(self, tier: GlyphTier) -> BarGlyphs {
        if tier == GlyphTier::Ascii {
            BarGlyphs::Ascii
        } else {
            self
        }
    }

    pub fn full(self) -> &'static str {
        match self {
            BarGlyphs::Blocks | BarGlyphs::Eighths => "█",
            BarGlyphs::Shapes => "▰",
            BarGlyphs::Dots => "●",
            BarGlyphs::Braille => "⣿",
            BarGlyphs::Ascii => "=",
        }
    }

    pub fn empty(self) -> &'static str {
        match self {
            BarGlyphs::Blocks => "░",
            BarGlyphs::Eighths => " ",
            BarGlyphs::Shapes => "▱",
            BarGlyphs::Dots => "○",
            BarGlyphs::Braille => "⣀",
            BarGlyphs::Ascii => "-",
        }
    }

    /// Partial-cell glyphs, from 1/8 to 7/8 full, if the set has them.
    pub fn partials(self) -> Option<[&'static str; 7]> {
        match self {
            BarGlyphs::Eighths => Some(["▏", "▎", "▍", "▌", "▋", "▊", "▉"]),
            _ => None,
        }
    }

    /// Characters drawn around the bar.
    pub fn frame(self) -> (&'static str, &'static str) {
        match self {
            BarGlyphs::Ascii => ("[", "]"),
            BarGlyphs::Eighths => ("▕", "▏"),
            _ => ("", ""),
        }
    }
}

/// Steps for a gauge (one glyph showing a fraction), lowest first.
pub fn gauge_steps(tier: GlyphTier) -> &'static [&'static str] {
    match tier {
        GlyphTier::Nerd => &[
            "\u{f0a9e}",
            "\u{f0a9f}",
            "\u{f0aa0}",
            "\u{f0aa1}",
            "\u{f0aa2}",
            "\u{f0aa3}",
            "\u{f0aa4}",
            "\u{f0aa5}",
        ],
        GlyphTier::Unicode => &["○", "◔", "◑", "◕", "●"],
        GlyphTier::Ascii => &[],
    }
}

/// Glyph for `ratio` (0..1) on a gauge; `None` when the tier has no gauge.
pub fn gauge(ratio: f64, tier: GlyphTier) -> Option<&'static str> {
    let steps = gauge_steps(tier);
    if steps.is_empty() {
        return None;
    }
    let i = (ratio.clamp(0.0, 1.0) * steps.len() as f64).floor() as usize;
    Some(steps[i.min(steps.len() - 1)])
}

/// Sparkline levels, lowest first.
pub fn spark_levels(tier: GlyphTier) -> &'static [&'static str] {
    match tier {
        GlyphTier::Ascii => &["_", ".", "-", "=", "^"],
        _ => &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::width::display_width;

    #[test]
    fn icons_are_single_cell_or_empty() {
        let keys = [
            IconKey::Model,
            IconKey::Context,
            IconKey::Git,
            IconKey::Worktree,
            IconKey::ApiShare,
            IconKey::Update,
            IconKey::Trend,
            IconKey::Compact,
            IconKey::Eta,
            IconKey::Rate,
            IconKey::Today,
            IconKey::Command,
        ];
        for key in keys {
            for tier in [GlyphTier::Nerd, GlyphTier::Unicode] {
                assert_eq!(display_width(icon(key, tier)), 1, "{key:?} {tier:?}");
            }
            assert_eq!(icon(key, GlyphTier::Ascii), "");
        }
    }

    #[test]
    fn gauge_covers_full_range() {
        assert_eq!(gauge(0.0, GlyphTier::Unicode), Some("○"));
        assert_eq!(gauge(0.5, GlyphTier::Unicode), Some("◑"));
        assert_eq!(gauge(1.0, GlyphTier::Unicode), Some("●"));
        assert_eq!(gauge(7.0, GlyphTier::Nerd), Some("\u{f0aa5}"));
        assert_eq!(gauge(0.5, GlyphTier::Ascii), None);
    }

    #[test]
    fn ascii_tier_forces_ascii_bars() {
        assert_eq!(
            BarGlyphs::Braille.for_tier(GlyphTier::Ascii),
            BarGlyphs::Ascii
        );
        assert_eq!(
            BarGlyphs::Braille.for_tier(GlyphTier::Unicode),
            BarGlyphs::Braille
        );
    }
}
