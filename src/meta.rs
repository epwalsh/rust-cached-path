use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::utils::now;
use crate::Error;

/// Holds information about a cached resource.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// The original resource name.
    pub resource: String,
    /// Path to the cached resource.
    pub resource_path: PathBuf,
    /// Path to the serialized meta.
    pub meta_path: PathBuf,
    /// The ETAG of the resource from the time it was cached, if there was one.
    pub etag: Option<String>,
    /// Time that the freshness of this cached resource will expire.
    pub expires: Option<f64>,
    /// Time this version of the resource was cached.
    pub creation_time: f64,
}

impl Meta {
    pub(crate) fn new(
        resource: String,
        resource_path: PathBuf,
        etag: Option<String>,
        freshness_lifetime: Option<u64>,
    ) -> Meta {
        let mut expires: Option<f64> = None;
        let creation_time = now();
        if let Some(lifetime) = freshness_lifetime {
            expires = Some(creation_time + (lifetime as f64));
        }
        let meta_path = Meta::meta_path(&resource_path);
        Meta {
            resource,
            resource_path,
            meta_path,
            etag,
            expires,
            creation_time,
        }
    }

    pub(crate) fn meta_path(resource_path: &Path) -> PathBuf {
        let mut meta_path = PathBuf::from(resource_path);
        let resource_file_name = meta_path.file_name().unwrap().to_str().unwrap();
        let meta_file_name = format!("{}.meta", resource_file_name);
        meta_path.set_file_name(&meta_file_name[..]);
        meta_path
    }

    pub(crate) async fn to_file(&self) -> Result<(), Error> {
        let serialized = serde_json::to_string(self).unwrap();
        fs::write(&self.meta_path, &serialized[..]).await?;
        Ok(())
    }

    /// Get the `Meta` from a cached resource.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cached_path::{cached_path, Meta};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cached_path::Error> {
    /// let resource = "https://github.com/epwalsh/rust-cached-path/blob/master/README.md";
    /// let path = cached_path(resource).await?;
    /// let meta = Meta::from_cache(&path).await?;
    /// assert_eq!(&meta.resource[..], resource);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_cache(resource_path: &Path) -> Result<Self, Error> {
        let meta_path = Meta::meta_path(resource_path);
        Meta::from_path(&meta_path).await
    }

    /// Read `Meta` from a path.
    pub(crate) async fn from_path(path: &Path) -> Result<Self, Error> {
        let serialized = fs::read_to_string(path).await?;
        let meta: Meta = serde_json::from_str(&serialized[..]).unwrap();
        Ok(meta)
    }

    /// Check if resource is still fresh.
    pub fn is_fresh(&self) -> bool {
        if let Some(expiration_time) = self.expires {
            expiration_time > now()
        } else {
            false
        }
    }
}
