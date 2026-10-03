use crate::error::{ErrorCode, FeregError, Result};

/// Incremental workspace peak relative to a caller's execution model.
///
/// An unmodeled scope makes any containing peak unmodeled. In particular, an
/// omitted fallback must never turn into a reported zero-byte attempt. The
/// Admission and expected heap use travel independently through the same
/// lifetime composition. Neither channel reads the allocator observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolverWorkspacePeak {
    admitted: u64,
    expected_heap: u64,
}

impl SolverWorkspacePeak {
    pub(crate) const ZERO: Self = Self {
        admitted: 0,
        expected_heap: 0,
    };
    pub(crate) const UNMODELED: Self = Self {
        admitted: u64::MAX,
        expected_heap: u64::MAX,
    };

    pub(crate) fn from_bytes(bytes: u128) -> Result<Self> {
        u64::try_from(bytes)
            .ok()
            .filter(|bytes| *bytes != u64::MAX)
            .map(|bytes| Self {
                admitted: bytes,
                expected_heap: bytes,
            })
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
        if self.admitted == u64::MAX {
            None
        } else {
            Some(self.admitted)
        }
    }

    /// Engineering forecast, separate from admission and opaque runtime headroom.
    pub const fn expected_heap_bytes(self) -> Option<u64> {
        if self.expected_heap == u64::MAX {
            None
        } else {
            Some(self.expected_heap)
        }
    }

    pub(crate) fn with_expected_heap(mut self, bytes: Option<u128>) -> Result<Self> {
        self.expected_heap = match bytes {
            Some(bytes) => Self::from_bytes(bytes)?.admitted,
            None => u64::MAX,
        };
        Ok(self)
    }

    /// Sequential attempts share a peak envelope, not a sum of their storage.
    #[must_use]
    pub(crate) fn max(self, other: Self) -> Self {
        Self {
            admitted: self.admitted.max(other.admitted),
            expected_heap: self.expected_heap.max(other.expected_heap),
        }
    }

    /// Translate a nested peak to its caller's base while charging storage
    /// retained throughout that nested scope exactly once.
    pub(crate) fn with_retained(self, bytes: u128) -> Result<Self> {
        self.with_retained_peak(Self::from_bytes(bytes)?)
    }

    /// Preserve each parent's expected heap instead of translating through its
    /// admission bytes (which may include stacks and worst-case reservations).
    pub(crate) fn with_retained_peak(self, parent: Self) -> Result<Self> {
        let add = |a: u64, b: u64| -> Result<u64> {
            if a == u64::MAX || b == u64::MAX {
                return Ok(u64::MAX);
            }
            Ok(Self::from_bytes(a as u128 + b as u128)?.admitted)
        };
        Ok(Self {
            admitted: add(self.admitted, parent.admitted)?,
            expected_heap: add(self.expected_heap, parent.expected_heap)?,
        })
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
        assert_eq!(std::mem::size_of::<SolverWorkspacePeak>(), 16);
        assert_eq!(std::mem::align_of::<SolverWorkspacePeak>(), 8);
        assert_eq!(std::mem::align_of::<crate::fe::SolveReport>(), 8);
    }

    #[test]
    fn forecasts_preserve_discarded_attempts_and_independent_parent_peaks() {
        let attempt = SolverWorkspacePeak::from_bytes(1000)
            .unwrap()
            .with_expected_heap(Some(30))
            .unwrap();
        let fallback = SolverWorkspacePeak::from_bytes(100).unwrap();
        let parent = SolverWorkspacePeak::from_bytes(500)
            .unwrap()
            .with_expected_heap(Some(20))
            .unwrap();
        let peak = attempt.max(fallback).with_retained_peak(parent).unwrap();
        assert_eq!(peak.bytes(), Some(1500));
        assert_eq!(peak.expected_heap_bytes(), Some(120));
        let unknown = attempt.with_expected_heap(None).unwrap().max(fallback);
        assert_eq!(unknown.bytes(), Some(1000));
        assert_eq!(unknown.expected_heap_bytes(), None);
        assert_eq!(
            unknown.with_retained(2).unwrap().expected_heap_bytes(),
            None
        );
    }
}
