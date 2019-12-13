//! This crate contains utilities for downloading and caching static HTTP resources.
//!
//! ## Usage
//!
//! The simplest way to use this crate is through the [`cached_path`](fn.cached_path.html)
//! function.
//! If you need finer control over the caching directory, HTTP client,
//! etc. you can construct a [`Cache`](struct.Cache.html) object directly.

use std::env;
use std::error;
use std::fmt;
use std::path::PathBuf;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use log::{debug, error, info};
use reqwest::header::ETAG;
use tempfile::NamedTempFile;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{self, AsyncWriteExt};

/// Try downloading and caching a static HTTP resource. If successful, the return value
/// is the local path to the cached resource. This function will always check the ETAG
/// of the resource to ensure the latest version is cached.
///
/// This also works for local files, in which case the return value is just the path original
/// path.
///
/// The cache location will be `std::env::temp_dir() / cache`.
pub async fn cached_path(resource: &str) -> Result<PathBuf, Box<dyn error::Error>> {
    let cache = Cache::new(env::temp_dir().join("cache/"), reqwest::Client::new()).await?;
    cache.cached_path(resource).await
}

/// When you need control over cache location or the HTTP client used to download
/// resources, you can create a `Cache` instance and then use the instance method `cached_path`.
pub struct Cache {
    root: PathBuf,
    http_client: reqwest::Client,
}

impl Cache {
    /// Create a new `Cache` instance.
    pub async fn new(
        root: PathBuf,
        http_client: reqwest::Client,
    ) -> Result<Self, Box<dyn error::Error>> {
        fs::create_dir_all(&root).await?;
        Ok(Cache { root, http_client })
    }

    /// Works just like [`cached_path`](fn.cached_path.html).
    pub async fn cached_path(&self, resource: &str) -> Result<PathBuf, Box<dyn error::Error>> {
        if !resource.starts_with("http") {
            info!("Treating resource as local file");
            let path = PathBuf::from(resource);
            if !path.is_file() {
                error!("File not found");
                return Err(Error::FileNotFound.into());
            } else {
                return Ok(path);
            }
        }

        let url = reqwest::Url::parse(resource).map_err(|_| Error::InvalidUrl)?;
        let etag = self.get_etag(&url).await?;
        let path = self.url_to_filepath(&url, etag);

        // If path doesn't exist locally, need to download.
        if !path.is_file() {
            info!("Downloading updated version of resource");
            self.download_resource(&url, &path).await?;
        }

        Ok(path)
    }

    async fn download_resource(
        &self,
        url: &reqwest::Url,
        path: &PathBuf,
    ) -> Result<(), Box<dyn error::Error>> {
        if let Ok(mut response) = self.http_client.get(url.clone()).send().await {
            debug!("Opened connection to resource");
            // First we make a temporary file and downlaod the contents of the resource into it.
            // Otherwise, if we wrote directly to the cache file and the download got
            // interrupted, we could be left with a corrupted cache file.
            let tempfile = NamedTempFile::new()?;
            // TODO: Seems inefficient to have two handles open, but we can't asyncronously
            // write to the `tempfile` handle, so we have to open a new handle
            // using tokio.
            let mut tempfile_write_handle =
                OpenOptions::new().write(true).open(tempfile.path()).await?;
            debug!("Starting download");
            while let Some(chunk) = response.chunk().await? {
                tempfile_write_handle.write_all(&chunk[..]).await?;
            }
            debug!("Download complete");
            // Resource successfully written to the tempfile, so we can copy the tempfile
            // over to the cache file.
            let mut tempfile_read_handle =
                OpenOptions::new().read(true).open(tempfile.path()).await?;
            let mut cache_file_write_handle = File::create(path).await?;
            debug!("Copying resource temp file to cache location");
            io::copy(&mut tempfile_read_handle, &mut cache_file_write_handle).await?;
            Ok(())
        } else {
            error!("Failed to download resource");
            Err(Error::HttpError.into())
        }
    }

    async fn get_etag(&self, url: &reqwest::Url) -> Result<Option<String>, Box<dyn error::Error>> {
        let r = self.http_client.head(url.clone()).send().await?;
        if let Some(etag) = r.headers().get(ETAG) {
            if let Ok(s) = etag.to_str() {
                Ok(Some(s.into()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn url_to_filepath(&self, url: &reqwest::Url, etag: Option<String>) -> PathBuf {
        let url_string = url.clone().into_string();
        let url_hash = hash_str(&url_string[..]);
        let filename: String;
        if let Some(tag) = etag {
            let etag_hash = hash_str(&tag[..]);
            filename = format!("{}.{}", url_hash, etag_hash);
        } else {
            filename = url_hash;
        }
        let filepath = PathBuf::from(filename);
        self.root.join(filepath)
    }
}

fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.input_str(s);
    hasher.result_str()
}

#[derive(Debug)]
pub enum Error {
    FileNotFound,
    HttpError,
    InvalidUrl,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::FileNotFound => write!(f, "file not found"),
            Self::HttpError => write!(f, "HTTP error"),
            Self::InvalidUrl => write!(f, "invalid URL"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_url_to_filename() {
        let cache = Cache::new(PathBuf::from("/tmp/cache"), reqwest::Client::new())
            .await
            .unwrap();
        let url = reqwest::Url::parse("http://localhost:5000/foo.txt").unwrap();
        let etag = String::from("abcd");
        assert_eq!(
            cache.url_to_filepath(&url, Some(etag)).to_str().unwrap(),
            format!(
                "/tmp/cache/{}.{}",
                "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
                "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
            )
        );
        assert_eq!(
            cache.url_to_filepath(&url, None).to_str().unwrap(),
            format!(
                "/tmp/cache/{}",
                "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
            )
        );
    }
}
