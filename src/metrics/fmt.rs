//! Number, money and time formatting shared by metrics.

use chrono::{DateTime, Datelike, FixedOffset};

/// `842`, `45.2k`, `124k`, `1.3M`.
pub fn tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=99_999 => trim_decimal(format!("{:.1}", n as f64 / 1_000.0)) + "k",
        100_000..=999_999 => format!("{}k", (n as f64 / 1_000.0).round() as u64),
        _ => trim_decimal(format!("{:.1}", n as f64 / 1_000_000.0)) + "M",
    }
}

fn trim_decimal(s: String) -> String {
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// Whole percent of `ratio` (0..1), at least 1 when non-zero.
pub fn percent(ratio: f64) -> String {
    let p = (ratio * 100.0).round();
    if p == 0.0 && ratio > 0.0 {
        "1".into()
    } else {
        format!("{}", p as i64)
    }
}

/// `$0` under a cent (as v1), `$1.23`, `$123` from a hundred up.
pub fn money(usd: f64) -> String {
    if usd < 0.01 {
        "$0".into()
    } else if usd < 100.0 {
        format!("${usd:.2}")
    } else {
        format!("${}", usd.round() as i64)
    }
}

/// Elapsed time in v1's style: `500ms`, `59s`, `3m45s`, `1h2m`.
pub fn duration_ms(ms: u64) -> String {
    match ms {
        0..=999 => format!("{ms}ms"),
        1_000..=59_999 => format!("{}s", ms / 1_000),
        60_000..=3_599_999 => {
            let (m, s) = (ms / 60_000, ms % 60_000 / 1_000);
            if s == 0 {
                format!("{m}m")
            } else {
                format!("{m}m{s}s")
            }
        }
        _ => {
            let (h, m) = (ms / 3_600_000, ms % 3_600_000 / 60_000);
            if m == 0 {
                format!("{h}h")
            } else {
                format!("{h}h{m}m")
            }
        }
    }
}

/// Time left: `0:42`, `3:42`, `1h05m`, `2d4h`.
pub fn countdown(secs: i64) -> String {
    let secs = secs.max(0);
    match secs {
        0..=3_599 => format!("{}:{:02}", secs / 60, secs % 60),
        3_600..=86_399 => format!("{}h{:02}m", secs / 3_600, secs % 3_600 / 60),
        _ => format!("{}d{}h", secs / 86_400, secs % 86_400 / 3_600),
    }
}

/// Wall-clock time of `epoch` in `tz`: `14:20` today, `Mon 14:20` within a
/// week, `10-28 14:20` otherwise.
pub fn clock(epoch: i64, now: i64, tz: FixedOffset) -> String {
    let at = DateTime::from_timestamp(epoch, 0).map(|t| t.with_timezone(&tz));
    let today = DateTime::from_timestamp(now, 0).map(|t| t.with_timezone(&tz));
    let (Some(at), Some(today)) = (at, today) else {
        return String::new();
    };
    let days = (at.date_naive() - today.date_naive()).num_days();
    if days == 0 {
        at.format("%H:%M").to_string()
    } else if (0..7).contains(&days) {
        at.format("%a %H:%M").to_string()
    } else {
        format!("{}-{} {}", at.month(), at.day(), at.format("%H:%M"))
    }
}

/// Signed whole-percent difference, e.g. `+8%` or `-3%`.
pub fn signed_percent(delta: f64) -> String {
    let p = (delta * 100.0).round() as i64;
    if p > 0 {
        format!("+{p}%")
    } else {
        format!("{p}%")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_tokens() {
        assert_eq!(tokens(842), "842");
        assert_eq!(tokens(45_200), "45.2k");
        assert_eq!(tokens(8_000), "8k");
        assert_eq!(tokens(124_499), "124k");
        assert_eq!(tokens(1_000_000), "1M");
        assert_eq!(tokens(1_270_000), "1.3M");
    }

    #[test]
    fn formats_money_like_v1() {
        assert_eq!(money(0.0), "$0");
        assert_eq!(money(0.005), "$0");
        assert_eq!(money(1.234), "$1.23");
        assert_eq!(money(2.5), "$2.50");
        assert_eq!(money(123.4), "$123");
    }

    #[test]
    fn formats_durations_like_v1() {
        assert_eq!(duration_ms(500), "500ms");
        assert_eq!(duration_ms(59_000), "59s");
        assert_eq!(duration_ms(90_000), "1m30s");
        assert_eq!(duration_ms(120_000), "2m");
        assert_eq!(duration_ms(3_725_000), "1h2m");
        assert_eq!(duration_ms(7_200_000), "2h");
    }

    #[test]
    fn formats_countdowns() {
        assert_eq!(countdown(42), "0:42");
        assert_eq!(countdown(222), "3:42");
        assert_eq!(countdown(3_900), "1h05m");
        assert_eq!(countdown(187_200), "2d4h");
        assert_eq!(countdown(-5), "0:00");
    }

    #[test]
    fn formats_clock_relative_to_today() {
        let utc = FixedOffset::east_opt(0).unwrap();
        let now = 1_790_812_800; // 2026-10-01 00:00:00 UTC, a Thursday
        assert_eq!(clock(now + 14 * 3600 + 20 * 60, now, utc), "14:20");
        assert_eq!(clock(now + 86_400 + 3600, now, utc), "Fri 01:00");
        assert_eq!(clock(now + 10 * 86_400, now, utc), "10-11 00:00");
        let plus8 = FixedOffset::east_opt(8 * 3600).unwrap();
        assert_eq!(clock(now, now, plus8), "08:00");
    }

    #[test]
    fn formats_percents() {
        assert_eq!(percent(0.624), "62");
        assert_eq!(percent(0.001), "1");
        assert_eq!(percent(0.0), "0");
        assert_eq!(signed_percent(0.08), "+8%");
        assert_eq!(signed_percent(-0.031), "-3%");
        assert_eq!(signed_percent(0.0), "0%");
    }
}
