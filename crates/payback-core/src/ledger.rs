//! Changes to loans and payments. Each one validates its input, works on a
//! copy and commits it, so a rejected change leaves the data untouched.

use crate::date;
use crate::error::{Error, Result};
use crate::id;
use crate::model::{clean_name, clean_note, clean_person, Loan, Method, Payment};
use crate::money::MAX_CENTS;
use crate::store::Store;

pub struct LoanInput {
    pub name: String,
    pub lender: String,
    pub borrower: String,
    pub principal_cents: i64,
    pub start_date: String,
}

#[derive(Default)]
pub struct LoanPatch {
    pub name: Option<String>,
    pub lender: Option<String>,
    pub borrower: Option<String>,
    pub principal_cents: Option<i64>,
    pub start_date: Option<String>,
}

pub struct PaymentInput {
    /// Set when undoing a delete, so the payment keeps its identity.
    pub id: Option<String>,
    pub amount_cents: i64,
    pub date: String,
    pub method: Method,
    pub note: String,
    pub created_at: Option<i64>,
}

#[derive(Default)]
pub struct PaymentPatch {
    pub amount_cents: Option<i64>,
    pub date: Option<String>,
    pub method: Option<Method>,
    pub note: Option<String>,
}

fn check_cents(c: i64, what: &str) -> Result<i64> {
    if c <= 0 {
        return Err(Error::invalid(format!("The {what} must be more than $0.00.")));
    }
    if c > MAX_CENTS {
        return Err(Error::invalid(format!("The {what} is too large.")));
    }
    Ok(c)
}

impl Store {
    pub fn create_loan(&mut self, input: LoanInput) -> Result<Loan> {
        let now = self.now();
        let loan = Loan {
            id: id::new_id(),
            name: clean_name(&input.name)?,
            lender: clean_person(&input.lender, "lender")?,
            borrower: clean_person(&input.borrower, "borrower")?,
            principal_cents: check_cents(input.principal_cents, "loan amount")?,
            start_date: date::validate(&input.start_date, "start date")?,
            created_at: now,
            updated_at: now,
            payments: Vec::new(),
        };
        let mut loans = self.loans().to_vec();
        loans.push(loan.clone());
        self.commit(loans)?;
        Ok(loan)
    }

    pub fn update_loan(&mut self, id: &str, patch: LoanPatch) -> Result<Loan> {
        let name = patch.name.as_deref().map(clean_name).transpose()?;
        let lender = patch.lender.as_deref().map(|s| clean_person(s, "lender")).transpose()?;
        let borrower = patch.borrower.as_deref().map(|s| clean_person(s, "borrower")).transpose()?;
        let principal = patch.principal_cents.map(|c| check_cents(c, "loan amount")).transpose()?;
        let start = patch.start_date.as_deref().map(|d| date::validate(d, "start date")).transpose()?;
        self.modify_loan(id, |l| {
            if let Some(n) = name {
                l.name = n;
            }
            if let Some(n) = lender {
                l.lender = n;
            }
            if let Some(n) = borrower {
                l.borrower = n;
            }
            if let Some(p) = principal {
                l.principal_cents = p;
            }
            if let Some(s) = start {
                l.start_date = s;
            }
            Ok(())
        })?;
        Ok(self.loan(id)?.clone())
    }

    /// Remove a loan and all its payments. Returns what was removed.
    pub fn delete_loan(&mut self, id: &str) -> Result<Loan> {
        let removed = self.loan(id)?.clone();
        let loans = self.loans().iter().filter(|l| l.id != id).cloned().collect();
        self.commit(loans)?;
        Ok(removed)
    }

    pub fn add_payment(&mut self, loan_id: &str, input: PaymentInput) -> Result<Payment> {
        let now = self.now();
        let payment = Payment {
            id: match input.id {
                Some(i) if id::is_valid(&i) => i,
                Some(_) => return Err(Error::invalid("That payment ID isn't valid.")),
                None => id::new_id(),
            },
            amount_cents: check_cents(input.amount_cents, "payment amount")?,
            date: date::validate(&input.date, "payment date")?,
            method: input.method,
            note: clean_note(&input.note)?,
            created_at: input.created_at.unwrap_or(now),
            updated_at: now,
        };
        let p = payment.clone();
        self.modify_loan(loan_id, move |l| {
            if l.payments.iter().any(|q| q.id == p.id) {
                return Err(Error::conflict("That payment is already recorded."));
            }
            l.payments.push(p);
            Ok(())
        })?;
        Ok(payment)
    }

    pub fn update_payment(&mut self, loan_id: &str, payment_id: &str, patch: PaymentPatch) -> Result<Payment> {
        let amount = patch.amount_cents.map(|c| check_cents(c, "payment amount")).transpose()?;
        let day = patch.date.as_deref().map(|d| date::validate(d, "payment date")).transpose()?;
        let note = patch.note.as_deref().map(clean_note).transpose()?;
        let now = self.now();
        self.modify_loan(loan_id, |l| {
            let p = l
                .payments
                .iter_mut()
                .find(|p| p.id == payment_id)
                .ok_or_else(|| Error::not_found("That payment no longer exists."))?;
            if let Some(a) = amount {
                p.amount_cents = a;
            }
            if let Some(d) = day {
                p.date = d;
            }
            if let Some(m) = patch.method {
                p.method = m;
            }
            if let Some(n) = note {
                p.note = n;
            }
            p.updated_at = now;
            Ok(p.clone())
        })
    }

    /// Remove a payment. Returns it so the UI can offer undo.
    pub fn delete_payment(&mut self, loan_id: &str, payment_id: &str) -> Result<Payment> {
        self.modify_loan(loan_id, |l| {
            let i = l
                .payments
                .iter()
                .position(|p| p.id == payment_id)
                .ok_or_else(|| Error::not_found("That payment no longer exists."))?;
            Ok(l.payments.remove(i))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let mut s = Store::in_memory();
        s.set_now(Some(1_000));
        s
    }

    fn pay(cents: i64, date: &str) -> PaymentInput {
        PaymentInput {
            id: None,
            amount_cents: cents,
            date: date.into(),
            method: Method::Cash,
            note: "  birthday  ".into(),
            created_at: None,
        }
    }

    #[test]
    fn loan_lifecycle() {
        let mut s = store();
        let l = s
            .create_loan(LoanInput {
                name: " Car ".into(),
                lender: "Rob".into(),
                borrower: "Sam".into(),
                principal_cents: 500_000,
                start_date: "2026-01-15".into(),
            })
            .unwrap();
        assert_eq!(l.name, "Car");
        assert_eq!((l.lender.as_str(), l.borrower.as_str()), ("Rob", "Sam"));
        let renamed =
            s.update_loan(&l.id, LoanPatch { borrower: Some(" Samuel ".into()), ..Default::default() }).unwrap();
        assert_eq!(renamed.borrower, "Samuel");
        let missing = s.create_loan(LoanInput {
            name: "X".into(),
            lender: "Rob".into(),
            borrower: " ".into(),
            principal_cents: 100,
            start_date: "2026-01-01".into(),
        });
        assert_eq!(missing.unwrap_err().to_string(), "Enter who borrowed the money.");
        let p = s.add_payment(&l.id, pay(20_000, "2026-02-01")).unwrap();
        assert_eq!(p.note, "birthday");
        s.add_payment(&l.id, pay(30_000, "2026-03-01")).unwrap();
        assert_eq!(s.loan(&l.id).unwrap().balance_cents(), 450_000);

        s.update_payment(&l.id, &p.id, PaymentPatch { amount_cents: Some(25_000), ..Default::default() }).unwrap();
        assert_eq!(s.loan(&l.id).unwrap().balance_cents(), 445_000);

        let removed = s.delete_payment(&l.id, &p.id).unwrap();
        assert_eq!(s.loan(&l.id).unwrap().balance_cents(), 470_000);
        // Undo puts back the same payment.
        s.add_payment(
            &l.id,
            PaymentInput {
                id: Some(removed.id.clone()),
                amount_cents: removed.amount_cents,
                date: removed.date.clone(),
                method: removed.method,
                note: removed.note.clone(),
                created_at: Some(removed.created_at),
            },
        )
        .unwrap();
        assert_eq!(s.loan(&l.id).unwrap().payment(&removed.id).unwrap().amount_cents, 25_000);
        let dup = s.add_payment(&l.id, PaymentInput { id: Some(removed.id.clone()), ..pay(1, "2026-03-01") });
        assert!(matches!(dup, Err(Error::Conflict(_))));

        s.update_loan(&l.id, LoanPatch { principal_cents: Some(600_000), ..Default::default() }).unwrap();
        assert_eq!(s.loan(&l.id).unwrap().balance_cents(), 545_000);
        s.delete_loan(&l.id).unwrap();
        assert!(s.loans().is_empty());
    }

    #[test]
    fn overpaying_is_allowed_and_bad_input_changes_nothing() {
        let mut s = store();
        let l = s
            .create_loan(LoanInput {
                name: "Small".into(),
                lender: "Rob".into(),
                borrower: "Sam".into(),
                principal_cents: 1_000,
                start_date: "2026-01-01".into(),
            })
            .unwrap();
        s.add_payment(&l.id, pay(1_500, "2026-01-02")).unwrap();
        assert_eq!(s.loan(&l.id).unwrap().balance_cents(), -500);

        let before = s.loans().to_vec();
        assert!(s.add_payment(&l.id, pay(0, "2026-01-02")).is_err());
        assert!(s.add_payment(&l.id, pay(100, "2026-02-30")).is_err());
        assert!(s.update_loan(&l.id, LoanPatch { name: Some("  ".into()), ..Default::default() }).is_err());
        assert!(s.update_loan(&l.id, LoanPatch { borrower: Some("".into()), ..Default::default() }).is_err());
        assert!(s.add_payment("nope", pay(100, "2026-01-02")).is_err());
        assert_eq!(s.loans(), &before[..]);
    }
}
