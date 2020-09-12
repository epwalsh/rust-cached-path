use crate::error::Error;
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::Path;
use tar::Archive;

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
        Err(Error::ExtractionError("unsupported archive format".into()))
    }
}

pub(crate) fn extract_archive<P: AsRef<Path>>(
    path: P,
    target: P,
    format: &ArchiveFormat,
) -> Result<(), Error> {
    match format {
        ArchiveFormat::TarGz => {
            let tar_gz = File::open(path)?;
            let tar = GzDecoder::new(tar_gz);
            let mut archive = Archive::new(tar);
            archive.unpack(target)?;
            Ok(())
        }
    }
}
