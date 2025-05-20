use crate::error::Error;
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use tempfile::tempdir_in;

/// Supported archive types.
pub(crate) enum ArchiveFormat {
    TarGz,
    Zip,
}

impl ArchiveFormat {
    fn is_tar<R: Read>(read: &mut R) -> bool {
        let mut buf = [0; 262];
        read.read_exact(&mut buf)
            .is_ok_and(|_| infer::archive::is_tar(&buf))
    }

    /// Parse archive type from resource extension.
    pub(crate) fn parse_from_extension(resource: &Path) -> Result<Self, Error> {
        if let Some(file_type) = infer::get_from_path(resource)? {
            let archive_type = match file_type.mime_type() {
                "application/gzip" if Self::is_tar(&mut GzDecoder::new(File::open(resource)?)) => {
                    Self::TarGz
                }
                "application/zip" => Self::Zip,
                _ => return Err(Error::ExtractionError("unsupported archive format".into())),
            };
            Ok(archive_type)
        } else {
            Err(Error::ExtractionError(
                "cannot determine archive file type".into(),
            ))
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
            let file = File::open(path)?;
            let mut archive =
                zip::ZipArchive::new(file).map_err(|e| Error::ExtractionError(e.to_string()))?;
            archive
                .extract(temp_target.path())
                .map_err(|e| Error::ExtractionError(e.to_string()))?;
        }
    };

    // Now rename the temp directory to the final target directory.
    fs::rename(temp_target, target)?;

    Ok(())
}
