pub mod minimax;

use serde_json::Value;
use std::collections::HashMap;

/// 单条用量指标
#[derive(Debug, Clone)]
pub struct UsageMetric {
    /// 显示标签，如 "7d"、"5h"
    pub label: String,
    /// 剩余百分比 0-100
    pub remaining_pct: u32,
}

/// 外部用量数据提供者 trait
pub trait UsageProvider: Send + Sync {
    fn name(&self) -> &str;
    fn fetch(&self, options: &HashMap<String, Value>) -> Option<Vec<UsageMetric>>;
}

/// 根据 provider 名称构建对应实现
pub fn build(provider_name: &str) -> Option<Box<dyn UsageProvider>> {
    match provider_name {
        "minimax" => Some(Box::new(minimax::MinimaxProvider)),
        _ => None,
    }
}
