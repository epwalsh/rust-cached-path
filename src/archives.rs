use crate::error::Error;
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use tempfile::tempdir_in;

/// Supported archive types.
pub(crate) enum ArchiveFormat {
    TarGz,
    #[cfg(feature = "lzma")]
    TarXz,
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
                #[cfg(feature = "lzma")]
                "application/x-xz"
                    if Self::is_tar(&mut xz::XzDecoder::new(File::open(resource)?)?) =>
                {
                    Self::TarXz
                }
                "application/zip" => Self::Zip,
                tpe => {
                    return Err(Error::ExtractionError(format!(
                        "unsupported file format: {tpe}"
                    )))
                }
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
        #[cfg(feature = "lzma")]
        ArchiveFormat::TarXz => {
            let xz_decoder = xz::XzDecoder::new(File::open(path)?)?;
            let mut archive = tar::Archive::new(xz_decoder);
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

#[cfg(feature = "lzma")]
mod xz {
    use std::io::Read;
    use std::thread::JoinHandle;
    pub(super) struct XzDecoder {
        decoder_handle: Option<JoinHandle<Result<(), lzma_rs::error::Error>>>,
        pipe_reader: std::io::PipeReader,
    }

    impl XzDecoder {
        pub(super) fn new<R: Read + Send + 'static>(reader: R) -> std::io::Result<Self> {
            let (pipe_reader, mut pipe_writer) = std::io::pipe()?;
            let decoder_handle = std::thread::spawn(move || {
                lzma_rs::xz_decompress(&mut std::io::BufReader::new(reader), &mut pipe_writer)
            });
            Ok(Self {
                decoder_handle: Some(decoder_handle),
                pipe_reader,
            })
        }
    }

    impl Read for XzDecoder {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
            let size = self.pipe_reader.read(buf)?;
            if let Some(handle) = self.decoder_handle.take_if(|h| h.is_finished()) {
                handle
                    .join()
                    .map_err(|_| std::io::Error::other("xz decompression thread panicked"))?
                    .map_err(|e| std::io::Error::other(format!("xz decompression error: {e}")))?;
            }
            Ok(size)
        }
    }

    #[cfg(test)]
    mod test {

        use super::XzDecoder;
        #[test]
        #[should_panic(expected = "xz decompression error")]
        fn test_xz_decoder_empty() {
            let mut decoder = XzDecoder::new(std::io::empty()).unwrap();
            std::io::copy(&mut decoder, &mut Vec::new()).unwrap();
        }

        #[test]
        #[should_panic(expected = "xz decompression error")]
        fn test_xz_decoder_bad() {
            let bad: &[u8] = &[0x42u8; 1024];
            let mut decoder = XzDecoder::new(bad).unwrap();
            std::io::copy(&mut decoder, &mut Vec::new()).unwrap();
        }
    }
}
