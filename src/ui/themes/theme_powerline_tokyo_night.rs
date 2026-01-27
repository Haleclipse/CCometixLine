use crate::config::{
    AnsiColor, ColorConfig, IconConfig, SegmentConfig, SegmentId, TextStyleConfig,
};
use std::collections::HashMap;

pub fn model_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::Model,
        enabled: true,
        icon: IconConfig {
            plain: "🤖".to_string(),
            nerd_font: "\u{e26d}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 252,
                g: 167,
                b: 234,
            }),
            text: Some(AnsiColor::Rgb {
                r: 252,
                g: 167,
                b: 234,
            }),
            background: Some(AnsiColor::Rgb {
                r: 25,
                g: 27,
                b: 41,
            }),
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn directory_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::Directory,
        enabled: true,
        icon: IconConfig {
            plain: "📁".to_string(),
            nerd_font: "\u{f024b}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 130,
                g: 170,
                b: 255,
            }),
            text: Some(AnsiColor::Rgb {
                r: 130,
                g: 170,
                b: 255,
            }),
            background: Some(AnsiColor::Rgb {
                r: 47,
                g: 51,
                b: 77,
            }),
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn git_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::Git,
        enabled: true,
        icon: IconConfig {
            plain: "🌿".to_string(),
            nerd_font: "\u{f02a2}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 195,
                g: 232,
                b: 141,
            }),
            text: Some(AnsiColor::Rgb {
                r: 195,
                g: 232,
                b: 141,
            }),
            background: Some(AnsiColor::Rgb {
                r: 30,
                g: 32,
                b: 48,
            }),
        },
        styles: TextStyleConfig::default(),
        options: {
            let mut opts = HashMap::new();
            opts.insert("show_sha".to_string(), serde_json::Value::Bool(false));
            opts
        },
    }
}

pub fn context_window_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::ContextWindow,
        enabled: true,
        icon: IconConfig {
            plain: "⚡️️".to_string(),
            nerd_font: "\u{f49b}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 192,
                g: 202,
                b: 245,
            }),
            text: Some(AnsiColor::Rgb {
                r: 192,
                g: 202,
                b: 245,
            }),
            background: Some(AnsiColor::Rgb {
                r: 61,
                g: 89,
                b: 161,
            }),
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn config_counts_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::ConfigCounts,
        enabled: true,
        icon: IconConfig {
            plain: "CFG".to_string(),
            nerd_font: "\u{f013}".to_string(), // gear icon
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 187,
                g: 154,
                b: 247,
            }), // Tokyo Night purple
            text: Some(AnsiColor::Rgb {
                r: 192,
                g: 202,
                b: 245,
            }),
            background: Some(AnsiColor::Rgb {
                r: 52,
                g: 47,
                b: 75,
            }), // Tokyo Night dark purple background
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn cost_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::Cost,
        enabled: false,
        icon: IconConfig {
            plain: "💰".to_string(),
            nerd_font: "\u{eec1}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 224,
                g: 175,
                b: 104,
            }),
            text: Some(AnsiColor::Rgb {
                r: 224,
                g: 175,
                b: 104,
            }),
            background: Some(AnsiColor::Rgb {
                r: 36,
                g: 40,
                b: 59,
            }), // Tokyo Night dark background
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn session_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::Session,
        enabled: false,
        icon: IconConfig {
            plain: "⏱️".to_string(),
            nerd_font: "\u{f1ad3}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 158,
                g: 206,
                b: 106,
            }),
            text: Some(AnsiColor::Rgb {
                r: 158,
                g: 206,
                b: 106,
            }),
            background: Some(AnsiColor::Rgb {
                r: 41,
                g: 46,
                b: 66,
            }), // Tokyo Night darker background
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn output_style_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::OutputStyle,
        enabled: false,
        icon: IconConfig {
            plain: "🎯".to_string(),
            nerd_font: "\u{f12f5}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 125,
                g: 207,
                b: 255,
            }),
            text: Some(AnsiColor::Rgb {
                r: 125,
                g: 207,
                b: 255,
            }),
            background: Some(AnsiColor::Rgb {
                r: 32,
                g: 35,
                b: 52,
            }), // Tokyo Night darkest background
        },
        styles: TextStyleConfig::default(),
        options: HashMap::new(),
    }
}

pub fn usage_segment() -> SegmentConfig {
    SegmentConfig {
        id: SegmentId::Usage,
        enabled: false,
        icon: IconConfig {
            plain: "📊".to_string(),
            nerd_font: "\u{f0a9e}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(AnsiColor::Rgb {
                r: 224,
                g: 175,
                b: 104,
            }),
            text: Some(AnsiColor::Rgb {
                r: 224,
                g: 175,
                b: 104,
            }),
            background: Some(AnsiColor::Rgb {
                r: 36,
                g: 40,
                b: 59,
            }),
        },
        styles: TextStyleConfig::default(),
        options: {
            let mut opts = HashMap::new();
            opts.insert(
                "api_base_url".to_string(),
                serde_json::Value::String("https://api.anthropic.com".to_string()),
            );
            opts.insert(
                "cache_duration".to_string(),
                serde_json::Value::Number(180.into()),
            );
            opts.insert("timeout".to_string(), serde_json::Value::Number(2.into()));
            opts
        },
    }
}
