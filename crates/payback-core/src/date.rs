//! Calendar dates as plain "YYYY-MM-DD" strings. There are no times or time
//! zones anywhere in Payback; the UI supplies "today".

use crate::error::{Error, Result};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Split a valid date into (year, month, day).
pub fn parts(s: &str) -> Option<(i32, u32, u32)> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i32 = s[0..4].parse().ok()?;
    let m: u32 = s[5..7].parse().ok()?;
    let d: u32 = s[8..10].parse().ok()?;
    if !(1900..=2200).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some((y, m, d))
}

/// Check that `s` is a real date like 2026-10-19.
pub fn validate(s: &str, what: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::invalid(format!("Enter the {what}.")));
    }
    parts(s).map(|_| s.to_string()).ok_or_else(|| Error::invalid(format!("The {what} isn't a valid date.")))
}

/// "Oct 19, 2026".
pub fn display(s: &str) -> String {
    match parts(s) {
        Some((y, m, d)) => format!("{} {d}, {y}", MONTHS[m as usize - 1]),
        None => s.to_string(),
    }
}

/// The UTC calendar date and time for a Unix timestamp, as
/// (year, month, day, hour, minute, second). Used only for file names.
pub fn utc_parts(ts: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400) as u32;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d, secs / 3600, (secs % 3600) / 60, secs % 60)
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dates() {
        assert!(validate("2026-10-19", "date").is_ok());
        assert!(validate("2024-02-29", "date").is_ok());
        assert!(validate("2025-02-29", "date").is_err());
        assert!(validate("2026-13-01", "date").is_err());
        assert!(validate("10/19/2026", "date").is_err());
        assert!(validate("", "date").is_err());
        assert_eq!(validate(" 2026-01-05 ", "date").unwrap(), "2026-01-05");
        assert_eq!(display("2026-01-05"), "Jan 5, 2026");
    }

    #[test]
    fn utc_conversion() {
        assert_eq!(utc_parts(0), (1970, 1, 1, 0, 0, 0));
        // 2026-10-19T20:15:30Z
        assert_eq!(utc_parts(1_792_440_930), (2026, 10, 19, 20, 15, 30));
        assert_eq!(utc_parts(951_782_400), (2000, 2, 29, 0, 0, 0));
    }
}
