use thiserror::Error;

/// Errors that can occur during caching.
#[derive(Error, Debug)]
pub enum Error {
    /// Arises when the resource looks like a local file but it doesn't exist.
    #[error("Treated resource as local file, but file does not exist ({0})")]
    ResourceNotFound(String),

    /// Arises when the resource looks like a URL, but is invalid.
    #[error("Unable to parse resource URL ({0})")]
    InvalidUrl(String),

    /// Arises when the cache is being used in offline mode, but it couldn't locate
    /// any cached versions of a remote resource.
    #[error("Offline mode is enabled but no cached versions of resouce exist ({0})")]
    NoCachedVersions(String),

    /// Arises when the cache is corrupted for some reason.
    ///
    /// If this error occurs, it is almost certainly the result of an external process
    /// "messing" with the cache directory, since `cached-path` takes great care
    /// to avoid accidental corruption on its own.
    #[error("Cache is corrupted ({0})")]
    CacheCorrupted(String),

    /// Arises when a resource is treated as archive, but the extraction process fails.
    #[error("Extracting archive failed ({0})")]
    ExtractionError(String),

    /// Any IO error that could arise while attempting to cache a remote resource.
    #[error("An IO error occurred")]
    IoError(#[from] std::io::Error),

    /// Arises when a bad HTTP status code is received while attempting to fetch
    /// a remote resource.
    #[error("HTTP response had status code {0}")]
    HttpStatusError(u16),

    /// Arises when an HTTP timeout error occurs while attempting to fetch a remote resource.
    #[error("HTTP response timeout out")]
    HttpTimeoutError,

    /// Arises when the HTTP client fails to build.
    #[error("HTTP builder error")]
    HttpBuilderError,

    /// Any other HTTP error that could occur while attempting to fetch a remote resource.
    #[error("An HTTP error occurred")]
    HttpError,
}

impl Error {
    pub(crate) fn is_retriable(&self) -> bool {
        match self {
            Error::HttpTimeoutError => true,
            Error::HttpStatusError(status_code) => match status_code {
                502 => true,
                503 => true,
                504 => true,
                _ => false,
            },
            _ => false,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        if err.is_status() {
            Error::HttpStatusError(err.status().unwrap().as_u16())
        } else if err.is_timeout() {
            Error::HttpTimeoutError
        } else if err.is_builder() {
            Error::HttpBuilderError
        } else {
            Error::HttpError
        }
    }
}
