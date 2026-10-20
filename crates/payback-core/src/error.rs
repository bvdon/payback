use std::fmt;

/// Every fallible operation in the core returns this error. It is shown to the
/// user as-is, so messages are written for people, not developers.
#[derive(Debug)]
pub enum Error {
    /// A record the caller referred to does not exist.
    NotFound(String),
    /// The request was well-formed but the values were not acceptable.
    Invalid(String),
    /// The request clashes with existing data.
    Conflict(String),
    Io(std::io::Error),
    /// Malformed JSON, or JSON of the wrong shape.
    Json(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Error::Invalid(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Error::Conflict(msg.into())
    }

    /// A short machine-readable category for the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "not_found",
            Error::Invalid(_) => "invalid",
            Error::Conflict(_) => "conflict",
            Error::Io(_) => "io",
            Error::Json(_) => "invalid",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(m) | Error::Invalid(m) | Error::Conflict(m) | Error::Json(m) => f.write_str(m),
            Error::Io(e) => write!(f, "File error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
