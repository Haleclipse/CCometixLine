use super::{Segment, SegmentData};
use crate::config::{InputData, ModelConfig, SegmentId};
use std::collections::HashMap;

#[derive(Default)]
pub struct ModelSegment;

impl ModelSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for ModelSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let mut metadata = HashMap::new();
        metadata.insert("model_id".to_string(), input.model.id.clone());
        metadata.insert("display_name".to_string(), input.model.display_name.clone());

        Some(SegmentData {
            primary: self.format_model_name(&input.model.id, &input.model.display_name),
            secondary: String::new(),
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
