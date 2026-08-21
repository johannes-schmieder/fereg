use crate::error::{ErrorCode, Result, StFixestError};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMode {
    Auto,
    Fast,
    Low,
}

impl MemoryMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "fast" => Ok(Self::Fast),
            "low" => Ok(Self::Low),
            _ => Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                format!("memory() must be auto, fast, or low; received {value:?}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThreadConfig {
    pub requested: Option<NonZeroUsize>,
    pub deterministic: bool,
}

impl ThreadConfig {
    pub fn from_requested(requested: usize, deterministic: bool) -> Result<Self> {
        if requested == 0 {
            return Ok(Self {
                requested: None,
                deterministic,
            });
        }
        let requested = NonZeroUsize::new(requested).ok_or(StFixestError::new(
            ErrorCode::InvalidArgument,
            "threads() must be positive",
        ))?;
        Ok(Self {
            requested: Some(requested),
            deterministic,
        })
    }

    #[must_use]
    pub fn available() -> usize {
        std::thread::available_parallelism()
            .map(NonZeroUsize::get)
            .unwrap_or(1)
    }

    #[must_use]
    pub fn resolved(self) -> usize {
        self.requested
            .map(NonZeroUsize::get)
            .unwrap_or_else(Self::available)
            .max(1)
    }
}
