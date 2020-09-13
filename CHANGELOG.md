# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Added a method `Cache::cached_path_with_options` and a corresponding `Options` struct.
- Added ability to automatically extract archives through the `Cache::cached_path_with_options` method.
- Added integration tests.

### Changed

- `Meta` struct is no longer public.
- `Cache::cached_path_in_subdir` is now deprecated.

### Removed

- Removed the `only_keep_latest` setting for the `Cache`.

## [v0.4.2](https://github.com/epwalsh/rust-cached-path/releases/tag/v0.4.3) - 2020-09-11

### Changed

- Updated the download method to stream the response into the target file.

## [v0.4.2](https://github.com/epwalsh/rust-cached-path/releases/tag/v0.4.2) - 2020-09-11

### Fixed

- `cached-path` now compiles on Windows.

## [v0.4.1](https://github.com/epwalsh/rust-cached-path/releases/tag/v0.4.1) - 2020-09-10

### Added

- Added a method `Cache::cached_path_in_subdir` to use a specified subdirectory of the cache root.

### Fixed

- Ensure cache directory exists every time `Cache::cached_path` or `Cache::cached_path_in_subdir` is called.

## [v0.4.0](https://github.com/epwalsh/rust-cached-path/releases/tag/v0.4.0) - 2020-09-10

### Fixed

- Fixed default timeout of `None`.
- `Meta` is now written to file before the tempfile of a downloaded resource is moved to its final cache location. This avoids a bug (albeit, an unlikely one) where the cache could be corrupted if writing the `Meta` to file fails.

## [v0.4.0-rc1](https://github.com/epwalsh/rust-cached-path/releases/tag/v0.4.0-rc1) - 2020-09-09

### Changed

- Switched to using `thiserror` and `anyhow` for error handling.

## [v0.3.0](https://github.com/epwalsh/rust-cached-path/releases/tag/v0.3.0) - 2020-06-13

### Changed

- API is now syncronous
- `root` configuration option renamed to `dir`.

## v0.2.0

### Added

- Added a file lock mechanism to make guard against parallel downloads of the same file.
- Added an "offline" mode.

### Changed

- Minor improvements to internal logic that make caching more robust.
