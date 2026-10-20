//! Money is kept in integer cents (US dollars). Parsing and formatting never
//! go through floating point.

use crate::error::{Error, Result};

/// The largest amount accepted: $1,000,000,000.00.
pub const MAX_CENTS: i64 = 100_000_000_000;

/// Parse what a person types: "250", "250.5", "$1,234.56", " 12.00 ".
/// Amounts must be positive with at most two decimal places.
pub fn parse_amount(input: &str, what: &str) -> Result<i64> {
    let bad = || Error::invalid(format!("Enter the {what} as a dollar amount, like 250 or 250.00."));
    let s = input.trim();
    let s = s.strip_prefix('$').unwrap_or(s).trim_start();
    if s.is_empty() {
        return Err(Error::invalid(format!("Enter the {what}.")));
    }
    if s.starts_with('-') {
        return Err(Error::invalid(format!("The {what} can't be negative.")));
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s, ""),
    };
    if frac.len() > 2 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(if frac.bytes().all(|b| b.is_ascii_digit()) {
            Error::invalid(format!("The {what} can have at most two digits after the decimal point."))
        } else {
            bad()
        });
    }
    // Commas are allowed only as thousands separators.
    if whole.contains(',') {
        let groups: Vec<&str> = whole.split(',').collect();
        let first_ok = !groups[0].is_empty() && groups[0].len() <= 3;
        if !first_ok || groups[1..].iter().any(|g| g.len() != 3) {
            return Err(bad());
        }
    }
    let digits: String = whole.chars().filter(|c| *c != ',').collect();
    if digits.is_empty() && frac.is_empty() {
        return Err(bad());
    }
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    if digits.len() > 12 {
        return Err(Error::invalid(format!("The {what} is too large.")));
    }
    let dollars: i64 = if digits.is_empty() { 0 } else { digits.parse().map_err(|_| bad())? };
    let cents: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().map_err(|_| bad())? * 10,
        _ => frac.parse().map_err(|_| bad())?,
    };
    let total = dollars * 100 + cents;
    if total == 0 {
        return Err(Error::invalid(format!("The {what} must be more than $0.00.")));
    }
    if total > MAX_CENTS {
        return Err(Error::invalid(format!("The {what} is too large.")));
    }
    Ok(total)
}

/// "$1,234.56", or "-$5.00" for negatives.
pub fn format_money(cents: i64) -> String {
    let neg = cents < 0;
    let c = cents.unsigned_abs();
    let whole = (c / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        #[allow(clippy::manual_is_multiple_of)] // keep building on older toolchains
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if neg { "-" } else { "" };
    format!("{sign}${grouped}.{:02}", c % 100)
}

/// Plain "1234.56" for CSV.
pub fn plain_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.unsigned_abs();
    format!("{sign}{}.{:02}", c / 100, c % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_amounts() {
        assert_eq!(parse_amount("250", "amount").unwrap(), 25000);
        assert_eq!(parse_amount(" $1,234.5 ", "amount").unwrap(), 123450);
        assert_eq!(parse_amount("$ 0.07", "amount").unwrap(), 7);
        assert_eq!(parse_amount(".5", "amount").unwrap(), 50);
        assert_eq!(parse_amount("12.", "amount").unwrap(), 1200);
        assert_eq!(parse_amount("1,000,000", "amount").unwrap(), 100_000_000);
        for bad in ["", "abc", "-5", "1.234", "1,00", "12,3456", "1e3", "0", "0.00", "$", "1.2.3", "10000000000000"] {
            assert!(parse_amount(bad, "amount").is_err(), "{bad} should fail");
        }
        let e = parse_amount("-5", "payment").unwrap_err().to_string();
        assert_eq!(e, "The payment can't be negative.");
    }

    #[test]
    fn formats_amounts() {
        assert_eq!(format_money(123456789), "$1,234,567.89");
        assert_eq!(format_money(5), "$0.05");
        assert_eq!(format_money(-100000), "-$1,000.00");
        assert_eq!(format_money(0), "$0.00");
        assert_eq!(plain_money(-5), "-0.05");
        assert_eq!(plain_money(250000), "2500.00");
    }
}
