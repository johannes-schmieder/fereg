use std::error::Error;
use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, FeregError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Syntax,
    InvalidArgument,
    ProtocolMismatch,
    Unsupported,
    UnsupportedWeight,
    NegativeWeight,
    InvalidFrequencyWeight,
    InvalidFactor,
    InsufficientObservations,
    InvalidData,
    NonConvergence,
    Allocation,
    Interrupted,
    SelfTestFailed,
    Internal,
    Panic,
}

impl ErrorCode {
    #[must_use]
    pub const fn stata_return_code(self) -> i32 {
        match self {
            Self::Syntax => 198,
            Self::InvalidArgument | Self::Unsupported => 498,
            Self::UnsupportedWeight => 101,
            Self::ProtocolMismatch => 499,
            Self::NegativeWeight => 402,
            Self::InvalidFrequencyWeight => 401,
            Self::InvalidFactor => 452,
            Self::InsufficientObservations => 2001,
            Self::InvalidData => 459,
            Self::NonConvergence => 430,
            Self::Allocation => 909,
            Self::Interrupted => 1,
            Self::SelfTestFailed => 3498,
            Self::Internal | Self::Panic => 3499,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::InvalidArgument => "invalid_argument",
            Self::ProtocolMismatch => "protocol_mismatch",
            Self::Unsupported => "unsupported",
            Self::UnsupportedWeight => "unsupported_weight",
            Self::NegativeWeight => "negative_weight",
            Self::InvalidFrequencyWeight => "invalid_frequency_weight",
            Self::InvalidFactor => "invalid_factor",
            Self::InsufficientObservations => "insufficient_observations",
            Self::InvalidData => "invalid_data",
            Self::NonConvergence => "nonconvergence",
            Self::Allocation => "allocation",
            Self::Interrupted => "interrupted",
            Self::SelfTestFailed => "selftest_failed",
            Self::Internal => "internal",
            Self::Panic => "panic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeregError {
    pub code: ErrorCode,
    pub message: String,
}

impl FeregError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn return_code(&self) -> i32 {
        self.code.stata_return_code()
    }
}

/// Allocation, admission or capacity failure.
#[must_use]
pub fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

/// Invalid argument or inconsistent dimensions.
#[must_use]
pub fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

/// Invalid data value or numerical state.
#[must_use]
pub fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

/// Unsupported request.
#[must_use]
pub fn unsupported(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}

impl Display for FeregError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.label(), self.message)
    }
}

impl Error for FeregError {}

#[cfg(test)]
mod tests {
    use super::ErrorCode;

    #[test]
    fn pinned_estimation_domain_return_codes_are_distinct() {
        assert_eq!(ErrorCode::NegativeWeight.stata_return_code(), 402);
        assert_eq!(ErrorCode::InvalidFrequencyWeight.stata_return_code(), 401);
        assert_eq!(
            ErrorCode::InsufficientObservations.stata_return_code(),
            2001
        );
        assert_eq!(ErrorCode::InvalidData.stata_return_code(), 459);
        assert_eq!(ErrorCode::NonConvergence.stata_return_code(), 430);
        assert_eq!(ErrorCode::UnsupportedWeight.stata_return_code(), 101);
    }
}
