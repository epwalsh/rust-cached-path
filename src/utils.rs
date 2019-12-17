use std::path::PathBuf;

use crypto::digest::Digest;
use crypto::sha2::Sha256;

pub(crate) fn meta_path(resource_path: &PathBuf) -> PathBuf {
    let mut meta_path = resource_path.clone();
    let resource_file_name = meta_path.file_name().unwrap().to_str().unwrap();
    let meta_file_name = format!("{}.meta", resource_file_name);
    meta_path.set_file_name(&meta_file_name[..]);
    meta_path
}

pub(crate) fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.input_str(s);
    hasher.result_str()
}
