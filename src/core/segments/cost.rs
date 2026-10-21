use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

#[derive(Default)]
pub struct CostSegment;

impl CostSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for CostSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let cost_data = input.cost.as_ref()?;

        // Primary display: total cost
        let cost = cost_data.total_cost_usd?;
        let primary = if cost == 0.0 || cost < 0.01 {
            "$0".to_string()
        } else {
            format!("${:.2}", cost)
        };

        // Secondary display: empty for cost segment
        let secondary = String::new();

        let mut metadata = HashMap::new();
        if let Some(cost) = cost_data.total_cost_usd {
            metadata.insert("cost".to_string(), cost.to_string());
        }

        Some(SegmentData {
            primary,
            secondary,
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Cost
    }
}
