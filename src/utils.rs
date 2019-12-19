use std::time::SystemTime;

use crypto::digest::Digest;
use crypto::sha2::Sha256;

pub(crate) fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.input_str(s);
    hasher.result_str()
}

pub(crate) fn now() -> f64 {
    // Safe to unwrap unless the system time is seriously screwed up.
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
