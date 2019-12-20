# rust-cached-path

[![crates.io](https://img.shields.io/crates/v/cached-path.svg)](https://crates.io/crates/cached-path)
[![Documentation](https://docs.rs/cached-path/badge.svg)](https://docs.rs/cached-path)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/cached-path.svg)](./LICENSE)
[![CI](https://github.com/epwalsh/rust-cached-path/workflows/CI/badge.svg)](https://github.com/epwalsh/rust-cached-path/actions?query=workflow%3ACI)

The idea behind `cached-path` is to provide a unified simple async interface for
accessing both local and remote files. This can be used behind other APIs that need
to access files agnostic to where they are located.

## Usage

For remote resources `cached-path` uses the ETAG to know when to update the cache.
The path returned is the local path to the latest cached version:

```rust
use cached_path::cached_path;

let path = cached_path("https://github.com/epwalsh/rust-cached-path/blob/master/README.md").await?;
assert!(path.is_file());
```

```bash
# From the command line:
$ cached-path https://github.com/epwalsh/rust-cached-path/blob/master/README.md
/tmp/cache/055968a99316f3a42e7bcff61d3f590227dd7b03d17e09c41282def7c622ba0f.efa33e7f611ef2d163fea874ce614bb6fa5ab2a9d39d5047425e39ebe59fe782
```

For local files, the path returned is just the original path supplied:

```rust
use cached_path::cached_path;

let path = cached_path("README.md").await?;
assert_eq!(path.to_str().unwrap(), "README.md");
```

```bash
# From the command line:
$ cached-path https://github.com/epwalsh/rust-cached-path/blob/master/README.md
README.md
```

It's easy to customize the configuration when you need more control over the cache
location or the HTTP client used to download files:

```rust
use cached_path::Cache;

let cache = Cache::builder()
    .root(std::env::temp_dir().join("my-cache/"))
    .connect_timeout(std::time::Duration::from_secs(3))
    .build()
    .await?;
let path = cache.cached_path("README.md").await?;
```

```bash
# From the command line:
$ cached-path --root /tmp/my-cache/ --connect-timeout 3 README.md
README.md
```

## Best practices

If your Rust code is going to call `cached_path` more than once, it's better
to create a `Cache` instance with the builder and then use the instance method
`Cache::cached_path` rather than calling the `cached_path` function on it's own,
as this requires some overhead to create a new HTTP client and ensure the cache
root exists.
