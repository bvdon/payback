//! The data file. Everything lives in one small JSON file that is rewritten
//! atomically on every change: write a temp file, flush it to disk, then
//! rename it over the original. A crash leaves either the old or the new
//! file, never half of one.

use crate::date;
use crate::error::{Error, Result};
use crate::json::{Json, ToJson};
use crate::interest::Interest;
use crate::model::Loan;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Bump when the file's shape changes, and keep reading older versions.
/// Version 2 added interest. Files with no interest-bearing loans are still
/// written as version 1 so that Payback 0.1.3 and older can open them; a
/// file with interest is version 2, which those versions refuse rather than
/// showing the wrong balance.
pub const FORMAT_VERSION: i64 = 2;
pub const DATA_FORMAT: &str = "payback-data";
pub const EXPORT_FORMAT: &str = "payback-export";
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub struct Store {
    path: Option<PathBuf>,
    loans: Vec<Loan>,
    fixed_now: Option<i64>,
}

/// Read loans from a data file or an exported `.payback` file.
pub fn parse_loans(text: &str) -> Result<Vec<Loan>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let not_ours = || Error::invalid("That file isn't a Payback file.");
    let v = Json::parse(text).map_err(|_| not_ours())?;
    let format = v.get("format").and_then(|f| f.as_str()).ok_or_else(not_ours)?;
    if format != DATA_FORMAT && format != EXPORT_FORMAT {
        return Err(not_ours());
    }
    let version = v.get("formatVersion").and_then(|n| n.as_i64()).ok_or_else(not_ours)?;
    if version > FORMAT_VERSION {
        return Err(Error::invalid(
            "That file was made by a newer version of Payback. Update Payback on this computer to open it.",
        ));
    }
    let mut loans: Vec<Loan> = Vec::new();
    for l in v.get("loans").and_then(|l| l.as_array()).ok_or_else(not_ours)? {
        let loan = Loan::from_json(l)?;
        if loans.iter().any(|x| x.id == loan.id) {
            return Err(Error::invalid("That file lists the same loan twice."));
        }
        loans.push(loan);
    }
    Ok(loans)
}

pub fn read_text(path: &Path) -> Result<String> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(Error::invalid("That file is too large to be a Payback file."));
    }
    let bytes = std::fs::read(path)?;
    String::from_utf8(bytes).map_err(|_| Error::invalid("That file isn't a Payback file."))
}

/// Write `contents` to `path` without ever leaving a partial file behind.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp_name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    // On Windows, std's rename replaces an existing file too.
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// The oldest format version that can hold `loans`.
pub fn format_version_for<'a>(loans: impl IntoIterator<Item = &'a Loan>) -> i64 {
    if loans.into_iter().any(|l| l.interest != Interest::None) {
        2
    } else {
        1
    }
}

pub fn data_file_json(loans: &[Loan]) -> Json {
    Json::obj()
        .with("format", DATA_FORMAT)
        .with("formatVersion", format_version_for(loans))
        .with("loans", loans.to_json())
}

impl Store {
    /// Open the data file, or start empty if it doesn't exist yet. A file that
    /// exists but can't be read is an error: it is never silently replaced.
    pub fn open(path: &Path) -> Result<Store> {
        let loans = if path.exists() {
            parse_loans(&read_text(path)?).map_err(|e| {
                Error::invalid(format!("Payback couldn't read its data file at {}. {e}", path.display()))
            })?
        } else {
            Vec::new()
        };
        Ok(Store { path: Some(path.to_path_buf()), loans, fixed_now: None })
    }

    pub fn in_memory() -> Store {
        Store { path: None, loans: Vec::new(), fixed_now: None }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Freeze the clock (tests and demos).
    pub fn set_now(&mut self, t: Option<i64>) {
        self.fixed_now = t;
    }

    pub fn now(&self) -> i64 {
        self.fixed_now.unwrap_or_else(date::now_unix)
    }

    /// Loans in the order they were created.
    pub fn loans(&self) -> &[Loan] {
        &self.loans
    }

    pub fn loan(&self, id: &str) -> Result<&Loan> {
        self.loans.iter().find(|l| l.id == id).ok_or_else(|| Error::not_found("That loan no longer exists."))
    }

    /// Save `loans` to disk, then make them current. If saving fails, nothing
    /// changes in memory either.
    pub fn commit(&mut self, mut loans: Vec<Loan>) -> Result<()> {
        loans.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        if let Some(path) = &self.path {
            write_atomic(path, data_file_json(&loans).stringify().as_bytes())?;
        }
        self.loans = loans;
        Ok(())
    }

    /// Apply `f` to a copy of the loan with `id` and commit the result.
    pub fn modify_loan<T>(&mut self, id: &str, f: impl FnOnce(&mut Loan) -> Result<T>) -> Result<T> {
        let mut loans = self.loans.clone();
        let loan =
            loans.iter_mut().find(|l| l.id == id).ok_or_else(|| Error::not_found("That loan no longer exists."))?;
        let out = f(loan)?;
        loan.updated_at = self.now();
        self.commit(loans)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{loan, payment};

    pub fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("payback-test-{name}-{}", crate::id::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn saves_and_reopens() {
        let dir = temp_dir("store");
        let path = dir.join("payback.json");
        let mut s = Store::open(&path).unwrap();
        assert!(s.loans().is_empty());
        s.commit(vec![loan(vec![payment("p", 100, "2026-02-01", 1)])]).unwrap();
        assert!(!dir.join("payback.json.tmp").exists());
        let again = Store::open(&path).unwrap();
        assert_eq!(again.loans(), s.loans());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_to_open_damaged_or_foreign_files() {
        let dir = temp_dir("damaged");
        let path = dir.join("payback.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(Store::open(&path).is_err());
        // Still there: a bad file is never replaced.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not json");
        assert!(parse_loans(r#"{"format":"other","formatVersion":1,"loans":[]}"#).is_err());
        let newer = parse_loans(r#"{"format":"payback-data","formatVersion":99,"loans":[]}"#).unwrap_err();
        assert!(newer.to_string().contains("newer version"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
