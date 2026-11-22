#![allow(clippy::needless_range_loop)]

use super::{
    DetectionProposal, MethodStatus, SeparationCertificate, SeparationCertificateProof,
    SeparationMethod, SeparationNote,
};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};

mod compact_model;
mod compact_tableau;
mod exact_terminal;
pub(crate) use compact_model::{
    compact_model, compact_model_scaled, Big, BinaryDesign, CompactInput, CompactModel,
    CompactWork, ExactDual, StreamedEffect, ValuedDesign, ValuedModel, ValuedSearch, Verifiers,
    WideRay,
};
pub(crate) use compact_model::{exact_dual_terminal, exact_dyadic_dual, Blocks};
pub(crate) use exact_terminal::raw_positive_dual;
mod nullspace;
mod tableau;

use nullspace::nullspace_basis;
#[cfg(test)]
use tableau::phase_one_cone;
use tableau::{phase_one_cone_interruptible, PhaseOneConeOutcome};

// Test fault plan; shipped builds compile this branch out.
crate::fault::fault_plan! {
    static TEST_RESERVATIONS_UNTIL_FAILURE: Option<usize> = None;
}

fn reserve<T>(target: &mut Vec<T>, additional: usize, label: &'static str) -> Result<()> {
    if TEST_RESERVATIONS_UNTIL_FAILURE.with(|remaining| match remaining.get() {
        None => false,
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(value) => {
            remaining.set(Some(value - 1));
            false
        }
    }) {
        return Err(FeregError::new(
            ErrorCode::Allocation,
            "injected model-simplex reservation failure",
        ));
    }
    target
        .try_reserve_exact(additional)
        .map_err(|_| FeregError::new(ErrorCode::Allocation, label))
}

fn fallible_filled<T: Clone>(elements: usize, value: T, label: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reserve(&mut output, elements, label)?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_collect<T>(
    values: impl IntoIterator<Item = T>,
    capacity: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reserve(&mut output, capacity, label)?;
    output.extend(values);
    Ok(output)
}

#[cfg(test)]
fn inject_reservation_failure(after_successful_reservations: usize) {
    TEST_RESERVATIONS_UNTIL_FAILURE.with(|remaining| {
        remaining.set(Some(after_successful_reservations));
    });
}

fn workspace_overflow() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "model-simplex workspace dimensions overflowed",
    )
}

fn checked_cells_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or_else(workspace_overflow)
}

fn checked_cells_mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right).ok_or_else(workspace_overflow)
}

/// Conservative peak of simultaneously live model-simplex storage in
/// eight-byte cells. The bound includes the caller-owned dense design, exact
/// positive/boundary row buffers, `Vec<Vec<f64>>` headers, null-space work,
/// phase-I work, and certified-result vectors. It uses the worst possible
/// nullity (`columns`) so it can be checked before the first kernel allocation.
pub(crate) fn simplex_workspace_peak_cells(
    rows: usize,
    positive: usize,
    boundary: usize,
    columns: usize,
) -> Result<usize> {
    if checked_cells_add(positive, boundary)? > rows {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "model-simplex outcome partition exceeds the observation count",
        ));
    }
    let design = checked_cells_mul(rows, columns)?;
    let positive_matrix = checked_cells_mul(positive, columns)?;
    let square_basis = checked_cells_mul(columns, columns)?;

    // Both exact row lists remain live during elimination. At the final basis
    // direction, the matrix, scales, permutation, completed basis directions,
    // current permuted/original directions, and three-cell Vec headers overlap.
    let nullspace = checked_cells_add(positive, boundary)?;
    let nullspace = checked_cells_add(nullspace, positive_matrix)?;
    let nullspace = checked_cells_add(nullspace, square_basis)?;
    let nullspace = checked_cells_add(nullspace, checked_cells_mul(6, columns)?)?;

    // At the feasible phase-I exit, the retained basis and boundary cone
    // overlap the tableau, basic-index vector, costs, primal solution, and
    // returned coordinates. A Vec header is three eight-byte cells on every
    // supported 64-bit target.
    let tableau_rows = checked_cells_add(boundary, 1)?;
    let tableau_width = checked_cells_add(
        checked_cells_add(
            checked_cells_mul(2, columns)?,
            checked_cells_mul(2, boundary)?,
        )?,
        2,
    )?;
    let tableau = checked_cells_mul(tableau_rows, tableau_width)?;
    let phase_one = checked_cells_add(square_basis, checked_cells_mul(boundary, columns)?)?;
    let phase_one = checked_cells_add(phase_one, tableau)?;
    let phase_one = checked_cells_add(phase_one, checked_cells_mul(8, columns)?)?;
    let phase_one = checked_cells_add(phase_one, checked_cells_mul(7, boundary)?)?;
    let phase_one = checked_cells_add(phase_one, 2)?;

    // `gamma` overlaps the observation direction during multiplication; after
    // that, the observation direction overlaps the exact boundary-sized drops.
    let result = checked_cells_add(rows, columns)?.max(checked_cells_add(rows, boundary)?);
    checked_cells_add(design, nullspace.max(phase_one).max(result))
}

#[cfg(test)]
pub(crate) fn model_simplex(
    outcome: &[f64],
    regressors: &DenseMatrix,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    max_tableau_cells: usize,
) -> Result<DetectionProposal> {
    model_simplex_interruptible(
        outcome,
        regressors,
        tolerance,
        certificate_tolerance,
        max_iterations,
        max_tableau_cells,
        &mut || Ok(()),
    )
}

pub(crate) fn model_simplex_interruptible(
    outcome: &[f64],
    regressors: &DenseMatrix,
    tolerance: f64,
    certificate_tolerance: f64,
    max_iterations: usize,
    max_tableau_cells: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<DetectionProposal> {
    if regressors.ncols() == 0 {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Simplex,
            MethodStatus::NoCandidates,
            "model simplex requires at least one explicit model column",
        ));
    }
    let positive_count = outcome.iter().filter(|value| **value > 0.0).count();
    let boundary_count = outcome.iter().filter(|value| **value == 0.0).count();
    if boundary_count == 0 {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Simplex,
            MethodStatus::NoCandidates,
            "model simplex found no zero-outcome observations",
        ));
    }
    let peak_cells = simplex_workspace_peak_cells(
        outcome.len(),
        positive_count,
        boundary_count,
        regressors.ncols(),
    )?;
    if peak_cells > max_tableau_cells {
        return Ok(DetectionProposal::empty(
            SeparationMethod::Simplex,
            MethodStatus::SkippedMemory,
            SeparationNote::with_detail(
                "model simplex exceeded its configured cell limit",
                peak_cells,
            ),
        ));
    }
    let mut positive = fallible_collect(
        outcome
            .iter()
            .enumerate()
            .filter_map(|(row, value)| (*value > 0.0).then_some(row)),
        positive_count,
        "unable to reserve model-simplex positive rows",
    )?;
    let boundary = fallible_collect(
        outcome
            .iter()
            .enumerate()
            .filter_map(|(row, value)| (*value == 0.0).then_some(row)),
        boundary_count,
        "unable to reserve model-simplex boundary rows",
    )?;
    let basis = nullspace_basis(regressors, &mut positive, tolerance, interrupt)?;
    // The numerical positive-row factorization above already visits a dense
    // trailing rectangle at every pivot. Give its independent exact check
    // that allowance on modest designs, within a fixed operation ceiling.
    let elimination_work = positive_count
        .saturating_mul(regressors.ncols())
        .saturating_mul(positive_count.min(regressors.ncols()));
    let proof_work = max_tableau_cells
        .saturating_mul(8)
        .max(elimination_work.min(32 << 20));
    if basis.is_empty() {
        if !exact_terminal::full_positive_rank(
            regressors,
            outcome,
            &positive[..regressors.ncols()],
            proof_work,
        )? {
            return Ok(uncertified(
                "positive-outcome rank lacks an exact full-rank minor",
            ));
        }
        return Ok(full_positive_rank_proposal(regressors.ncols()));
    }
    // Each retained pivot row ID replaces at least one full row omitted from
    // the worst-case square nullspace basis already charged to the kernel.
    let proposed_rank = regressors.ncols() - basis.len();
    let pivot_rows = fallible_collect(
        positive[..proposed_rank].iter().copied(),
        proposed_rank,
        "unable to reserve simplex positive pivot representatives",
    )?;
    drop(positive);
    let cone = boundary_cone(regressors, &boundary, &basis)?;
    drop(boundary);
    let gamma = match signed_basis_certificate(&basis, &cone, tolerance)? {
        Some(gamma) => Some(gamma),
        None => exact_terminal::single_coordinate(regressors, outcome)?,
    };
    if let Some(gamma) = gamma {
        drop(cone);
        drop(basis);
        return certified_result(
            outcome,
            regressors,
            gamma,
            0,
            certificate_tolerance,
            "single null-space direction has a common sign on boundary observations".into(),
        );
    }

    let rows = boundary_count
        .checked_add(1)
        .ok_or_else(|| FeregError::new(ErrorCode::Allocation, "simplex row count overflowed"))?;
    let original_variables = basis
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(boundary_count))
        .ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "simplex variable count overflowed")
        })?;
    let total_variables = original_variables.checked_add(rows).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Allocation,
            "simplex total variable count overflowed",
        )
    })?;
    let cells = rows
        .checked_mul(total_variables.checked_add(1).ok_or_else(|| {
            FeregError::new(ErrorCode::Allocation, "simplex tableau width overflowed")
        })?)
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "simplex tableau dimensions overflowed",
            )
        })?;
    debug_assert!(cells <= peak_cells);

    let phase_one = phase_one_cone_interruptible(&cone, max_iterations, tolerance, interrupt)?;
    let (coordinates, iterations) = match phase_one {
        PhaseOneConeOutcome::Feasible {
            coordinates,
            iterations,
        } => (coordinates, iterations),
        PhaseOneConeOutcome::Infeasible { dual, iterations } => {
            let Some(certificate) =
                verify_phase_one_farkas_certificate(&cone, &dual, certificate_tolerance)?
            else {
                return Ok(DetectionProposal {
                    method: SeparationMethod::Simplex,
                    dropped: Vec::new(),
                    iterations,
                    status: MethodStatus::Uncertified,
                    certificate: None,
                    exact_fe_certificate: None,
                    direction: None,
                    note: "model simplex phase-I dual failed independent Farkas verification"
                        .into(),
                });
            };
            drop(cone);
            drop(basis);
            if !exact_terminal::no_recession(
                regressors,
                outcome,
                &dual[..boundary_count],
                proof_work,
                peak_cells - regressors.as_slice().len() - pivot_rows.len(),
                &pivot_rows,
            )? {
                return Ok(uncertified(
                    "simplex infeasibility lacks a full-design no-separation proof",
                ));
            }
            return Ok(DetectionProposal {
                method: SeparationMethod::Simplex,
                dropped: Vec::new(),
                iterations,
                status: MethodStatus::NoSeparation,
                certificate: Some(certificate),
                exact_fe_certificate: None,
                direction: None,
                note:
                    "model simplex phase-I infeasibility passed an independent Farkas certificate"
                        .into(),
            });
        }
    };
    let gamma = combine_basis(&basis, &coordinates)?;
    drop(coordinates);
    drop(cone);
    drop(basis);
    certified_result(
        outcome,
        regressors,
        gamma,
        iterations,
        certificate_tolerance,
        "model simplex phase-I certificate passed".into(),
    )
}

/// Independently verify the phase-I Farkas multiplier against the original
/// cone feasibility system
///
/// ```text
/// A c - s = 0,   1's = 1,   c free, s >= 0.
/// ```
///
/// For `y = (u, v)`, infeasibility follows from `A'u = 0`,
/// `v - u_i >= 0`, and `v < 0`.  These conditions imply `y'M >= 0` but
/// `y'b < 0`, which is impossible for any nonnegative primal solution.  This
/// routine consumes neither the final tableau nor its reduced costs.
fn verify_phase_one_farkas_certificate(
    cone: &[Vec<f64>],
    dual: &[f64],
    tolerance: f64,
) -> Result<Option<SeparationCertificate>> {
    verify_indexed_farkas_certificate(cone, &[], dual, tolerance)
}

fn verify_indexed_farkas_certificate(
    cone: &[Vec<f64>],
    groups: &[u32],
    dual: &[f64],
    tolerance: f64,
) -> Result<Option<SeparationCertificate>> {
    let boundary = if groups.is_empty() {
        cone.len()
    } else {
        groups.len()
    };
    let nullity = cone.first().map_or(0, Vec::len);
    if boundary == 0
        || nullity == 0
        || dual.len() != boundary + 1
        || groups.iter().any(|&index| index as usize >= cone.len())
        || cone.iter().any(|row| row.len() != nullity)
        || cone
            .iter()
            .flat_map(|row| row.iter())
            .chain(dual)
            .any(|value| !value.is_finite())
    {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "model-simplex Farkas certificate is not finite and dimensionally aligned",
        ));
    }
    let scale = dual
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if !scale.is_finite() || scale <= f64::MIN_POSITIVE {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "model-simplex Farkas certificate is zero",
        ));
    }
    let normalization_dual = dual[boundary];
    let mut stationarity = 0.0_f64;
    for column in 0..nullity {
        let mut value = 0.0_f64;
        for (row, &weight) in dual[..boundary].iter().enumerate() {
            if !groups.is_empty() {
                crate::interrupt::row_checkpoint(row)?;
            }
            let projected = groups.get(row).map_or(row, |&index| index as usize);
            value = cone[projected][column].mul_add(weight, value);
        }
        stationarity = stationarity.max(value.abs());
    }
    let inequality_violation = (0..boundary)
        .map(|row| (dual[row] - normalization_dual).max(0.0))
        .fold(0.0_f64, f64::max);
    Ok(farkas_residual_certificate(
        boundary,
        nullity,
        scale,
        normalization_dual,
        stationarity,
        inequality_violation,
        tolerance,
    ))
}

pub(super) fn farkas_residual_certificate(
    boundary: usize,
    nullity: usize,
    scale: f64,
    normalization_dual: f64,
    stationarity: f64,
    inequality_violation: f64,
    tolerance: f64,
) -> Option<SeparationCertificate> {
    let stationarity_relative = stationarity / scale;
    let inequality_violation_relative = inequality_violation / scale;
    let infeasibility_margin_relative = -normalization_dual / scale;
    let certificate = SeparationCertificate {
        proof: SeparationCertificateProof::SimplexDualInfeasibility {
            boundary_rows: boundary,
            nullity,
        },
        interior_zero_relative: stationarity_relative,
        boundary_violation_relative: inequality_violation_relative,
        span_residual_relative: 0.0,
        negative_margin_relative: infeasibility_margin_relative,
        direction_scale: scale,
        positive_maximum: 0.0,
        passed: stationarity_relative <= tolerance
            && inequality_violation_relative <= tolerance
            && infeasibility_margin_relative >= tolerance,
    };
    certificate.passed.then_some(certificate)
}

pub(crate) fn full_positive_rank_proposal(columns: usize) -> DetectionProposal {
    let mut result = DetectionProposal::empty(
        SeparationMethod::Simplex,
        MethodStatus::NoSeparation,
        "finite regressors have full column rank on the positive-outcome sample",
    );
    result.certificate = Some(no_separation_certificate(
        SeparationCertificateProof::SimplexFullPositiveRank { columns },
    ));
    result
}

pub(crate) fn no_separation_certificate(
    proof: SeparationCertificateProof,
) -> SeparationCertificate {
    SeparationCertificate {
        proof,
        interior_zero_relative: 0.0,
        boundary_violation_relative: 0.0,
        span_residual_relative: 0.0,
        negative_margin_relative: 0.0,
        direction_scale: 0.0,
        positive_maximum: 0.0,
        passed: true,
    }
}

fn certified_result(
    outcome: &[f64],
    regressors: &DenseMatrix,
    gamma: Vec<f64>,
    iterations: usize,
    tolerance: f64,
    note: SeparationNote,
) -> Result<DetectionProposal> {
    if gamma.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "model simplex coefficient direction became nonfinite",
        ));
    }
    // The simplex construction uses the comparator's nonnegative cone.  Flip
    // it once at the detector boundary to the package-wide nonpositive sign.
    let mut z = multiply(regressors, &gamma)?;
    drop(gamma);
    for value in &mut z {
        *value = -*value;
    }
    if z.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "model simplex certificate became nonfinite",
        ));
    }
    let scale = z
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if !scale.is_finite() || scale <= f64::MIN_POSITIVE {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "model simplex returned a zero recession direction",
        ));
    }
    let threshold = tolerance * scale;
    let interior_zero = outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome > 0.0)
        .fold(0.0_f64, |maximum, (_, value)| maximum.max(value.abs()));
    let positive_maximum = outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, f64::max);
    let dropped = fallible_collect(
        outcome
            .iter()
            .zip(&z)
            .enumerate()
            .filter_map(|(row, (outcome, value))| {
                (*outcome == 0.0 && *value < -threshold).then_some(row)
            }),
        outcome.iter().filter(|value| **value == 0.0).count(),
        "unable to reserve model-simplex certified drops",
    )?;
    let negative_margin = dropped
        .iter()
        .map(|row| -z[*row])
        .fold(f64::INFINITY, f64::min);
    if dropped.is_empty() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "model simplex feasible direction has no strict-negative boundary row",
        ));
    }
    let most_negative = outcome
        .iter()
        .zip(&z)
        .filter(|(outcome, _)| **outcome == 0.0)
        .map(|(_, value)| -*value)
        .fold(0.0_f64, f64::max);
    let certificate = SeparationCertificate {
        proof: SeparationCertificateProof::RecessionDirection,
        interior_zero_relative: interior_zero / scale,
        boundary_violation_relative: positive_maximum.max(0.0) / scale,
        span_residual_relative: 0.0,
        negative_margin_relative: negative_margin / scale,
        direction_scale: scale,
        positive_maximum: positive_maximum.max(0.0),
        passed: interior_zero <= threshold
            && positive_maximum <= threshold
            && most_negative > threshold
            && negative_margin > threshold,
    };
    if !certificate.passed {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            format!(
                "model simplex returned an uncertified direction: interior={:.3e}, boundary={:.3e}",
                certificate.interior_zero_relative, certificate.boundary_violation_relative
            ),
        ));
    }
    Ok(DetectionProposal {
        method: SeparationMethod::Simplex,
        dropped,
        iterations,
        status: MethodStatus::Certified,
        certificate: Some(certificate),
        exact_fe_certificate: None,
        direction: Some(z),
        note,
    })
}

fn uncertified(note: &'static str) -> DetectionProposal {
    DetectionProposal::empty(SeparationMethod::Simplex, MethodStatus::Uncertified, note)
}

fn boundary_cone(
    regressors: &DenseMatrix,
    boundary: &[usize],
    basis: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>> {
    let mut cone = Vec::new();
    reserve(
        &mut cone,
        boundary.len(),
        "unable to reserve model-simplex boundary cone rows",
    )?;
    for &row in boundary {
        let mut values = fallible_filled(
            basis.len(),
            0.0_f64,
            "unable to reserve model-simplex boundary cone row",
        )?;
        for (column, direction) in basis.iter().enumerate() {
            values[column] = (0..regressors.ncols()).fold(0.0_f64, |sum, regressor| {
                regressors[(row, regressor)].mul_add(direction[regressor], sum)
            });
        }
        cone.push(values);
    }
    Ok(cone)
}

fn signed_basis_certificate(
    basis: &[Vec<f64>],
    cone: &[Vec<f64>],
    tolerance: f64,
) -> Result<Option<Vec<f64>>> {
    for column in 0..basis.len() {
        for sign in [1.0_f64, -1.0_f64] {
            let mut minimum = f64::INFINITY;
            let mut maximum = 0.0_f64;
            for row in cone {
                let value = sign * row[column];
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
            let scale = maximum.abs().max(minimum.abs()).max(1.0);
            if minimum >= -tolerance * scale && maximum > tolerance * scale {
                let mut direction = fallible_filled(
                    basis[column].len(),
                    0.0_f64,
                    "unable to reserve model-simplex signed basis direction",
                )?;
                for (output, value) in direction.iter_mut().zip(&basis[column]) {
                    *output = sign * *value;
                }
                return Ok(Some(direction));
            }
        }
    }
    Ok(None)
}

fn combine_basis(basis: &[Vec<f64>], coordinates: &[f64]) -> Result<Vec<f64>> {
    if basis.is_empty() || basis.len() != coordinates.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "model-simplex basis and coordinates are misaligned",
        ));
    }
    let mut gamma = fallible_filled(
        basis[0].len(),
        0.0_f64,
        "unable to reserve model-simplex coefficient direction",
    )?;
    for (direction, coefficient) in basis.iter().zip(coordinates) {
        if direction.len() != gamma.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "model-simplex basis dimensions are inconsistent",
            ));
        }
        for column in 0..gamma.len() {
            gamma[column] = coefficient.mul_add(direction[column], gamma[column]);
        }
    }
    Ok(gamma)
}

fn multiply(regressors: &DenseMatrix, gamma: &[f64]) -> Result<Vec<f64>> {
    if regressors.ncols() != gamma.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "model-simplex direction and design are misaligned",
        ));
    }
    let mut output = fallible_filled(
        regressors.nrows(),
        0.0_f64,
        "unable to reserve model-simplex observation direction",
    )?;
    for row in 0..regressors.nrows() {
        output[row] = (0..regressors.ncols()).fold(0.0_f64, |sum, column| {
            regressors[(row, column)].mul_add(gamma[column], sum)
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
