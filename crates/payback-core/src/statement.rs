//! Loan statements to share or print: a one-or-more page PDF and a CSV.
//! Both list the loan amount first and then every payment, oldest first,
//! with the balance after each.

use crate::csv::Writer;
use crate::date;
use crate::interest::{format_rate, Interest};
use crate::model::Loan;
use crate::money::{format_money, plain_money};
use crate::pdf::{fit, wrap, Document, Font, Rgb};

const INK: Rgb = Rgb(0.11, 0.12, 0.13);
const MUTED: Rgb = Rgb(0.42, 0.44, 0.41);
const RULE: Rgb = Rgb(0.85, 0.86, 0.84);
const TEAL: Rgb = Rgb(0.055, 0.455, 0.408);
const AMBER: Rgb = Rgb(0.54, 0.35, 0.0);

const MARGIN: f64 = 54.0;
const ROW: f64 = 20.0;
const LINE: f64 = 12.0;

// Column positions on a Letter page. Loans with interest add two columns
// and give the note less room.
const X_DATE: f64 = MARGIN;
const X_TYPE: f64 = 140.0;
const X_NOTE: f64 = 210.0;
const X_BALANCE: f64 = 558.0; // right edge

struct Columns {
    note_w: f64,
    /// Right edges.
    amount: f64,
    interest: Option<(f64, f64)>,
}

const PLAIN: Columns = Columns { note_w: 200.0, amount: 482.0, interest: None };
const WITH_INTEREST: Columns = Columns { note_w: 104.0, amount: 380.0, interest: Some((440.0, 500.0)) };

pub fn file_stem(loan: &Loan, today: &str) -> String {
    let slug: String = loan
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "loan".to_string() } else { slug };
    format!("{slug}-{today}")
}

fn balance_text(cents: i64) -> String {
    if cents < 0 {
        format!("{} over", format_money(-cents))
    } else {
        format_money(cents)
    }
}

fn has_interest(loan: &Loan) -> bool {
    loan.interest != Interest::None
}

/// "5.25% simple interest", or "No interest".
pub fn interest_text(loan: &Loan) -> String {
    match loan.interest {
        Interest::None => "No interest".to_string(),
        Interest::Simple(r) => format!("{} simple interest", format_rate(r)),
    }
}

pub fn csv(loan: &Loan, today: &str) -> String {
    let s = loan.schedule(today);
    let mut w = Writer::new();
    let p = loan.principal_cents;
    if !has_interest(loan) {
        w.row(&["Date", "Entry", "Type", "Note", "Amount", "Balance"]);
        w.row(&[loan.start_date.as_str(), "Loan", "", "", &plain_money(p), &plain_money(p)]);
        for r in &s.rows {
            let pay = r.payment;
            w.row(&[
                &pay.date,
                "Payment",
                pay.method.label(),
                &pay.note,
                &plain_money(-pay.amount_cents),
                &plain_money(r.balance_after),
            ]);
        }
        return w.finish();
    }
    // Amount is always the change to the balance; a payment's amount is split
    // into what went to interest and what went to principal.
    w.row(&["Date", "Entry", "Type", "Note", "Amount", "To interest", "To principal", "Balance"]);
    let what = interest_text(loan);
    w.row(&[loan.start_date.as_str(), "Loan", "", &what, &plain_money(p), "", "", &plain_money(p)]);
    for r in &s.rows {
        let pay = r.payment;
        w.row(&[
            &pay.date,
            "Payment",
            pay.method.label(),
            &pay.note,
            &plain_money(-pay.amount_cents),
            &plain_money(-r.to_interest),
            &plain_money(-r.to_principal),
            &plain_money(r.balance_after),
        ]);
    }
    if s.accrued_since > 0 {
        let note = format!("{} days since {}", s.days_since, date::display(&s.since));
        w.row(&[
            &s.as_of,
            "Interest so far",
            "",
            &note,
            &plain_money(s.accrued_since),
            "",
            "",
            &plain_money(s.balance()),
        ]);
    }
    w.finish()
}

struct Table {
    doc: Document,
    page: usize,
    y: f64,
    cols: &'static Columns,
}

/// One table row's text.
struct Row<'a> {
    day: &'a str,
    kind: &'a str,
    note: &'a str,
    amount: String,
    /// Paid toward interest and principal, for loans with interest.
    split: Option<(String, String)>,
    balance: String,
    balance_color: Rgb,
    bold: bool,
}

impl Table {
    fn header(&mut self) {
        let y = self.y;
        let c = self.cols;
        let p = self.doc.page(self.page);
        p.text(X_DATE, y, 8.5, Font::Bold, MUTED, "DATE");
        p.text(X_TYPE, y, 8.5, Font::Bold, MUTED, "TYPE");
        p.text(X_NOTE, y, 8.5, Font::Bold, MUTED, "NOTE");
        p.text_right(c.amount, y, 8.5, Font::Bold, MUTED, "AMOUNT");
        if let Some((xi, xp)) = c.interest {
            p.text_right(xi, y, 8.5, Font::Bold, MUTED, "INTEREST");
            p.text_right(xp, y, 8.5, Font::Bold, MUTED, "PRINCIPAL");
        }
        p.text_right(X_BALANCE, y, 8.5, Font::Bold, MUTED, "BALANCE");
        p.line(MARGIN, y + 6.0, X_BALANCE, y + 6.0, 0.75, INK);
        self.y += ROW;
    }

    fn new_page(&mut self) {
        self.page = self.doc.add_page();
        self.y = MARGIN + 12.0;
        self.header();
    }

    fn row(&mut self, r: Row) {
        let c = self.cols;
        let lines = wrap(r.note, c.note_w, 9.5, Font::Regular, 3);
        let extra = lines.len().saturating_sub(1) as f64 * LINE;
        if self.y + extra > self.doc.height - 72.0 {
            self.new_page();
        }
        let y = self.y;
        let font = if r.bold { Font::Bold } else { Font::Regular };
        let p = self.doc.page(self.page);
        p.text(X_DATE, y, 9.5, font, INK, &date::display(r.day));
        p.text(X_TYPE, y, 9.5, font, INK, r.kind);
        for (i, l) in lines.iter().enumerate() {
            p.text(X_NOTE, y + i as f64 * LINE, 9.5, Font::Regular, MUTED, l);
        }
        p.text_right(c.amount, y, 9.5, font, INK, &r.amount);
        if let (Some((xi, xp)), Some((i, pr))) = (c.interest, &r.split) {
            p.text_right(xi, y, 9.5, Font::Regular, MUTED, i);
            p.text_right(xp, y, 9.5, Font::Regular, MUTED, pr);
        }
        p.text_right(X_BALANCE, y, 9.5, Font::Bold, r.balance_color, &r.balance);
        p.line(MARGIN, y + extra + 7.0, X_BALANCE, y + extra + 7.0, 0.4, RULE);
        self.y += ROW + extra;
    }
}

/// A Letter-size PDF statement. `today` is the statement date (YYYY-MM-DD).
pub fn pdf(loan: &Loan, today: &str) -> Vec<u8> {
    let s = loan.schedule(today);
    let interest = has_interest(loan);
    let mut doc = Document::letter(&format!("{} — loan statement", loan.name));
    let page = doc.add_page();
    let right = X_BALANCE;
    {
        let p = doc.page(page);
        p.text(MARGIN, 72.0, 20.0, Font::Bold, INK, "Loan statement");
        p.text_right(right, 72.0, 9.5, Font::Regular, MUTED, &format!("As of {}", date::display(&s.as_of)));
        let name = fit(&loan.name, right - MARGIN, 13.0, Font::Bold);
        p.text(MARGIN, 96.0, 13.0, Font::Bold, INK, &name);
        let mut started = format!("Started {}", date::display(&loan.start_date));
        if interest {
            started = format!("{started} · {}", interest_text(loan));
        }
        let sub = match (loan.lender.as_str(), loan.borrower.as_str()) {
            ("", "") => started,
            (l, b) => {
                let who = |s: &str| if s.is_empty() { "—".to_string() } else { s.to_string() };
                format!("Lent by {} to {} · {started}", who(l), who(b))
            }
        };
        p.text(MARGIN, 113.0, 9.5, Font::Regular, MUTED, &fit(&sub, right - MARGIN, 9.5, Font::Regular));

        // Summary figures.
        let balance = s.balance();
        let owed_label = if interest { "BALANCE TODAY" } else { "BALANCE REMAINING" };
        let (bal_label, bal_value, bal_color) = match balance {
            b if b < 0 => ("OVERPAID BY", format_money(-b), AMBER),
            0 => (owed_label, "Paid off".to_string(), TEAL),
            b => (owed_label, format_money(b), TEAL),
        };
        let third = if interest {
            ("INTEREST SO FAR", format_money(s.interest_total), INK)
        } else {
            ("PAYMENTS", loan.payments.len().to_string(), INK)
        };
        let figures = [
            ("LOAN AMOUNT", format_money(loan.principal_cents), INK),
            ("TOTAL PAID", format_money(loan.paid_cents()), INK),
            third,
            (bal_label, bal_value, bal_color),
        ];
        let top = 136.0;
        p.line(MARGIN, top, right, top, 0.4, RULE);
        let col = (right - MARGIN) / 4.0;
        for (i, (label, value, color)) in figures.iter().enumerate() {
            let x = MARGIN + col * i as f64;
            p.text(x, top + 20.0, 8.0, Font::Bold, MUTED, label);
            p.text(x, top + 40.0, 15.0, Font::Bold, *color, value);
        }
        p.line(MARGIN, top + 56.0, right, top + 56.0, 0.4, RULE);
        if interest && balance > 0 {
            let owed = format!(
                "{} principal + {} interest. Interest builds up by about {} a day; each payment pays the interest first.",
                format_money(s.principal),
                format_money(s.interest_owed),
                format_money(s.daily)
            );
            p.text(MARGIN, top + 74.0, 9.0, Font::Regular, MUTED, &fit(&owed, right - MARGIN, 9.0, Font::Regular));
        }
    }

    let cols = if interest { &WITH_INTEREST } else { &PLAIN };
    let mut t = Table { doc, page, y: if interest { 250.0 } else { 232.0 }, cols };
    t.header();
    let p = loan.principal_cents;
    t.row(Row {
        day: &loan.start_date,
        kind: "Loan",
        note: "",
        amount: format_money(p),
        split: None,
        balance: format_money(p),
        balance_color: INK,
        bold: true,
    });
    for r in &s.rows {
        let pay = r.payment;
        t.row(Row {
            day: &pay.date,
            kind: pay.method.label(),
            note: &pay.note,
            amount: format_money(-pay.amount_cents),
            split: Some((format_money(-r.to_interest), format_money(-r.to_principal))),
            balance: balance_text(r.balance_after),
            balance_color: if r.balance_after < 0 { AMBER } else { INK },
            bold: false,
        });
    }
    if interest && s.accrued_since > 0 {
        let note = format!("{} days since {}", s.days_since, date::display(&s.since));
        t.row(Row {
            day: &s.as_of,
            kind: "Interest",
            note: &note,
            amount: format!("+{}", format_money(s.accrued_since)),
            split: None,
            balance: balance_text(s.balance()),
            balance_color: INK,
            bold: false,
        });
    }
    if s.rows.is_empty() {
        let y = t.y + 4.0;
        t.doc.page(t.page).text(MARGIN, y, 9.5, Font::Regular, MUTED, "No payments yet.");
    }

    let mut doc = t.doc;
    let n = doc.page_count();
    let h = doc.height;
    for i in 0..n {
        let footer = format!("Page {} of {n}", i + 1);
        let p = doc.page(i);
        p.text(MARGIN, h - 36.0, 8.0, Font::Regular, MUTED, "Made with Payback");
        p.text_right(right, h - 36.0, 8.0, Font::Regular, MUTED, &footer);
    }
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{loan, payment};

    #[test]
    fn csv_lists_loan_then_payments() {
        let mut l = loan(vec![payment("b", 30_000, "2026-03-01", 2), payment("a", 20_000, "2026-02-01", 1)]);
        l.payments[0].note = "rent, March".into();
        let rows = crate::csv::parse(&csv(&l, "2026-10-19"));
        assert_eq!(rows[0], vec!["Date", "Entry", "Type", "Note", "Amount", "Balance"]);
        assert_eq!(rows[1], vec!["2026-01-01", "Loan", "", "", "1000.00", "1000.00"]);
        assert_eq!(rows[2], vec!["2026-02-01", "Payment", "Venmo", "", "-200.00", "800.00"]);
        assert_eq!(rows[3], vec!["2026-03-01", "Payment", "Venmo", "rent, March", "-300.00", "500.00"]);
    }

    #[test]
    fn pdf_paginates() {
        let many: Vec<_> = (0..80).map(|i| payment(&format!("p{i}"), 100, "2026-02-01", i)).collect();
        let bytes = pdf(&loan(many), "2026-10-19");
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 3"), "expected 3 pages");
        assert!(s.contains("(Page 3 of 3)"));
        let empty = String::from_utf8_lossy(&pdf(&loan(vec![]), "2026-10-19")).to_string();
        assert!(empty.contains("(No payments yet.)"));
        assert!(empty.contains("(Lent by Rob to Sam \\267 Started Jan 1, 2026)"));
        assert_eq!(file_stem(&loan(vec![]), "2026-10-19"), "car-2026-10-19");
    }

    #[test]
    fn interest_statements() {
        let mut l = loan(vec![payment("a", 10_000, "2026-01-31", 1)]);
        l.interest = Interest::Simple(6_000);
        let rows = crate::csv::parse(&csv(&l, "2026-03-02"));
        assert_eq!(rows[0], vec!["Date", "Entry", "Type", "Note", "Amount", "To interest", "To principal", "Balance"]);
        assert_eq!(rows[1][3], "6% simple interest");
        assert_eq!(rows[2][4..], ["-100.00", "-4.93", "-95.07", "904.93"]);
        // 30 days on $904.93 at 6% is $4.46.
        assert_eq!(rows[3][..2], ["2026-03-02", "Interest so far"]);
        assert_eq!(rows[3][4..], ["4.46", "", "", "909.39"]);
        let pdf = String::from_utf8_lossy(&pdf(&l, "2026-03-02")).to_string();
        assert!(pdf.contains("(INTEREST SO FAR)"));
        assert!(pdf.contains("($9.39)"));
        assert!(pdf.contains("(BALANCE TODAY)"));
    }
}
