//! The command interface the UI talks to. Every command takes a JSON object
//! and returns JSON; the desktop shell and the dev server both route here,
//! so the app behaves identically in either.

use crate::error::{Error, Result};
use crate::json::{Args, Json, ToJson};
use crate::ledger::{LoanInput, LoanPatch, PaymentInput, PaymentPatch};
use crate::model::{Method, Payment};
use crate::money::parse_amount;
use crate::store::Store;
use crate::{date, statement, transfer};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Export {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub mime: &'static str,
}

pub struct Api {
    store: Store,
}

/// Serialize an error for the UI.
pub fn error_json(e: &Error) -> String {
    Json::obj().with("kind", e.kind()).with("message", e.to_string()).stringify()
}

/// An amount given either as text the person typed (`amount`) or as exact
/// cents (`amountCents`, used when undoing a delete).
fn amount(a: Args, what: &str) -> Result<Option<i64>> {
    if let Some(c) = a.opt_i64("amountCents")? {
        return Ok(Some(c));
    }
    a.opt_str("amount")?.map(|s| parse_amount(s, what)).transpose()
}

fn method(a: Args) -> Result<Option<Method>> {
    a.opt_str("method")?.map(Method::parse).transpose()
}

fn text(a: Args, key: &str) -> Result<Option<String>> {
    Ok(a.opt_str(key)?.map(|s| s.to_string()))
}

fn ids(a: Args, key: &str) -> Result<Option<Vec<String>>> {
    if !a.has(key) || a.raw().get(key).is_some_and(|v| v.is_null()) {
        return Ok(None);
    }
    a.array(key)?
        .iter()
        .map(|v| v.as_str().map(|s| s.to_string()).ok_or_else(|| Error::Json(format!("'{key}' must be a list of IDs"))))
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

/// "Today" comes from the UI (the person's own calendar); fall back to UTC.
fn today(a: Args) -> Result<String> {
    match a.opt_str("today")? {
        Some(d) => date::validate(d, "date"),
        None => {
            let (y, m, d, ..) = date::utc_parts(date::now_unix());
            Ok(format!("{y:04}-{m:02}-{d:02}"))
        }
    }
}

impl Api {
    pub fn new(store: Store) -> Api {
        Api { store }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Text in, text out: the shape both transports use. Errors come back as
    /// `Err(json)` with `kind` and `message`.
    pub fn call(&mut self, cmd: &str, args: &str) -> std::result::Result<String, String> {
        let parsed = if args.trim().is_empty() { Ok(Json::obj()) } else { Json::parse(args) };
        parsed.and_then(|a| self.call_json(cmd, &a)).map(|v| v.stringify()).map_err(|e| error_json(&e))
    }

    pub fn call_json(&mut self, cmd: &str, args: &Json) -> Result<Json> {
        let a = Args::new(args);
        let s = &mut self.store;
        Ok(match cmd {
            "app.bootstrap" => Json::obj()
                .with("version", VERSION)
                .with("loans", Json::Arr(s.loans().iter().map(|l| l.view()).collect())),

            "loans.create" => {
                let principal = amount(a, "loan amount")?.ok_or_else(|| Error::invalid("Enter the loan amount."))?;
                let loan = s.create_loan(LoanInput {
                    name: a.opt_str("name")?.unwrap_or("").to_string(),
                    lender: a.opt_str("lender")?.unwrap_or("").to_string(),
                    borrower: a.opt_str("borrower")?.unwrap_or("").to_string(),
                    principal_cents: principal,
                    start_date: a.opt_str("startDate")?.unwrap_or("").to_string(),
                })?;
                loan.view()
            }
            "loans.update" => s
                .update_loan(
                    a.req_str("id")?,
                    LoanPatch {
                        name: text(a, "name")?,
                        lender: text(a, "lender")?,
                        borrower: text(a, "borrower")?,
                        principal_cents: amount(a, "loan amount")?,
                        start_date: text(a, "startDate")?,
                    },
                )?
                .view(),
            "loans.delete" => s.delete_loan(a.req_str("id")?)?.to_json(),

            "payments.add" => {
                let cents = amount(a, "payment amount")?.ok_or_else(|| Error::invalid("Enter the payment amount."))?;
                s.add_payment(
                    a.req_str("loanId")?,
                    PaymentInput {
                        id: text(a, "id")?,
                        amount_cents: cents,
                        date: a.opt_str("date")?.unwrap_or("").to_string(),
                        method: method(a)?.ok_or_else(|| Error::invalid("Pick a payment type."))?,
                        note: a.opt_str("note")?.unwrap_or("").to_string(),
                        created_at: a.opt_i64("createdAt")?,
                    },
                )?
                .to_json()
            }
            "payments.update" => s
                .update_payment(
                    a.req_str("loanId")?,
                    a.req_str("id")?,
                    PaymentPatch {
                        amount_cents: amount(a, "payment amount")?,
                        date: text(a, "date")?,
                        method: method(a)?,
                        note: text(a, "note")?,
                    },
                )?
                .to_json(),
            "payments.delete" => {
                let p: Payment = s.delete_payment(a.req_str("loanId")?, a.req_str("id")?)?;
                p.to_json()
            }

            "import.preview" => {
                let changes = transfer::preview(s, a.req_str("text")?)?;
                Json::obj().with("loans", changes.to_json())
            }
            "import.apply" => {
                let only = ids(a, "loanIds")?;
                let (added, updated) = transfer::apply(s, a.req_str("text")?, only.as_deref())?;
                Json::obj().with("added", added).with("updated", updated)
            }

            _ => return Err(Error::invalid(format!("Unknown command '{cmd}'"))),
        })
    }

    /// Commands that produce a file to save.
    pub fn export(&self, cmd: &str, args: &Json) -> Result<Export> {
        let a = Args::new(args);
        let s = &self.store;
        let today = today(a)?;
        Ok(match cmd {
            "export.statementPdf" | "export.statementCsv" => {
                let loan = s.loan(a.req_str("loanId")?)?;
                let stem = statement::file_stem(loan, &today);
                if cmd == "export.statementPdf" {
                    Export {
                        bytes: statement::pdf(loan, &today),
                        file_name: format!("{stem}.pdf"),
                        mime: "application/pdf",
                    }
                } else {
                    Export {
                        bytes: statement::csv(loan).into_bytes(),
                        file_name: format!("{stem}.csv"),
                        mime: "text/csv; charset=utf-8",
                    }
                }
            }
            "export.data" => {
                let loans: Vec<_> = match ids(a, "loanIds")? {
                    Some(ids) => ids.iter().map(|id| s.loan(id)).collect::<Result<_>>()?,
                    None => s.loans().iter().collect(),
                };
                if loans.is_empty() {
                    return Err(Error::invalid("There are no loans to export yet."));
                }
                let file_name = match loans.as_slice() {
                    [one] => format!("{}.payback", statement::file_stem(one, &today)),
                    _ => format!("payback-loans-{today}.payback"),
                };
                Export {
                    bytes: transfer::export_text(&loans, s.now(), VERSION).into_bytes(),
                    file_name,
                    mime: "application/json",
                }
            }
            _ => return Err(Error::invalid(format!("Unknown export '{cmd}'"))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(api: &mut Api, cmd: &str, args: &str) -> Json {
        Json::parse(&api.call(cmd, args).unwrap()).unwrap()
    }

    #[test]
    fn end_to_end_through_json() {
        let mut api = Api::new(Store::in_memory());
        let loan = call(
            &mut api,
            "loans.create",
            r#"{"name":"Car","lender":"Rob","borrower":"Sam","amount":"$5,000","startDate":"2026-01-15"}"#,
        );
        let id = loan.get("id").unwrap().as_str().unwrap().to_string();
        assert_eq!(loan.get("balanceCents").unwrap().as_i64(), Some(500_000));

        let args =
            format!(r#"{{"loanId":"{id}","amount":"250.50","date":"2026-02-01","method":"Venmo","note":"Feb"}}"#);
        let p = call(&mut api, "payments.add", &args);
        let pid = p.get("id").unwrap().as_str().unwrap().to_string();
        let b = call(&mut api, "app.bootstrap", "");
        let l = &b.get("loans").unwrap().as_array().unwrap()[0];
        assert_eq!(l.get("balanceCents").unwrap().as_i64(), Some(474_950));

        let err = api.call(
            "payments.add",
            &format!(r#"{{"loanId":"{id}","amount":"abc","date":"2026-02-01","method":"cash"}}"#),
        );
        let err = Json::parse(&err.unwrap_err()).unwrap();
        assert_eq!(err.get("kind").unwrap().as_str(), Some("invalid"));
        assert!(err
            .get("message")
            .unwrap()
            .as_str()
            .unwrap()
            .starts_with("Enter the payment amount as a dollar amount"));

        call(&mut api, "payments.update", &format!(r#"{{"loanId":"{id}","id":"{pid}","amount":"300"}}"#));
        let deleted = call(&mut api, "payments.delete", &format!(r#"{{"loanId":"{id}","id":"{pid}"}}"#));
        assert_eq!(deleted.get("amountCents").unwrap().as_i64(), Some(30_000));

        let x = api.export("export.data", &Json::parse(r#"{"today":"2026-10-19"}"#).unwrap()).unwrap();
        assert_eq!(x.file_name, "car-2026-10-19.payback");
        let text = String::from_utf8(x.bytes).unwrap();
        let mut other = Api::new(Store::in_memory());
        let prev = call(&mut other, "import.preview", &Json::obj().with("text", text.as_str()).stringify());
        assert_eq!(prev.get("loans").unwrap().as_array().unwrap()[0].get("status").unwrap().as_str(), Some("new"));
        let applied = call(&mut other, "import.apply", &Json::obj().with("text", text.as_str()).stringify());
        assert_eq!(applied.get("added").unwrap().as_i64(), Some(1));

        let pdf = api
            .export(
                "export.statementPdf",
                &Json::parse(&format!(r#"{{"loanId":"{id}","today":"2026-10-19"}}"#)).unwrap(),
            )
            .unwrap();
        assert_eq!(pdf.file_name, "car-2026-10-19.pdf");
        assert!(api.call("nope.nope", "{}").is_err());
    }
}
