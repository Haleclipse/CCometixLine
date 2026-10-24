use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Display width of plain text (no escape sequences), measured per grapheme
/// cluster. Nerd Font glyphs in the private use areas count as one cell.
pub fn display_width(s: &str) -> usize {
    s.graphemes(true).map(grapheme_width).sum()
}

fn grapheme_width(g: &str) -> usize {
    UnicodeWidthStr::width(g).min(2)
}

/// Cut `s` to at most `max` cells, ending with `…` when anything was removed.
pub fn truncate(s: &str, max: usize) -> String {
    if display_width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for g in s.graphemes(true) {
        let w = grapheme_width(g);
        if used + w > max - 1 {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_common_glyphs() {
        assert_eq!(display_width("Opus 4.8"), 8);
        assert_eq!(display_width("中文"), 4);
        assert_eq!(display_width("▰▱█░⣿"), 5);
        assert_eq!(display_width("\u{e0b0}\u{f0a9e}"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
        assert_eq!(display_width("👍🏽"), 2);
    }

    #[test]
    fn truncates_by_cells() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
        assert_eq!(truncate("中文字", 4), "中…");
        assert_eq!(truncate("abc", 0), "");
    }
}
