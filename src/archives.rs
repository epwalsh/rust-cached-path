use crate::error::Error;
use std::path::Path;

pub(crate) fn is_archive(resource: &str) -> bool {
    unimplemented!();
}

pub(crate) fn extract_archive<P: AsRef<Path>>(path: P, target: P) -> Result<(), Error> {
    // TODO: extract to temp directory in same parent directory of `target`, then rename
    // to target if successful.
    unimplemented!();
}
