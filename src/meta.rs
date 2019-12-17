use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::utils::meta_path;
use crate::Error;

/// Holds information about a cached resource.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub resource: String,
    pub etag: Option<String>,
}

impl Meta {
    pub(crate) async fn to_file(&self, resource_path: &PathBuf) -> Result<(), Error> {
        let meta_path = meta_path(resource_path);
        let serialized = serde_json::to_string(self).unwrap();
        fs::write(meta_path, &serialized[..]).await?;
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
    pub async fn from_cache(resource_path: &PathBuf) -> Result<Self, Error> {
        let meta_path = meta_path(resource_path);
        let serialized = fs::read_to_string(meta_path).await?;
        let meta: Meta = serde_json::from_str(&serialized[..]).unwrap();
        Ok(meta)
    }
}
