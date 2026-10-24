use serde::{Deserialize, Serialize};

/// A concrete terminal color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
    Ansi256(u8),
    Ansi16(u8),
}

/// How many colors the output may use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorDepth {
    #[default]
    #[serde(rename = "truecolor")]
    TrueColor,
    #[serde(rename = "256")]
    Ansi256,
    #[serde(rename = "16")]
    Ansi16,
    None,
}

/// xterm's default 16-color palette, used to approximate ANSI colors as RGB.
const ANSI16_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

const ANSI16_NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

impl Color {
    /// Parse `#rrggbb`, `#rgb`, `256:n`, `16:n` or an ANSI color name such as
    /// `red` or `bright_blue`.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            return parse_hex(hex);
        }
        if let Some(n) = s.strip_prefix("256:") {
            return n.parse().ok().map(Color::Ansi256);
        }
        if let Some(n) = s.strip_prefix("16:") {
            return n.parse().ok().filter(|n| *n < 16).map(Color::Ansi16);
        }
        let name = s.to_ascii_lowercase().replace(['-', ' '], "_");
        ANSI16_NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| Color::Ansi16(i as u8))
    }

    /// The color in the notation `parse` accepts.
    pub fn notation(self) -> String {
        match self {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            Color::Ansi256(n) => format!("256:{n}"),
            Color::Ansi16(n) => ANSI16_NAMES[n as usize % 16].to_string(),
        }
    }

    /// Approximate RGB value; ANSI colors use xterm's defaults.
    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Ansi16(n) => ANSI16_RGB[n as usize % 16],
            Color::Ansi256(n) => ansi256_to_rgb(n),
        }
    }

    /// The closest color representable at `depth`, or `None` when color is off.
    pub fn downgrade(self, depth: ColorDepth) -> Option<Color> {
        match (depth, self) {
            (ColorDepth::None, _) => None,
            (ColorDepth::TrueColor, c) => Some(c),
            (ColorDepth::Ansi256, Color::Rgb(r, g, b)) => {
                Some(Color::Ansi256(rgb_to_ansi256(r, g, b)))
            }
            (ColorDepth::Ansi256, c) => Some(c),
            (ColorDepth::Ansi16, Color::Ansi16(n)) => Some(Color::Ansi16(n)),
            (ColorDepth::Ansi16, c) => {
                let (r, g, b) = c.to_rgb();
                Some(Color::Ansi16(rgb_to_ansi16(r, g, b)))
            }
        }
    }

    /// SGR parameters for this color as foreground or background.
    pub fn sgr(self, background: bool) -> String {
        match self {
            Color::Ansi16(n) => {
                let base = match (background, n < 8) {
                    (false, true) => 30,
                    (false, false) => 82,
                    (true, true) => 40,
                    (true, false) => 92,
                };
                (base + n as u16).to_string()
            }
            Color::Ansi256(n) => format!("{};5;{n}", if background { 48 } else { 38 }),
            Color::Rgb(r, g, b) => format!("{};2;{r};{g};{b}", if background { 48 } else { 38 }),
        }
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    let digits: Vec<u8> = match hex.len() {
        6 => (0..3)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
            .collect::<Option<_>>()?,
        3 => hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| (d * 17) as u8))
            .collect::<Option<_>>()?,
        _ => return None,
    };
    Some(Color::Rgb(digits[0], digits[1], digits[2]))
}

fn ansi256_to_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..=15 => ANSI16_RGB[n as usize],
        16..=231 => {
            let i = n - 16;
            (
                CUBE_LEVELS[(i / 36) as usize],
                CUBE_LEVELS[(i / 6 % 6) as usize],
                CUBE_LEVELS[(i % 6) as usize],
            )
        }
        _ => {
            let v = 8 + (n - 232) * 10;
            (v, v, v)
        }
    }
}

fn distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2) as u32;
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    let nearest_level = |v: u8| {
        CUBE_LEVELS
            .iter()
            .enumerate()
            .min_by_key(|(_, l)| (v as i32 - **l as i32).abs())
            .map(|(i, _)| i as u8)
            .unwrap_or(0)
    };
    let cube = 16 + 36 * nearest_level(r) + 6 * nearest_level(g) + nearest_level(b);
    let avg = ((r as u32 + g as u32 + b as u32) / 3) as u8;
    let gray = if avg < 8 {
        232
    } else if avg > 238 {
        255
    } else {
        232 + (avg - 8) / 10
    };
    let target = (r, g, b);
    if distance(ansi256_to_rgb(gray), target) < distance(ansi256_to_rgb(cube), target) {
        gray
    } else {
        cube
    }
}

fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> u8 {
    ANSI16_RGB
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| distance(**c, (r, g, b)))
        .map(|(i, _)| i as u8)
        .unwrap_or(7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notations() {
        assert_eq!(Color::parse("#a6e3a1"), Some(Color::Rgb(0xa6, 0xe3, 0xa1)));
        assert_eq!(Color::parse("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(Color::parse("256:208"), Some(Color::Ansi256(208)));
        assert_eq!(Color::parse("16:12"), Some(Color::Ansi16(12)));
        assert_eq!(Color::parse("16:16"), None);
        assert_eq!(Color::parse("Bright-Red"), Some(Color::Ansi16(9)));
        assert_eq!(Color::parse("#12345"), None);
        assert_eq!(Color::parse("chartreuse"), None);
    }

    #[test]
    fn notation_round_trips() {
        for c in [Color::Rgb(1, 2, 3), Color::Ansi256(200), Color::Ansi16(13)] {
            assert_eq!(Color::parse(&c.notation()), Some(c));
        }
    }

    #[test]
    fn downgrades_to_nearest() {
        let pure_red = Color::Rgb(255, 0, 0);
        assert_eq!(
            pure_red.downgrade(ColorDepth::Ansi256),
            Some(Color::Ansi256(196))
        );
        assert_eq!(
            pure_red.downgrade(ColorDepth::Ansi16),
            Some(Color::Ansi16(9))
        );
        assert_eq!(
            Color::Rgb(128, 128, 128).downgrade(ColorDepth::Ansi256),
            Some(Color::Ansi256(244))
        );
        assert_eq!(
            Color::Ansi16(3).downgrade(ColorDepth::Ansi256),
            Some(Color::Ansi16(3))
        );
        assert_eq!(pure_red.downgrade(ColorDepth::None), None);
    }

    #[test]
    fn sgr_codes() {
        assert_eq!(Color::Ansi16(1).sgr(false), "31");
        assert_eq!(Color::Ansi16(9).sgr(false), "91");
        assert_eq!(Color::Ansi16(9).sgr(true), "101");
        assert_eq!(Color::Ansi256(208).sgr(true), "48;5;208");
        assert_eq!(Color::Rgb(1, 2, 3).sgr(false), "38;2;1;2;3");
    }
}
