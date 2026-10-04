//! Model restrictions in identified coordinates, with an unrestricted constant.

use super::vce::{fallible_copy, filled, PpmlTestDistribution, PpmlWaldReport};
use crate::error::{ErrorCode, FeregError, Result};

/// Sparse restrictions beta[j] + loading[j] * beta[reference] = 0.
/// The reference direction represents the certified residualized constant.
#[derive(Debug, Clone, PartialEq)]
pub struct PpmlWaldReference {
    pub(crate) slot: usize,
    pub(crate) loadings: Vec<f64>,
    pub(crate) span_error: f64,
    pub(crate) span_tolerance: f64,
}

impl PpmlWaldReference {
    #[must_use]
    pub const fn slot(&self) -> usize {
        self.slot
    }
    #[must_use]
    pub fn loadings(&self) -> &[f64] {
        &self.loadings
    }
    #[must_use]
    pub const fn span_error(&self) -> f64 {
        self.span_error
    }
    #[must_use]
    pub const fn span_tolerance(&self) -> f64 {
        self.span_tolerance
    }
}

pub(crate) fn validate_reference(
    tested: &[usize],
    reference: Option<&PpmlWaldReference>,
    kept: &[usize],
) -> Result<()> {
    if reference.is_some_and(|reference| {
        reference.loadings.len() != tested.len()
            || !kept.contains(&reference.slot)
            || tested.contains(&reference.slot)
            || tested.len() + 1 != kept.len()
            || kept
                .iter()
                .any(|slot| *slot != reference.slot && !tested.contains(slot))
            || reference
                .loadings
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1.0)
            || !reference.span_error.is_finite()
            || reference.span_error < 0.0
            || !reference.span_tolerance.is_finite()
            || reference.span_tolerance <= 0.0
            || reference.span_tolerance >= 1.0
            || reference.span_error > reference.span_tolerance
    }) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML constant contrast metadata is inconsistent",
        ));
    }
    Ok(())
}

fn apply_contrasts(
    coefficients: &[f64],
    covariance: &[f64],
    width: usize,
    tested: &[usize],
    reference: &PpmlWaldReference,
    rhs: &mut [f64],
    matrix: &mut [f64],
) {
    let anchor = reference.slot;
    for (row, &source_row) in tested.iter().enumerate() {
        let left = reference.loadings[row];
        rhs[row] = coefficients[source_row] + left * coefficients[anchor];
        for (column, &source_column) in tested.iter().enumerate() {
            let right = reference.loadings[column];
            matrix[row * tested.len() + column] = covariance[source_row * width + source_column]
                + right * covariance[source_row * width + anchor]
                + left * covariance[anchor * width + source_column]
                + left * right * covariance[anchor * width + anchor];
        }
    }
}

pub(crate) fn compute_ppml_wald(
    coefficients: &[f64],
    covariance: &[f64],
    full_dimension: usize,
    tested: &[usize],
    reference: Option<&PpmlWaldReference>,
    tolerance: f64,
) -> Result<PpmlWaldReport> {
    if tested.is_empty() {
        return Ok(PpmlWaldReport {
            statistic: None,
            degrees_of_freedom: 0,
            tested_columns: Vec::new(),
            reference: copy_reference(reference)?,
            distribution: PpmlTestDistribution::ChiSquared,
            solve_relative_infinity_norm: 0.0,
        });
    }
    let dimension = tested.len();
    let mut matrix = filled(
        dimension.checked_mul(dimension).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "PPML Wald dimensions overflow")
        })?,
        0.0_f64,
        "unable to reserve PPML Wald covariance",
    )?;
    let mut rhs = filled(
        dimension,
        0.0_f64,
        "unable to reserve PPML Wald right-hand side",
    )?;
    for (row, &source_row) in tested.iter().enumerate() {
        rhs[row] = coefficients[source_row];
        for (column, &source_column) in tested.iter().enumerate() {
            matrix[row * dimension + column] =
                covariance[source_row * full_dimension + source_column];
        }
    }
    if let Some(reference) = reference {
        apply_contrasts(
            coefficients,
            covariance,
            full_dimension,
            tested,
            reference,
            &mut rhs,
            &mut matrix,
        );
        if rhs.iter().chain(&matrix).any(|value| !value.is_finite()) {
            return Err(FeregError::new(
                ErrorCode::NonConvergence,
                "PPML constant contrast arithmetic is not finite",
            ));
        }
    }
    let scale = matrix
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let mut lower = filled(
        matrix.len(),
        0.0_f64,
        "unable to reserve PPML Wald Cholesky factor",
    )?;
    for row in 0..dimension {
        for column in 0..=row {
            let mut value = matrix[row * dimension + column];
            for inner in 0..column {
                value -= lower[row * dimension + inner] * lower[column * dimension + inner];
            }
            if row == column {
                if value <= tolerance * scale {
                    return Ok(PpmlWaldReport {
                        statistic: None,
                        degrees_of_freedom: dimension,
                        tested_columns: fallible_copy(
                            tested,
                            "unable to reserve PPML singular-Wald slot identities",
                        )?,
                        reference: copy_reference(reference)?,
                        distribution: PpmlTestDistribution::ChiSquared,
                        solve_relative_infinity_norm: 0.0,
                    });
                }
                lower[row * dimension + column] = value.sqrt();
            } else {
                lower[row * dimension + column] = value / lower[column * dimension + column];
            }
        }
    }
    let mut solution = fallible_copy(&rhs, "unable to reserve PPML Wald solution")?;
    for row in 0..dimension {
        for column in 0..row {
            solution[row] -= lower[row * dimension + column] * solution[column];
        }
        solution[row] /= lower[row * dimension + row];
    }
    for row in (0..dimension).rev() {
        for column in (row + 1)..dimension {
            solution[row] -= lower[column * dimension + row] * solution[column];
        }
        solution[row] /= lower[row * dimension + row];
    }
    let mut statistic = 0.0_f64;
    let mut residual_norm = 0.0_f64;
    let mut reference_norm = f64::MIN_POSITIVE;
    for row in 0..dimension {
        statistic = rhs[row].mul_add(solution[row], statistic);
        let mut fitted = 0.0_f64;
        for column in 0..dimension {
            fitted = matrix[row * dimension + column].mul_add(solution[column], fitted);
        }
        residual_norm = residual_norm.max((fitted - rhs[row]).abs());
        reference_norm = reference_norm.max(fitted.abs()).max(rhs[row].abs());
    }
    let solve_error = residual_norm / reference_norm;
    if !statistic.is_finite() || statistic < -tolerance || !solve_error.is_finite() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "PPML Wald chi-squared calculation is not publishable",
        ));
    }
    Ok(PpmlWaldReport {
        statistic: Some(statistic.max(0.0)),
        degrees_of_freedom: dimension,
        tested_columns: fallible_copy(tested, "unable to reserve PPML Wald slot identities")?,
        reference: copy_reference(reference)?,
        distribution: PpmlTestDistribution::ChiSquared,
        solve_relative_infinity_norm: solve_error,
    })
}

fn copy_reference(reference: Option<&PpmlWaldReference>) -> Result<Option<PpmlWaldReference>> {
    reference
        .map(|reference| {
            Ok(PpmlWaldReference {
                slot: reference.slot,
                loadings: fallible_copy(
                    &reference.loadings,
                    "unable to reserve PPML Wald contrast loadings",
                )?,
                span_error: reference.span_error,
                span_tolerance: reference.span_tolerance,
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_contrast_uses_both_dummies_and_retains_singular_policy() {
        let reference = PpmlWaldReference {
            slot: 1,
            loadings: vec![-1.0],
            span_error: 0.0,
            span_tolerance: 1e-12,
        };
        validate_reference(&[0], Some(&reference), &[0, 1]).unwrap();
        let beta = [2.0_f64.ln(), 5.0_f64.ln()];
        let covariance = [0.2, 0.03, 0.03, 0.1];
        let got = compute_ppml_wald(&beta, &covariance, 2, &[0], Some(&reference), 1e-10).unwrap();
        assert!((got.statistic.unwrap() - (beta[0] - beta[1]).powi(2) / 0.24).abs() < 1e-12);
        let singular =
            compute_ppml_wald(&beta, &[1., 1., 1., 1.], 2, &[0], Some(&reference), 1e-10).unwrap();
        assert_eq!(singular.statistic, None);
        assert_eq!(singular.degrees_of_freedom, 1);
    }

    #[test]
    fn finite_contrast_inputs_that_overflow_fail_numerically() {
        let reference = PpmlWaldReference {
            slot: 1,
            loadings: vec![1.0],
            span_error: 0.0,
            span_tolerance: 1e-12,
        };
        for (beta, covariance) in [
            ([1.0, 1.0], [6e307; 4]),
            ([1e308, 1e308], [1.0, 0.0, 0.0, 1.0]),
        ] {
            let error = compute_ppml_wald(&beta, &covariance, 2, &[0], Some(&reference), 1e-10)
                .expect_err("overflow must not become a successful singular test");
            assert_eq!(error.code, ErrorCode::NonConvergence);
        }
    }

    #[test]
    fn invalid_constant_contrast_metadata_fails_closed() {
        let reference = PpmlWaldReference {
            slot: 1,
            loadings: vec![-1.0],
            span_error: 0.0,
            span_tolerance: 1e-12,
        };
        for alteration in 0..4 {
            let mut bad = reference.clone();
            match alteration {
                0 => bad.slot = 0,
                1 => bad.loadings.clear(),
                2 => bad.loadings[0] = f64::NAN,
                _ => bad.span_error = 1e-6,
            }
            assert!(validate_reference(&[0], Some(&bad), &[0, 1]).is_err());
        }
        assert!(validate_reference(&[0], Some(&reference), &[0, 1, 2]).is_err());
    }
}
