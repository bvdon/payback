//! Checking for a newer release. The desktop shell fetches GitHub's "latest
//! release" JSON; this module reads it and compares versions, so the logic
//! stays testable and free of networking.

use crate::error::{Error, Result};
use crate::json::Json;

/// What a check for updates found.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    /// This version is the newest (or newer, like a development build).
    UpToDate,
    /// A newer version is out, e.g. "0.1.6".
    Available(String),
}

/// "0.1.4", "v0.1.4" or "0.2" as (major, minor, patch). Anything after a
/// `-` or `+` (like "1.0.0-beta") is ignored.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim();
    let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let mut next = |required: bool| match parts.next() {
        Some(p) => p.parse::<u64>().ok(),
        None if !required => Some(0),
        None => None,
    };
    let v = (next(true)?, next(false)?, next(false)?);
    parts.next().is_none().then_some(v)
}

/// The version of the latest release, from GitHub's
/// `/repos/{owner}/{repo}/releases/latest` response.
pub fn latest_version(release_json: &str) -> Result<String> {
    let bad = || Error::invalid("GitHub's answer didn't include a version number.");
    let v = Json::parse(release_json).map_err(|_| bad())?;
    let tag = v.get("tag_name").and_then(|t| t.as_str()).ok_or_else(bad)?;
    parse_version(tag).ok_or_else(bad)?;
    Ok(tag.trim().trim_start_matches(['v', 'V']).to_string())
}

/// Compare the running version with the latest release.
pub fn compare(current: &str, latest: &str) -> Result<UpdateStatus> {
    let cur = parse_version(current).ok_or_else(|| Error::invalid("This copy of Payback has an unreadable version."))?;
    let new = parse_version(latest).ok_or_else(|| Error::invalid("GitHub's answer didn't include a version number."))?;
    Ok(if new > cur { UpdateStatus::Available(latest.to_string()) } else { UpdateStatus::UpToDate })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("0.1.4"), Some((0, 1, 4)));
        assert_eq!(parse_version(" v1.2.3 "), Some((1, 2, 3)));
        assert_eq!(parse_version("0.2"), Some((0, 2, 0)));
        assert_eq!(parse_version("1.0.0-beta.1"), Some((1, 0, 0)));
        for bad in ["", "v", "1.x", "1.2.3.4", "latest"] {
            assert_eq!(parse_version(bad), None, "{bad}");
        }
    }

    #[test]
    fn compares_with_latest_release() {
        let json = r#"{"tag_name":"v0.1.10","name":"Payback v0.1.10","draft":false}"#;
        let latest = latest_version(json).unwrap();
        assert_eq!(latest, "0.1.10");
        // Numeric, not alphabetical: 0.1.10 is newer than 0.1.9.
        assert_eq!(compare("0.1.9", &latest).unwrap(), UpdateStatus::Available("0.1.10".into()));
        assert_eq!(compare("0.1.10", &latest).unwrap(), UpdateStatus::UpToDate);
        assert_eq!(compare("0.2.0", &latest).unwrap(), UpdateStatus::UpToDate);
        assert!(latest_version(r#"{"message":"Not Found"}"#).is_err());
        assert!(latest_version("<html>").is_err());
    }
}
