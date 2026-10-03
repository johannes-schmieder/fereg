use crate::error::{ErrorCode, FeregError, Result};
use std::num::{NonZeroU16, NonZeroUsize};

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
            _ => Err(FeregError::new(
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
    // Fits in the descriptor's existing padding on supported targets. Keep
    // request identity separate from the admitted execution ceiling.
    ceiling: Option<NonZeroU16>,
}

impl ThreadConfig {
    pub fn from_requested(requested: usize, deterministic: bool) -> Result<Self> {
        if requested == 0 {
            return Ok(Self {
                requested: None,
                deterministic,
                ceiling: None,
            });
        }
        let requested = NonZeroUsize::new(requested).ok_or(FeregError::new(
            ErrorCode::InvalidArgument,
            "threads() must be positive",
        ))?;
        Ok(Self {
            requested: Some(requested),
            deterministic,
            ceiling: None,
        })
    }

    #[must_use]
    pub fn available() -> usize {
        std::thread::available_parallelism()
            .map(NonZeroUsize::get)
            .unwrap_or(1)
    }

    /// Lower the execution ceiling without rewriting the user's request.
    #[must_use]
    pub fn with_ceiling(mut self, maximum: NonZeroU16) -> Self {
        self.ceiling = Some(self.ceiling.map_or(maximum, |old| old.min(maximum)));
        self
    }

    #[must_use]
    pub fn resolved(self) -> usize {
        self.requested
            .map(NonZeroUsize::get)
            .unwrap_or_else(Self::available)
            .max(1)
            .min(
                self.ceiling
                    .map_or(usize::MAX, |limit| usize::from(limit.get())),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_ceiling_preserves_request_and_descriptor_size() {
        assert_eq!(
            std::mem::size_of::<ThreadConfig>(),
            2 * std::mem::size_of::<usize>()
        );
        let eight = NonZeroU16::new(8).unwrap();
        for requested in [0, 1, 4, 8, 16, 28] {
            for deterministic in [false, true] {
                let original = ThreadConfig::from_requested(requested, deterministic).unwrap();
                let capped = original.with_ceiling(eight);
                assert_eq!(capped.requested, original.requested);
                assert_eq!(capped.deterministic, deterministic);
                assert_eq!(capped.resolved(), original.resolved().min(8));
                assert_eq!(
                    capped.with_ceiling(NonZeroU16::new(16).unwrap()).resolved(),
                    capped.resolved()
                );
                assert_eq!(
                    capped.with_ceiling(NonZeroU16::new(1).unwrap()).resolved(),
                    1
                );
            }
        }
    }
}
