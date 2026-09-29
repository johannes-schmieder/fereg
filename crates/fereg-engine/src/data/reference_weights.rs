//! Reference objective and score scales on one immutable final sample (SPEC §3).
use super::{PreparedWeights, WeightKind};
use crate::{
    error::{ErrorCode, FeregError, Result},
    spec::Estimator,
    threads::blocked_sum,
};

#[derive(Debug, Clone, Copy)]
pub struct ReferenceWeights<'a> {
    raw: Option<&'a [f64]>,
    kind: Option<WeightKind>,
    physical: usize,
    effective: usize,
    divisor: f64,
    multiplier: f64,
    pub raw_sum: Option<f64>,
    pub log_raw_sum: Option<f64>,
}
impl<'a> ReferenceWeights<'a> {
    pub fn new(raw: &'a PreparedWeights, n: usize, estimator: Estimator) -> Result<Self> {
        exact_count(n)?;
        if n == 0 {
            return Err(invalid("reference weights require a nonempty sample"));
        }
        let (kind, values) = match raw {
            PreparedWeights::None => (None, None),
            PreparedWeights::Provided { kind, values } => (Some(*kind), Some(values.as_slice())),
        };
        if kind == Some(WeightKind::Importance)
            || (estimator == Estimator::Ppml && kind == Some(WeightKind::Analytic))
        {
            return Err(FeregError::new(
                ErrorCode::UnsupportedWeight,
                "unsupported estimator weight kind",
            ));
        }
        let mut result = Self {
            raw: values,
            kind,
            physical: n,
            effective: n,
            divisor: 1.,
            multiplier: 1.,
            raw_sum: None,
            log_raw_sum: None,
        };
        let Some(values) = values else {
            return Ok(result);
        };
        if values.len() != n || values.iter().any(|x| !x.is_finite() || *x <= 0.) {
            return Err(invalid(
                "reference weights are not positive, finite and sample aligned",
            ));
        }
        if kind == Some(WeightKind::Frequency) {
            let mut count = 0usize;
            for &value in values {
                if value.fract() != 0. || value > u32::MAX as f64 {
                    return Err(FeregError::new(
                        ErrorCode::InvalidFrequencyWeight,
                        "invalid frequency multiplicity",
                    ));
                }
                count = count
                    .checked_add(value as usize)
                    .ok_or_else(|| invalid("frequency sum overflows"))?;
            }
            result.effective = count;
            result.raw_sum = Some(exact_count(count)?);
            result.log_raw_sum = Some((count as f64).ln());
        } else {
            result.divisor = values.iter().copied().fold(0., f64::max);
            let scaled = blocked_sum(n, |i| values[i] / result.divisor);
            result.multiplier = n as f64 / scaled;
            let sum = result.divisor * scaled;
            result.raw_sum = sum.is_finite().then_some(sum);
            result.log_raw_sum = Some(result.divisor.ln() + scaled.ln());
            let first_sum = blocked_sum(n, |i| result.value(i));
            result.multiplier *= n as f64 / first_sum;
            if values.iter().enumerate().any(|(i, _)| {
                let value = result.value(i);
                !value.is_finite() || value <= 0.
            }) || (blocked_sum(n, |i| result.value(i)) - n as f64).abs()
                > 64. * f64::EPSILON * n as f64
            {
                return Err(FeregError::new(
                    ErrorCode::NonConvergence,
                    "reference weights are not representable on the computational scale",
                ));
            }
        }
        Ok(result)
    }
    #[must_use]
    pub fn value(self, row: usize) -> f64 {
        assert!(row < self.physical);
        self.raw.map_or(1., |v| {
            let scaled = (v[row] * self.multiplier) / self.divisor;
            if scaled.is_finite() && scaled >= f64::MIN_POSITIVE {
                scaled
            } else {
                crate::linalg::scale_ratio(v[row], self.multiplier, self.divisor, 1.)
            }
        })
    }
    pub fn mean(self, values: &[f64]) -> Result<f64> {
        if values.len() != self.physical {
            return Err(invalid("weighted mean and sample lengths differ"));
        }
        self.mean_by(|row| values[row])
    }
    pub fn mean_by(self, value: impl Fn(usize) -> f64) -> Result<f64> {
        let mut scale = 0_f64;
        for row in 0..self.physical {
            let value = value(row);
            if !value.is_finite() {
                return Err(invalid("weighted mean contains a nonfinite value"));
            }
            scale = scale.max(value.abs());
        }
        if scale == 0. {
            return Ok(0.);
        }
        let sum_w = blocked_sum(self.physical, |row| self.value(row));
        let mean = blocked_sum(self.physical, |row| {
            self.value(row) / sum_w * (value(row) / scale)
        }) * scale;
        if mean.is_finite() {
            Ok(mean)
        } else {
            Err(invalid("weighted mean is not representable"))
        }
    }
    /// HC meat treats a frequency row as independent repeated observations.
    #[must_use]
    pub fn robust_root(self, row: usize) -> f64 {
        let value = self.value(row);
        if self.kind == Some(WeightKind::Frequency) {
            value.sqrt()
        } else {
            value
        }
    }
    #[must_use]
    pub fn kind(self) -> Option<WeightKind> {
        self.kind
    }
    #[must_use]
    pub fn nobs(self) -> usize {
        self.physical
    }
    #[must_use]
    pub fn effective(self) -> usize {
        self.effective
    }
    #[must_use]
    pub fn log_multiplier(self) -> f64 {
        self.multiplier.ln() - self.divisor.ln()
    }
}
pub(crate) fn exact_count(n: usize) -> Result<f64> {
    if (n as u128) > (1u128 << 53) {
        return Err(invalid("count exceeds exact floating-point integer range"));
    }
    Ok(n as f64)
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}
