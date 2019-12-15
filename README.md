# rust-cached-path

[![crates.io](https://img.shields.io/crates/v/cached-path.svg)](https://crates.io/crates/cached-path)
[![Documentation](https://docs.rs/cached-path/badge.svg)](https://docs.rs/cached-path)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/cached-path.svg)](./LICENSE)
[![CI](https://github.com/epwalsh/rust-cached-path/workflows/CI/badge.svg)](https://github.com/epwalsh/rust-cached-path/actions?query=workflow%3ACI)

The idea behind `cache-path` is to provide a single, simple async interface for
accessing both local and remote files. This can be used behind other APIs that need
to access files agnostic to where they are located.

For remote resources, `cached_path` downloads and caches the latest version of the resource.
Each time `cached_path` is called for a remote file, the ETAG is checked against the cached
version and if it's out of date the file will be downloaded again. The path returned is the
path to the cached file:

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
you can create a instance of the `Cache` struct and then use the method `.cached_path`.

License: Apache 2.0
