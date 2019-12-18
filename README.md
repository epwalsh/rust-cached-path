# rust-cached-path

[![crates.io](https://img.shields.io/crates/v/cached-path.svg)](https://crates.io/crates/cached-path)
[![Documentation](https://docs.rs/cached-path/badge.svg)](https://docs.rs/cached-path)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/cached-path.svg)](./LICENSE)
[![CI](https://github.com/epwalsh/rust-cached-path/workflows/CI/badge.svg)](https://github.com/epwalsh/rust-cached-path/actions?query=workflow%3ACI)

The idea behind `cached-path` is to provide a unified simple async interface for
accessing both local and remote files. This can be used behind other APIs that need
to access files agnostic to where they are located.

For remote resources `cached-path` uses the ETAG to know when to update the cache.
The path returned is the local path to the latest cached version:

```rust
use cached_path::cached_path;

let path = cached_path("https://github.com/epwalsh/rust-cached-path/blob/master/README.md").await?;
assert!(path.is_file());
```

For local files, the path returned is just the original path supplied:

```rust
use cached_path::cached_path;

let path = cached_path("README.md").await?;
assert_eq!(path.to_str().unwrap(), "README.md");
```

When you need more control over the cache location or the HTTP client used to download files,
you can build a `Cache` object and then use
the method `Cache::cached_path`:

```rust
use cached_path::Cache;

let cache = Cache::builder()
    .root(std::env::temp_dir().join("my-cache/"))
    .connect_timeout(std::time::Duration::from_secs(3))
    .build()
    .await?;
let path = cache.cached_path("README.md").await?;
```

This is the recommended way to use `cached-path` when you're going to be calling it more than
once.
