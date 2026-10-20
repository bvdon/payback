//! Interest on a loan. A loan is either interest-free or charges simple
//! interest at a yearly rate.
//!
//! Simple interest accrues each day on the principal still owed, never on
//! unpaid interest, so it doesn't compound. Days are counted actual/365. A
//! payment first covers the interest accrued so far and only the rest
//! reduces the principal (the "US Rule" most personal and car loans use).
//! Interest is rounded to the cent once per stretch between payments.

use crate::date;
use crate::error::{Error, Result};
use crate::model::{Loan, Payment};

/// The highest rate accepted: 100% a year.
pub const MAX_RATE: i64 = 100_000;
/// Rates are kept in thousandths of a percent: 5.25% is 5_250.
const RATE_SCALE: i128 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interest {
    None,
    /// Simple interest at this many thousandths of a percent a year.
    Simple(i64),
}

impl Interest {
    pub fn key(self) -> &'static str {
        match self {
            Interest::None => "none",
            Interest::Simple(_) => "simple",
        }
    }

    /// Thousandths of a percent a year; 0 when interest-free.
    pub fn rate(self) -> i64 {
        match self {
            Interest::None => 0,
            Interest::Simple(r) => r,
        }
    }

    /// Build from the stored or submitted kind and rate.
    pub fn from_parts(kind: &str, rate: i64) -> Result<Interest> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Interest::None),
            "simple" => {
                if rate <= 0 {
                    return Err(Error::invalid("The interest rate must be more than 0%."));
                }
                if rate > MAX_RATE {
                    return Err(Error::invalid("The interest rate can be at most 100% a year."));
                }
                Ok(Interest::Simple(rate))
            }
            _ => Err(Error::invalid("Pick the interest: none or simple.")),
        }
    }
}

/// Parse a rate as typed: "5", "5.25", "4.125%". Up to three decimals.
pub fn parse_rate(input: &str) -> Result<i64> {
    let bad = || Error::invalid("Enter the interest rate as a percentage, like 5 or 4.25.");
    let s = input.trim();
    let s = s.strip_suffix('%').unwrap_or(s).trim_end();
    if s.is_empty() {
        return Err(Error::invalid("Enter the interest rate."));
    }
    if s.starts_with('-') {
        return Err(Error::invalid("The interest rate can't be negative."));
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() && frac.is_empty() {
        return Err(bad());
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    if frac.len() > 3 {
        return Err(Error::invalid("The interest rate can have at most three digits after the decimal point."));
    }
    if whole.len() > 3 {
        return Err(Error::invalid("The interest rate can be at most 100% a year."));
    }
    let w: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| bad())? };
    let f: i64 = if frac.is_empty() { 0 } else { format!("{frac:0<3}").parse().map_err(|_| bad())? };
    Interest::from_parts("simple", w * 1000 + f).map(|i| i.rate())
}

/// "5%", "5.25%", "4.125%".
pub fn format_rate(rate: i64) -> String {
    let frac = format!("{:03}", rate % 1000);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{}%", rate / 1000)
    } else {
        format!("{}.{frac}%", rate / 1000)
    }
}

/// Interest on `principal` cents for `days` days, to the nearest cent.
pub fn accrue(principal: i64, rate: i64, days: i64) -> i64 {
    if principal <= 0 || rate <= 0 || days <= 0 {
        return 0;
    }
    let num = principal as i128 * rate as i128 * days as i128;
    let den = 365 * RATE_SCALE;
    ((num + den / 2) / den) as i64
}

/// Whole days from `from` to `to` (both YYYY-MM-DD); 0 if `to` isn't later.
pub fn days_between(from: &str, to: &str) -> i64 {
    match (date::day_number(from), date::day_number(to)) {
        (Some(a), Some(b)) if b > a => b - a,
        _ => 0,
    }
}

/// One payment as applied to its loan.
pub struct Applied<'a> {
    pub payment: &'a Payment,
    /// The stretch this payment closes: from the previous payment (or the
    /// start date) to this payment's date.
    pub from: String,
    pub days: i64,
    /// Principal owed during that stretch.
    pub principal_before: i64,
    /// Interest that built up over the stretch.
    pub accrued: i64,
    pub to_interest: i64,
    pub to_principal: i64,
    /// Interest still owed after this payment (when it didn't cover it all).
    pub interest_owed_after: i64,
    pub balance_after: i64,
}

/// A loan worked through payment by payment, up to a given day.
pub struct Schedule<'a> {
    /// Oldest first.
    pub rows: Vec<Applied<'a>>,
    /// The day the totals are worked out to: today, or the last payment if
    /// that's later.
    pub as_of: String,
    /// The open stretch since the last payment (or the start date).
    pub since: String,
    pub days_since: i64,
    pub accrued_since: i64,
    /// Principal still owed. Negative means overpaid.
    pub principal: i64,
    /// Interest built up and not yet paid, as of `as_of`.
    pub interest_owed: i64,
    /// All interest charged from the start to `as_of`.
    pub interest_total: i64,
    pub interest_paid: i64,
    /// Interest per day at the principal now owed, to the nearest cent.
    pub daily: i64,
}

impl Schedule<'_> {
    /// What it would take to pay the loan off on `as_of`. Negative means overpaid.
    pub fn balance(&self) -> i64 {
        self.principal + self.interest_owed
    }
}

/// Work through `loan` up to `today`. Each payment first pays the interest
/// built up since the previous one; the rest reduces the principal.
pub fn schedule<'a>(loan: &'a Loan, today: &str) -> Schedule<'a> {
    let rate = loan.interest.rate();
    let mut principal = loan.principal_cents;
    let mut owed = 0;
    let mut total = 0;
    let mut last = loan.start_date.clone();
    let mut rows = Vec::new();
    for p in loan.payments_in_order() {
        let days = days_between(&last, &p.date);
        let principal_before = principal;
        let accrued = accrue(principal, rate, days);
        owed += accrued;
        total += accrued;
        let to_interest = p.amount_cents.min(owed);
        owed -= to_interest;
        let to_principal = p.amount_cents - to_interest;
        principal -= to_principal;
        rows.push(Applied {
            payment: p,
            from: last.clone(),
            days,
            principal_before,
            accrued,
            to_interest,
            to_principal,
            interest_owed_after: owed,
            balance_after: principal + owed,
        });
        if p.date > last {
            last = p.date.clone();
        }
    }
    let days_since = days_between(&last, today);
    let accrued_since = accrue(principal, rate, days_since);
    let as_of = if days_since > 0 { today.to_string() } else { last.clone() };
    Schedule {
        rows,
        as_of,
        since: last,
        days_since,
        accrued_since,
        principal,
        interest_owed: owed + accrued_since,
        interest_total: total + accrued_since,
        interest_paid: total - owed,
        daily: accrue(principal, rate, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_rates() {
        assert_eq!(parse_rate("5").unwrap(), 5_000);
        assert_eq!(parse_rate(" 5.25% ").unwrap(), 5_250);
        assert_eq!(parse_rate("4.125").unwrap(), 4_125);
        assert_eq!(parse_rate(".5").unwrap(), 500);
        assert_eq!(parse_rate("100").unwrap(), 100_000);
        for bad in ["", "abc", "-1", "0", "0.000", "100.001", "1.2345", "5,5", "%", "1.2.3", "1e2"] {
            assert!(parse_rate(bad).is_err(), "{bad} should fail");
        }
        assert_eq!(format_rate(5_000), "5%");
        assert_eq!(format_rate(5_250), "5.25%");
        assert_eq!(format_rate(4_125), "4.125%");
        assert_eq!(format_rate(500), "0.5%");
    }

    #[test]
    fn accrues_simple_interest() {
        // $10,000 at 5% for a year is $500.
        assert_eq!(accrue(1_000_000, 5_000, 365), 50_000);
        // $1,000 at 6% for 30 days is $4.93 (4.9315 rounded).
        assert_eq!(accrue(100_000, 6_000, 30), 493);
        assert_eq!(accrue(-500, 6_000, 30), 0);
        assert_eq!(days_between("2026-01-01", "2027-01-01"), 365);
        assert_eq!(days_between("2024-02-28", "2024-03-01"), 2);
        assert_eq!(days_between("2026-03-01", "2026-02-01"), 0);
        assert!(Interest::from_parts("simple", 0).is_err());
        assert_eq!(Interest::from_parts("", 0).unwrap(), Interest::None);
    }

    #[test]
    fn payments_cover_interest_first() {
        use crate::model::tests::{loan, payment};
        // $1,000 at 6% from Jan 1.
        let mut l = loan(vec![
            payment("a", 10_000, "2026-01-31", 1), // 30 days: $4.93 interest, $95.07 principal
            payment("b", 300, "2026-03-02", 2),    // 30 days on $904.93: $4.46, so $1.46 stays owed
        ]);
        l.interest = Interest::Simple(6_000);
        let s = schedule(&l, "2026-04-01");
        let a = &s.rows[0];
        assert_eq!((a.days, a.accrued, a.to_interest, a.to_principal), (30, 493, 493, 9_507));
        assert_eq!(a.balance_after, 90_493);
        let b = &s.rows[1];
        assert_eq!((b.accrued, b.to_interest, b.to_principal, b.interest_owed_after), (446, 300, 0, 146));
        // 30 more days on the same principal, plus what was left unpaid.
        assert_eq!((s.days_since, s.accrued_since), (30, 446));
        assert_eq!(s.principal, 90_493);
        assert_eq!(s.interest_owed, 146 + 446);
        assert_eq!(s.interest_total, 493 + 446 + 446);
        assert_eq!(s.interest_paid, 793);
        assert_eq!(s.balance(), 90_493 + 592);
        assert_eq!(s.daily, 15);
        assert_eq!(s.as_of, "2026-04-01");

        // Interest-free loans come out exactly as principal minus payments.
        l.interest = Interest::None;
        let s = schedule(&l, "2026-04-01");
        assert_eq!((s.balance(), s.interest_total), (100_000 - 10_300, 0));

        // Overpaying stops interest; a future-dated payment moves "as of".
        let mut big = loan(vec![payment("c", 200_000, "2026-12-31", 1)]);
        big.interest = Interest::Simple(5_000);
        let s = schedule(&big, "2026-06-01");
        assert_eq!(s.rows[0].accrued, 4_986); // 364 days on $1,000 at 5%
        assert_eq!(s.balance(), -(200_000 - 100_000 - 4_986));
        assert_eq!((s.as_of.as_str(), s.daily), ("2026-12-31", 0));
    }
}
