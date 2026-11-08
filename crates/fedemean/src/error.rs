//! Error outcomes. A decline (the route does not apply) is not an error; see
//! [`crate::Outcome`].

use std::fmt;

/// A failed projection. On any error the contents of the borrowed columns and
/// coefficient buffers are unspecified and must be discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The caller's cancellation callback asked to stop.
    Cancelled,
    /// A fallible allocation failed.
    OutOfMemory(&'static str),
    /// Inconsistent dimensions or arguments.
    InvalidInput(&'static str),
    /// Invalid values (weights, nonfinite data) or numerical state.
    InvalidData(&'static str),
    /// A combination of choices that this build does not implement.
    Unsupported(&'static str),
}

impl Error {
    /// The message without the category.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::Cancelled => "projection was cancelled",
            Self::OutOfMemory(m)
            | Self::InvalidInput(m)
            | Self::InvalidData(m)
            | Self::Unsupported(m) => m,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Error {}

pub(crate) type Result<T> = std::result::Result<T, Error>;
