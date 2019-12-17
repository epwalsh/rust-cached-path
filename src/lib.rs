//! The idea behind `cache-path` is to provide a single, simple async interface for
//! accessing both local and remote files. This can be used behind other APIs that need
//! to access files agnostic to where they are located.
//!
//! For remote resources, `cached_path` downloads and caches the latest version of the resource.
//! Each time `cached_path` is called for a remote file, the ETAG is checked against the cached
//! version and if it's out of date the file will be downloaded again. The path returned is the
//! path to the cached file:
//!
//! ```rust
//! use cached_path::cached_path;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error + std::marker::Send + std::marker::Sync>> {
//! let path = cached_path("https://github.com/epwalsh/rust-cached-path/blob/master/README.md").await?;
//! assert!(path.is_file());
//! # Ok(())
//! # }
//! ```
//!
//! For local files, the path returned is just the original path supplied:
//!
//! ```rust
//! use cached_path::cached_path;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error + std::marker::Send + std::marker::Sync>> {
//! let path = cached_path("README.md").await?;
//! assert_eq!(path.to_str().unwrap(), "README.md");
//! # Ok(())
//! # }
//! ```
//!
//! When you need more control over the cache location or the HTTP client used to download files,
//! you can create a instance of the `Cache` struct and then use the method `.cached_path`.

use std::env;
use std::path::PathBuf;

#[macro_use]
extern crate lazy_static;

mod cache;
mod error;
mod meta;
pub(crate) mod utils;

pub use crate::cache::Cache;
pub use crate::error::Error;
pub use crate::meta::Meta;

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

/// Try downloading and caching a static HTTP resource. If successful, the return value
/// is the local path to the cached resource. This function will always check the ETAG
/// of the resource to ensure the latest version is cached.
///
/// This also works for local files, in which case the return value is just the original
/// path.
pub async fn cached_path(
    resource: &str,
) -> Result<PathBuf, Box<dyn std::error::Error + std::marker::Send + std::marker::Sync>> {
    let root = DEFAULT_CACHE_ROOT.clone();
    let cache = Cache::new(root, reqwest::Client::new()).await?;
    cache.cached_path(resource).await
}
