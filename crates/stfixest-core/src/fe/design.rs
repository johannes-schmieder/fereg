use crate::error::{ErrorCode, Result, StFixestError};

trait LevelIndexReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemReservation;

impl LevelIndexReservation for SystemReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

/// Compressed observation membership for one categorical effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelIndex {
    offsets: Vec<usize>,
    observations: Vec<usize>,
}

impl LevelIndex {
    pub fn from_codes(codes: &[u32], n_levels: usize) -> Result<Self> {
        let mut reservation = SystemReservation;
        Self::from_codes_with_reservation(codes, n_levels, &mut reservation)
    }

    fn from_codes_with_reservation<R: LevelIndexReservation>(
        codes: &[u32],
        n_levels: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let offset_count = n_levels
            .checked_add(1)
            .ok_or_else(|| allocation("fixed-effect level-index offset count overflows"))?;
        let mut counts = reserved_filled(
            reservation,
            n_levels,
            0_usize,
            "unable to reserve fixed-effect level counts",
        )?;
        for code in codes {
            let level = *code as usize;
            if level >= n_levels {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    format!("fixed-effect code {level} exceeds level count {n_levels}"),
                ));
            }
            counts[level] = counts[level]
                .checked_add(1)
                .ok_or_else(|| allocation("fixed-effect level membership count overflows"))?;
        }
        let mut offsets = reserved_filled(
            reservation,
            offset_count,
            0_usize,
            "unable to reserve fixed-effect level offsets",
        )?;
        for level in 0..n_levels {
            offsets[level + 1] = offsets[level]
                .checked_add(counts[level])
                .ok_or_else(|| allocation("fixed-effect cumulative membership count overflows"))?;
        }
        if offsets[n_levels] != codes.len() {
            return Err(StFixestError::new(
                ErrorCode::Internal,
                "fixed-effect level membership count is inconsistent with codes",
            ));
        }
        let mut next = Vec::new();
        reservation.reserve_vec(
            &mut next,
            n_levels,
            "unable to reserve fixed-effect level cursors",
        )?;
        next.extend_from_slice(&offsets[..n_levels]);
        let mut observations = reserved_filled(
            reservation,
            codes.len(),
            0_usize,
            "unable to reserve fixed-effect level observations",
        )?;
        for (observation, code) in codes.iter().enumerate() {
            let level = *code as usize;
            let destination = next[level];
            if destination >= observations.len() {
                return Err(StFixestError::new(
                    ErrorCode::Internal,
                    "fixed-effect level cursor exceeds membership storage",
                ));
            }
            observations[destination] = observation;
            next[level] = destination
                .checked_add(1)
                .ok_or_else(|| allocation("fixed-effect level cursor overflows"))?;
        }
        Ok(Self {
            offsets,
            observations,
        })
    }

    #[must_use]
    pub fn members(&self, level: usize) -> &[usize] {
        &self.observations[self.offsets[level]..self.offsets[level + 1]]
    }

    #[must_use]
    pub fn n_levels(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    #[must_use]
    pub fn nobs(&self) -> usize {
        self.observations.len()
    }

    pub fn checked_members(&self, level: usize) -> Result<&[usize]> {
        let start = self.offsets.get(level).copied().ok_or_else(|| {
            StFixestError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect level index is out of range",
            )
        })?;
        let end = self.offsets.get(level + 1).copied().ok_or_else(|| {
            StFixestError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect level index is out of range",
            )
        })?;
        self.observations.get(start..end).ok_or_else(|| {
            StFixestError::new(
                ErrorCode::Internal,
                "fixed-effect membership offsets are inconsistent",
            )
        })
    }
}

fn reserved_filled<T: Clone, R: LevelIndexReservation>(
    reservation: &mut R,
    elements: usize,
    value: T,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve_vec(&mut output, elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

fn allocation(message: &'static str) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}

/// One categorical fixed-effect term with optional group-specific slopes.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectTerm {
    codes: Vec<u32>,
    n_levels: usize,
    intercept: bool,
    slopes: Vec<Vec<f64>>,
    level_index: LevelIndex,
}

impl EffectTerm {
    pub fn new(
        codes: Vec<u32>,
        n_levels: usize,
        intercept: bool,
        slopes: Vec<Vec<f64>>,
    ) -> Result<Self> {
        if n_levels == 0 {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect term must contain at least one level",
            ));
        }
        if !intercept && slopes.is_empty() {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect term must contain an intercept or slope",
            ));
        }
        if slopes.iter().any(|slope| slope.len() != codes.len()) {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect slope length does not match codes",
            ));
        }
        if slopes
            .iter()
            .flat_map(|slope| slope.iter())
            .any(|value| !value.is_finite())
        {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "fixed-effect slopes must be finite",
            ));
        }
        let level_index = LevelIndex::from_codes(&codes, n_levels)?;
        Ok(Self {
            codes,
            n_levels,
            intercept,
            slopes,
            level_index,
        })
    }

    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub const fn has_intercept(&self) -> bool {
        self.intercept
    }

    #[must_use]
    pub fn slopes(&self) -> &[Vec<f64>] {
        &self.slopes
    }

    #[must_use]
    pub fn level_index(&self) -> &LevelIndex {
        &self.level_index
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        (if self.intercept { 1 } else { 0 }) + self.slopes.len()
    }

    #[must_use]
    pub fn basis_value(&self, channel: usize, observation: usize) -> f64 {
        if self.intercept {
            if channel == 0 {
                1.0
            } else {
                self.slopes[channel - 1][observation]
            }
        } else {
            self.slopes[channel][observation]
        }
    }
}

/// Validated collection of fixed-effect terms sharing one observation axis.
#[derive(Debug, Clone, PartialEq)]
pub struct FeDesign {
    nobs: usize,
    effects: Vec<EffectTerm>,
}

impl FeDesign {
    pub fn new(nobs: usize, effects: Vec<EffectTerm>) -> Result<Self> {
        if effects.iter().any(|effect| effect.codes().len() != nobs) {
            return Err(StFixestError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect terms do not share the declared observation count",
            ));
        }
        Ok(Self { nobs, effects })
    }

    #[must_use]
    pub const fn nobs(&self) -> usize {
        self.nobs
    }

    #[must_use]
    pub fn effects(&self) -> &[EffectTerm] {
        &self.effects
    }

    #[must_use]
    pub fn max_channels(&self) -> usize {
        self.effects
            .iter()
            .map(EffectTerm::channels)
            .max()
            .unwrap_or(0)
    }

    /// Total coefficient slots before identification normalization.
    #[must_use]
    pub fn total_coefficients(&self) -> usize {
        self.effects
            .iter()
            .map(|effect| effect.n_levels().saturating_mul(effect.channels()))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct InjectedReservation {
        fail_at: Option<usize>,
        calls: usize,
    }

    impl LevelIndexReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected fixed-effect level-index allocation failure",
                ));
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test fixed-effect level-index reservation failed"))
        }
    }

    #[test]
    fn level_index_membership_is_exact_and_empty_safe() {
        let index = LevelIndex::from_codes(&[1, 0, 1, 2, 0], 3).expect("level index");
        assert_eq!(index.n_levels(), 3);
        assert_eq!(index.nobs(), 5);
        assert_eq!(index.checked_members(0).expect("level zero"), &[1, 4]);
        assert_eq!(index.checked_members(1).expect("level one"), &[0, 2]);
        assert_eq!(index.checked_members(2).expect("level two"), &[3]);
        assert_eq!(
            index.checked_members(3).expect_err("out of range").code,
            ErrorCode::InvalidArgument
        );

        let empty = LevelIndex::from_codes(&[], 0).expect("empty level index");
        assert_eq!(empty.n_levels(), 0);
        assert_eq!(empty.nobs(), 0);
    }

    #[test]
    fn every_level_index_reservation_fails_closed() {
        let codes = [1, 0, 1, 2, 0];
        let mut successful = InjectedReservation::default();
        LevelIndex::from_codes_with_reservation(&codes, 3, &mut successful)
            .expect("record reservations");
        assert_eq!(successful.calls, 4);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                calls: 0,
            };
            let error = LevelIndex::from_codes_with_reservation(&codes, 3, &mut injected)
                .expect_err("injected reservation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
    }

    #[test]
    fn level_index_rejects_out_of_range_codes() {
        assert_eq!(
            LevelIndex::from_codes(&[0, 2], 2)
                .expect_err("out-of-range code")
                .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            LevelIndex::from_codes(&[], usize::MAX)
                .expect_err("offset-count overflow")
                .code,
            ErrorCode::Allocation
        );
    }
}
