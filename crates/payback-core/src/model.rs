//! The data: loans, each holding its payments. The same JSON shapes are used
//! on disk, in exported `.payback` files and over the API.

use crate::date;
use crate::error::{Error, Result};
use crate::id;
use crate::json::{Args, Json, ToJson};

pub const MAX_NAME: usize = 100;
pub const MAX_NOTE: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Check,
    PayPal,
    Venmo,
    Cash,
    Other,
}

impl Method {
    pub const ALL: [Method; 5] = [Method::Check, Method::PayPal, Method::Venmo, Method::Cash, Method::Other];

    pub fn key(self) -> &'static str {
        match self {
            Method::Check => "check",
            Method::PayPal => "paypal",
            Method::Venmo => "venmo",
            Method::Cash => "cash",
            Method::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Method::Check => "Check",
            Method::PayPal => "PayPal",
            Method::Venmo => "Venmo",
            Method::Cash => "Cash",
            Method::Other => "Other",
        }
    }

    pub fn parse(s: &str) -> Result<Method> {
        let s = s.trim();
        Method::ALL
            .into_iter()
            .find(|m| m.key().eq_ignore_ascii_case(s) || m.label().eq_ignore_ascii_case(s))
            .ok_or_else(|| Error::invalid("Pick a payment type: Check, PayPal, Venmo, Cash or Other."))
    }
}

impl ToJson for Method {
    fn to_json(&self) -> Json {
        Json::Str(self.key().into())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Payment {
    pub id: String,
    pub amount_cents: i64,
    pub date: String,
    pub method: Method,
    pub note: String,
    /// Unix seconds; orders payments made on the same day.
    pub created_at: i64,
    pub updated_at: i64,
}

crate::impl_to_json!(Payment {
    id => "id",
    amount_cents => "amountCents",
    date => "date",
    method => "method",
    note => "note",
    created_at => "createdAt",
    updated_at => "updatedAt",
});

#[derive(Clone, Debug, PartialEq)]
pub struct Loan {
    pub id: String,
    pub name: String,
    /// Who lent the money. Empty for loans made before 0.1.1.
    pub lender: String,
    /// Who borrowed it. Empty for loans made before 0.1.1.
    pub borrower: String,
    pub principal_cents: i64,
    pub start_date: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub payments: Vec<Payment>,
}

crate::impl_to_json!(Loan {
    id => "id",
    name => "name",
    lender => "lender",
    borrower => "borrower",
    principal_cents => "principalCents",
    start_date => "startDate",
    created_at => "createdAt",
    updated_at => "updatedAt",
    payments => "payments",
});

pub fn clean_name(s: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::invalid("Give the loan a name."));
    }
    if s.chars().count() > MAX_NAME {
        return Err(Error::invalid(format!("The loan name can be at most {MAX_NAME} characters.")));
    }
    Ok(s.to_string())
}

/// A lender or borrower name. `role` is "lender" or "borrower".
pub fn clean_person(s: &str, role: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::invalid(format!(
            "Enter who {}.",
            if role == "lender" { "lent the money" } else { "borrowed the money" }
        )));
    }
    if s.chars().count() > MAX_NAME {
        return Err(Error::invalid(format!("The {role}'s name can be at most {MAX_NAME} characters.")));
    }
    Ok(s.to_string())
}

/// Names read from files may be blank (older versions didn't have them).
fn stored_person(a: Args, key: &str) -> Result<String> {
    let s = a.opt_str(key)?.unwrap_or("").trim();
    if s.chars().count() > MAX_NAME {
        return Err(Error::invalid("A loan in the file has a name that's too long."));
    }
    Ok(s.to_string())
}

pub fn clean_note(s: &str) -> Result<String> {
    let s = s.trim();
    if s.chars().count() > MAX_NOTE {
        return Err(Error::invalid(format!("The note can be at most {MAX_NOTE} characters.")));
    }
    Ok(s.to_string())
}

/// Amounts stored in files must be positive and in range.
fn stored_cents(a: Args, key: &str, what: &str) -> Result<i64> {
    let c = a.req_i64(key).map_err(|_| Error::invalid(format!("A {what} in the file is missing its amount.")))?;
    if c <= 0 || c > crate::money::MAX_CENTS {
        return Err(Error::invalid(format!("A {what} in the file has an amount that's out of range.")));
    }
    Ok(c)
}

fn stored_id(a: Args, what: &str) -> Result<String> {
    let id = a.opt_str("id")?.unwrap_or("");
    if !id::is_valid(id) {
        return Err(Error::invalid(format!("A {what} in the file has a missing or invalid ID.")));
    }
    Ok(id.to_string())
}

impl Payment {
    /// Read a payment from a stored or exported file, validating everything.
    pub fn from_json(v: &Json) -> Result<Payment> {
        let a = Args::new(v);
        let date = date::validate(a.opt_str("date")?.unwrap_or(""), "payment date")
            .map_err(|_| Error::invalid("A payment in the file has an invalid date."))?;
        Ok(Payment {
            id: stored_id(a, "payment")?,
            amount_cents: stored_cents(a, "amountCents", "payment")?,
            date,
            method: Method::parse(a.opt_str("method")?.unwrap_or("other"))?,
            note: clean_note(a.opt_str("note")?.unwrap_or(""))?,
            created_at: a.opt_i64("createdAt")?.unwrap_or(0),
            updated_at: a.opt_i64("updatedAt")?.unwrap_or(0),
        })
    }
}

impl Loan {
    pub fn from_json(v: &Json) -> Result<Loan> {
        let a = Args::new(v);
        let start_date = date::validate(a.opt_str("startDate")?.unwrap_or(""), "start date")
            .map_err(|_| Error::invalid("A loan in the file has an invalid start date."))?;
        let mut payments = Vec::new();
        for p in a.array("payments")? {
            let p = Payment::from_json(p)?;
            if payments.iter().any(|q: &Payment| q.id == p.id) {
                return Err(Error::invalid("A loan in the file lists the same payment twice."));
            }
            payments.push(p);
        }
        Ok(Loan {
            id: stored_id(a, "loan")?,
            name: clean_name(a.opt_str("name")?.unwrap_or(""))?,
            lender: stored_person(a, "lender")?,
            borrower: stored_person(a, "borrower")?,
            principal_cents: stored_cents(a, "principalCents", "loan")?,
            start_date,
            created_at: a.opt_i64("createdAt")?.unwrap_or(0),
            updated_at: a.opt_i64("updatedAt")?.unwrap_or(0),
            payments,
        })
    }

    pub fn paid_cents(&self) -> i64 {
        self.payments.iter().map(|p| p.amount_cents).sum()
    }

    /// Principal minus payments. Negative means overpaid.
    pub fn balance_cents(&self) -> i64 {
        self.principal_cents - self.paid_cents()
    }

    /// Payments oldest first: by date, then by when they were entered.
    pub fn payments_in_order(&self) -> Vec<&Payment> {
        let mut v: Vec<&Payment> = self.payments.iter().collect();
        v.sort_by(|a, b| a.date.cmp(&b.date).then(a.created_at.cmp(&b.created_at)).then(a.id.cmp(&b.id)));
        v
    }

    /// Payments oldest first, each with the balance left after it.
    pub fn ledger(&self) -> Vec<(&Payment, i64)> {
        let mut balance = self.principal_cents;
        self.payments_in_order()
            .into_iter()
            .map(|p| {
                balance -= p.amount_cents;
                (p, balance)
            })
            .collect()
    }

    pub fn payment(&self, id: &str) -> Result<&Payment> {
        self.payments.iter().find(|p| p.id == id).ok_or_else(|| Error::not_found("That payment no longer exists."))
    }

    /// The loan as the UI sees it: stored fields plus computed totals, with
    /// payments newest first and a running balance on each.
    pub fn view(&self) -> Json {
        let ledger = self.ledger();
        let last = ledger.last().map(|(p, _)| p.date.clone());
        let payments: Vec<Json> =
            ledger.iter().rev().map(|(p, bal)| p.to_json().with("balanceAfterCents", *bal)).collect();
        let mut v = self.to_json();
        v.set("payments", Json::Arr(payments));
        v.with("paidCents", self.paid_cents())
            .with("balanceCents", self.balance_cents())
            .with("paymentCount", self.payments.len())
            .with("lastPaymentDate", last)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn payment(id: &str, cents: i64, date: &str, created: i64) -> Payment {
        Payment {
            id: id.into(),
            amount_cents: cents,
            date: date.into(),
            method: Method::Venmo,
            note: String::new(),
            created_at: created,
            updated_at: created,
        }
    }

    pub fn loan(payments: Vec<Payment>) -> Loan {
        Loan {
            id: "loan1".into(),
            name: "Car".into(),
            lender: "Rob".into(),
            borrower: "Sam".into(),
            principal_cents: 100_000,
            start_date: "2026-01-01".into(),
            created_at: 1,
            updated_at: 1,
            payments,
        }
    }

    #[test]
    fn ledger_orders_and_runs_balance() {
        let l = loan(vec![
            payment("c", 10_000, "2026-03-01", 5),
            payment("a", 25_000, "2026-02-01", 9),
            payment("b", 5_000, "2026-03-01", 2),
        ]);
        let ledger: Vec<(&str, i64)> = l.ledger().iter().map(|(p, b)| (p.id.as_str(), *b)).collect();
        assert_eq!(ledger, vec![("a", 75_000), ("b", 70_000), ("c", 60_000)]);
        assert_eq!(l.paid_cents(), 40_000);
        assert_eq!(l.balance_cents(), 60_000);
        let v = l.view();
        let ps = v.get("payments").unwrap().as_array().unwrap();
        assert_eq!(ps[0].get("id").unwrap().as_str(), Some("c"));
        assert_eq!(ps[0].get("balanceAfterCents").unwrap().as_i64(), Some(60_000));
        assert_eq!(v.get("lastPaymentDate").unwrap().as_str(), Some("2026-03-01"));
    }

    #[test]
    fn json_roundtrip_and_validation() {
        let l = loan(vec![payment("p1", 500, "2026-02-02", 3)]);
        let back = Loan::from_json(&Json::parse(&l.to_json().stringify()).unwrap()).unwrap();
        assert_eq!(back, l);
        let mut bad = l.to_json();
        bad.set("principalCents", Json::Int(-1));
        assert!(Loan::from_json(&bad).is_err());
        let mut dup = l.clone();
        dup.payments.push(dup.payments[0].clone());
        assert!(Loan::from_json(&dup.to_json()).is_err());
        // Files from before lender/borrower existed still load, with blanks.
        let mut old = l.to_json();
        if let Json::Obj(fields) = &mut old {
            fields.retain(|(k, _)| k != "lender" && k != "borrower");
        }
        let old = Loan::from_json(&old).unwrap();
        assert_eq!((old.lender.as_str(), old.borrower.as_str()), ("", ""));
        assert_eq!(Method::parse("PAYPAL").unwrap(), Method::PayPal);
        assert!(Method::parse("bitcoin").is_err());
    }
}
