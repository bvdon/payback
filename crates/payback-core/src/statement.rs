//! Loan statements to share or print: a one-or-more page PDF and a CSV.
//! Both list the loan amount first and then every payment, oldest first,
//! with the balance after each.

use crate::csv::Writer;
use crate::date;
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

// Column positions on a Letter page.
const X_DATE: f64 = MARGIN;
const X_TYPE: f64 = 140.0;
const X_NOTE: f64 = 210.0;
const NOTE_W: f64 = 200.0;
const X_AMOUNT: f64 = 482.0; // right edge
const X_BALANCE: f64 = 558.0; // right edge

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

pub fn csv(loan: &Loan) -> String {
    let mut w = Writer::new();
    w.row(&["Date", "Entry", "Type", "Note", "Amount", "Balance"]);
    let p = loan.principal_cents;
    w.row(&[loan.start_date.as_str(), "Loan", "", "", &plain_money(p), &plain_money(p)]);
    for (pay, bal) in loan.ledger() {
        w.row(&[
            &pay.date,
            "Payment",
            pay.method.label(),
            &pay.note,
            &plain_money(-pay.amount_cents),
            &plain_money(bal),
        ]);
    }
    w.finish()
}

struct Table {
    doc: Document,
    page: usize,
    y: f64,
}

impl Table {
    fn header(&mut self) {
        let y = self.y;
        let p = self.doc.page(self.page);
        p.text(X_DATE, y, 8.5, Font::Bold, MUTED, "DATE");
        p.text(X_TYPE, y, 8.5, Font::Bold, MUTED, "TYPE");
        p.text(X_NOTE, y, 8.5, Font::Bold, MUTED, "NOTE");
        p.text_right(X_AMOUNT, y, 8.5, Font::Bold, MUTED, "AMOUNT");
        p.text_right(X_BALANCE, y, 8.5, Font::Bold, MUTED, "BALANCE");
        p.line(MARGIN, y + 6.0, X_BALANCE, y + 6.0, 0.75, INK);
        self.y += ROW;
    }

    fn new_page(&mut self) {
        self.page = self.doc.add_page();
        self.y = MARGIN + 12.0;
        self.header();
    }

    #[allow(clippy::too_many_arguments)]
    fn row(&mut self, day: &str, kind: &str, note: &str, amount: &str, balance: &str, balance_color: Rgb, bold: bool) {
        let lines = wrap(note, NOTE_W, 9.5, Font::Regular, 3);
        let extra = lines.len().saturating_sub(1) as f64 * LINE;
        if self.y + extra > self.doc.height - 72.0 {
            self.new_page();
        }
        let y = self.y;
        let font = if bold { Font::Bold } else { Font::Regular };
        let p = self.doc.page(self.page);
        p.text(X_DATE, y, 9.5, font, INK, &date::display(day));
        p.text(X_TYPE, y, 9.5, font, INK, kind);
        for (i, l) in lines.iter().enumerate() {
            p.text(X_NOTE, y + i as f64 * LINE, 9.5, Font::Regular, MUTED, l);
        }
        p.text_right(X_AMOUNT, y, 9.5, font, INK, amount);
        p.text_right(X_BALANCE, y, 9.5, Font::Bold, balance_color, balance);
        p.line(MARGIN, y + extra + 7.0, X_BALANCE, y + extra + 7.0, 0.4, RULE);
        self.y += ROW + extra;
    }
}

/// A Letter-size PDF statement. `today` is the statement date (YYYY-MM-DD).
pub fn pdf(loan: &Loan, today: &str) -> Vec<u8> {
    let mut doc = Document::letter(&format!("{} — loan statement", loan.name));
    let page = doc.add_page();
    let right = X_BALANCE;
    {
        let p = doc.page(page);
        p.text(MARGIN, 72.0, 20.0, Font::Bold, INK, "Loan statement");
        p.text_right(right, 72.0, 9.5, Font::Regular, MUTED, &format!("As of {}", date::display(today)));
        let name = fit(&loan.name, right - MARGIN, 13.0, Font::Bold);
        p.text(MARGIN, 96.0, 13.0, Font::Bold, INK, &name);
        let started = format!("Started {}", date::display(&loan.start_date));
        let sub = match (loan.lender.as_str(), loan.borrower.as_str()) {
            ("", "") => started,
            (l, b) => {
                let who = |s: &str| if s.is_empty() { "—".to_string() } else { s.to_string() };
                format!("Lent by {} to {} · {started}", who(l), who(b))
            }
        };
        p.text(MARGIN, 113.0, 9.5, Font::Regular, MUTED, &fit(&sub, right - MARGIN, 9.5, Font::Regular));

        // Summary figures.
        let balance = loan.balance_cents();
        let (bal_label, bal_value, bal_color) = match balance {
            b if b < 0 => ("OVERPAID BY", format_money(-b), AMBER),
            0 => ("BALANCE REMAINING", "Paid off".to_string(), TEAL),
            b => ("BALANCE REMAINING", format_money(b), TEAL),
        };
        let figures = [
            ("LOAN AMOUNT", format_money(loan.principal_cents), INK),
            ("TOTAL PAID", format_money(loan.paid_cents()), INK),
            ("PAYMENTS", loan.payments.len().to_string(), INK),
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
    }

    let mut t = Table { doc, page, y: 232.0 };
    t.header();
    let p = loan.principal_cents;
    t.row(&loan.start_date, "Loan", "", &format_money(p), &format_money(p), INK, true);
    let ledger = loan.ledger();
    for (pay, bal) in &ledger {
        let color = if *bal < 0 { AMBER } else { INK };
        t.row(
            &pay.date,
            pay.method.label(),
            &pay.note,
            &format_money(-pay.amount_cents),
            &balance_text(*bal),
            color,
            false,
        );
    }
    if ledger.is_empty() {
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
        let rows = crate::csv::parse(&csv(&l));
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
}
