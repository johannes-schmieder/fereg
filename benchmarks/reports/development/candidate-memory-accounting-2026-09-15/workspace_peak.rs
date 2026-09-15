use crate::error::{ErrorCode, FeregError, Result};

/// Incremental workspace peak relative to a caller's execution model.
///
/// An unmodeled scope makes any containing peak unmodeled. In particular, an
/// omitted fallback must never turn into a reported zero-byte attempt. The
/// compact carrier keeps solver diagnostics at eight-byte alignment; planning
/// and admission continue to use checked u128 arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolverWorkspacePeak(u64);

impl SolverWorkspacePeak {
    pub(crate) const ZERO: Self = Self(0);
    pub(crate) const UNMODELED: Self = Self(u64::MAX);

    pub(crate) fn from_bytes(bytes: u128) -> Result<Self> {
        u64::try_from(bytes)
            .ok()
            .filter(|bytes| *bytes != u64::MAX)
            .map(Self)
            .ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "solver workspace peak is not addressable",
                )
            })
    }

    /// None means at least one scope has no complete workspace model.
    #[must_use]
    pub const fn bytes(self) -> Option<u64> {
        if self.0 == u64::MAX {
            None
        } else {
            Some(self.0)
        }
    }

    /// Sequential attempts share a peak envelope, not a sum of their storage.
    #[must_use]
    pub(crate) fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    /// Translate a nested peak to its caller's base while charging storage
    /// retained throughout that nested scope exactly once.
    pub(crate) fn with_retained(self, bytes: u128) -> Result<Self> {
        match self.bytes() {
            Some(peak) => Self::from_bytes((peak as u128).checked_add(bytes).ok_or_else(|| {
                FeregError::new(
                    ErrorCode::Allocation,
                    "retained solver workspace peak overflows",
                )
            })?),
            None => Ok(Self::UNMODELED),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_attempts_charge_maximum_with_resident_overlap_once() {
        let construction = SolverWorkspacePeak::from_bytes(140).unwrap();
        let discarded = SolverWorkspacePeak::from_bytes(400).unwrap();
        let fallback = SolverWorkspacePeak::from_bytes(200).unwrap();
        let nested = discarded.max(fallback).with_retained(80).unwrap();
        let whole = construction.max(nested);
        assert_eq!(whole.bytes(), Some(480));
        assert_eq!(fallback.max(discarded), discarded.max(fallback));
        assert_eq!(whole.with_retained(1_000).unwrap().bytes(), Some(1_480));
    }

    #[test]
    fn unmodeled_is_contagious_and_distinct_from_zero() {
        let zero = SolverWorkspacePeak::from_bytes(0).unwrap();
        assert_eq!(zero.bytes(), Some(0));
        for known in [zero, SolverWorkspacePeak::from_bytes(42).unwrap()] {
            assert_eq!(known.max(SolverWorkspacePeak::UNMODELED).bytes(), None);
            assert_eq!(SolverWorkspacePeak::UNMODELED.max(known).bytes(), None);
        }
        assert_eq!(
            SolverWorkspacePeak::UNMODELED
                .with_retained(42)
                .unwrap()
                .bytes(),
            None
        );
    }

    #[test]
    fn addressability_sentinel_and_arithmetic_overflow_fail_closed() {
        let largest = SolverWorkspacePeak::from_bytes((u64::MAX - 1) as u128).unwrap();
        assert_eq!(largest.bytes(), Some(u64::MAX - 1));
        for bytes in [u64::MAX as u128, u64::MAX as u128 + 1, u128::MAX] {
            assert_eq!(
                SolverWorkspacePeak::from_bytes(bytes).unwrap_err().code,
                ErrorCode::Allocation
            );
        }
        assert_eq!(
            largest.with_retained(1).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!(
            largest.with_retained(u128::MAX).unwrap_err().code,
            ErrorCode::Allocation
        );
    }

    #[test]
    fn diagnostic_carrier_does_not_raise_solver_report_alignment() {
        assert_eq!(std::mem::size_of::<SolverWorkspacePeak>(), 8);
        assert_eq!(std::mem::align_of::<SolverWorkspacePeak>(), 8);
        assert_eq!(std::mem::align_of::<crate::fe::SolveReport>(), 8);
    }
}
