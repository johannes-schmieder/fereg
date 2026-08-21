use crate::error::{ErrorCode, Result, StFixestError};

/// Compressed observation membership for one categorical effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelIndex {
    offsets: Vec<usize>,
    observations: Vec<usize>,
}

impl LevelIndex {
    pub fn from_codes(codes: &[u32], n_levels: usize) -> Result<Self> {
        let mut counts = vec![0_usize; n_levels];
        for code in codes {
            let level = *code as usize;
            if level >= n_levels {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    format!("fixed-effect code {level} exceeds level count {n_levels}"),
                ));
            }
            counts[level] += 1;
        }
        let mut offsets = vec![0_usize; n_levels + 1];
        for level in 0..n_levels {
            offsets[level + 1] = offsets[level] + counts[level];
        }
        let mut next = offsets[..n_levels].to_vec();
        let mut observations = vec![0_usize; codes.len()];
        for (observation, code) in codes.iter().enumerate() {
            let level = *code as usize;
            observations[next[level]] = observation;
            next[level] += 1;
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
}
