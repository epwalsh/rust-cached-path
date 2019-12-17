use std::path::PathBuf;

use failure::ResultExt;
use log::{debug, info};
use reqwest::header::ETAG;
use tempfile::NamedTempFile;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{self, AsyncWriteExt};

use crate::utils::hash_str;
use crate::{Error, ErrorKind, Meta};

/// When you need control over cache location or the HTTP client used to download
/// resources, you can create a `Cache` instance and then use the instance method `cached_path`.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
    http_client: reqwest::Client,
}

impl Cache {
    /// Create a new `Cache` instance.
    pub async fn new(root: PathBuf, http_client: reqwest::Client) -> Result<Self, Error> {
        debug!("Using {} as cache root", root.to_string_lossy());
        fs::create_dir_all(&root).await?;
        Ok(Cache { root, http_client })
    }

    /// Works just like [`cached_path`](fn.cached_path.html).
    pub async fn cached_path(&self, resource: &str) -> Result<PathBuf, Error> {
        if !resource.starts_with("http") {
            info!("Treating resource as local file");

            let path = PathBuf::from(resource);
            if !path.is_file() {
                return Err(ErrorKind::ResourceNotFound(String::from(resource)).into());
            } else {
                return Ok(path);
            }
        }

        let url = reqwest::Url::parse(resource)
            .map_err(|_| ErrorKind::InvalidUrl(String::from(resource)))?;
        let etag = self.get_etag(&url).await?;
        let path = self.url_to_filepath(&url, &etag);

        // If resource + meta data already exist and meta data matches, no need to download again.
        if path.exists() {
            debug!("Cached version is up-to-date");
            return Ok(path);
        }

        info!("Downloading updated version of resource");

        self.download_resource(&url, &path).await?;

        debug!("Writing meta data to file");

        let meta = Meta {
            resource: String::from(resource),
            etag,
        };
        meta.to_file(&path).await?;

        Ok(path)
    }

    async fn download_resource(&self, url: &reqwest::Url, path: &PathBuf) -> Result<(), Error> {
        let mut response = self
            .http_client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;

        debug!("Opened connection to resource");

        // First we make a temporary file and downlaod the contents of the resource into it.
        // Otherwise, if we wrote directly to the cache file and the download got
        // interrupted, we could be left with a corrupted cache file.
        let tempfile = NamedTempFile::new().context(ErrorKind::IoError(None))?;
        let mut tempfile_write_handle =
            OpenOptions::new().write(true).open(tempfile.path()).await?;

        debug!("Starting download");

        while let Some(chunk) = response.chunk().await? {
            tempfile_write_handle.write_all(&chunk[..]).await?;
        }

        debug!("Download complete");

        // Resource successfully written to the tempfile, so we can copy the tempfile
        // over to the cache file.
        let mut tempfile_read_handle = OpenOptions::new().read(true).open(tempfile.path()).await?;
        let mut cache_file_write_handle = File::create(path).await?;

        debug!("Copying resource temp file to cache location");

        io::copy(&mut tempfile_read_handle, &mut cache_file_write_handle).await?;

        Ok(())
    }

    async fn get_etag(&self, url: &reqwest::Url) -> Result<Option<String>, Error> {
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

    fn url_to_filepath(&self, url: &reqwest::Url, etag: &Option<String>) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::meta_path;
    use httpmock::Method::{GET, HEAD};
    use httpmock::{mock, with_mock_server};
    use std::path::Path;
    use tempfile::tempdir;

    static ETAG_KEY: reqwest::header::HeaderName = ETAG;

    #[tokio::test]
    async fn test_url_to_filename() {
        let cache_dir = tempdir().unwrap();
        let cache = Cache::new(cache_dir.path().to_owned(), reqwest::Client::new())
            .await
            .unwrap();

        let url = reqwest::Url::parse("http://localhost:5000/foo.txt").unwrap();
        let etag = String::from("abcd");

        assert_eq!(
            cache.url_to_filepath(&url, &Some(etag)).to_str().unwrap(),
            format!(
                "{}/{}.{}",
                cache_dir.path().to_str().unwrap(),
                "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
                "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
            )
        );
        assert_eq!(
            cache.url_to_filepath(&url, &None).to_str().unwrap(),
            format!(
                "{}/{}",
                cache_dir.path().to_str().unwrap(),
                "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
            )
        );
    }

    #[tokio::test]
    async fn test_get_cached_path_local_file() {
        // Setup cache.
        let cache_dir = tempdir().unwrap();
        let cache = Cache::new(cache_dir.path().to_owned(), reqwest::Client::new())
            .await
            .unwrap();

        let path = cache.cached_path("README.md").await.unwrap();
        assert_eq!(path, Path::new("README.md"));
    }

    #[tokio::test]
    async fn test_get_cached_path_non_existant_local_file_fails() {
        // Setup cache.
        let cache_dir = tempdir().unwrap();
        let cache = Cache::new(cache_dir.path().to_owned(), reqwest::Client::new())
            .await
            .unwrap();

        let result = cache.cached_path("BLAH").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[with_mock_server]
    async fn test_download_resource() {
        // For debugging:
        // let _ = env_logger::try_init();

        // Setup cache.
        let cache_dir = tempdir().unwrap();
        let cache = Cache::new(cache_dir.path().to_owned(), reqwest::Client::new())
            .await
            .unwrap();

        let resource = "http://localhost:5000/resource.txt";

        // Mock the resource.
        let mut mock_1 = mock(GET, "/resource.txt")
            .return_status(200)
            .return_body("Hello, World!")
            .create();
        let mut mock_1_header = mock(HEAD, "/resource.txt")
            .return_status(200)
            .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
            .create();

        // Get the cached path.
        let path = cache.cached_path(&resource[..]).await.unwrap();

        assert_eq!(mock_1.times_called(), 1);
        assert_eq!(mock_1_header.times_called(), 1);

        // Ensure the file and meta exist.
        assert!(path.is_file());
        assert!(meta_path(&path).is_file());

        // Ensure the contents of the file are correct.
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(&contents[..], "Hello, World!");

        // Now update the resource.
        mock_1.delete();
        mock_1_header.delete();
        let mock_2 = mock(GET, "/resource.txt")
            .return_status(200)
            .return_body("Well hello again")
            .create();
        let mock_2_header = mock(HEAD, "/resource.txt")
            .return_status(200)
            .return_header(&ETAG_KEY.to_string()[..], "fake-etag-2")
            .create();

        // Get the new cached path.
        let new_path = cache.cached_path(&resource[..]).await.unwrap();

        assert_eq!(mock_2.times_called(), 1);
        assert_eq!(mock_2_header.times_called(), 1);

        // This should be different from the old path.
        assert_ne!(path, new_path);

        // Ensure the file and meta exist.
        assert!(new_path.is_file());
        assert!(meta_path(&new_path).is_file());

        // Ensure the contents of the file are correct.
        let new_contents = std::fs::read_to_string(&new_path).unwrap();
        assert_eq!(&new_contents[..], "Well hello again");
    }
}
