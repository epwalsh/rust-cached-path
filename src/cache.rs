use std::default::Default;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use failure::ResultExt;
use log::{debug, error, info, warn};
use rand::distributions::{Distribution, Uniform};
use reqwest::header::ETAG;
use reqwest::{Client, ClientBuilder};
use tempfile::NamedTempFile;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{self, AsyncWriteExt};
use tokio::time::{self, delay_for};

use crate::utils::hash_str;
use crate::{Error, ErrorKind, Meta};

lazy_static! {
    /// The default cache directory. This can be set through the environment
    /// variable `RUST_CACHED_PATH_ROOT`. Otherwise it will be set to a subdirectory
    /// named 'cache' of the default system temp directory.
    pub static ref DEFAULT_CACHE_ROOT: PathBuf = {
        if let Some(root_str) = env::var_os("RUST_CACHED_PATH_ROOT") {
            PathBuf::from(root_str)
        } else {
            env::temp_dir().join("cache/")
        }
    };
}

/// Builder to facilitate creating [`Cache`](struct.Cache.html) objects.
#[derive(Debug)]
pub struct CacheBuilder {
    config: Config,
}

#[derive(Debug)]
struct Config {
    root: Option<PathBuf>,
    client_builder: ClientBuilder,
    max_retries: u32,
}

impl CacheBuilder {
    /// Construct a new `CacheBuilder`.
    pub fn new() -> CacheBuilder {
        CacheBuilder {
            config: Config {
                root: None,
                client_builder: ClientBuilder::new(),
                max_retries: 3,
            },
        }
    }

    /// Construct a new `CacheBuilder` with a `ClientBuilder`.
    pub fn with_client_builder(client_builder: ClientBuilder) -> CacheBuilder {
        CacheBuilder::new().client_builder(client_builder)
    }

    /// Set the root directory.
    pub fn root(mut self, root: PathBuf) -> CacheBuilder {
        self.config.root = Some(root);
        self
    }

    /// Set the `ClientBuilder`.
    pub fn client_builder(mut self, client_builder: ClientBuilder) -> CacheBuilder {
        self.config.client_builder = client_builder;
        self
    }

    /// Enable a request timeout.
    pub fn timeout(mut self, timeout: Duration) -> CacheBuilder {
        self.config.client_builder = self.config.client_builder.timeout(timeout);
        self
    }

    /// Enable a timeout for the connect phase of each HTTP request.
    pub fn connect_timeout(mut self, timeout: Duration) -> CacheBuilder {
        self.config.client_builder = self.config.client_builder.connect_timeout(timeout);
        self
    }

    /// Set maximum number of retries for HTTP requests.
    pub fn max_retries(mut self, max_retries: u32) -> CacheBuilder {
        self.config.max_retries = max_retries;
        self
    }

    /// Build the `Cache` object.
    pub async fn build(self) -> Result<Cache, Error> {
        let root = self
            .config
            .root
            .unwrap_or_else(|| DEFAULT_CACHE_ROOT.clone());
        let http_client = self.config.client_builder.build()?;

        fs::create_dir_all(&root).await?;

        Ok(Cache {
            root,
            http_client,
            max_retries: self.config.max_retries,
        })
    }
}

impl Default for CacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// When you need control over cache location or the HTTP client used to download
/// resources, you can create a [`Cache`](struct.Cache.html) instance and then use the
/// instance method [`cached_path`](struct.Cache.html#method.cached_path).
///
/// If you're going to be making more than a handful of calls to `cached_path`, this
/// is the recommended way to do it.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
    http_client: Client,
    max_retries: u32,
}

impl Cache {
    /// Create a new `Cache` instance.
    pub async fn new() -> Result<Self, Error> {
        Cache::builder().build().await
    }

    /// Create a `CacheBuilder`.
    pub fn builder() -> CacheBuilder {
        CacheBuilder::new()
    }

    /// Works just like [`cached_path`](fn.cached_path.html).
    pub async fn cached_path(&self, resource: &str) -> Result<PathBuf, Error> {
        if !resource.starts_with("http") {
            info!("Treating {} as local file", resource);
            let path = PathBuf::from(resource);
            if !path.is_file() {
                return Err(ErrorKind::ResourceNotFound(String::from(resource)).into());
            } else {
                return Ok(path);
            }
        }

        let url = reqwest::Url::parse(resource)
            .map_err(|_| ErrorKind::InvalidUrl(String::from(resource)))?;

        let etag: Option<String>;
        let mut retries: u32 = 0;
        loop {
            match self.get_etag(&url).await {
                Ok(result) => {
                    etag = result;
                    break;
                }
                Err(err) => {
                    if retries >= self.max_retries {
                        error!("Max retries exceeded for {}", resource);
                        return Err(err);
                    }
                    if !err.is_retriable() {
                        return Err(err);
                    }
                    retries += 1;
                    let retry_delay = self.get_retry_delay(retries);
                    warn!(
                        "ETAG fetch failed for {}, retrying in {} milliseconds...",
                        resource, retry_delay
                    );
                    delay_for(time::Duration::from_millis(u64::from(retry_delay))).await;
                }
            }
        }

        let path = self.url_to_filepath(&url, &etag);
        if path.exists() {
            info!("Cached version is up-to-date");
            return Ok(path);
        }

        let mut retries: u32 = 0;
        loop {
            match self.download_resource(&url, &path).await {
                Ok(_) => break,
                Err(err) => {
                    if retries >= self.max_retries {
                        error!("Max retries exceeded for {}", resource);
                        return Err(err);
                    }
                    if !err.is_retriable() {
                        return Err(err);
                    }
                    retries += 1;
                    let retry_delay = self.get_retry_delay(retries);
                    warn!(
                        "Download failed for {}, retrying in {} milliseconds...",
                        resource, retry_delay
                    );
                    delay_for(time::Duration::from_millis(u64::from(retry_delay))).await;
                }
            }
        }

        debug!("Writing meta data to file for {}", resource);
        let meta = Meta {
            resource: String::from(resource),
            etag,
        };
        meta.to_file(&path).await?;

        Ok(path)
    }

    fn get_retry_delay(&self, retries: u32) -> u32 {
        let between = Uniform::from(0..1000);
        let mut rng = rand::thread_rng();
        2u32.pow(retries - 1) * 1000 + between.sample(&mut rng)
    }

    async fn download_resource(&self, url: &reqwest::Url, path: &PathBuf) -> Result<(), Error> {
        debug!("Attempting connection to {}", url);

        let mut response = self
            .http_client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;

        debug!("Opened connection to {}", url);

        // First we make a temporary file and downlaod the contents of the resource into it.
        // Otherwise, if we wrote directly to the cache file and the download got
        // interrupted, we could be left with a corrupted cache file.
        let tempfile = NamedTempFile::new().context(ErrorKind::IoError(None))?;
        let mut tempfile_write_handle =
            OpenOptions::new().write(true).open(tempfile.path()).await?;

        debug!("Starting download of {}", url);

        while let Some(chunk) = response.chunk().await? {
            tempfile_write_handle.write_all(&chunk[..]).await?;
        }

        debug!("Download complete for {}", url);

        // Resource successfully written to the tempfile, so we can copy the tempfile
        // over to the cache file.
        let mut tempfile_read_handle = OpenOptions::new().read(true).open(tempfile.path()).await?;
        let mut cache_file_write_handle = File::create(path).await?;

        debug!("Copying resource temp file to cache location for {}", url);

        io::copy(&mut tempfile_read_handle, &mut cache_file_write_handle).await?;

        Ok(())
    }

    async fn get_etag(&self, url: &reqwest::Url) -> Result<Option<String>, Error> {
        debug!("Fetching ETAG for {}", url);

        let r = self.http_client.head(url.clone()).send().await?;

        if let Some(etag) = r.headers().get(ETAG) {
            if let Ok(s) = etag.to_str() {
                Ok(Some(s.into()))
            } else {
                debug!("No ETAG for {}", url);
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
        let cache = Cache::builder()
            .root(cache_dir.path().to_owned())
            .build()
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
        let cache = Cache::builder()
            .root(cache_dir.path().to_owned())
            .build()
            .await
            .unwrap();

        let path = cache.cached_path("README.md").await.unwrap();
        assert_eq!(path, Path::new("README.md"));
    }

    #[tokio::test]
    async fn test_get_cached_path_non_existant_local_file_fails() {
        // Setup cache.
        let cache_dir = tempdir().unwrap();
        let cache = Cache::builder()
            .root(cache_dir.path().to_owned())
            .build()
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
        let cache = Cache::builder()
            .root(cache_dir.path().to_owned())
            .build()
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
