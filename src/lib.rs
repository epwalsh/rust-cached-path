//! The idea behind `cached-path` is to provide a unified simple async interface for
//! accessing both local and remote files. This can be used behind other APIs that need
//! to access files agnostic to where they are located.
//!
//! # Usage
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
//! ```bash
//! # From the command line:
//! $ cached-path https://github.com/epwalsh/rust-cached-path/blob/master/README.md
//! /tmp/cache/055968a99316f3a42e7bcff61d3f590227dd7b03d17e09c41282def7c622ba0f.efa33e7f611ef2d163fea874ce614bb6fa5ab2a9d39d5047425e39ebe59fe782
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
//! ```bash
//! # From the command line:
//! $ cached-path https://github.com/epwalsh/rust-cached-path/blob/master/README.md
//! README.md
//! ```
//!
//! It's easy to customize the configuration when you need more control over the cache
//! location or the HTTP client used to download files:
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
//! ```bash
//! # From the command line:
//! $ cached-path --root /tmp/my-cache/ --connect-timeout 3 README.md
//! README.md
//! ```

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

lazy_static! {
    static ref CACHE: Cache = { Cache::builder().build_sync().unwrap() };
}

/// Try downloading and caching a static HTTP resource. If successful, the return value
/// is the local path to the cached resource. This function will always check the ETAG
/// of the resource to ensure the latest version is cached.
///
/// This also works for local files, in which case the return value is just the original
/// path.
pub async fn cached_path(resource: &str) -> Result<PathBuf, Error> {
    Ok(CACHE.cached_path(resource).await?)
}
