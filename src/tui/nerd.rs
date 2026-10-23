//! Every Nerd Fonts icon by name, for the icon browser. The table is
//! `assets/nerd-glyphs.txt` (see `scripts/nerd-glyphs.mjs`), read the first
//! time the browser opens; the status line never touches it.

use std::sync::OnceLock;

const TABLE: &str = include_str!("../../assets/nerd-glyphs.txt");

/// One icon and every name it goes by, e.g. `md-folder`.
pub struct Glyph {
    pub glyph: char,
    pub names: Vec<&'static str>,
}

/// Every icon: the classes with the most icons first, each in codepoint
/// order.
pub fn all() -> &'static [Glyph] {
    static ALL: OnceLock<Vec<Glyph>> = OnceLock::new();
    ALL.get_or_init(|| {
        TABLE
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|line| {
                let mut parts = line.split(' ');
                let cp = u32::from_str_radix(parts.next()?, 16).ok()?;
                Some(Glyph {
                    glyph: char::from_u32(cp)?,
                    names: parts.collect(),
                })
            })
            .collect()
    })
}

/// The Nerd Fonts release the table comes from.
pub fn version() -> &'static str {
    TABLE
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(3))
        .unwrap_or_default()
}

/// The name prefixes that narrow a search to one class (`md-`, `fa-`, …),
/// largest class first.
pub fn classes() -> &'static [&'static str] {
    static CLASSES: OnceLock<Vec<&str>> = OnceLock::new();
    CLASSES.get_or_init(|| {
        let mut out: Vec<&str> = Vec::new();
        for g in all() {
            let class = g.names[0].split('-').next().unwrap_or_default();
            if !out.contains(&class) {
                out.push(class);
            }
        }
        out
    })
}

/// A word's codepoint, written `f024b`, `U+F024B` or `0xf024b`.
fn codepoint(word: &str) -> Option<u32> {
    let hex = word
        .strip_prefix("u+")
        .or_else(|| word.strip_prefix("0x"))
        .unwrap_or(word);
    (4..=6)
        .contains(&hex.len())
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

/// How well `word` names `g`, and the length of the name it matched; none
/// when it doesn't. The icon itself or its codepoint beats any name; then
/// a whole name, a whole word of one, the start of a word, anywhere in it.
fn score(g: &Glyph, word: &str) -> Option<(u32, usize)> {
    let mut chars = word.chars();
    if chars.next() == Some(g.glyph) && chars.next().is_none()
        || codepoint(word) == Some(g.glyph as u32)
    {
        return Some((100, 0));
    }
    g.names
        .iter()
        .filter_map(|name| {
            let mut words = name.split(['-', '_']);
            let s = if *name == word {
                50
            } else if words.clone().any(|w| w == word) {
                30
            } else if words.any(|w| w.starts_with(word)) {
                20
            } else if name.contains(word) {
                10
            } else {
                return None;
            };
            Some((s, name.len()))
        })
        .max_by_key(|&(s, len)| (s, std::cmp::Reverse(len)))
}

/// Positions in `all()` of the icons `query` finds, best first. Each word
/// must match: part of a name (`folder`, `md-`), a codepoint, or the icon
/// pasted in. An empty query finds every icon, in table order.
pub fn search(query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut found: Vec<(u32, usize, usize)> = all()
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            let (mut total, mut len) = (0, 0);
            for word in &words {
                let (s, l) = score(g, word)?;
                total += s;
                len = len.max(l);
            }
            Some((total, len, i))
        })
        .collect();
    found.sort_by_key(|&(s, len, i)| (std::cmp::Reverse(s), len, i));
    found.into_iter().map(|(_, _, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(query: &str) -> &'static str {
        all()[search(query)[0]].names[0]
    }

    #[test]
    fn holds_every_nerd_font_icon() {
        assert_eq!(all().len(), 10_617);
        assert_eq!(all().iter().map(|g| g.names.len()).sum::<usize>(), 10_995);
        assert_eq!(version(), "3.5.1");
        assert_eq!(&classes()[..4], ["md", "fa", "dev", "cod"]);
        // The status line's own folder icon is in it under its name.
        let folder = &all()[search("md-folder")[0]];
        assert_eq!((folder.glyph, folder.names[0]), ('\u{f024b}', "md-folder"));
    }

    #[test]
    fn finds_by_name_codepoint_or_the_icon_itself() {
        assert_eq!(first("folder"), "md-folder");
        assert_eq!(first("f024b"), "md-folder");
        assert_eq!(first("U+F024B"), "md-folder");
        assert_eq!(first("\u{f024b}"), "md-folder");
        // Every word must match; a class prefix narrows to that class.
        let git = search("cod- git");
        assert!(!git.is_empty());
        assert!(git.iter().all(|&i| {
            all()[i]
                .names
                .iter()
                .any(|n| n.starts_with("cod-") && n.contains("git"))
        }));
        assert!(search("folder zzzz").is_empty());
        assert_eq!(search("").len(), all().len());
    }
}
