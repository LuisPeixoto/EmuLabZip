//! The error type returned by every backend.

use std::io;

/// Errors from collectors. Messages are written for humans: the UI shows them as they are.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The ADB server or a device refused a request.
    #[error("adb: {0}")]
    Adb(String),
    /// `xcrun simctl` (or another Apple tool) failed.
    #[error("simctl: {0}")]
    Simctl(String),
    /// A required tool (`adb`, `xcrun`) is not installed or not on the PATH.
    #[error("{0} was not found. {1}")]
    ToolMissing(&'static str, &'static str),
    #[error("device `{0}` is not connected")]
    DeviceNotFound(String),
    #[error("{0}")]
    NotFound(String),
    /// The feature does not exist on this platform or host.
    #[error("{0} is not supported here")]
    Unsupported(&'static str),
    /// A tool printed output we did not understand.
    #[error("could not parse {what}: {detail}")]
    Parse { what: &'static str, detail: String },
    #[error("invalid {what}: `{value}`")]
    Invalid { what: &'static str, value: String },
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Error {
    pub fn parse(what: &'static str, detail: impl Into<String>) -> Self {
        Error::Parse { what, detail: detail.into() }
    }

    pub fn invalid(what: &'static str, value: impl Into<String>) -> Self {
        Error::Invalid { what, value: value.into() }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
