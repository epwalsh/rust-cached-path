use std::error;
use std::fmt;
use std::fs::{self, create_dir_all};
use std::path::PathBuf;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use reqwest::header::ETAG;
use tempfile::NamedTempFile;

pub fn cached_path(
    resource: &str,
) -> Result<PathBuf, Box<dyn error::Error>> {
    let cache = Cache::new(
        PathBuf::from("/tmp/cache"),
        reqwest::Client::new(),
    )?;
    cache.cached_path(resource)
}

pub struct Cache {
    root: PathBuf,
    http_client: reqwest::Client,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(PathBuf::from("/tmp/cache"), reqwest::Client::new()).unwrap()
    }
}

impl Cache {
    pub fn new(root: PathBuf, http_client: reqwest::Client) -> Result<Self, Box<dyn error::Error>> {
        create_dir_all(&root)?;
        Ok(Cache { root, http_client })
    }

    pub fn cached_path(&self, resource: &str) -> Result<PathBuf, Box<dyn error::Error>> {
        if !resource.starts_with("http") {
            // Treat as local file, see if it exists.
            let path = PathBuf::from(resource);
            if !path.is_file() {
                return Err(Error::FileNotFound.into());
            } else {
                return Ok(path);
            }
        }

        let url = reqwest::Url::parse(resource).map_err(|_| Error::InvalidUrl)?;
        let etag = self.get_etag(&url);
        let path = self.url_to_filepath(&url, etag);

        // If path doesn't exist locally, need to download.
        if !path.is_file() {
            self.download_resource(&url, &path)?;
        }

        return Ok(path);
    }

    fn download_resource(
        &self,
        url: &reqwest::Url,
        path: &PathBuf,
    ) -> Result<(), Box<dyn error::Error>> {
        if let Ok(mut response) = self.http_client.get(url.clone()).send() {
            // We write the content to a temporary file, and then if successful,
            // we copy the temp file to it's cache file.
            let mut tempfile = NamedTempFile::new()?;
            response.copy_to(&mut tempfile)?;

            // Now copy over the contents of the temp file to the cache file.
            fs::copy(tempfile.path(), path)?;
            Ok(())
        } else {
            Err(Error::HttpError.into())
        }
    }

    fn get_etag(&self, url: &reqwest::Url) -> Option<String> {
        match self.http_client.head(url.clone()).send() {
            Ok(r) => {
                if let Some(etag) = r.headers().get(ETAG) {
                    if let Ok(s) = etag.to_str() {
                        Some(s.into())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
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

    #[test]
    fn test_url_to_filename() {
        let cache = Cache::default();
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
