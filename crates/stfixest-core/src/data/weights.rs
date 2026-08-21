use crate::error::{ErrorCode, Result, StFixestError};

/// Stata weight type carried into the native estimator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightKind {
    None,
    Frequency,
    Analytic,
    Probability,
    Importance,
}

impl WeightKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Frequency => "fweight",
            Self::Analytic => "aweight",
            Self::Probability => "pweight",
            Self::Importance => "iweight",
        }
    }
}

/// Validate one nonmissing Stata weight and return its native value.
///
/// Missing weights are handled by sample construction before this function is
/// called. All supported weight types must be strictly positive. Frequency
/// weights additionally must be integer-valued and exactly representable as a
/// nonnegative `u64`.
pub fn validate_weight(kind: WeightKind, value: f64) -> Result<f64> {
    if kind == WeightKind::None {
        return Ok(1.0);
    }
    if !value.is_finite() || value <= 0.0 {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            format!("{} must be finite and strictly positive", kind.label()),
        ));
    }
    if kind == WeightKind::Frequency && (value.fract() != 0.0 || value > u64::MAX as f64) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "frequency weights must be positive integers representable as u64",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_supported_weights() {
        assert_eq!(validate_weight(WeightKind::None, f64::NAN), Ok(1.0));
        assert_eq!(validate_weight(WeightKind::Frequency, 3.0), Ok(3.0));
        assert!(validate_weight(WeightKind::Frequency, 1.5).is_err());
        assert!(validate_weight(WeightKind::Analytic, 0.0).is_err());
        assert!(validate_weight(WeightKind::Probability, f64::INFINITY).is_err());
    }
}
