# rust-cached-path

[![crates.io](https://img.shields.io/crates/v/cached-path.svg)](https://crates.io/crates/cached-path)
[![Documentation](https://docs.rs/cached-path/badge.svg)](https://docs.rs/cached-path)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/cached-path.svg)](./LICENSE)
[![CI](https://github.com/epwalsh/rust-cached-path/workflows/CI/badge.svg)](https://github.com/epwalsh/rust-cached-path/actions?query=workflow%3ACI)

Provides a simple interface `cached_path::cached_path` for accessing both local and remote files.

For remote resources, `cached_path` downloads and caches the latest version of the resource. Each time `cached_path` is called for a remote file, the ETAG is checked against the cached version and if it's out of date the file will be downloaded again. The path returned is the path to the cached file:

```rust
>>> let path = cached_path("https://github.com/epwalsh/rust-cached-path/blob/master/README.md");
>>> println!("{}", path.to_str().unwrap());
/tmp/cache/d629f792e430b3c76a1291bb2766b0a047e36fae0588f9dbc1ae51decdff691b.70bec105b4158ed9a1747fea67a43f5dee97855c64d62b6ec3742f4cfdb5feda
```

For local files, the path returned is just the original path supplied:

```rust
>>> let path = cached_path("README.md")
>>> println!("{}", path.to_str().unwrap());
README.md
```

When you need more control over the cache location or the HTTP client used to download files, you can create a instance of the `cached_path::Cache` struct and then use the method `.cached_path`.
