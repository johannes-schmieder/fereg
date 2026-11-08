//! The weighted FE moment certificate of a residual.

use crate::error::{Error, Result};

/// Weighted fixed-effect orthogonality certificate of returned residuals.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Certificate {
    /// Largest raw level moment `|sum_{i in level} w_i r_i|` over levels and
    /// columns (saturates to infinity when not representable).
    pub absolute_infinity_norm: f64,
    /// Largest level moment relative to `sqrt(level mass)` and the column
    /// scale; the certified quantity, compared with the tolerance.
    pub relative_infinity_norm: f64,
    /// Root-mean-square of the relative level moments.
    pub relative_two_norm: f64,
    pub passed: bool,
}

/// Running FE moment certificate, folded level-major and RHS-minor.
#[derive(Default)]
pub(crate) struct MomentFold {
    pub(crate) absolute_infinity_norm: f64,
    pub(crate) relative_infinity_norm: f64,
    pub(crate) relative_square_sum: f64,
    pub(crate) relative_count: usize,
}

impl MomentFold {
    /// Fold one level's group means, one per RHS, at its group weight.
    pub(crate) fn level(
        &mut self,
        group_weight: f64,
        means: &[f64],
        rhs_scales: &[f64],
        rhs_relative_infinity_norms: &mut [f64],
        raw_scale: f64,
    ) -> Result<()> {
        let root_group_weight = group_weight.sqrt();
        if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
            return Err(Error::InvalidData(
                "borrowed MAP certificate group weight is invalid",
            ));
        }
        for (rhs, &mean) in means.iter().enumerate() {
            let denominator = rhs_scales[rhs];
            let relative = if denominator == 0.0 {
                if mean == 0.0 {
                    0.0
                } else {
                    return Err(Error::InvalidData(
                        "borrowed MAP zero-scale certificate has a nonzero moment",
                    ));
                }
            } else {
                (mean.abs() / denominator) * root_group_weight
            };
            if !relative.is_finite() {
                return Err(Error::InvalidData(
                    "borrowed MAP certificate ratio is not finite",
                ));
            }
            self.relative_infinity_norm = self.relative_infinity_norm.max(relative);
            rhs_relative_infinity_norms[rhs] = rhs_relative_infinity_norms[rhs].max(relative);
            // Exactly rounded, as the host's hardware entry for the same sum.
            self.relative_square_sum = relative.mul_add(relative, self.relative_square_sum);
            if !self.relative_square_sum.is_finite() {
                return Err(Error::InvalidData(
                    "borrowed MAP certificate two-norm accumulation is not finite",
                ));
            }
            self.relative_count = self
                .relative_count
                .checked_add(1)
                .ok_or(Error::OutOfMemory(
                    "borrowed MAP certificate moment count overflows",
                ))?;
            let normalized_moment = mean.abs() * group_weight;
            let raw_moment = normalized_moment * raw_scale;
            self.absolute_infinity_norm =
                self.absolute_infinity_norm.max(if raw_moment.is_finite() {
                    raw_moment
                } else if normalized_moment == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                });
        }
        Ok(())
    }

    pub(crate) fn merge(&mut self, part: &Self) {
        self.absolute_infinity_norm = self.absolute_infinity_norm.max(part.absolute_infinity_norm);
        self.relative_infinity_norm = self.relative_infinity_norm.max(part.relative_infinity_norm);
        self.relative_square_sum += part.relative_square_sum;
        self.relative_count += part.relative_count;
    }

    pub(crate) fn finish(self, tolerance: f64) -> Result<Certificate> {
        let relative_two_norm = if self.relative_count == 0 {
            0.0
        } else {
            (self.relative_square_sum / self.relative_count as f64).sqrt()
        };
        if !relative_two_norm.is_finite() {
            return Err(Error::InvalidData(
                "borrowed MAP certificate two norm is not finite",
            ));
        }
        Ok(Certificate {
            absolute_infinity_norm: self.absolute_infinity_norm,
            relative_infinity_norm: self.relative_infinity_norm,
            relative_two_norm,
            passed: self.relative_infinity_norm <= tolerance,
        })
    }
}
