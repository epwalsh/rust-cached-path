use failure::{Backtrace, Context, Fail};
use std::fmt;
use std::io;

/// Any error that can occur during caching.
#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

/// Error kinds that can occur during caching.
#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    /// Arises when the resource looks like a local file but it doesn't exist.
    #[fail(
        display = "Treated resource as local file, but file does not exist ({})",
        _0
    )]
    ResourceNotFound(String),

    /// Arises when the resource looks like a URL, but is invalid.
    #[fail(display = "Unable to parse resource URL ({})", _0)]
    InvalidUrl(String),

    /// Arises when the cache is being used in offline mode, but it couldn't locate
    /// any cached versions of a remote resource.
    #[fail(
        display = "Offline mode is enabled but no cached versions of resouce exist ({})",
        _0
    )]
    NoCachedVersions(String),

    /// Any IO error that could arise while attempting to cache a remote resource.
    #[fail(display = "An IO error occurred: {:?}", _0)]
    IoError(Option<io::ErrorKind>),

    /// Arises when a bad HTTP status code is received while attempting to fetch
    /// a remote resource.
    #[fail(display = "HTTP response had status code {}", _0)]
    HttpStatusError(u16),

    /// Arises when an HTTP timeout error occurs while attempting to fetch a remote resource.
    #[fail(display = "HTTP response timeout out")]
    HttpTimeoutError,

    /// Arises when the HTTP client fails to build.
    #[fail(display = "HTTP builder error")]
    HttpBuilderError,

    /// Any other HTTP error that could occur while attempting to fetch a remote resource.
    #[fail(display = "An HTTP error occurred")]
    HttpError,
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        self.inner.get_context().clone()
    }

    pub(crate) fn is_retriable(&self) -> bool {
        match self.inner.get_context() {
            ErrorKind::HttpTimeoutError => true,
            ErrorKind::HttpStatusError(status_code) => match status_code {
                502 => true,
                503 => true,
                504 => true,
                _ => false,
            },
            _ => false,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error {
            inner: Context::new(ErrorKind::IoError(Some(err.kind()))),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        if err.is_status() {
            Error {
                inner: Context::new(ErrorKind::HttpStatusError(err.status().unwrap().as_u16())),
            }
        } else if err.is_timeout() {
            Error {
                inner: Context::new(ErrorKind::HttpTimeoutError),
            }
        } else if err.is_builder() {
            Error {
                inner: Context::new(ErrorKind::HttpBuilderError),
            }
        } else {
            Error {
                inner: Context::new(ErrorKind::HttpError),
            }
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}
