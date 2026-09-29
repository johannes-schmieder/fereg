//! Shared FWL assembly for OLS responses and PPML working/final-Hessian frames.
use super::{Fit, FrozenBasis, Policy, Problem, Role, Slot};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    fe::{Certificate, Engine, RhsDiagnostics},
    threads::{compensated_sum, ThreadTeam},
};
#[derive(Debug, PartialEq)]
pub struct AbsorbedFit {
    pub finite: Fit,
    pub within: DenseMatrix,
    pub within_outcome: Vec<f64>,
    pub absorbed_coefficients: DenseMatrix,
    pub residuals: Vec<f64>,
    pub projection_certificate: Certificate,
    pub model_certificate: Certificate,
    pub projection_diagnostics: Vec<RhsDiagnostics>,
    /// Extra derived-RHS solve when cancellation prevents the combined alpha
    /// from passing the unchanged final-model FE certificate.
    pub recovery_diagnostics: Vec<RhsDiagnostics>,
    pub relative_residual_reconstruction: f64,
    pub relative_original_finite_score: f64,
}

pub fn solve_absorbed(
    engine: &mut Engine<'_, '_>,
    raw: &DenseMatrix,
    outcome: &[f64],
    roles: &[Role],
    policy: Policy,
    team: &ThreadTeam,
) -> Result<AbsorbedFit> {
    solve_absorbed_with_basis(engine, raw, outcome, roles, policy, None, team)
}

/// The same FWL assembly with an optional, sample-aligned canonical basis.
pub fn solve_absorbed_with_basis(
    engine: &mut Engine<'_, '_>,
    raw: &DenseMatrix,
    outcome: &[f64],
    roles: &[Role],
    policy: Policy,
    basis: Option<&FrozenBasis>,
    team: &ThreadTeam,
) -> Result<AbsorbedFit> {
    let design = engine.design();
    let n = design.nobs();
    let k = raw.ncols();
    if raw.nrows() != n
        || outcome.len() != n
        || roles.len() != k
        || outcome.iter().chain(raw.as_slice()).any(|v| !v.is_finite())
    {
        return Err(invalid("absorbed WLS inputs are malformed or nonfinite"));
    }
    let width = k
        .checked_add(1)
        .ok_or_else(|| invalid("absorbed WLS width overflows"))?;
    let mut rhs = DenseMatrix::zeros(n, width)?;
    for row in 0..n {
        rhs[(row, 0)] = outcome[row];
        for column in 0..k {
            rhs[(row, column + 1)] = raw[(row, column)];
        }
    }
    let projection = engine.project(&rhs, team)?;
    let mut within = DenseMatrix::zeros(n, k)?;
    let within_outcome = projection.residuals.column(0)?;
    for row in 0..n {
        for column in 0..k {
            within[(row, column)] = projection.residuals[(row, column + 1)];
        }
    }
    let mut slots = reserved(k)?;
    for (column, &role) in roles.iter().enumerate() {
        let mut norm = 0_f64;
        for row in 0..n {
            norm = norm.hypot(design.weights().at(row).sqrt() * raw[(row, column)]);
        }
        if !norm.is_finite() {
            return Err(numerical("absorbed WLS reference norm overflows"));
        }
        slots.push(Slot {
            role,
            reference_norm: norm,
        });
    }
    let finite = super::solve_with_basis(
        &Problem {
            design: &within,
            outcome: &within_outcome,
            weights: design.weights(),
            slots: &slots,
            has_absorbed_intercept: design.topology().effects().iter().any(|e| e.intercept),
        },
        policy,
        basis,
        team,
    )?;
    let mut absorbed_coefficients = DenseMatrix::zeros(design.layout().total, 1)?;
    for index in 0..design.layout().total {
        absorbed_coefficients[(index, 0)] = compensated_sum(
            std::iter::once(projection.coefficients[(index, 0)]).chain(
                finite
                    .coefficients
                    .iter()
                    .enumerate()
                    .map(|(column, beta)| -beta * projection.coefficients[(index, column + 1)]),
            ),
        );
    }
    let tolerance = engine.plan().tolerance();
    design.normalize_coefficients(&mut absorbed_coefficients, tolerance, team)?;
    let mut target = DenseMatrix::zeros(n, 1)?;
    for row in 0..n {
        target[(row, 0)] = outcome[row] - raw.row_combination(row, &finite.coefficients, 0.);
    }
    let mut residual = design.reconstruct(&target, &absorbed_coefficients, team)?;
    let mut recovery_diagnostics = Vec::new();
    let model_certificate =
        match design.certify(&target, &absorbed_coefficients, &residual, tolerance, team) {
            Ok(certificate) => certificate,
            Err(error) if error.code == ErrorCode::NonConvergence => {
                // Separate projections can cancel to a much smaller final RHS.
                // Its own SPEC scale, not the pre-fit scales, must certify.
                let recovery = engine.project(&target, team)?;
                absorbed_coefficients = recovery.coefficients;
                residual = recovery.residuals;
                recovery_diagnostics = recovery.diagnostics;
                recovery.certificate
            }
            Err(error) => return Err(error),
        };
    let residuals = residual.into_row_major();
    let mut reconstruction = 0_f64;
    for row in 0..n {
        let scale = outcome[row]
            .abs()
            .max(finite.fitted[row].abs())
            .max(residuals[row].abs())
            .max(1.);
        let error = (residuals[row] - finite.residuals[row]).abs() / scale;
        if !error.is_finite() {
            return Err(numerical("absorbed WLS residual comparison is not finite"));
        }
        reconstruction = reconstruction.max(error);
    }
    if reconstruction > tolerance.max(policy.score_tolerance) {
        return Err(numerical(
            "absorbed WLS original and within residuals disagree",
        ));
    }
    let original_score = original_score(engine, raw, outcome, &finite.kept, &residuals)?;
    if original_score > tolerance.max(policy.score_tolerance) {
        return Err(numerical(
            "absorbed WLS original finite score did not certify",
        ));
    }
    Ok(AbsorbedFit {
        finite,
        within,
        within_outcome,
        absorbed_coefficients,
        residuals,
        projection_certificate: projection.certificate,
        model_certificate,
        projection_diagnostics: projection.diagnostics,
        recovery_diagnostics,
        relative_residual_reconstruction: reconstruction,
        relative_original_finite_score: original_score,
    })
}
fn original_score(
    engine: &Engine<'_, '_>,
    raw: &DenseMatrix,
    outcome: &[f64],
    kept: &[usize],
    residuals: &[f64],
) -> Result<f64> {
    let weights = engine.design().weights();
    let mut reference = 0_f64;
    let mut residual_norm = 0_f64;
    for row in 0..outcome.len() {
        let root = weights.at(row).sqrt();
        reference = reference.hypot(root * outcome[row]);
        residual_norm = residual_norm.hypot(root * residuals[row]);
    }
    if !reference.is_finite() || !residual_norm.is_finite() {
        return Err(numerical(
            "absorbed WLS original score scale is not representable",
        ));
    }
    let scale = reference.max(residual_norm);
    let mut maximum = 0_f64;
    if scale == 0. {
        return Ok(0.);
    }
    for &column in kept {
        let mut norm = 0_f64;
        for row in 0..outcome.len() {
            norm = norm.hypot(weights.at(row).sqrt() * raw[(row, column)]);
        }
        if !norm.is_finite() || norm == 0. {
            return Err(numerical(
                "identified original WLS basis has an invalid norm",
            ));
        }
        let score = compensated_sum((0..outcome.len()).map(|row| {
            let root = weights.at(row).sqrt();
            (root * raw[(row, column)] / norm) * (root * residuals[row] / scale)
        }))
        .abs();
        if !score.is_finite() {
            return Err(numerical(
                "absorbed WLS original finite score is not representable",
            ));
        }
        maximum = maximum.max(score);
    }
    Ok(maximum)
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
