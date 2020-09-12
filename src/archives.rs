use crate::error::Error;
use std::path::Path;

/// Supported archive types.
pub(crate) enum ArchiveFormat {
    TarGz,
}

impl ArchiveFormat {
    /// Parse archive type from resource extension.
    pub(crate) fn parse_from_extension(resource: &str) -> Result<Self, Error> {
        if resource.ends_with(".tar.gz") {
            return Ok(Self::TarGz);
        }
        return Err(Error::ExtractionError("unsupported archive format".into()));
    }
}

pub(crate) fn extract_archive<P: AsRef<Path>>(
    path: P,
    target: P,
    format: &ArchiveFormat,
) -> Result<(), Error> {
    // TODO: extract to temp directory in same parent directory of `target`, then rename
    // to target if successful.
    unimplemented!();
}
