//! Permanent random IDs for loans and payments. They let a data file
//! exported on one computer be matched up with the same loan on another.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn random_u64() -> u64 {
    // RandomState is seeded from the OS's random source; mixing in the clock
    // and a counter keeps IDs unique even if two hashers share a seed.
    let mut h = RandomState::new().build_hasher();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    h.write_u128(nanos);
    h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.finish()
}

/// 128 random bits as 32 lowercase hex characters.
pub fn new_id() -> String {
    format!("{:016x}{:016x}", random_u64(), random_u64())
}

/// IDs from files may have been written by another version; accept anything
/// short and plain.
pub fn is_valid(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_hex() {
        let a = new_id();
        let b = new_id();
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(is_valid(&a));
        assert!(!is_valid(""));
        assert!(!is_valid("has space"));
    }
}
