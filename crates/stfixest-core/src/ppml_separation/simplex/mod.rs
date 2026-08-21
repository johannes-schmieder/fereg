#![allow(clippy::needless_range_loop)]

use super::{DetectionResult, MethodStatus, SeparationCertificate};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::FeDesign;

mod nullspace;
mod tableau;

use nullspace::nullspace_basis;
use tableau::phase_one_cone;

pub(crate) fn finite_simplex(
    outcome: &[f64],
    regressors: &DenseMatrix,
    fixed_effects: Option<&FeDesign>,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    max_tableau_cells: usize,
) -> Result<DetectionResult> {
    if fixed_effects.is_some_and(|design| !design.effects().is_empty()) {
        return Ok(DetectionResult::empty(
            MethodStatus::SkippedUnsupported,
            "finite simplex is skipped with absorbed effects; iterated rectifier handles the full design",
        ));
    }
    if regressors.ncols() == 0 {
        return Ok(DetectionResult::empty(
            MethodStatus::NoCandidates,
            "finite simplex requires at least one finite regressor",
        ));
    }
    let positive = outcome
        .iter()
        .enumerate()
        .filter_map(|(row, value)| (*value > 0.0).then_some(row))
        .collect::<Vec<_>>();
    let boundary = outcome
        .iter()
        .enumerate()
        .filter_map(|(row, value)| (*value == 0.0).then_some(row))
        .collect::<Vec<_>>();
    if boundary.is_empty() {
        return Ok(DetectionResult::empty(
            MethodStatus::NoCandidates,
            "finite simplex found no zero-outcome observations",
        ));
    }

    let basis = nullspace_basis(regressors, &positive, tolerance)?;
    if basis.is_empty() {
        return Ok(DetectionResult::empty(
            MethodStatus::NoSeparation,
            "finite regressors have full column rank on the positive-outcome sample",
        ));
    }
    let cone = boundary_cone(regressors, &boundary, &basis);
    if let Some(gamma) = signed_basis_certificate(&basis, &cone, tolerance) {
        return certified_result(
            outcome,
            regressors,
            gamma,
            0,
            certificate_tolerance,
            "single null-space direction has a common sign on boundary observations",
        );
    }

    let rows = boundary.len() + 1;
    let original_variables = 2 * basis.len() + boundary.len();
    let total_variables = original_variables + rows;
    let cells = rows
        .checked_mul(total_variables.saturating_add(1))
        .ok_or_else(|| {
            StFixestError::new(
                ErrorCode::Allocation,
                "simplex tableau dimensions overflowed",
            )
        })?;
    if cells > max_tableau_cells {
        return Ok(DetectionResult::empty(
            MethodStatus::SkippedUnsupported,
            format!(
                "finite simplex tableau requires {cells} cells, above the configured {max_tableau_cells} limit"
            ),
        ));
    }

    let feasible = phase_one_cone(&cone, max_iterations, tolerance)?;
    let Some((coordinates, iterations)) = feasible else {
        return Ok(DetectionResult {
            dropped: Vec::new(),
            iterations: max_iterations,
            status: MethodStatus::NoSeparation,
            certificate: None,
            note: "finite simplex phase-I problem is infeasible".to_owned(),
        });
    };
    let gamma = combine_basis(&basis, &coordinates);
    certified_result(
        outcome,
        regressors,
        gamma,
        iterations,
        certificate_tolerance,
        "finite simplex phase-I certificate passed",
    )
}

fn certified_result(
    outcome: &[f64],
    regressors: &DenseMatrix,
    gamma: Vec<f64>,
    iterations: usize,
    tolerance: f64,
    note: &str,
) -> Result<DetectionResult> {
    if gamma.iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "finite simplex coefficient direction became nonfinite",
        ));
    }
    let z = multiply(regressors, &gamma);
    if z.iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "finite simplex certificate became nonfinite",
        ));
    }
    let scale = z
        .iter()
        .fold(1.0_f64, |maximum, value| maximum.max(value.abs()));
    let threshold = tolerance * scale;
    let interior_zero = outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
    let minimum_boundary = outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::INFINITY, f64::min);
    let positive_maximum = outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max);
    let certificate = SeparationCertificate {
        interior_zero_relative: interior_zero / scale,
        boundary_violation_relative: (-minimum_boundary).max(0.0) / scale,
        span_residual_relative: 0.0,
        positive_maximum,
        passed: interior_zero <= threshold
            && minimum_boundary >= -threshold
            && positive_maximum > threshold,
    };
    if !certificate.passed {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            format!(
                "finite simplex returned an uncertified direction: interior={:.3e}, boundary={:.3e}",
                certificate.interior_zero_relative,
                certificate.boundary_violation_relative
            ),
        ));
    }
    let dropped = outcome
        .iter()
        .zip(&z)
        .enumerate()
        .filter_map(|(row, (outcome, value))| {
            (*outcome == 0.0 && *value > threshold).then_some(row)
        })
        .collect();
    Ok(DetectionResult {
        dropped,
        iterations,
        status: MethodStatus::Certified,
        certificate: Some(certificate),
        note: note.to_owned(),
    })
}

fn boundary_cone(
    regressors: &DenseMatrix,
    boundary: &[usize],
    basis: &[Vec<f64>],
) -> Vec<Vec<f64>> {
    boundary
        .iter()
        .map(|row| {
            basis
                .iter()
                .map(|direction| {
                    (0..regressors.ncols()).fold(0.0_f64, |sum, column| {
                        regressors[(*row, column)].mul_add(direction[column], sum)
                    })
                })
                .collect()
        })
        .collect()
}

fn signed_basis_certificate(
    basis: &[Vec<f64>],
    cone: &[Vec<f64>],
    tolerance: f64,
) -> Option<Vec<f64>> {
    for column in 0..basis.len() {
        for sign in [1.0_f64, -1.0_f64] {
            let values = cone
                .iter()
                .map(|row| sign * row[column])
                .collect::<Vec<_>>();
            let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
            let maximum = values.iter().copied().fold(0.0_f64, f64::max);
            let scale = maximum.abs().max(minimum.abs()).max(1.0);
            if minimum >= -tolerance * scale && maximum > tolerance * scale {
                return Some(basis[column].iter().map(|value| sign * value).collect());
            }
        }
    }
    None
}

fn combine_basis(basis: &[Vec<f64>], coordinates: &[f64]) -> Vec<f64> {
    let mut gamma = vec![0.0_f64; basis[0].len()];
    for (direction, coefficient) in basis.iter().zip(coordinates) {
        for column in 0..gamma.len() {
            gamma[column] = coefficient.mul_add(direction[column], gamma[column]);
        }
    }
    gamma
}

fn multiply(regressors: &DenseMatrix, gamma: &[f64]) -> Vec<f64> {
    (0..regressors.nrows())
        .map(|row| {
            (0..regressors.ncols()).fold(0.0_f64, |sum, column| {
                regressors[(row, column)].mul_add(gamma[column], sum)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
