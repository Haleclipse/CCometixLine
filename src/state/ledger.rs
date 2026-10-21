//! Today's spend across sessions. Each session records the cost it had when
//! first seen today (its baseline) and the highest cost since; a cost that
//! goes down (as after `/clear`) banks what was spent so far.

use super::{dir, read_json, write_json};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const KEEP_SECS: i64 = 86_400;
/// Rewrite at least this often while nothing but timestamps changed.
const REFRESH_SECS: i64 = 30;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Spent today before the session's cost last went down.
    #[serde(default)]
    pub banked: f64,
    pub base: f64,
    pub total: f64,
    pub seen: i64,
}

impl Entry {
    fn today(&self) -> f64 {
        self.banked + (self.total - self.base).max(0.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    /// Local date the ledger covers, `YYYY-MM-DD`.
    pub date: String,
    #[serde(default)]
    pub sessions: BTreeMap<String, Entry>,
    #[serde(default)]
    pub saved_at: i64,
}

impl Ledger {
    /// Fold in `cost` for `session` at `now` on local date `date`. Returns
    /// whether anything but timestamps changed.
    pub fn record(&mut self, date: &str, session: &str, cost: f64, now: i64) -> bool {
        let mut changed = false;
        if self.date != date {
            // New day: sessions still active carry over from their last total.
            self.sessions.retain(|_, e| now - e.seen <= KEEP_SECS);
            for e in self.sessions.values_mut() {
                *e = Entry {
                    banked: 0.0,
                    base: e.total,
                    total: e.total,
                    seen: e.seen,
                };
            }
            self.date = date.to_string();
            changed = true;
        }
        let entry = self.sessions.entry(session.to_string()).or_insert_with(|| {
            changed = true;
            Entry {
                banked: 0.0,
                base: cost,
                total: cost,
                seen: now,
            }
        });
        if cost < entry.total {
            entry.banked += (entry.total - entry.base).max(0.0);
            entry.base = cost;
            entry.total = cost;
            changed = true;
        } else if cost > entry.total {
            entry.total = cost;
            changed = true;
        }
        entry.seen = now;
        self.sessions.retain(|_, e| now - e.seen <= KEEP_SECS);
        changed
    }

    pub fn today(&self) -> f64 {
        self.sessions.values().map(Entry::today).sum()
    }
}

/// Record this render's cost and return today's total.
pub fn update(session: &str, cost: f64, date: &str, now: i64) -> f64 {
    let path = dir().join("ledger.json");
    let mut ledger: Ledger = read_json(&path).unwrap_or_default();
    let changed = ledger.record(date, session, cost, now);
    if changed || now - ledger.saved_at >= REFRESH_SECS {
        ledger.saved_at = now;
        let _ = write_json(&path, &ledger);
    }
    ledger.today()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_todays_increment() {
        let mut l = Ledger::default();
        l.record("2026-10-01", "a", 5.0, 0);
        l.record("2026-10-01", "a", 7.5, 10);
        l.record("2026-10-01", "b", 1.0, 20);
        l.record("2026-10-01", "b", 2.0, 30);
        assert_eq!(l.today(), 3.5);
    }

    #[test]
    fn banks_spend_when_cost_resets() {
        let mut l = Ledger::default();
        l.record("2026-10-01", "a", 0.0, 0);
        l.record("2026-10-01", "a", 4.0, 10);
        l.record("2026-10-01", "a", 0.0, 20);
        l.record("2026-10-01", "a", 1.5, 30);
        assert_eq!(l.today(), 5.5);
    }

    #[test]
    fn rolls_over_at_midnight() {
        let mut l = Ledger::default();
        l.record("2026-10-01", "a", 0.0, 0);
        l.record("2026-10-01", "a", 3.0, 100);
        assert!(l.record("2026-10-02", "a", 4.0, 200));
        assert_eq!(l.today(), 1.0);
    }

    #[test]
    fn forgets_idle_sessions() {
        let mut l = Ledger::default();
        l.record("2026-10-01", "old", 1.0, 0);
        l.record("2026-10-01", "old", 2.0, 10);
        l.record("2026-10-01", "new", 0.0, KEEP_SECS + 100);
        assert!(!l.sessions.contains_key("old"));
    }
}
