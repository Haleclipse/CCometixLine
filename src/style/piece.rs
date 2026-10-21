//! What widgets produce: text pieces tagged with a role. The painter turns
//! roles into colors for the active palette and skin, so widgets never pick
//! colors themselves.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    #[default]
    Normal,
    Good,
    Warn,
    Crit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Icon,
    /// The main value, in the item's color.
    Text,
    /// Like `Text`, bold.
    Strong,
    /// Secondary detail.
    Muted,
    /// A filled cell of a graph at the given severity.
    Fill(Severity),
    /// A filled cell colored by position along ok → warn → crit (0..1).
    Gradient(f32),
    /// An empty cell of a graph.
    Empty,
    /// Something good, e.g. lines added or being under budget.
    Good,
    /// Something bad, e.g. lines removed or being over budget.
    Bad,
    /// One `▀` cell of a pace bar: the top half shows usage, the bottom half
    /// how much of the window has passed.
    Pace {
        used: bool,
        elapsed: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub text: String,
    pub role: Role,
    pub link: Option<String>,
}

impl Piece {
    pub fn new(text: impl Into<String>, role: Role) -> Self {
        Piece {
            text: text.into(),
            role,
            link: None,
        }
    }

    pub fn with_link(mut self, link: Option<String>) -> Self {
        self.link = link;
        self
    }
}

pub fn pieces_width(pieces: &[Piece]) -> usize {
    pieces
        .iter()
        .map(|p| crate::render::width::display_width(&p.text))
        .sum()
}
