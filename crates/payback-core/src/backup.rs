//! Backups: timestamped copies of the data file, an automatic daily copy
//! with rotation, and restore.

use crate::date;
use crate::error::{Error, Result};
use crate::store::{data_file_json, parse_loans, read_text, write_atomic, Store};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const PREFIX: &str = "payback-";
const SUFFIX: &str = ".json";

/// UTC, so names sort correctly whatever the time zone.
fn stamp(ts: i64) -> String {
    let (y, mo, d, h, mi, s) = date::utc_parts(ts);
    format!("{y}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Backup files in `dir`, oldest first.
pub fn list_backups(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(PREFIX) && n.ends_with(SUFFIX) && !n.contains("before-restore"))
                .unwrap_or(false)
        })
        .collect();
    out.sort();
    Ok(out)
}

impl Store {
    fn write_backup(&self, dir: &Path, label: &str) -> Result<PathBuf> {
        let base = format!("{PREFIX}{label}{}", stamp(self.now()));
        let mut path = dir.join(format!("{base}{SUFFIX}"));
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("{base}-{n}{SUFFIX}"));
            n += 1;
        }
        write_atomic(&path, data_file_json(self.loans()).stringify().as_bytes())?;
        Ok(path)
    }

    /// Save a timestamped copy of all data in `dir`.
    pub fn backup_now(&self, dir: &Path) -> Result<PathBuf> {
        self.write_backup(dir, "")
    }

    /// Back up if the newest backup is older than `min_age_secs`, then keep
    /// only the newest `keep` files. Returns the new backup's path, if any.
    pub fn auto_backup(&self, dir: &Path, keep: usize, min_age_secs: u64) -> Result<Option<PathBuf>> {
        if self.loans().is_empty() {
            return Ok(None);
        }
        let existing = list_backups(dir)?;
        let newest_age = existing
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok())
            .filter_map(|m| SystemTime::now().duration_since(m).ok())
            .map(|d| d.as_secs())
            .min();
        if matches!(newest_age, Some(age) if age < min_age_secs) {
            return Ok(None);
        }
        let path = self.backup_now(dir)?;
        let all = list_backups(dir)?;
        if all.len() > keep {
            for old in &all[..all.len() - keep] {
                let _ = std::fs::remove_file(old);
            }
        }
        Ok(Some(path))
    }

    /// Replace all data with a backup (or any Payback data or export file).
    /// The current data is saved to `safety_dir` first, so a restore can
    /// itself be undone. Returns the safety copy's path.
    pub fn restore_from(&mut self, backup: &Path, safety_dir: &Path) -> Result<PathBuf> {
        let loans = parse_loans(&read_text(backup)?).map_err(|e| match e {
            Error::Io(_) => Error::invalid("Payback couldn't read that file."),
            other => other,
        })?;
        let safety = self.write_backup(safety_dir, "before-restore-")?;
        self.commit(loans)?;
        Ok(safety)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::LoanInput;

    fn temp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("payback-test-backup-{}", crate::id::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn auto_backup_rotates_and_restore_round_trips() {
        let dir = temp_dir();
        let backups = dir.join("backups");
        let mut s = Store::open(&dir.join("payback.json")).unwrap();
        // Nothing to back up yet.
        assert_eq!(s.auto_backup(&backups, 3, 0).unwrap(), None);
        s.create_loan(LoanInput {
            name: "A".into(),
            lender: "Rob".into(),
            borrower: "Sam".into(),
            principal_cents: 100,
            start_date: "2026-01-01".into(),
        })
        .unwrap();
        for _ in 0..5 {
            assert!(s.auto_backup(&backups, 3, 0).unwrap().is_some());
        }
        assert_eq!(list_backups(&backups).unwrap().len(), 3);
        // A recent backup exists, so no new one.
        assert_eq!(s.auto_backup(&backups, 3, 3600).unwrap(), None);

        let snapshot = s.backup_now(&backups).unwrap();
        s.create_loan(LoanInput {
            name: "B".into(),
            lender: "Rob".into(),
            borrower: "Sam".into(),
            principal_cents: 100,
            start_date: "2026-01-01".into(),
        })
        .unwrap();
        assert_eq!(s.loans().len(), 2);
        let safety = s.restore_from(&snapshot, &backups).unwrap();
        assert_eq!(s.loans().len(), 1);
        assert!(safety.file_name().unwrap().to_str().unwrap().contains("before-restore"));
        // The restore was saved to disk.
        assert_eq!(Store::open(&dir.join("payback.json")).unwrap().loans().len(), 1);

        std::fs::write(dir.join("junk.json"), "junk").unwrap();
        assert!(s.restore_from(&dir.join("junk.json"), &backups).is_err());
        assert_eq!(s.loans().len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
