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
    #[cfg(feature = "lzma")]
    TarLzma,
    Zip,
}

// see https://github.com/bojand/infer/issues/91
#[allow(clippy::nonminimal_bool)]
fn is_lzma(buf: &[u8]) -> bool {
    buf.len() > 4
        && buf[0] == 0x5D
        && buf[1] == 0x00
        && buf[2] == 0x00
        && (buf[3] == 0x80
            || buf[3] == 0x01
            || buf[3] == 0x10
            || buf[3] == 0x08
            || buf[3] == 0x20
            || buf[3] == 0x40
            || buf[3] == 0x80
            || buf[3] == 0x00)
        && (buf[4] == 0x00 || buf[4] == 0x01 || buf[4] == 0x02)
}

fn infer() -> infer::Infer {
    let mut infer = infer::Infer::new();
    infer.add("application/x-lzma", "lzma", is_lzma);
    infer
}

impl ArchiveFormat {
    fn is_tar<R: Read>(read: &mut R) -> bool {
        let mut buf = [0; 262];
        read.read_exact(&mut buf)
            .is_ok_and(|_| infer::archive::is_tar(&buf))
    }

    /// Parse archive type from resource extension.
    pub(crate) fn parse_from_extension(resource: &Path) -> Result<Self, Error> {
        if let Some(file_type) = infer().get_from_path(resource)? {
            let archive_type = match file_type.mime_type() {
                "application/gzip" if Self::is_tar(&mut GzDecoder::new(File::open(resource)?)) => {
                    Self::TarGz
                }
                #[cfg(feature = "lzma")]
                "application/x-xz"
                    if Self::is_tar(&mut lzma::LzmaDecoder::new(
                        lzma::Codec::Xz,
                        File::open(resource)?,
                    )?) =>
                {
                    Self::TarXz
                }
                #[cfg(feature = "lzma")]
                "application/x-lzma"
                    if Self::is_tar(&mut lzma::LzmaDecoder::new(
                        lzma::Codec::Lzma,
                        File::open(resource)?,
                    )?) =>
                {
                    Self::TarLzma
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
            let xz_decoder = lzma::LzmaDecoder::new(lzma::Codec::Xz, File::open(path)?)?;
            let mut archive = tar::Archive::new(xz_decoder);
            archive.unpack(&temp_target)?;
        }
        #[cfg(feature = "lzma")]
        ArchiveFormat::TarLzma => {
            let lzma_decoder = lzma::LzmaDecoder::new(lzma::Codec::Lzma, File::open(path)?)?;
            let mut archive = tar::Archive::new(lzma_decoder);
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
mod lzma {
    use std::io::Read;
    use std::thread::JoinHandle;

    #[derive(Clone, Copy)]
    pub(super) enum Codec {
        Lzma,
        Xz,
    }

    impl std::fmt::Display for Codec {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Codec::Lzma => write!(f, "lzma"),
                Codec::Xz => write!(f, "xz"),
            }
        }
    }

    pub(super) struct LzmaDecoder {
        codec: Codec,
        decoder_handle: Option<JoinHandle<Result<(), lzma_rs::error::Error>>>,
        pipe_reader: std::io::PipeReader,
    }

    impl LzmaDecoder {
        pub(super) fn new<R: Read + Send + 'static>(
            codec: Codec,
            reader: R,
        ) -> std::io::Result<Self> {
            let (pipe_reader, mut pipe_writer) = std::io::pipe()?;
            let decoder_handle = std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(reader);
                match codec {
                    Codec::Lzma => lzma_rs::lzma_decompress(&mut reader, &mut pipe_writer),
                    Codec::Xz => lzma_rs::xz_decompress(&mut reader, &mut pipe_writer),
                }
            });
            Ok(Self {
                codec,
                decoder_handle: Some(decoder_handle),
                pipe_reader,
            })
        }
    }

    impl Read for LzmaDecoder {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
            let size = self.pipe_reader.read(buf);
            if let Some(handle) = self.decoder_handle.take_if(|h| h.is_finished()) {
                handle
                    .join()
                    .map_err(|_| {
                        std::io::Error::other(format!(
                            "{} decompression thread panicked",
                            self.codec
                        ))
                    })?
                    .map_err(|e| {
                        std::io::Error::other(format!("{} decompression error: {e}", self.codec))
                    })?;
            }
            // handle 0-byte read edge case
            match size {
                Ok(0) if self.decoder_handle.is_some() => {
                    // we read nothing, but the thread is still running, most likely a race condition, retry
                    self.read(buf)
                }
                other => other,
            }
        }
    }

    #[cfg(test)]
    mod test {

        use super::*;

        #[test]
        #[should_panic(expected = "xz decompression error")]
        fn test_xz_decoder_empty() {
            let mut decoder = LzmaDecoder::new(Codec::Xz, std::io::empty()).unwrap();
            std::io::copy(&mut decoder, &mut Vec::new()).unwrap();
        }

        #[test]
        #[should_panic(expected = "xz decompression error")]
        fn test_xz_decoder_bad() {
            let bad: &[u8] = &[0x42u8; 1024];
            let mut decoder = LzmaDecoder::new(Codec::Xz, bad).unwrap();
            std::io::copy(&mut decoder, &mut Vec::new()).unwrap();
        }

        #[test]
        #[should_panic(expected = "lzma decompression error")]
        fn test_lzma_decoder_empty() {
            let mut decoder = LzmaDecoder::new(Codec::Lzma, std::io::empty()).unwrap();
            std::io::copy(&mut decoder, &mut Vec::new()).unwrap();
        }

        #[test]
        #[should_panic(expected = "lzma decompression error")]
        fn test_lzma_decoder_bad() {
            let bad: &[u8] = &[0x42u8; 1024];
            let mut decoder = LzmaDecoder::new(Codec::Lzma, bad).unwrap();
            std::io::copy(&mut decoder, &mut Vec::new()).unwrap();
        }
    }
}
