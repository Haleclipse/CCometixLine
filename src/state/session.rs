//! Per-session history: one sample per change, plus the last input for
//! replay in the TUI.

use super::{dir, prompt_key, read_json, safe_name, sometimes, sweep, write_json};
use crate::input::Input;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

const MAX_SAMPLES: usize = 120;
/// Record a sample at least this often even when nothing changed, so rates
/// have a time base.
const HEARTBEAT_SECS: i64 = 60;
/// Rewrite the file at least this often while the last input changes.
const SAVE_EVERY_SECS: i64 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Unix epoch seconds.
    pub t: i64,
    /// The prompt this sample belongs to (`prompt_key` of `prompt_id`), so
    /// history can be grouped by turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    /// 5-hour window use in percent, and its reset time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h5: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h5_reset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d7: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d7_reset: Option<i64>,
}

impl Sample {
    pub fn from_input(input: &Input, now: i64) -> Sample {
        let rl = input.rate_limits.as_ref();
        Sample {
            t: now,
            p: input.prompt_id.as_deref().map(prompt_key),
            ctx: input
                .context_window
                .as_ref()
                .and_then(|c| c.total_input_tokens)
                .filter(|t| *t > 0),
            cost: input.cost.as_ref().and_then(|c| c.total_cost_usd),
            h5: rl.and_then(|r| r.five_hour.as_ref()?.used_percentage),
            h5_reset: rl
                .and_then(|r| r.five_hour.as_ref()?.resets_at)
                .map(|t| t as i64),
            d7: rl.and_then(|r| r.seven_day.as_ref()?.used_percentage),
            d7_reset: rl
                .and_then(|r| r.seven_day.as_ref()?.resets_at)
                .map(|t| t as i64),
        }
    }

    fn same_values(&self, other: &Sample) -> bool {
        Sample { t: 0, ..*self } == Sample { t: 0, ..*other }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub samples: Vec<Sample>,
    /// The latest stdin payload, for previews and the TUI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_input: Option<serde_json::Value>,
    #[serde(default)]
    pub saved_at: i64,
}

fn path(session_id: &str) -> Option<PathBuf> {
    Some(
        dir()
            .join("sessions")
            .join(format!("{}.json", safe_name(session_id)?)),
    )
}

impl SessionState {
    pub fn load(session_id: &str) -> SessionState {
        path(session_id)
            .and_then(|p| read_json(&p))
            .unwrap_or_default()
    }

    /// Add `sample` if values changed or the heartbeat is due. Returns
    /// whether a sample was added.
    pub fn record(&mut self, sample: Sample) -> bool {
        let due = match self.samples.last() {
            None => true,
            Some(last) => !last.same_values(&sample) || sample.t - last.t >= HEARTBEAT_SECS,
        };
        if due {
            self.samples.push(sample);
            if self.samples.len() > MAX_SAMPLES {
                self.samples.drain(..self.samples.len() - MAX_SAMPLES);
            }
        }
        due
    }

    /// Save if a sample was added or the last save is old enough.
    pub fn save_if_needed(&mut self, session_id: &str, now: i64, changed: bool) {
        if !changed && now - self.saved_at < SAVE_EVERY_SECS {
            return;
        }
        let Some(p) = path(session_id) else {
            return;
        };
        self.saved_at = now;
        let _ = write_json(&p, self);
        sometimes(100, || {
            sweep(
                &dir().join("sessions"),
                Duration::from_secs(7 * 86_400),
                200,
            );
        });
    }
}

/// Context size at the end of each turn, oldest first; the last entry is the
/// current turn so far. A turn is one prompt when samples carry prompt keys,
/// otherwise each distinct reading.
pub fn context_turns(samples: &[Sample]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mut last_p = None;
    for s in samples {
        let Some(v) = s.ctx.map(|v| v as f64) else {
            continue;
        };
        match (s.p, out.last_mut()) {
            (Some(p), Some(last)) if last_p == Some(p) => *last = v,
            (None, Some(last)) if last_p.is_none() && *last == v => {}
            _ => out.push(v),
        }
        last_p = s.p;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: i64, ctx: u64, cost: f64) -> Sample {
        Sample {
            t,
            ctx: Some(ctx),
            cost: Some(cost),
            ..Sample::default()
        }
    }

    #[test]
    fn records_changes_and_heartbeats_only() {
        let mut st = SessionState::default();
        assert!(st.record(s(0, 100, 0.1)));
        assert!(!st.record(s(10, 100, 0.1)));
        assert!(st.record(s(20, 150, 0.2)));
        assert!(st.record(s(90, 150, 0.2)));
        assert_eq!(st.samples.len(), 3);
    }

    #[test]
    fn caps_history() {
        let mut st = SessionState::default();
        for i in 0..(MAX_SAMPLES as i64 + 10) {
            st.record(s(i, i as u64 + 1, 0.0));
        }
        assert_eq!(st.samples.len(), MAX_SAMPLES);
        assert_eq!(st.samples[0].t, 10);
    }

    #[test]
    fn turns_without_prompt_keys_skip_repeats() {
        let samples = [s(0, 10, 0.0), s(1, 10, 0.1), s(2, 20, 0.2), s(3, 15, 0.3)];
        assert_eq!(context_turns(&samples), vec![10.0, 20.0, 15.0]);
    }

    #[test]
    fn turns_end_where_prompts_change() {
        let at = |t, p, ctx| Sample {
            p: Some(p),
            ..s(t, ctx, 0.0)
        };
        let samples = [
            at(0, 1, 10),
            at(1, 1, 14),
            at(2, 2, 14),
            at(3, 2, 30),
            at(4, 2, 35),
            at(5, 3, 35),
        ];
        assert_eq!(context_turns(&samples), vec![14.0, 35.0, 35.0]);
    }
}
