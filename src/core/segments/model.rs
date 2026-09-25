use super::{Segment, SegmentData};
use crate::config::{InputData, ModelConfig, SegmentId};
use std::collections::HashMap;

pub struct ModelSegment {
    /// Whether the effort level is appended to the model name
    /// (`show_effort` option of the model segment, default: true)
    show_effort: bool,
}

impl Default for ModelSegment {
    fn default() -> Self {
        Self { show_effort: true }
    }
}

impl ModelSegment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_show_effort(mut self, show_effort: bool) -> Self {
        self.show_effort = show_effort;
        self
    }
}

impl Segment for ModelSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let mut metadata = HashMap::new();
        metadata.insert("model_id".to_string(), input.model.id.clone());
        metadata.insert("display_name".to_string(), input.model.display_name.clone());

        let effort_level = input
            .effort
            .as_ref()
            .and_then(|e| e.level.as_deref())
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string);

        let secondary = match &effort_level {
            Some(level) if self.show_effort => format!("· {}", level),
            _ => String::new(),
        };

        if let Some(level) = effort_level {
            metadata.insert("effort_level".to_string(), level);
        }

        Some(SegmentData {
            primary: self.format_model_name(&input.model.id, &input.model.display_name),
            secondary,
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Model
    }
}

impl ModelSegment {
    fn format_model_name(&self, id: &str, display_name: &str) -> String {
        let model_config = ModelConfig::load();

        if let Some(config_name) = model_config.get_display_name(id) {
            // Model recognized by config, display_name already includes modifier suffix
            config_name
        } else {
            // Fallback: prefer upstream display_name, fall back to model_id if empty
            let base = if display_name.is_empty() {
                id.to_string()
            } else {
                display_name.to_string()
            };
            // Still apply context modifier suffix (e.g., " 1M") if present
            match model_config.get_display_suffix(id) {
                Some(suffix) => format!("{}{}", base, suffix),
                None => base,
            }
        }
    }
}
