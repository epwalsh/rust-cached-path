use crate::error::Error;
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::path::Path;
use tempfile::tempdir_in;
use zip_extensions::read::zip_extract;

/// Supported archive types.
pub(crate) enum ArchiveFormat {
    TarGz,
    Zip,
}

impl ArchiveFormat {
    /// Parse archive type from resource extension.
    pub(crate) fn parse_from_extension(resource: &str) -> Result<Self, Error> {
        if resource.ends_with(".tar.gz") {
            Ok(Self::TarGz)
        } else if resource.ends_with(".zip") {
            Ok(Self::Zip)
        } else {
            Err(Error::ExtractionError("unsupported archive format".into()))
        }
    }
}

pub(crate) fn extract_archive<P: AsRef<Path>>(
    path: P,
    target: P,
    format: &ArchiveFormat,
) -> Result<(), Error> {
    // We'll first extract to a temp directory in the same parent as the target directory.
    let target_parent_dir = target.as_ref().parent().unwrap();
    let temp_target = tempdir_in(target_parent_dir)?;

    match format {
        ArchiveFormat::TarGz => {
            let tar_gz = File::open(path)?;
            let tar = GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(&temp_target)?;
        }
        ArchiveFormat::Zip => {
            zip_extract(
                &path.as_ref().to_path_buf(),
                &temp_target.path().to_path_buf(),
            )
            .map_err(|e| Error::ExtractionError(format!("{:?}", e)))?;
        }
    };

    // Now rename the temp directory to the final target directory.
    fs::rename(temp_target, target)?;

    Ok(())
}
