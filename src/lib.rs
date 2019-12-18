//! The idea behind `cached-path` is to provide a unified simple async interface for
//! accessing both local and remote files. This can be used behind other APIs that need
//! to access files agnostic to where they are located.
//!
//! For remote resources `cached-path` uses the ETAG to know when to update the cache.
//! The path returned is the local path to the latest cached version:
//!
//! ```rust
//! use cached_path::cached_path;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), cached_path::Error> {
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
//! # async fn main() -> Result<(), cached_path::Error> {
//! let path = cached_path("README.md").await?;
//! assert_eq!(path.to_str().unwrap(), "README.md");
//! # Ok(())
//! # }
//! ```
//!
//! When you need more control over the cache location or the HTTP client used to download files,
//! you can build a [`Cache`](struct.Cache.html) object and then use
//! the method [`Cache::cached_path`](struct.Cache.html#method.cached_path):
//!
//! ```rust
//! use cached_path::Cache;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), cached_path::Error> {
//! let cache = Cache::builder()
//!     .root(std::env::temp_dir().join("my-cache/"))
//!     .connect_timeout(std::time::Duration::from_secs(3))
//!     .build()
//!     .await?;
//! let path = cache.cached_path("README.md").await?;
//! # Ok(())
//! # }
//! ```
//!
//! This is the recommended way to use `cached-path` when you're going to be calling it more than
//! once.

use std::path::PathBuf;

#[macro_use]
extern crate lazy_static;

mod cache;
mod error;
mod meta;
pub(crate) mod utils;

pub use crate::cache::{Cache, CacheBuilder, DEFAULT_CACHE_ROOT};
pub use crate::error::{Error, ErrorKind};
pub use crate::meta::Meta;

/// Try downloading and caching a static HTTP resource. If successful, the return value
/// is the local path to the cached resource. This function will always check the ETAG
/// of the resource to ensure the latest version is cached.
///
/// This also works for local files, in which case the return value is just the original
/// path.
pub async fn cached_path(resource: &str) -> Result<PathBuf, Error> {
    let cache = Cache::new().await?;
    Ok(cache.cached_path(resource).await?)
}
