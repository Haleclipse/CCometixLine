use std::fmt;

/// A Claude Code version, compared numerically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Version {
            major,
            minor,
            patch,
        }
    }

    /// Parse the first `x.y.z` in `s`, so `2.1.285`, `v2.1.285` and
    /// `2.1.285 (Claude Code)` all work.
    pub fn parse(s: &str) -> Option<Self> {
        let start = s.find(|c: char| c.is_ascii_digit())?;
        let mut parts = s[start..]
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .next()?
            .split('.')
            .map(|p| p.parse::<u32>().ok());
        Some(Version::new(
            parts.next()??,
            parts.next()??,
            parts.next().flatten().unwrap_or(0),
        ))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Oldest Claude Code that passes `COLUMNS` to the status line command.
pub const MIN_SUPPORTED: Version = Version::new(2, 1, 153);

/// Claude Code versions that introduced capabilities newer than
/// `MIN_SUPPORTED`. Anything older is assumed present.
pub mod gate {
    use super::Version;

    pub const PROMPT_CACHE: Version = Version::new(2, 1, 251);
    pub const SPEND_LIMIT: Version = Version::new(2, 1, 251);
    pub const MISS_CAUSE: Version = Version::new(2, 1, 260);
    pub const SPEND_USD: Version = Version::new(2, 1, 284);
    pub const MODS: Version = Version::new(2, 1, 287);
    pub const SUBAGENT_AGENT_TYPE: Version = Version::new(2, 1, 293);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_forms() {
        assert_eq!(Version::parse("2.1.285"), Some(Version::new(2, 1, 285)));
        assert_eq!(Version::parse("v2.1.153"), Some(Version::new(2, 1, 153)));
        assert_eq!(
            Version::parse("2.1.280 (Claude Code)"),
            Some(Version::new(2, 1, 280))
        );
        assert_eq!(Version::parse("2.1"), Some(Version::new(2, 1, 0)));
        assert_eq!(Version::parse("dev"), None);
    }

    #[test]
    fn orders_numerically() {
        assert!(Version::new(2, 1, 153) > Version::new(2, 1, 99));
        assert!(Version::parse("2.1.287").unwrap() >= gate::MODS);
        assert!(Version::parse("2.1.250").unwrap() < gate::PROMPT_CACHE);
    }
}
