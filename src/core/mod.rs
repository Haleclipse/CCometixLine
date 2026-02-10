pub mod activity;
pub mod multiline;
pub mod segments;
pub mod statusline;

pub use multiline::{render_multiline, MultilineConfig, MultilineRenderer};
pub use statusline::{collect_all_segments, StatusLineGenerator};
