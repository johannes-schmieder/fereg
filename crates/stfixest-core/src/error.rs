use std::error::Error;
use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, StFixestError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Syntax,
    InvalidArgument,
    ProtocolMismatch,
    Unsupported,
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
