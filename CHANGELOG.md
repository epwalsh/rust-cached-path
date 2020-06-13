# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Changed

- API is now syncronous

## v0.2.0

### Added

- Added a file lock mechanism to make guard against parallel downloads of the same file.
- Added an "offline" mode.

### Changed

- Minor improvements to internal logic that make caching more robust.
