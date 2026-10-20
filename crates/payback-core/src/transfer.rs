//! Sharing between computers. One person exports loans to a `.payback`
//! file; the other imports it. Loans are matched by their permanent ID: an
//! incoming loan replaces the local copy of the same loan (after a preview
//! shows exactly what changes), and an unknown loan is added.

use crate::error::{Error, Result};
use crate::json::{Json, ToJson};
use crate::model::{Loan, Payment};
use crate::store::{format_version_for, parse_loans, Store, EXPORT_FORMAT};

/// The contents of a `.payback` file holding `loans`.
pub fn export_text(loans: &[&Loan], exported_at: i64, app_version: &str) -> String {
    Json::obj()
        .with("format", EXPORT_FORMAT)
        .with("formatVersion", format_version_for(loans.iter().copied()))
        .with("exportedAt", exported_at)
        .with("appVersion", app_version)
        .with("loans", loans.to_json())
        .stringify()
}

/// What importing one loan would do.
#[derive(Debug, PartialEq)]
pub struct LoanChange {
    pub id: String,
    pub name: String,
    /// "new", "update" or "same".
    pub status: &'static str,
    pub previous_name: Option<String>,
    pub details_changed: bool,
    pub payments_added: usize,
    pub payments_removed: usize,
    pub payments_edited: usize,
    pub balance_before_cents: Option<i64>,
    pub balance_after_cents: i64,
}

crate::impl_to_json!(LoanChange {
    id => "id",
    name => "name",
    status => "status",
    previous_name => "previousName",
    details_changed => "detailsChanged",
    payments_added => "paymentsAdded",
    payments_removed => "paymentsRemoved",
    payments_edited => "paymentsEdited",
    balance_before_cents => "balanceBeforeCents",
    balance_after_cents => "balanceAfterCents",
});

fn same_payment(a: &Payment, b: &Payment) -> bool {
    a.amount_cents == b.amount_cents && a.date == b.date && a.method == b.method && a.note == b.note
}

fn compare(local: Option<&Loan>, incoming: &Loan, today: &str) -> LoanChange {
    let mut c = LoanChange {
        id: incoming.id.clone(),
        name: incoming.name.clone(),
        status: "new",
        previous_name: None,
        details_changed: false,
        payments_added: incoming.payments.len(),
        payments_removed: 0,
        payments_edited: 0,
        balance_before_cents: None,
        balance_after_cents: incoming.balance_cents(today),
    };
    let Some(local) = local else { return c };
    c.balance_before_cents = Some(local.balance_cents(today));
    if local.name != incoming.name {
        c.previous_name = Some(local.name.clone());
    }
    c.details_changed = local.name != incoming.name
        || local.lender != incoming.lender
        || local.borrower != incoming.borrower
        || local.principal_cents != incoming.principal_cents
        || local.start_date != incoming.start_date
        || local.interest != incoming.interest;
    c.payments_added = incoming.payments.iter().filter(|p| local.payment(&p.id).is_err()).count();
    c.payments_removed = local.payments.iter().filter(|p| incoming.payment(&p.id).is_err()).count();
    c.payments_edited =
        incoming.payments.iter().filter(|p| local.payment(&p.id).map(|q| !same_payment(p, q)).unwrap_or(false)).count();
    let changed = c.details_changed || c.payments_added + c.payments_removed + c.payments_edited > 0;
    c.status = if changed { "update" } else { "same" };
    c
}

/// What importing `text` would change, with balances as of `today`.
pub fn preview(store: &Store, text: &str, today: &str) -> Result<Vec<LoanChange>> {
    let incoming = parse_loans(text)?;
    if incoming.is_empty() {
        return Err(Error::invalid("That file doesn't contain any loans."));
    }
    Ok(incoming.iter().map(|l| compare(store.loan(&l.id).ok(), l, today)).collect())
}

/// Import the loans in `text` whose IDs are in `only` (all when `None`).
/// Returns (added, updated).
pub fn apply(store: &mut Store, text: &str, only: Option<&[String]>) -> Result<(usize, usize)> {
    let incoming = parse_loans(text)?;
    let mut loans = store.loans().to_vec();
    let (mut added, mut updated) = (0, 0);
    for l in incoming {
        if only.is_some_and(|ids| !ids.contains(&l.id)) {
            continue;
        }
        match loans.iter_mut().find(|x| x.id == l.id) {
            Some(existing) => {
                // Only the status is used here, which doesn't depend on the date.
                if compare(Some(existing), &l, "").status != "same" {
                    *existing = l;
                    updated += 1;
                }
            }
            None => {
                loans.push(l);
                added += 1;
            }
        }
    }
    if added + updated > 0 {
        store.commit(loans)?;
    }
    Ok((added, updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interest::Interest;
    use crate::ledger::{LoanInput, PaymentInput, PaymentPatch};
    use crate::model::Method;

    const T: &str = "2026-10-19";

    fn pay(cents: i64) -> PaymentInput {
        PaymentInput {
            id: None,
            amount_cents: cents,
            date: "2026-05-01".into(),
            method: Method::Check,
            note: String::new(),
            created_at: None,
        }
    }

    #[test]
    fn roundtrip_between_two_computers() {
        // Older brother creates the loan and two payments, then exports.
        let mut older = Store::in_memory();
        let loan = older
            .create_loan(LoanInput {
                name: "Truck".into(),
                lender: "Rob".into(),
                borrower: "Sam".into(),
                principal_cents: 300_000,
                start_date: "2026-04-01".into(),
                interest: Interest::None,
            })
            .unwrap();
        let first = older.add_payment(&loan.id, pay(10_000)).unwrap();
        older.add_payment(&loan.id, pay(20_000)).unwrap();
        let file = export_text(&[older.loan(&loan.id).unwrap()], 1, "0.1.0");

        // Younger brother imports: the loan is new to him.
        let mut younger = Store::in_memory();
        let p = preview(&younger, &file, T).unwrap();
        assert_eq!(p[0].status, "new");
        assert_eq!(p[0].payments_added, 2);
        assert_eq!(apply(&mut younger, &file, None).unwrap(), (1, 0));
        assert_eq!(younger.loan(&loan.id).unwrap().balance_cents(T), 270_000);

        // Importing the same file again changes nothing.
        assert_eq!(preview(&younger, &file, T).unwrap()[0].status, "same");
        assert_eq!(apply(&mut younger, &file, None).unwrap(), (0, 0));

        // Older brother edits one payment, deletes nothing, adds one; younger
        // brother had recorded one of his own that the file doesn't have.
        older
            .update_payment(&loan.id, &first.id, PaymentPatch { amount_cents: Some(15_000), ..Default::default() })
            .unwrap();
        older.add_payment(&loan.id, pay(5_000)).unwrap();
        younger.add_payment(&loan.id, pay(1_000)).unwrap();
        let file2 = export_text(&[older.loan(&loan.id).unwrap()], 2, "0.1.0");
        let c = &preview(&younger, &file2, T).unwrap()[0];
        assert_eq!(c.status, "update");
        assert_eq!((c.payments_added, c.payments_removed, c.payments_edited), (1, 1, 1));
        assert_eq!(c.balance_before_cents, Some(269_000));
        assert_eq!(c.balance_after_cents, 260_000);
        assert_eq!(apply(&mut younger, &file2, None).unwrap(), (0, 1));
        assert_eq!(younger.loan(&loan.id).unwrap(), older.loan(&loan.id).unwrap());
    }

    #[test]
    fn selective_and_bad_imports() {
        let mut a = Store::in_memory();
        let l1 = a
            .create_loan(LoanInput {
                name: "One".into(),
                lender: "Rob".into(),
                borrower: "Sam".into(),
                principal_cents: 100,
                start_date: "2026-01-01".into(),
                interest: Interest::None,
            })
            .unwrap();
        let l2 = a
            .create_loan(LoanInput {
                name: "Two".into(),
                lender: "Rob".into(),
                borrower: "Sam".into(),
                principal_cents: 200,
                start_date: "2026-01-01".into(),
                interest: Interest::None,
            })
            .unwrap();
        let file = export_text(&a.loans().iter().collect::<Vec<_>>(), 1, "0.1.0");
        let mut b = Store::in_memory();
        assert_eq!(apply(&mut b, &file, Some(std::slice::from_ref(&l2.id))).unwrap(), (1, 0));
        assert!(b.loan(&l1.id).is_err());
        assert!(preview(&b, "hello", T).is_err());
        assert!(preview(&b, r#"{"format":"payback-export","formatVersion":1,"loans":[]}"#, T).is_err());
    }
}
