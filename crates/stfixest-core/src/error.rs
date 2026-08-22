use std::error::Error;
use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, StFixestError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Syntax,
    InvalidArgument,
    ProtocolMismatch,
    Unsupported,
    NegativeWeight,
    InvalidFrequencyWeight,
    InsufficientObservations,
    InvalidData,
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
            Self::ProtocolMismatch => 499,
            Self::NegativeWeight => 402,
            Self::InvalidFrequencyWeight => 401,
            Self::InsufficientObservations => 2001,
            Self::InvalidData => 459,
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
            Self::NegativeWeight => "negative_weight",
            Self::InvalidFrequencyWeight => "invalid_frequency_weight",
            Self::InsufficientObservations => "insufficient_observations",
            Self::InvalidData => "invalid_data",
            Self::Allocation => "allocation",
            Self::Interrupted => "interrupted",
            Self::SelfTestFailed => "selftest_failed",
            Self::Internal => "internal",
            Self::Panic => "panic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StFixestError {
    pub code: ErrorCode,
    pub message: String,
}

impl StFixestError {
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

impl Display for StFixestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.label(), self.message)
    }
}

impl Error for StFixestError {}

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
    }
}
