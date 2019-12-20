use std::default::Default;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use failure::ResultExt;
use glob::glob;
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
    max_backoff: u32,
    freshness_lifetime: Option<u64>,
}

impl CacheBuilder {
    /// Construct a new `CacheBuilder`.
    pub fn new() -> CacheBuilder {
        CacheBuilder {
            config: Config {
                root: None,
                client_builder: ClientBuilder::new(),
                max_retries: 3,
                max_backoff: 5000,
                freshness_lifetime: None,
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

    /// Set the maximum backoff delay in milliseconds for retrying HTTP requests.
    pub fn max_backoff(mut self, max_backoff: u32) -> CacheBuilder {
        self.config.max_backoff = max_backoff;
        self
    }

    /// Set the default freshness lifetime, in seconds. The default is None, meaning
    /// the ETAG for an external resource will always be checked for a fresher value.
    pub fn freshness_lifetime(mut self, freshness_lifetime: u64) -> CacheBuilder {
        self.config.freshness_lifetime = Some(freshness_lifetime);
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
            max_backoff: self.config.max_backoff,
            freshness_lifetime: self.config.freshness_lifetime,
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
    pub root: PathBuf,
    http_client: Client,
    max_retries: u32,
    max_backoff: u32,
    freshness_lifetime: Option<u64>,
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
        // If resource doesn't look like a URL, treat as local path, but return
        // an error if the path doesn't exist.
        if !resource.starts_with("http") {
            info!("Treating {} as local file", resource);
            let path = PathBuf::from(resource);
            if !path.is_file() {
                return Err(ErrorKind::ResourceNotFound(String::from(resource)).into());
            } else {
                return Ok(path);
            }
        }

        // Otherwise we attempt to parse the URL.
        let url = reqwest::Url::parse(resource)
            .map_err(|_| ErrorKind::InvalidUrl(String::from(resource)))?;

        // Find any existing cached versions of resource and check if they are still
        // fresh according to the `freshness_lifetime` setting.
        let versions = self.find_existing(resource).await; // already sorted, latest is first.
        if !versions.is_empty() {
            debug!("Found {} existing versions of {}", versions.len(), resource);
            if versions[0].is_fresh() {
                // Oh hey, the latest version is still fresh! We can clean up any
                // older versions and return the latest.
                info!("Latest existing version of {} is still fresh", resource);
                Cache::clean_up(&versions, Some(&versions[0].resource_path)).await;
                return Ok(versions[0].resource_path.clone());
            } else {
                // Existing versions are older than their freshness lifetimes, so we'll
                // query for the ETAG of the resource and then compare that with existing
                // versions.
                let etag = self.try_get_etag(resource, &url).await?;
                let path = self.resource_to_filepath(resource, &etag);
                if path.exists() {
                    // Oh cool! The cache is up-to-date according to the ETAG.
                    // We'll return the up-to-date version and clean up any other
                    // dangling ones.
                    info!("Cached version of {} is up-to-date", resource);
                    Cache::clean_up(&versions, Some(&path)).await;
                    return Ok(path);
                }
            }
        } else {
            debug!("No existing versions found for {}", resource);
        }

        // No up-to-date version cached, so we have to try downloading it.
        let meta = self.try_download_resource(resource, &url).await?;
        info!("New version of {} cached", resource);
        meta.to_file().await?;
        Cache::clean_up(&versions, Some(&meta.resource_path)).await;
        Ok(meta.resource_path)
    }

    /// Find existing versions of a cached resource, sorted by most recent first.
    async fn find_existing(&self, resource: &str) -> Vec<Meta> {
        let mut existing_meta: Vec<Meta> = vec![];
        let glob_string = format!(
            "{}.*.meta",
            self.resource_to_filepath(resource, &None).to_str().unwrap(),
        );
        for meta_path in glob(&glob_string).unwrap().filter_map(Result::ok) {
            if let Ok(meta) = Meta::from_path(&meta_path).await {
                existing_meta.push(meta);
            }
        }
        existing_meta
            .sort_unstable_by(|a, b| b.creation_time.partial_cmp(&a.creation_time).unwrap());
        existing_meta
    }

    async fn clean_up(versions: &[Meta], keep: Option<&Path>) {
        for meta in versions {
            if let Some(path) = keep {
                if path == meta.resource_path {
                    continue;
                }
            }
            debug!(
                "Removing old version at {}",
                meta.resource_path.to_str().unwrap()
            );
            fs::remove_file(&meta.meta_path).await.ok();
            fs::remove_file(&meta.resource_path).await.ok();
        }
    }

    fn get_retry_delay(&self, retries: u32) -> u32 {
        let between = Uniform::from(0..1000);
        let mut rng = rand::thread_rng();
        std::cmp::min(
            2u32.pow(retries - 1) * 1000 + between.sample(&mut rng),
            self.max_backoff,
        )
    }

    async fn try_download_resource(
        &self,
        resource: &str,
        url: &reqwest::Url,
    ) -> Result<Meta, Error> {
        let mut retries: u32 = 0;
        loop {
            match self.download_resource(resource, &url).await {
                Ok(meta) => {
                    return Ok(meta);
                }
                Err(err) => {
                    if retries >= self.max_retries {
                        error!("Max retries exceeded for {}", resource);
                        return Err(err);
                    }
                    if !err.is_retriable() {
                        error!("Download failed for {} with fatal error", resource);
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
    }

    async fn download_resource(&self, resource: &str, url: &reqwest::Url) -> Result<Meta, Error> {
        debug!("Attempting connection to {}", url);

        let mut response = self
            .http_client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;

        debug!("Opened connection to {}", url);

        let mut etag: Option<String> = None;
        if let Some(val) = response.headers().get(ETAG) {
            if let Ok(s) = val.to_str() {
                etag = Some(s.into());
                debug!("Found new ETAG for {}", resource);
            } else {
                warn!("Error parsing ETAG for {}", resource);
            }
        }
        let path = self.resource_to_filepath(resource, &etag);

        // First we make a temporary file and download the contents of the resource into it.
        // Otherwise if we wrote directly to the cache file and the download got
        // interrupted we could be left with a corrupted cache file.
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
        let mut cache_file_write_handle = File::create(&path).await?;

        debug!("Copying resource temp file to cache location for {}", url);

        io::copy(&mut tempfile_read_handle, &mut cache_file_write_handle).await?;

        let meta = Meta::new(String::from(resource), path, etag, self.freshness_lifetime);

        Ok(meta)
    }

    async fn try_get_etag(
        &self,
        resource: &str,
        url: &reqwest::Url,
    ) -> Result<Option<String>, Error> {
        let mut retries: u32 = 0;
        loop {
            match self.get_etag(&url).await {
                Ok(etag) => return Ok(etag),
                Err(err) => {
                    if retries >= self.max_retries {
                        error!("Max retries exceeded for {}", resource);
                        return Err(err);
                    }
                    if !err.is_retriable() {
                        error!("ETAG fetch for {} failed with fatal error", resource);
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
    }

    async fn get_etag(&self, url: &reqwest::Url) -> Result<Option<String>, Error> {
        debug!("Fetching ETAG for {}", url);
        let response = self
            .http_client
            .head(url.clone())
            .send()
            .await?
            .error_for_status()?;
        if let Some(etag) = response.headers().get(ETAG) {
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

    fn resource_to_filepath(&self, resource: &str, etag: &Option<String>) -> PathBuf {
        let resource_hash = hash_str(resource);
        let filename: String;

        if let Some(tag) = etag {
            let etag_hash = hash_str(&tag[..]);
            filename = format!("{}.{}", resource_hash, etag_hash);
        } else {
            filename = resource_hash;
        }

        let filepath = PathBuf::from(filename);

        self.root.join(filepath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let resource = "http://localhost:5000/foo.txt";
        let etag = String::from("abcd");

        assert_eq!(
            cache
                .resource_to_filepath(resource, &Some(etag))
                .to_str()
                .unwrap(),
            format!(
                "{}/{}.{}",
                cache_dir.path().to_str().unwrap(),
                "b5696dbf866311125e26a62bef0125854dd40f010a70be9cfd23634c997c1874",
                "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
            )
        );
        assert_eq!(
            cache
                .resource_to_filepath(resource, &None)
                .to_str()
                .unwrap(),
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
    async fn test_cached_path() {
        // For debugging:
        // let _ = env_logger::try_init();

        // Setup cache.
        let cache_dir = tempdir().unwrap();
        let cache = Cache::builder()
            .root(cache_dir.path().to_owned())
            .freshness_lifetime(300)
            .build()
            .await
            .unwrap();

        let resource = "http://localhost:5000/resource.txt";

        // Mock the resource.
        let mut mock_1_head = mock(HEAD, "/resource.txt")
            .return_status(200)
            .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
            .create();
        let mut mock_1_get = mock(GET, "/resource.txt")
            .return_status(200)
            .return_header(&ETAG_KEY.to_string()[..], "fake-etag")
            .return_body("Hello, World!")
            .create();

        // Get the cached path.
        let path = cache.cached_path(&resource[..]).await.unwrap();
        assert_eq!(
            path,
            cache.resource_to_filepath(&resource, &Some(String::from("fake-etag")))
        );

        assert_eq!(mock_1_head.times_called(), 0);
        assert_eq!(mock_1_get.times_called(), 1);

        // Ensure the file and meta exist.
        assert!(path.is_file());
        assert!(Meta::meta_path(&path).is_file());

        // Ensure the contents of the file are correct.
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(&contents[..], "Hello, World!");

        // When we attempt to get the resource again, the cache should still be fresh.
        let mut meta = Meta::from_cache(&path).await.unwrap();
        assert!(meta.is_fresh());
        let same_path = cache.cached_path(&resource[..]).await.unwrap();
        assert_eq!(same_path, path);
        assert!(path.is_file());
        assert!(Meta::meta_path(&path).is_file());

        // Didn't have to call HEAD or GET again.
        assert_eq!(mock_1_head.times_called(), 0);
        assert_eq!(mock_1_get.times_called(), 1);

        // Now expire the resource to continue testing.
        meta.expires = None;
        meta.to_file().await.unwrap();

        // After calling again when the resource is no longer fresh, the ETAG
        // should have been queried again with HEAD, but the resource should not have been
        // downloaded again with GET.
        let same_path = cache.cached_path(&resource[..]).await.unwrap();
        assert_eq!(same_path, path);
        assert!(path.is_file());
        assert!(Meta::meta_path(&path).is_file());
        assert_eq!(mock_1_head.times_called(), 1);
        assert_eq!(mock_1_get.times_called(), 1);

        // Now update the resource.
        mock_1_head.delete();
        mock_1_get.delete();
        let mock_2_head = mock(HEAD, "/resource.txt")
            .return_status(200)
            .return_header(&ETAG_KEY.to_string()[..], "fake-etag-2")
            .create();
        let mock_2_get = mock(GET, "/resource.txt")
            .return_status(200)
            .return_header(&ETAG_KEY.to_string()[..], "fake-etag-2")
            .return_body("Well hello again")
            .create();

        // Get the new cached path.
        let new_path = cache.cached_path(&resource[..]).await.unwrap();
        assert_eq!(
            new_path,
            cache.resource_to_filepath(&resource, &Some(String::from("fake-etag-2")))
        );

        assert_eq!(mock_2_head.times_called(), 1);
        assert_eq!(mock_2_get.times_called(), 1);

        // This should be different from the old path.
        assert_ne!(path, new_path);

        // Ensure the file and meta exist.
        assert!(new_path.is_file());
        assert!(Meta::meta_path(&new_path).is_file());

        // Ensure the old version was cleaned up.
        assert!(!path.is_file());
        assert!(!Meta::meta_path(&path).is_file());

        // Ensure the contents of the file are correct.
        let new_contents = std::fs::read_to_string(&new_path).unwrap();
        assert_eq!(&new_contents[..], "Well hello again");
    }
}
