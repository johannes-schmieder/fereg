//! Shared FWL assembly for OLS responses and PPML working/final-Hessian frames.
use super::{recovery, work::Phase, Fit, FrozenBasis, Policy, Problem, Role, Slot, Work};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    fe::{Certificate, Engine, Projection, RhsDiagnostics, SetupDiagnostics},
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
    pub setup_diagnostics: Vec<SetupDiagnostics>,
    /// Derived-RHS solves when combined coefficients fail the final-model FE
    /// certificate or the original-response finite-score guard.
    pub recovery_diagnostics: Vec<RhsDiagnostics>,
    /// Bounded FE corrections needed by the original finite-score scale.
    pub model_refinements: usize,
    pub relative_residual_reconstruction: f64,
    pub relative_original_finite_score: f64,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct AbsorbedControls<'a> {
    pub finite: Policy,
    pub basis: Option<&'a FrozenBasis>,
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
    solve_absorbed_recorded(
        engine,
        raw,
        outcome,
        roles,
        AbsorbedControls {
            finite: policy,
            basis,
        },
        &mut Work::default(),
        team,
    )
}
/// Work is recorded before finite fitting, certificate rejection or speculative
/// callers can discard the frame. Numerical errors preserve consumed work.
pub fn solve_absorbed_recorded(
    engine: &mut Engine<'_, '_>,
    raw: &DenseMatrix,
    outcome: &[f64],
    roles: &[Role],
    controls: AbsorbedControls<'_>,
    work: &mut Work,
    team: &ThreadTeam,
) -> Result<AbsorbedFit> {
    let AbsorbedControls {
        finite: policy,
        basis,
    } = controls;
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
    let mut projection = recorded_projection(engine, &rhs, Phase::Projection, work, team)?;
    let mut within = DenseMatrix::zeros(n, k)?;
    let within_outcome = projection.residuals.column(0)?;
    for row in 0..n {
        for column in 0..k {
            within[(row, column)] = projection.residuals[(row, column + 1)];
        }
    }
    let slots = reference_slots(raw, roles, design.weights())?;
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
    let recovered = recovery::finish(
        engine,
        recovery::Model {
            raw,
            outcome,
            finite: &finite,
            policy,
        },
        target,
        absorbed_coefficients,
        work,
        team,
    )?;
    projection
        .setup_diagnostics
        .try_reserve(recovered.setup.len())
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Allocation,
                "unable to reserve recovery setup diagnostics",
            )
        })?;
    projection.setup_diagnostics.extend(recovered.setup);
    let residuals = recovered.residual.into_row_major();
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
        return Err(numerical(&format!(
            "absorbed WLS original and within residuals disagree: {reconstruction:.17e} > {:.17e}; original score={:.17e}, model refinements={}",
            tolerance.max(policy.score_tolerance), recovered.original_score, recovered.refinements,
        )));
    }
    Ok(AbsorbedFit {
        finite,
        within,
        within_outcome,
        absorbed_coefficients: recovered.coefficients,
        residuals,
        projection_certificate: projection.certificate,
        model_certificate: recovered.certificate,
        projection_diagnostics: projection.diagnostics,
        setup_diagnostics: projection.setup_diagnostics,
        recovery_diagnostics: recovered.diagnostics,
        model_refinements: recovered.refinements,
        relative_residual_reconstruction: reconstruction,
        relative_original_finite_score: recovered.original_score,
    })
}
pub(super) fn recorded_projection(
    engine: &mut Engine<'_, '_>,
    rhs: &DenseMatrix,
    phase: Phase,
    work: &mut Work,
    team: &ThreadTeam,
) -> Result<Projection> {
    let result = engine.project(rhs, team);
    // Fatal validation/allocation errors can precede resetting the last report.
    // They cannot become resumed frames or produce a published estimator.
    if result
        .as_ref()
        .is_err_and(|e| e.code != ErrorCode::NonConvergence)
    {
        return result;
    }
    work.observe(
        phase,
        engine.last_diagnostics(),
        engine.last_setup_diagnostics(),
    )?;
    result
}
pub(super) fn original_score(
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
pub(super) fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

fn reference_slots(
    raw: &DenseMatrix,
    roles: &[Role],
    weights: crate::data::Weights<'_>,
) -> Result<Vec<Slot>> {
    let mut slots = reserved(raw.ncols())?;
    for (column, &role) in roles.iter().enumerate() {
        let mut norm = 0_f64;
        for row in 0..raw.nrows() {
            norm = norm.hypot(weights.at(row).sqrt() * raw[(row, column)]);
        }
        if !norm.is_finite() {
            return Err(numerical("absorbed WLS reference norm overflows"));
        }
        slots.push(Slot {
            role,
            reference_norm: norm,
        });
    }
    Ok(slots)
}
impl AbsorbedFit {
    /// Independently recertify the authoritative basis on this freshly projected
    /// design before comparing it with the canonical candidate's omission map.
    pub fn revalidate_basis(
        &self,
        design: &crate::fe::Design<'_>,
        raw: &DenseMatrix,
        roles: &[Role],
        basis: &FrozenBasis,
        policy: Policy,
        team: &ThreadTeam,
    ) -> Result<Fit> {
        if raw.nrows() != design.nobs()
            || raw.ncols() != roles.len()
            || self.within.nrows() != raw.nrows()
            || self.within.ncols() != raw.ncols()
        {
            return Err(invalid(
                "basis revalidation has mismatched design dimensions",
            ));
        }
        let slots = reference_slots(raw, roles, design.weights())?;
        super::solve_with_basis(
            &Problem {
                design: &self.within,
                outcome: &self.within_outcome,
                weights: design.weights(),
                slots: &slots,
                has_absorbed_intercept: design.topology().effects().iter().any(|e| e.intercept),
            },
            policy,
            Some(basis),
            team,
        )
    }
}
