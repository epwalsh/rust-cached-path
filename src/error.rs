use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    FileNotFound(String),
    InvalidUrl(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::FileNotFound(ref path) => write!(
                f,
                "resource treated as local file but file doesn't exist ({})",
                path
            ),
            Error::InvalidUrl(ref url) => write!(f, "invalid URL ({})", url),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}
