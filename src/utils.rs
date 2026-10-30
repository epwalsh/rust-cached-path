use std::fmt::Write;

use sha2::{Digest, Sha256};
use std::time::SystemTime;

pub(crate) fn hash_str(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut buf, byte| {
            let _ = write!(buf, "{byte:02x}");
            buf
        })
}

pub(crate) fn now() -> f64 {
    // Safe to unwrap unless the system time is seriously screwed up.
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_str() {
        assert_eq!(
            hash_str("hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
