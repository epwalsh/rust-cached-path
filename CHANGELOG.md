# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
