//! Coordinate system of the pinned PPML multiway CGM repair.
//! This is an inference convention; the estimated coefficients and FE
//! normalization continue to use the estimator's own certified basis.

use super::family::CompensatedSum;
use super::prepared_irls::PreparedPpmlIrlsFit;
use super::prepared_model::PreparedPpmlModelGeneration;
use super::psd::{inspect_published_ppml_covariance, PpmlPsdReport};
use crate::error::{allocation, invalid_argument, ErrorCode, FeregError, Result};
use crate::fe::BorrowedMapWeights;
use crate::memory::fallible_filled;
use crate::ols::prepared_fit_view::visit_prepared_column_range;
use crate::ols::PreparedOlsColumnKind;

const SCAN_ROWS: usize = 65_536;

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML reference covariance arithmetic is not finite",
        ))
    }
}

/// Map unweighted sample SDs to Stata's original slots before numerical
/// separation. Later generations can drop rows, levels and dependent columns.
pub(super) fn column_scales(
    generation: &PreparedPpmlModelGeneration,
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<Vec<f64>> {
    let model = generation.model();
    let rows = generation.outcome().len();
    let mut scales = fallible_filled(
        model.coefficients.len(),
        1.0,
        "unable to reserve PPML regressor scales",
    )?;
    // Leave the structural residual DoF check authoritative for a one-row fit.
    if rows <= 1 {
        return Ok(scales);
    }
    let data = generation.fit_view();
    for (generated, column) in model.columns.iter().enumerate() {
        poll()?;
        let sd = match column.kind {
            PreparedOlsColumnKind::Intercept => 1.0,
            PreparedOlsColumnKind::Continuous {
                prepared_source_index,
                ..
            } => sample_sd(
                data.sample()
                    .regressors
                    .get(prepared_source_index)
                    .ok_or_else(|| {
                        invalid_argument("PPML continuous covariance source is unavailable")
                    })?,
                poll,
            )?,
            _ => {
                let mut ones = 0_usize;
                for start in (0..rows).step_by(SCAN_ROWS) {
                    poll()?;
                    visit_prepared_column_range(
                        data,
                        generated,
                        start..(start + SCAN_ROWS).min(rows),
                        |_, x| {
                            ones += usize::from(x == 1.0);
                            Ok(())
                        },
                    )?;
                }
                ((ones as f64 / rows as f64) * ((rows - ones) as f64 / (rows - 1) as f64)).sqrt()
            }
        };
        scales[column.coefficient_slot.get() as usize - 1] = finite(sd)?.max(1e-3);
    }
    Ok(scales)
}

fn sample_sd(values: &[f64], poll: &mut dyn FnMut() -> Result<()>) -> Result<f64> {
    // Pin the statistical convention, while avoiding cancellation in E[x²]−E[x]².
    // A power-of-two scale keeps the centered differences representable.
    let mut magnitude = 0.0_f64;
    for chunk in values.chunks(SCAN_ROWS) {
        poll()?;
        for &v in chunk {
            magnitude = magnitude.max(v.abs());
        }
    }
    if magnitude == 0.0 {
        return Ok(0.0);
    }
    let scale = f64::from_bits(magnitude.to_bits() & (0x7ff_u64 << 52)).max(f64::MIN_POSITIVE);
    let anchor = values[0] / scale;
    let mut sum = CompensatedSum::default();
    for chunk in values.chunks(SCAN_ROWS) {
        poll()?;
        for &v in chunk {
            sum.add(v / scale - anchor);
        }
    }
    let center = sum.total() / values.len() as f64;
    let mut squares = CompensatedSum::default();
    for chunk in values.chunks(SCAN_ROWS) {
        poll()?;
        for &v in chunk {
            let d = (v / scale - anchor) - center;
            squares.add(d * d);
        }
    }
    finite(scale * (squares.total() / (values.len() - 1) as f64).sqrt())
}

pub(super) struct ReferenceCovariance {
    pub(super) scales: Vec<f64>,
    pub(super) transform: Vec<f64>,
    pub(super) augment: bool,
}

impl ReferenceCovariance {
    pub(super) fn new(
        generation: &PreparedPpmlModelGeneration,
        fit: &PreparedPpmlIrlsFit,
        original_scales: &[f64],
        reported_rank: usize,
        poll: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Self> {
        let kept = fit.linear().basis().kept_generated_columns();
        let rank = kept.len();
        let augment = reported_rank == rank + 1;
        let mut scales = fallible_filled(
            rank,
            0.0,
            "unable to reserve PPML identified regressor scales",
        )?;
        for (scale, &column) in scales.iter_mut().zip(kept) {
            let original = generation.model().columns[column].coefficient_slot.get() as usize - 1;
            *scale = *original_scales
                .get(original)
                .ok_or_else(|| invalid_argument("PPML covariance scale slot is absent"))?;
        }
        if !augment {
            return Ok(Self {
                scales,
                transform: Vec::new(),
                augment,
            });
        }
        let dimension = reported_rank;
        let square = dimension
            .checked_mul(dimension)
            .ok_or_else(|| allocation("PPML reference covariance width overflow"))?;
        let mut transform = fallible_filled(
            square,
            0.0,
            "unable to reserve PPML augmented covariance transform",
        )?;
        let bread = fit.linear().identified_bread();
        for (row, source) in bread.chunks(rank.max(1)).enumerate() {
            transform[row * dimension..row * dimension + rank].copy_from_slice(source);
        }
        {
            let rows = fit.mean().len();
            // Strict final-frame validation has already matched these weights
            // to the bit pattern of q * mu on every retained row.
            let working = fit.linear().weights();
            let mass = match working {
                BorrowedMapWeights::Provided(values) => {
                    super::vce::compensated_positive_sum(values)?
                }
                BorrowedMapWeights::Unit => rows as f64,
            };
            transform[rank * dimension + rank] = finite(1.0 / mass)?;
            let mut means = fallible_filled(rank, 0.0, "unable to reserve PPML reference means")?;
            for (column, &generated) in kept.iter().enumerate() {
                let mut sum = CompensatedSum::default();
                for start in (0..rows).step_by(SCAN_ROWS) {
                    poll()?;
                    visit_prepared_column_range(
                        generation.fit_view(),
                        generated,
                        start..(start + SCAN_ROWS).min(rows),
                        |row, x| {
                            let w = match working {
                                BorrowedMapWeights::Provided(values) => values[row],
                                BorrowedMapWeights::Unit => 1.0,
                            };
                            sum.add((w / mass) * x);
                            Ok(())
                        },
                    )?;
                }
                means[column] = finite(sum.total())?;
            }
            // The centered score [x_within, 1] maps to slope influence
            // B*score and intercept influence score_cons/sum(W) − mean(x)'*B*score.
            for column in 0..rank {
                let mut sum = CompensatedSum::default();
                for (row, &mean) in means.iter().enumerate() {
                    let left = -mean;
                    let right = transform[row * dimension + column];
                    let product = left * right;
                    sum.add(product);
                    sum.add(left.mul_add(right, -product));
                }
                transform[rank * dimension + column] = finite(sum.total())?;
            }
        }
        Ok(Self {
            scales,
            transform,
            augment,
        })
    }

    /// Standardizing the completed original-unit covariance is the same
    /// congruence as changing units in every score and bread coordinate.
    pub(super) fn standardize(&self, matrix: &mut [f64]) -> Result<()> {
        let rank = self.scales.len();
        let dimension = rank + usize::from(self.augment);
        for row in 0..dimension {
            let a = self.scales.get(row).copied().unwrap_or(1.0);
            for column in 0..dimension {
                let b = self.scales.get(column).copied().unwrap_or(1.0);
                let value = &mut matrix[row * dimension + column];
                let pair = a * b;
                *value = finite(if pair.is_finite() {
                    *value * pair
                } else {
                    (*value * a.min(b)) * a.max(b)
                })?;
            }
        }
        Ok(())
    }

    pub(super) fn publish(
        &self,
        mut matrix: Vec<f64>,
        mut report: PpmlPsdReport,
        tolerance: f64,
    ) -> Result<(Vec<f64>, PpmlPsdReport)> {
        let rank = self.scales.len();
        let dimension = rank + usize::from(self.augment);
        for row in 0..rank {
            for column in 0..rank {
                // Compact in place, in increasing storage order.
                let a = self.scales[row];
                let b = self.scales[column];
                let value = matrix[row * dimension + column];
                let pair = a * b;
                matrix[row * rank + column] = finite(if pair.is_finite() {
                    value / pair
                } else {
                    (value / a.min(b)) / a.max(b)
                })?;
            }
        }
        matrix.truncate(rank * rank);
        // At this stage only the actual published spectrum is needed. The
        // independent result boundary additionally checks distance to a
        // further clipping of roundoff-negative eigenvalues.
        let (_, actual) = inspect_published_ppml_covariance(&matrix, rank, 0, tolerance)?;
        report.standardized_repair = true;
        report.final_rank = actual.rank;
        report.final_minimum_eigenvalue = actual.minimum_eigenvalue;
        report.eigensystem_reconstruction_relative_error = report
            .eigensystem_reconstruction_relative_error
            .max(actual.reconstruction_relative_error);
        report.orthogonality_relative_error = report
            .orthogonality_relative_error
            .max(actual.orthogonality_relative_error);
        Ok((matrix, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_sd_uses_physical_rows_and_survives_large_translation() {
        let mut poll = || Ok(());
        // Differences are exact powers of two; the naive cross-product
        // variance loses the dispersion entirely at this location.
        let anchor = 2.0_f64.powi(500);
        let unit = 2.0_f64.powi(450);
        let x = [anchor - unit, anchor, anchor + unit];
        assert_eq!(sample_sd(&x, &mut poll).unwrap(), unit);
        assert_eq!(sample_sd(&[anchor; 8], &mut poll).unwrap(), 0.);
        assert_eq!(sample_sd(&[-1., 0., 1.], &mut poll).unwrap(), 1.);
    }
}
