use super::certificate::CertificateContext;
use super::map::absorb_map_with_context;
use super::projection::ParallelProjectionPlan;
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_accelerated_map, absorb_map, FeDesign, FeSolverKind, LsmrConfig, MapConfig, SolveReport,
    SolveStopReason,
};
use crate::memory::SolverWorkspacePeak;
use crate::memory::{generic_fe_solver_bytes, owned_fe_matrix_bytes, FeSolverMemoryBudget};

/// Frozen safety rule selected on calibration cases: direct LSMR solves two
/// decimal orders tighter than the requested publication tolerance, bounded
/// at `1e-14` so ordinary requests do not chase sub-roundoff recurrence noise.
pub(crate) fn direct_lsmr_tolerance(requested_tolerance: f64) -> f64 {
    if requested_tolerance <= 1e-14 {
        requested_tolerance
    } else {
        (requested_tolerance * 1e-2).max(1e-14)
    }
}

/// fedemean's memory limit within `budget`: the usable bytes beyond the
/// caller's execution bytes.
fn fedemean_memory_limit(budget: Option<FeSolverMemoryBudget>) -> Option<u128> {
    budget.map(|b| b.usable_bytes.saturating_sub(b.execution_bytes))
}

/// Unified fixed-effect absorption configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsorbConfig {
    pub solver: FeSolverKind,
    /// Sanctioned developer-only routing qualification arm; None is incumbent.
    pub routing: Option<super::RoutingPreset>,
    /// Developer-only coefficient-metric IT experiment; false retains incumbent IT.
    pub it_coefficients: bool,
    /// Maximum distinct-neighbor degree eliminated in the experimental CMG graph.
    pub cmg_schur_degree: usize,
    /// Sampled clique trees per high-degree worker; zero retains exact stars.
    pub cmg_schur_samples: u32,
    /// Deterministic sampled-elimination seed, restricted to exact Stata integers.
    pub cmg_schur_seed: u64,
    pub map: MapConfig,
    pub lsmr: LsmrConfig,
    /// Reduction and preconditioner of an explicit [`FeSolverKind::Pcg`]
    /// request (the default otherwise).
    pub pcg: super::PcgRequest,
    /// Whether `Auto` may retry the other certified solver after a failed certificate.
    pub certified_fallback: bool,
    /// Internal joint execution-plus-solver workspace budget. This is populated
    /// by the existing memory-limit seam and is not public estimator syntax.
    pub solver_memory_budget: Option<FeSolverMemoryBudget>,
    /// Retain coefficient-space solver products for zero-extra-solve saved-FE
    /// recovery. Callers enable this only when saved channels are requested.
    pub retain_coefficients: bool,
    /// With `certified_fallback` off, a two-FE PCG certificate miss accepts
    /// its rejected candidate when this re-check passes at the same tolerance
    /// instead of returning `NonConvergence`. Only the separation detector's
    /// last-resort attempt sets it (SPEC §14); `None` everywhere else.
    pub declined_recheck: Option<super::CertificateRecheck>,
}

impl Default for AbsorbConfig {
    fn default() -> Self {
        Self {
            solver: FeSolverKind::Auto,
            routing: None,
            it_coefficients: false,
            cmg_schur_degree: 5,
            cmg_schur_samples: 0,
            cmg_schur_seed: 0,
            map: MapConfig::default(),
            lsmr: LsmrConfig::default(),
            pcg: super::PcgRequest::default(),
            certified_fallback: true,
            solver_memory_budget: None,
            retain_coefficients: false,
            declined_recheck: None,
        }
    }
}

impl AbsorbConfig {
    pub(crate) fn validate_cmg_schur_options(self) -> Result<()> {
        if self.routing.is_some() && self.solver != FeSolverKind::Auto {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "routing qualification requires automatic solver selection",
            ));
        }
        if self.solver != FeSolverKind::Pcg && self.pcg != super::PcgRequest::default() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "a CG reduction or preconditioner requires the explicit CG request",
            ));
        }
        if self.solver != FeSolverKind::CmgSchur
            && (self.cmg_schur_samples != 0 || self.cmg_schur_seed != 0)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "CMG sampling options require explicit solver(cg, reduce(eliminate))",
            ));
        }
        super::cmg_schur::Sampling {
            samples: self.cmg_schur_samples,
            seed: self.cmg_schur_seed,
        }
        .validate()
    }
}

/// Residualize with explicit solver selection or deterministic automatic routing.
pub fn absorb_fixed_effects(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
    config.validate_cmg_schur_options()?;
    match config.solver {
        FeSolverKind::Map => {
            let peak = admit_generic_solver(design, values, config, FeSolverKind::Map, 0)?;
            let mut report = absorb_map(design, weights, values, config.map)?;
            report.solver_workspace_peak = peak;
            Ok(report)
        }
        FeSolverKind::Lsmr => absorb_explicit_lsmr(design, weights, values, config).map(|(r, _)| r),
        FeSolverKind::Cg | FeSolverKind::IronsTuck | FeSolverKind::Squarem => {
            let peak = admit_generic_solver(design, values, config, config.solver, 0)?;
            let mut report =
                absorb_accelerated_map(design, weights, values, config.map, config.solver)?;
            report.solver_workspace_peak = peak;
            Ok(report)
        }
        FeSolverKind::Cmg | FeSolverKind::CmgSchur => Err(FeregError::new(
            ErrorCode::Unsupported,
            "CMG solvers require a prepared intercept-only categorical topology",
        )),
        FeSolverKind::Elimination => Err(elimination_request()),
        FeSolverKind::Pcg => Err(pcg_intercepts_only()),
        FeSolverKind::Auto => absorb_auto(design, weights, values, config),
    }
}

/// Explicit `solver(lsmr)` on an owned design (slope designs): fedemean's
/// modified LSMR on the full system, certified independently at the request
/// after at most one correction. A fit it does not certify (a numerical or
/// memory decline, or a missed certificate) and a design it does not take
/// continue, with `certified_fallback`, with MAP from the original input,
/// reported as the fallback with the reason; without it the uncertified
/// report (or, for a design fedemean does not take, `Unsupported`) returns.
/// The admission charges fereg's held buffers (input copy, coefficient bank)
/// and its certificate scratch; fedemean admits its own workspace within the
/// budget left beside the held buffers. With `retain_coefficients`, also
/// fedemean's fitted FE coefficients (channel-major).
pub(crate) fn absorb_explicit_lsmr(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<(SolveReport, Option<super::FeCoefficientBatch>)> {
    let layout = config
        .retain_coefficients
        .then(|| super::fedemean_slope::Layout::new(design))
        .transpose()?;
    // The input copy and the coefficient bank coexist with fedemean's
    // workspace within the budget.
    let held = owned_fe_matrix_bytes(values.nrows(), values.ncols())?
        + layout
            .as_ref()
            .map_or(0, |layout| layout.bytes(values.ncols()));
    let admitted = admit_generic_solver(design, values, config, FeSolverKind::Lsmr, held)?;
    let original = fallible_matrix_copy(values, "unable to reserve explicit LSMR fallback matrix")?;
    let mut coefficients = layout
        .as_ref()
        .map(|layout| layout.zeros(values.ncols()))
        .transpose()?;
    let attempt = super::lsmr::absorb_lsmr_with(
        design,
        weights,
        values,
        config.lsmr,
        (
            config.lsmr.tolerance,
            fedemean_memory_limit(config.solver_memory_budget)
                .map(|limit| limit.saturating_sub(held)),
        ),
        layout.as_ref().zip(coefficients.as_deref_mut()),
    )?;
    let (iterations, attempted_peak, reason) = match attempt {
        Some((mut report, route)) => {
            report.solver_workspace_peak = match route {
                // fedemean's own workspace beside the held buffers; fereg's
                // certificate runs after fedemean released it.
                super::lsmr::LsmrRoute::Fedemean => report
                    .solver_workspace_peak
                    .with_retained(held)?
                    .max(admitted),
                super::lsmr::LsmrRoute::NoFixedEffects => admitted,
            };
            if report.certified() {
                let batch = match (layout, coefficients) {
                    (Some(layout), Some(retained)) if route == super::lsmr::LsmrRoute::Fedemean => {
                        Some(layout.batch(retained, values.ncols())?)
                    }
                    _ => None,
                };
                return Ok((report, batch));
            }
            if !config.certified_fallback {
                return Ok((report, None));
            }
            let reason = report.warnings.last().map_or_else(
                || "fedemean's modified LSMR did not certify".to_owned(),
                Clone::clone,
            );
            (report.iterations, report.solver_workspace_peak, reason)
        }
        None if !config.certified_fallback => return Err(super::lsmr::ineligible()),
        None => (
            0,
            admitted,
            "fedemean's full system does not take this fixed-effect design".to_owned(),
        ),
    };
    values.as_mut_slice().copy_from_slice(original.as_slice());
    drop(original);
    let fallback_peak = admit_generic_solver(design, values, config, FeSolverKind::Map, 0)?;
    let mut fallback = absorb_map(design, weights, values, config.map)?;
    fallback.solver_workspace_peak = attempted_peak.max(fallback_peak);
    fallback.requested_solver = FeSolverKind::Lsmr;
    fallback.fallback_used = true;
    fallback.warnings.insert(
        0,
        format!(
            "explicit LSMR path did not certify after {iterations} iterations ({reason}); retried MAP from the original input"
        ),
    );
    Ok((fallback, None))
}

/// Explicit `solver(cg, reduce() precondition())` other than CMG runs on
/// categorical intercept designs only.
pub(crate) fn pcg_intercepts_only() -> FeregError {
    FeregError::new(
        ErrorCode::Unsupported,
        "solver(cg) with reduce() or a jacobi, block, schwarz or none preconditioner needs categorical intercept fixed effects only (no slopes); use solver(lsmr) or solver(map)",
    )
}

/// The exact elimination is an Auto route only; it cannot be requested.
pub(crate) fn elimination_request() -> FeregError {
    FeregError::new(
        ErrorCode::Unsupported,
        "the exact elimination is selected by Auto and cannot be requested",
    )
}

fn absorb_auto(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
    // Validate before optional geometry can index current-frame weights.
    if values.nrows() != design.nobs()
        || !config.map.tolerance.is_finite()
        || config.map.tolerance <= 0.
        || config.map.max_iterations == 0
        || config.map.check_every == 0
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "general FE projection inputs are invalid",
        ));
    }
    let mut peak = SolverWorkspacePeak::ZERO;
    let mut prior = None;
    // A coefficient-space solve preserves the original projection identity.
    // Reserve half the shared budget for one MAP continuation from untouched
    // inputs; an FE moment certificate alone cannot authorize a CG handoff.
    if design.effects().len() > 1 && config.map.max_iterations > 1 {
        // The candidate coexists with fedemean's workspace.
        let held = owned_fe_matrix_bytes(values.nrows(), values.ncols())?;
        match admit_generic_solver(design, values, config, FeSolverKind::Lsmr, held) {
            Ok(admitted) => {
                let mut candidate =
                    fallible_matrix_copy(values, "unable to reserve general FE candidate")?;
                peak = SolverWorkspacePeak::from_bytes(held)?;
                // fedemean's modified LSMR on the full system, certified
                // independently at the request (see `absorb_lsmr_with`).
                let policy = LsmrConfig {
                    tolerance: config.map.tolerance,
                    max_iterations: config
                        .lsmr
                        .max_iterations
                        .min(config.map.max_iterations / 2),
                    ..config.lsmr
                };
                let attempt = super::lsmr::absorb_lsmr_with(
                    design,
                    weights,
                    &mut candidate,
                    policy,
                    (
                        config.map.tolerance,
                        fedemean_memory_limit(config.solver_memory_budget)
                            .map(|limit| limit.saturating_sub(held)),
                    ),
                    None,
                )?;
                // `None`: fedemean is not eligible; MAP alone, as before.
                if let Some((mut report, route)) = attempt {
                    report.requested_solver = FeSolverKind::Auto;
                    report.solver_workspace_peak = if route == super::lsmr::LsmrRoute::Fedemean {
                        report
                            .solver_workspace_peak
                            .with_retained(held)?
                            .max(admitted)
                    } else {
                        admitted
                    };
                    peak = report.solver_workspace_peak;
                    if report.certificate.passed {
                        report.stop_reason = SolveStopReason::Certified;
                        *values = candidate;
                        return Ok(report);
                    }
                    prior = Some(report);
                }
            }
            Err(e) if e.code == ErrorCode::Allocation => {}
            Err(e) => return Err(e),
        }
    }
    let consumed = prior.as_ref().map_or(0, |r| r.iterations);
    let remaining = config
        .map
        .max_iterations
        .checked_sub(consumed)
        .filter(|n| *n > 0)
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::NonConvergence,
                "general FE solve exhausted its shared work budget",
            )
        })?;
    let admitted = admit_generic_solver(design, values, config, FeSolverKind::Map, 0)?;
    let mut context = CertificateContext::new(design, weights);
    let mut retained = admitted;
    if let Ok(bytes) = context.cache_bytes() {
        if let Ok(cached) = retained.with_retained(bytes) {
            if config
                .solver_memory_budget
                .is_none_or(|b| cached.bytes().is_some_and(|bytes| b.fits(bytes as u128)))
            {
                match context.cache() {
                    Ok(()) => retained = cached,
                    Err(e) if e.code == ErrorCode::Allocation => {}
                    Err(e) => return Err(e),
                }
            }
        }
    }
    if let Ok(bytes) = context.parallel_workspace_bytes(values.ncols()) {
        if let Ok(parallel) = retained.with_retained(bytes) {
            if config
                .solver_memory_budget
                .is_none_or(|b| parallel.bytes().is_some_and(|bytes| b.fits(bytes as u128)))
            {
                context.set_parallel_rhs(values.ncols());
                retained = parallel;
            }
        }
    }
    let plan = ParallelProjectionPlan::for_design(design, values.ncols())
        .ok()
        .flatten();
    let (plan, retained) = admit_parallel_projection(plan, retained, config.solver_memory_budget);
    peak = peak.max(retained);
    let mut report = absorb_map_with_context(
        &context,
        values,
        MapConfig {
            max_iterations: remaining,
            ..config.map
        },
        plan,
    )?;
    if !report.certified() {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "general FE projection exhausted its iteration budget",
        ));
    }
    report.requested_solver = FeSolverKind::Auto;
    report.solver_workspace_peak = peak;
    if let Some(prior) = prior {
        report.iterations += prior.iterations;
        report.setup_seconds += prior.setup_seconds;
        report.solve_seconds += prior.solve_seconds;
        report.certification_seconds += prior.certification_seconds;
        report.fallback_used = true;
        report.requested_preconditioner = prior.requested_preconditioner;
        report.used_preconditioner = prior.used_preconditioner;
    }
    Ok(report)
}

/// Optional projection storage is last in admission priority: it cannot evict
/// the existing serial route, its geometry, or its admitted certificate scratch.
fn admit_parallel_projection(
    plan: Option<ParallelProjectionPlan>,
    base: SolverWorkspacePeak,
    budget: Option<FeSolverMemoryBudget>,
) -> (Option<ParallelProjectionPlan>, SolverWorkspacePeak) {
    if let Some(plan) = plan {
        if let Ok(peak) = plan.bytes().and_then(|bytes| base.with_retained(bytes)) {
            if budget
                .is_none_or(|budget| peak.bytes().is_some_and(|bytes| budget.fits(bytes as u128)))
            {
                return (Some(plan), peak);
            }
        }
    }
    (None, base)
}

/// Admit a generic owned route: fereg's own workspace for `solver` (for
/// LSMR, the certificate and projection scratch fereg holds beside
/// fedemean, which admits its own workspace within the remaining budget)
/// beside `held` bytes the caller keeps alive.
fn admit_generic_solver(
    design: &FeDesign,
    values: &DenseMatrix,
    config: AbsorbConfig,
    solver: FeSolverKind,
    held: u128,
) -> Result<SolverWorkspacePeak> {
    let required = generic_fe_solver_bytes(design, values.ncols(), solver)?
        .checked_add(held)
        .ok_or_else(|| {
            FeregError::new(
                ErrorCode::Allocation,
                "generic FE joint workspace overflows",
            )
        })?;
    if let Some(budget) = config.solver_memory_budget {
        if !budget.fits(required) {
            return Err(FeregError::new(ErrorCode::Allocation, format!(
                "generic {solver:?} requires {required} workspace bytes beside {} execution bytes; usable={}",
                budget.execution_bytes, budget.usable_bytes)));
        }
    }
    SolverWorkspacePeak::from_bytes(required)
}

fn fallible_matrix_copy(input: &DenseMatrix, label: &'static str) -> Result<DenseMatrix> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(input.as_slice().len())
        .map_err(|_| FeregError::new(ErrorCode::Allocation, label))?;
    values.extend_from_slice(input.as_slice());
    DenseMatrix::from_row_major(input.nrows(), input.ncols(), values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{EffectTerm, SolveStopReason};

    #[test]
    fn cmg_sampling_options_reject_other_routes_and_inexact_seeds_before_allocation() {
        let design = FeDesign::new(
            2,
            vec![EffectTerm::new(vec![0, 0], 1, true, vec![]).unwrap()],
        )
        .unwrap();
        let original = DenseMatrix::from_columns(&[vec![1.0, -1.0]]).unwrap();
        for solver in [
            FeSolverKind::Auto,
            FeSolverKind::Map,
            FeSolverKind::Lsmr,
            FeSolverKind::Cmg,
            FeSolverKind::CmgSchur,
        ] {
            for (samples, seed) in [(0, 1), (1, 1_u64 << 53), (1, 0)] {
                if solver == FeSolverKind::CmgSchur && seed == 0 {
                    continue;
                }
                let config = AbsorbConfig {
                    solver,
                    cmg_schur_samples: samples,
                    cmg_schur_seed: seed,
                    solver_memory_budget: Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: 0,
                    }),
                    ..AbsorbConfig::default()
                };
                let mut values = original.clone();
                let error =
                    absorb_fixed_effects(&design, &[1.0; 2], &mut values, config).unwrap_err();
                assert_eq!(error.code, ErrorCode::InvalidArgument);
                assert_eq!(values, original);
            }
        }
        let boundary = AbsorbConfig {
            solver: FeSolverKind::CmgSchur,
            cmg_schur_samples: 1,
            cmg_schur_seed: (1_u64 << 53) - 1,
            ..AbsorbConfig::default()
        };
        boundary.validate_cmg_schur_options().unwrap();
    }

    #[test]
    fn auto_slope_design_returns_a_certified_bounded_route() {
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![vec![0.0, 1.0, 0.0, 1.0]])
            .expect("effect");
        let other = EffectTerm::new(vec![0, 1, 0, 1], 2, true, vec![]).expect("other");
        let design = FeDesign::new(4, vec![effect, other]).expect("design");
        let mut values = DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("values");
        let report = absorb_fixed_effects(&design, &[1.0; 4], &mut values, AbsorbConfig::default())
            .expect("absorb");
        assert_eq!(report.requested_solver, FeSolverKind::Auto);
        assert!(matches!(
            report.used_solver,
            FeSolverKind::Map | FeSolverKind::Cg | FeSolverKind::Lsmr
        ));
        assert!(matches!(report.stop_reason, SolveStopReason::Certified));
    }

    #[test]
    fn auto_fails_closed_when_projection_exhausts_the_total_budget() {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 1, 1, 2, 2, 0], 3, true, vec![]).expect("second");
        let design = FeDesign::new(6, vec![first, second]).expect("design");
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("values");
        let error = absorb_fixed_effects(
            &design,
            &[1.0; 6],
            &mut values,
            AbsorbConfig {
                map: MapConfig {
                    max_iterations: 1,
                    check_every: 1,
                    tolerance: 1e-12,
                    symmetric: true,
                },
                lsmr: LsmrConfig {
                    tolerance: 1e-10,
                    ..LsmrConfig::default()
                },
                ..AbsorbConfig::default()
            },
        )
        .expect_err("uncertified auto result must fail closed");
        assert_eq!(error.code, ErrorCode::NonConvergence);
    }

    #[test]
    fn explicit_lsmr_falls_back_to_certified_map_when_budget_is_exhausted() {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).expect("first");
        let second = EffectTerm::new(vec![0, 1, 1, 2, 2, 0], 3, true, vec![]).expect("second");
        let design = FeDesign::new(6, vec![first, second]).expect("design");
        let mut values =
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("values");
        let report = absorb_fixed_effects(
            &design,
            &[1.0; 6],
            &mut values,
            AbsorbConfig {
                solver: FeSolverKind::Lsmr,
                lsmr: LsmrConfig {
                    max_iterations: 1,
                    tolerance: 1e-14,
                    ..LsmrConfig::default()
                },
                ..AbsorbConfig::default()
            },
        )
        .expect("fallback");
        assert_eq!(report.requested_solver, FeSolverKind::Lsmr);
        assert_eq!(report.used_solver, FeSolverKind::Map);
        assert!(report.fallback_used);
        assert!(report.certified());
    }
}

#[cfg(test)]
mod generic_budget_regression {
    use super::*;
    use crate::fe::EffectTerm;

    fn crossed_slope() -> (FeDesign, Vec<f64>, Vec<f64>) {
        let n = 48;
        let left = (0..n).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let right = (0..n).map(|row| ((row / 4) % 3) as u32).collect::<Vec<_>>();
        let slope = (0..n)
            .map(|row| (row / 12) as f64 - 1.5)
            .collect::<Vec<_>>();
        // The four equal-weight observations in every crossed cell have
        // residuals [1,-1,-1,1], orthogonal to both intercept and slope.
        let noise = (0..n)
            .map(|row| [1.0, -1.0, -1.0, 1.0][row / 12])
            .collect::<Vec<_>>();
        let outcome = (0..n)
            .map(|row| 2.0 * left[row] as f64 + 3.0 * right[row] as f64 * slope[row] + noise[row])
            .collect::<Vec<_>>();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(left, 4, true, vec![]).unwrap(),
                EffectTerm::new(right, 3, false, vec![slope]).unwrap(),
            ],
        )
        .unwrap();
        (design, outcome, noise)
    }

    fn check_budget(solver: FeSolverKind) {
        let (design, outcome, noise) = crossed_slope();
        let weights = vec![1.0; design.nobs()];
        let mut config = AbsorbConfig {
            solver,
            ..AbsorbConfig::default()
        };
        config.map.tolerance = 1e-12;
        config.lsmr.tolerance = 1e-12;
        let mut values = DenseMatrix::from_columns(&[outcome.clone()]).unwrap();
        let report = absorb_fixed_effects(&design, &weights, &mut values, config).unwrap();
        assert!(report.certified());
        assert!(values
            .as_slice()
            .iter()
            .zip(&noise)
            .all(|(a, b)| (a - b).abs() < 1e-10));
        config.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 0,
            usable_bytes: 1,
        });
        let mut values = DenseMatrix::from_columns(&[outcome]).unwrap();
        let error = absorb_fixed_effects(&design, &weights, &mut values, config)
            .expect_err("generic slope projection must enforce its joint workspace budget");
        assert_eq!(error.code, ErrorCode::Allocation);
    }
    #[test]
    fn generic_explicit_fallback_keeps_the_discarded_lsmr_envelope() {
        let first = EffectTerm::new(vec![0, 0, 1, 1, 2, 2], 3, true, vec![]).unwrap();
        let second = EffectTerm::new(vec![0, 1, 1, 2, 2, 0], 3, true, vec![]).unwrap();
        let design = FeDesign::new(6, vec![first, second]).unwrap();
        let input = vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0];
        let mut config = AbsorbConfig {
            solver: FeSolverKind::Lsmr,
            lsmr: LsmrConfig {
                max_iterations: 1,
                tolerance: 1e-14,
                ..LsmrConfig::default()
            },
            ..AbsorbConfig::default()
        };
        let mut original = DenseMatrix::from_columns(&[input.clone()]).unwrap();
        let original_report =
            absorb_fixed_effects(&design, &[1.0; 6], &mut original, config).unwrap();
        let attempt = generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr).unwrap()
            + owned_fe_matrix_bytes(6, 1).unwrap();
        let fallback = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map).unwrap();
        assert!(attempt > fallback);
        assert_eq!(
            original_report.solver_workspace_peak.bytes(),
            Some(attempt as u64)
        );
        config.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 137,
            usable_bytes: 137 + attempt,
        });
        let mut values = DenseMatrix::from_columns(&[input]).unwrap();
        let report = absorb_fixed_effects(&design, &[1.0; 6], &mut values, config).unwrap();
        assert_eq!(report.used_solver, FeSolverKind::Map);
        assert!(report.certified() && report.fallback_used);
        assert_eq!(report.solver_workspace_peak.bytes(), Some(attempt as u64));
        assert_eq!(values, original);
        assert_eq!(report.certificate, original_report.certificate);
        assert_eq!(report.iterations, original_report.iterations);
    }

    #[test]
    fn optional_geometry_admission_preserves_affordable_map_and_charges_cache() {
        let (design, outcome, _) = crossed_slope();
        let weights = vec![1.0; design.nobs()];
        let config = AbsorbConfig::default();
        let required = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map).unwrap();
        let cache = CertificateContext::new(&design, &weights)
            .cache_bytes()
            .unwrap();
        // The LSMR attempt (fereg's scratch beside the input copy) is
        // admitted from `attempt` bytes; fedemean's own workspace does not fit
        // beside it in these budgets, so it declines and MAP continues,
        // reported as the fallback, with the attempt's envelope in its peak.
        let attempt = generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr).unwrap()
            + owned_fe_matrix_bytes(design.nobs(), 1).unwrap();
        let mut reference = None;
        for usable in [required, required + cache - 1, required + cache] {
            let mut values = DenseMatrix::from_columns(&[outcome.clone()]).unwrap();
            let report = absorb_fixed_effects(
                &design,
                &weights,
                &mut values,
                AbsorbConfig {
                    solver_memory_budget: Some(FeSolverMemoryBudget {
                        execution_bytes: 137,
                        usable_bytes: 137 + usable,
                    }),
                    ..config
                },
            )
            .unwrap();
            assert_eq!(report.used_solver, FeSolverKind::Map);
            assert_eq!(report.fallback_used, usable >= attempt);
            let map = if usable < required + cache {
                required
            } else {
                required + cache
            };
            let envelope = if usable >= attempt { attempt } else { 0 };
            assert_eq!(
                report.solver_workspace_peak.bytes(),
                Some(map.max(envelope) as u64)
            );
            let result = (
                values,
                report.certificate,
                report.iterations,
                report.pilot_contraction,
            );
            if let Some(previous) = &reference {
                assert_eq!(&result, previous);
            }
            reference = Some(result);
        }
    }

    #[test]
    fn optional_geometry_is_released_before_an_exact_budget_map_continuation() {
        let n = 480;
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    (0..n).map(|r| (r % 17) as u32).collect(),
                    17,
                    true,
                    vec![(0..n).map(|r| (r as f64).sin()).collect()],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..n).map(|r| (r % 13) as u32).collect(),
                    13,
                    true,
                    vec![(0..n).map(|r| (r as f64 * 0.7).cos()).collect()],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let weights = vec![1.0; n];
        let mut original = DenseMatrix::zeros(n, 8).unwrap();
        for row in 0..n {
            for rhs in 0..8 {
                original[(row, rhs)] = ((row * 13 + rhs * 7) as f64 * 0.31).sin()
                    + (row % 17) as f64 * (row as f64).sin();
            }
        }
        let config = AbsorbConfig {
            map: MapConfig {
                tolerance: 1e-12,
                ..MapConfig::default()
            },
            ..AbsorbConfig::default()
        };
        let _map = generic_fe_solver_bytes(&design, 8, FeSolverKind::Map).unwrap();
        let cg = generic_fe_solver_bytes(&design, 8, FeSolverKind::Map).unwrap();
        let cache = CertificateContext::new(&design, &weights)
            .cache_bytes()
            .unwrap();
        let mut reference = None;
        for budget in [cg, cg + cache - 1, cg + cache] {
            let mut values = original.clone();
            let report = absorb_fixed_effects(
                &design,
                &weights,
                &mut values,
                AbsorbConfig {
                    solver_memory_budget: Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: budget,
                    }),
                    ..config
                },
            )
            .unwrap();
            assert_eq!(report.used_solver, FeSolverKind::Map);
            assert!(!report.fallback_used);
            assert_eq!(
                report.solver_workspace_peak.bytes(),
                Some(if budget < cg + cache { cg } else { cg + cache } as u64)
            );
            let result = (
                values,
                report.certificate,
                report.iterations,
                report.pilot_contraction,
            );
            if let Some(previous) = &reference {
                assert_eq!(&result, previous);
            }
            reference = Some(result);
        }
    }

    #[test]
    fn parallel_scratch_can_be_released_without_discarding_affordable_map_geometry() {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| {
                let n = 16_416;
                let design = FeDesign::new(
                    n,
                    vec![
                        EffectTerm::new(
                            (0..n).map(|r| (r % 170) as u32).collect(),
                            170,
                            true,
                            vec![(0..n).map(|r| (r as f64).sin()).collect()],
                        )
                        .unwrap(),
                        EffectTerm::new(
                            (0..n).map(|r| (r % 130) as u32).collect(),
                            130,
                            true,
                            vec![(0..n).map(|r| (r as f64 * 0.7).cos()).collect()],
                        )
                        .unwrap(),
                    ],
                )
                .unwrap();
                let weights = vec![1.0; n];
                let mut original = DenseMatrix::zeros(n, 8).unwrap();
                for row in 0..n {
                    for rhs in 0..8 {
                        original[(row, rhs)] = ((row * 13 + rhs * 7) as f64 * 0.31).sin()
                            + (row % 170) as f64 * (row as f64).sin();
                    }
                }
                let config = AbsorbConfig {
                    map: MapConfig {
                        tolerance: 1e-12,
                        ..MapConfig::default()
                    },
                    ..AbsorbConfig::default()
                };
                let _map = generic_fe_solver_bytes(&design, 8, FeSolverKind::Map).unwrap();
                let cg = generic_fe_solver_bytes(&design, 8, FeSolverKind::Map).unwrap();
                let cache = CertificateContext::new(&design, &weights)
                    .cache_bytes()
                    .unwrap();
                let mut context = CertificateContext::new(&design, &weights);
                context.cache().unwrap();
                let parallel = context.parallel_workspace_bytes(8).unwrap();
                assert!(parallel > 0);
                let projection = ParallelProjectionPlan::for_design(&design, 8)
                    .unwrap()
                    .unwrap()
                    .bytes()
                    .unwrap();
                let mut reference = None;
                for budget in [
                    cg,
                    cg + cache - 1,
                    cg + cache,
                    cg + cache + parallel,
                    cg + cache + parallel + projection - 1,
                    cg + cache + parallel + projection,
                ] {
                    let mut values = original.clone();
                    let report = absorb_fixed_effects(
                        &design,
                        &weights,
                        &mut values,
                        AbsorbConfig {
                            solver_memory_budget: Some(FeSolverMemoryBudget {
                                execution_bytes: 0,
                                usable_bytes: budget,
                            }),
                            ..config
                        },
                    )
                    .unwrap();
                    assert_eq!(report.used_solver, FeSolverKind::Map);
                    assert!(!report.fallback_used);
                    let continuation = if budget < cg + cache {
                        cg
                    } else if budget < cg + cache + parallel {
                        cg + cache
                    } else {
                        cg + cache + parallel
                    };
                    let expected = if continuation + projection <= budget {
                        continuation + projection
                    } else {
                        continuation
                    };
                    assert_eq!(report.solver_workspace_peak.bytes(), Some(expected as u64));
                    let result = (
                        values,
                        report.certificate,
                        report.iterations,
                        report.pilot_contraction,
                    );
                    if let Some(previous) = &reference {
                        assert_eq!(&result, previous);
                    }
                    reference = Some(result);
                }
            });
    }

    #[test]
    fn parallel_projection_exact_budget_preserves_geometry_and_certificate_priority() {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| {
                let n = 48 * 342;
                let left: Vec<_> = (0..n).map(|r| (r % 4) as u32).collect();
                let right: Vec<_> = (0..n).map(|r| ((r / 4) % 3) as u32).collect();
                let slope: Vec<_> = (0..n).map(|r| ((r / 12) % 4) as f64 - 1.5).collect();
                let outcome: Vec<_> = (0..n)
                    .map(|r| {
                        2.0 * left[r] as f64
                            + 3.0 * right[r] as f64 * slope[r]
                            + [1.0, -1.0, -1.0, 1.0][(r / 12) % 4]
                    })
                    .collect();
                let design = FeDesign::new(
                    n,
                    vec![
                        EffectTerm::new(left, 4, true, vec![]).unwrap(),
                        EffectTerm::new(right, 3, false, vec![slope]).unwrap(),
                    ],
                )
                .unwrap();
                let weights = vec![1.0; n];
                let config = AbsorbConfig::default();
                let required = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map).unwrap();
                let mut context = CertificateContext::new(&design, &weights);
                context.cache().unwrap();
                let cached = required + context.cache_bytes().unwrap();
                let parallel = context.parallel_workspace_bytes(1).unwrap();
                assert_eq!(parallel, 4 * 2 * 8);
                let mut reference = None;
                let projection = ParallelProjectionPlan::for_design(&design, 1)
                    .unwrap()
                    .unwrap()
                    .bytes()
                    .unwrap();
                for budget in [
                    cached,
                    cached + parallel - 1,
                    cached + parallel,
                    cached + parallel + projection - 1,
                    cached + parallel + projection,
                ] {
                    let mut values = DenseMatrix::from_columns(&[outcome.clone()]).unwrap();
                    let report = absorb_fixed_effects(
                        &design,
                        &weights,
                        &mut values,
                        AbsorbConfig {
                            solver_memory_budget: Some(FeSolverMemoryBudget {
                                execution_bytes: 137,
                                usable_bytes: 137 + budget,
                            }),
                            ..config
                        },
                    )
                    .unwrap();
                    assert_eq!(report.used_solver, FeSolverKind::Map);
                    assert_eq!(
                        report.solver_workspace_peak.bytes(),
                        Some(if budget < cached + parallel {
                            cached
                        } else if budget < cached + parallel + projection {
                            cached + parallel
                        } else {
                            cached + parallel + projection
                        } as u64)
                    );
                    let result = (
                        values,
                        report.certificate,
                        report.iterations,
                        report.pilot_contraction,
                    );
                    if let Some(previous) = &reference {
                        assert_eq!(&result, previous);
                    }
                    reference = Some(result);
                }
            });
    }

    #[test]
    fn generic_auto_enforces_joint_budget() {
        check_budget(FeSolverKind::Auto);
    }
    #[test]
    fn generic_map_enforces_joint_budget() {
        check_budget(FeSolverKind::Map);
    }
    #[test]
    fn generic_lsmr_enforces_joint_budget() {
        check_budget(FeSolverKind::Lsmr);
    }
    #[test]
    fn generic_cg_enforces_joint_budget() {
        check_budget(FeSolverKind::Cg);
    }

    #[test]
    fn explicit_owned_lsmr_runs_fedemean_and_certifies_the_request() {
        let (design, outcome, _) = crossed_slope();
        let weights = (0..design.nobs())
            .map(|row| 1.0 + (row % 5) as f64)
            .collect::<Vec<_>>();
        let lsmr = LsmrConfig {
            tolerance: 1e-8,
            ..LsmrConfig::default()
        };
        let config = AbsorbConfig {
            solver: FeSolverKind::Lsmr,
            lsmr,
            ..AbsorbConfig::default()
        };
        let mut routed = DenseMatrix::from_columns(&[outcome.clone()]).unwrap();
        let report = absorb_fixed_effects(&design, &weights, &mut routed, config).unwrap();
        let mut expected_values = DenseMatrix::from_columns(&[outcome]).unwrap();
        let expected = super::super::lsmr::absorb_lsmr_certified_at(
            &design,
            &weights,
            &mut expected_values,
            lsmr,
            lsmr.tolerance,
        )
        .unwrap();
        assert_eq!(routed, expected_values);
        assert_eq!(report.used_solver, FeSolverKind::Lsmr);
        assert!(!report.fallback_used);
        assert_eq!(report.iterations, expected.iterations);
        assert_eq!(report.certificate, expected.certificate);
        assert!(report.certified());
    }

    #[test]
    fn generic_exact_workspace_and_one_byte_short_preserve_input() {
        let (design, outcome, noise) = crossed_slope();
        let weights = vec![1.0; design.nobs()];
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for solver in [
                        FeSolverKind::Map,
                        FeSolverKind::Cg,
                        FeSolverKind::IronsTuck,
                        FeSolverKind::Squarem,
                        FeSolverKind::Lsmr,
                    ] {
                        for preconditioner in [
                            crate::fe::LsmrPreconditioner::Off,
                            crate::fe::LsmrPreconditioner::Diagonal,
                            crate::fe::LsmrPreconditioner::Schwarz,
                            crate::fe::LsmrPreconditioner::Auto,
                        ] {
                            let mut config = AbsorbConfig {
                                solver,
                                ..AbsorbConfig::default()
                            };
                            config.lsmr.preconditioner = preconditioner;
                            let reserve = generic_fe_solver_bytes(&design, 1, solver).unwrap()
                                + if solver == FeSolverKind::Lsmr {
                                    owned_fe_matrix_bytes(design.nobs(), 1).unwrap()
                                } else {
                                    0
                                };
                            for short in [false, true] {
                                let mut values =
                                    DenseMatrix::from_columns(&[outcome.clone()]).unwrap();
                                config.solver_memory_budget = Some(FeSolverMemoryBudget {
                                    execution_bytes: 137,
                                    usable_bytes: 137 + reserve - u128::from(short),
                                });
                                let result =
                                    absorb_fixed_effects(&design, &weights, &mut values, config);
                                if short {
                                    assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
                                    assert_eq!(values.as_slice(), outcome);
                                } else {
                                    let report = result.unwrap();
                                    assert!(report.certified());
                                    // Explicit LSMR on fedemean publishes its own
                                    // workspace beside the held input copy, within
                                    // the admitted envelope; the others publish it.
                                    let peak = report.solver_workspace_peak.bytes().unwrap();
                                    if solver == FeSolverKind::Lsmr {
                                        assert!(u128::from(peak) <= reserve);
                                    } else {
                                        assert_eq!(peak, reserve as u64);
                                    }
                                    assert!(values
                                        .as_slice()
                                        .iter()
                                        .zip(&noise)
                                        .all(|(a, b)| (a - b).abs() < 1e-10));
                                }
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn generic_auto_admits_map_without_reserving_an_unused_lsmr() {
        let (design, outcome, noise) = crossed_slope();
        let mut values = DenseMatrix::from_columns(&[outcome]).unwrap();
        let mut config = AbsorbConfig::default();
        let reserve = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map).unwrap();
        // The LSMR attempt also holds an input copy beside fereg's scratch.
        assert!(
            reserve
                < generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr).unwrap()
                    + owned_fe_matrix_bytes(design.nobs(), 1).unwrap()
        );
        config.solver_memory_budget = Some(FeSolverMemoryBudget {
            execution_bytes: 137,
            usable_bytes: 137 + reserve,
        });
        let report =
            absorb_fixed_effects(&design, &vec![1.0; design.nobs()], &mut values, config).unwrap();
        assert!(report.certified());
        assert_eq!(report.used_solver, FeSolverKind::Map);
        assert!(!report.fallback_used);
        assert_eq!(report.solver_workspace_peak.bytes(), Some(reserve as u64));
        assert!(values
            .as_slice()
            .iter()
            .zip(&noise)
            .all(|(a, b)| (a - b).abs() < 1e-10));
    }

    #[test]
    fn generic_auto_admits_map_when_slope_lsmr_does_not_fit() {
        let levels = 12;
        let n = levels * 4;
        let left = (0..n).map(|row| (row / 4) as u32).collect::<Vec<_>>();
        let right = (0..n)
            .map(|row| ((row / 4 + (row / 2) % 2) % levels) as u32)
            .collect::<Vec<_>>();
        let slope = (0..n)
            .map(|row| 1.0 + ((row / 2) % 3) as f64)
            .collect::<Vec<_>>();
        let weights = (0..n)
            .map(|row| 1.0 + ((row / 2) % 5) as f64)
            .collect::<Vec<_>>();
        let noise = (0..n)
            .map(|row| if row % 2 == 0 { -1.0 } else { 1.0 })
            .collect::<Vec<_>>();
        let y = (0..n)
            .map(|row| 2.0 * left[row] as f64 + 3.0 * right[row] as f64 * slope[row] + noise[row])
            .collect::<Vec<_>>();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(left, levels, true, vec![]).unwrap(),
                EffectTerm::new(right, levels, false, vec![slope]).unwrap(),
            ],
        )
        .unwrap();
        let config = AbsorbConfig {
            map: MapConfig {
                tolerance: 1e-12,
                ..MapConfig::default()
            },
            ..AbsorbConfig::default()
        };
        let mut unlimited = DenseMatrix::from_columns(&[y.clone()]).unwrap();
        let baseline = absorb_fixed_effects(&design, &weights, &mut unlimited, config).unwrap();
        assert_eq!(baseline.used_solver, FeSolverKind::Lsmr);
        let cg_bytes = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map).unwrap();
        // The LSMR attempt holds an input copy beside fereg's scratch.
        let lsmr_bytes = generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr).unwrap()
            + owned_fe_matrix_bytes(design.nobs(), 1).unwrap();
        assert!(lsmr_bytes > cg_bytes);
        for short in [false, true] {
            let mut values = DenseMatrix::from_columns(&[y.clone()]).unwrap();
            let result = absorb_fixed_effects(
                &design,
                &weights,
                &mut values,
                AbsorbConfig {
                    solver_memory_budget: Some(FeSolverMemoryBudget {
                        execution_bytes: 137,
                        usable_bytes: 137 + cg_bytes - u128::from(short),
                    }),
                    ..config
                },
            );
            if short {
                assert_eq!(result.unwrap_err().code, ErrorCode::Allocation);
            } else {
                let report = result.unwrap();
                assert!(report.certified());
                assert_eq!(report.used_solver, FeSolverKind::Map);
                assert!(!report.fallback_used);
                // The larger LSMR route was never allocated. Only the admitted
                // pilot and CG continuation belong to this peak.
                assert_eq!(report.solver_workspace_peak.bytes(), Some(cg_bytes as u64));
                assert!(values
                    .as_slice()
                    .iter()
                    .zip(&noise)
                    .all(|(a, b)| (a - b).abs() < 1e-8));
            }
        }
    }
}

#[cfg(test)]
mod explicit_slope_lsmr {
    use super::*;
    use crate::fe::{EffectTerm, LsmrPreconditioner};

    /// Case 1 of the within slope boundary fixture: one `g##c.(z1 z2)`
    /// effect whose slopes sit near ±2^24 and differ by units, so the raw
    /// level Gram loses their within-level variation.
    fn boundary() -> (FeDesign, Vec<f64>, DenseMatrix) {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/ols/within_plugin_slope_boundary.csv"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        let column = |name: &str| header.iter().position(|h| *h == name).unwrap();
        let rows: Vec<Vec<&str>> = lines
            .map(|line| line.split(',').collect::<Vec<_>>())
            .filter(|row| row[column("case")] == "1")
            .collect();
        let value = |row: &Vec<&str>, name: &str| row[column(name)].parse::<f64>().unwrap();
        let levels = [3.0, 11.0, 29.0, 97.0];
        let codes = rows
            .iter()
            .map(|row| levels.iter().position(|&l| l == value(row, "g")).unwrap() as u32)
            .collect();
        let slopes = ["z1", "z2"]
            .iter()
            .map(|name| rows.iter().map(|row| value(row, name)).collect())
            .collect();
        let design = FeDesign::new(
            rows.len(),
            vec![EffectTerm::new(codes, 4, true, slopes).unwrap()],
        )
        .unwrap();
        let weights = rows.iter().map(|row| value(row, "w")).collect();
        let values = DenseMatrix::from_columns(&[
            rows.iter().map(|row| value(row, "y")).collect(),
            rows.iter().map(|row| value(row, "q")).collect(),
        ])
        .unwrap();
        (design, weights, values)
    }

    /// The independent projection of `values` on one effect's channel span:
    /// per level, a weighted least-squares fit in the level's centered basis
    /// (the intercept, then each slope less its weighted level mean), solved
    /// by modified Gram-Schmidt.
    fn centered_level_projection(
        design: &FeDesign,
        weights: &[f64],
        values: &DenseMatrix,
    ) -> DenseMatrix {
        let effect = &design.effects()[0];
        let mut residual = values.clone();
        for level in 0..effect.n_levels() {
            let rows = effect.level_index().members(level).to_vec();
            let mass: f64 = rows.iter().map(|&r| weights[r]).sum();
            let mut basis: Vec<Vec<f64>> = vec![vec![1.0; rows.len()]];
            for slope in effect.slopes() {
                let mean = rows.iter().map(|&r| weights[r] * slope[r]).sum::<f64>() / mass;
                basis.push(rows.iter().map(|&r| slope[r] - mean).collect());
            }
            let dot = |a: &[f64], b: &[f64]| {
                rows.iter()
                    .enumerate()
                    .map(|(i, &r)| weights[r] * a[i] * b[i])
                    .sum::<f64>()
            };
            let mut orthonormal: Vec<Vec<f64>> = Vec::new();
            for mut column in basis {
                let scale = dot(&column, &column).sqrt();
                for q in &orthonormal {
                    let c = dot(&column, q);
                    column.iter_mut().zip(q).for_each(|(v, q)| *v -= c * q);
                }
                let norm = dot(&column, &column).sqrt();
                if norm > 1e-10 * scale {
                    orthonormal.push(column.iter().map(|v| v / norm).collect());
                }
            }
            for rhs in 0..values.ncols() {
                let mut y: Vec<f64> = rows.iter().map(|&r| values[(r, rhs)]).collect();
                for q in &orthonormal {
                    let c = dot(&y, q);
                    y.iter_mut().zip(q).for_each(|(v, q)| *v -= c * q);
                }
                for (i, &r) in rows.iter().enumerate() {
                    residual[(r, rhs)] = y[i];
                }
            }
        }
        residual
    }

    /// Case 1 of the slope boundary fixture (slopes near ±2^24 that differ
    /// by units) certifies on fedemean at the fixture's tolerance and
    /// tighter, with no fallback: its operator and certificate take these
    /// slopes in each level's centered basis, so the raw level Gram's loss of
    /// their within-level variation no longer floors the certificate. The
    /// residuals match the independent centered projection, repeat bitwise,
    /// and are the same without the certified fallback.
    #[test]
    fn the_offset_slope_boundary_case_certifies_on_fedemean() {
        let (design, weights, original) = boundary();
        let reference = centered_level_projection(&design, &weights, &original);
        let scale = original
            .as_slice()
            .iter()
            .fold(0.0_f64, |m, v| m.max(v.abs()));
        for tolerance in [1e-8, 1e-10, 1e-12] {
            let config = AbsorbConfig {
                solver: FeSolverKind::Lsmr,
                lsmr: LsmrConfig {
                    tolerance,
                    max_iterations: 100,
                    preconditioner: LsmrPreconditioner::Off,
                    ..LsmrConfig::default()
                },
                ..AbsorbConfig::default()
            };
            let mut repeated = None;
            for certified_fallback in [true, true, false] {
                let mut values = original.clone();
                let report = absorb_fixed_effects(
                    &design,
                    &weights,
                    &mut values,
                    AbsorbConfig {
                        certified_fallback,
                        ..config
                    },
                )
                .unwrap();
                assert!(report.certified(), "{tolerance:e}");
                assert_eq!(report.requested_solver, FeSolverKind::Lsmr);
                assert_eq!(report.used_solver, FeSolverKind::Lsmr);
                assert!(!report.fallback_used && report.warnings.is_empty());
                for (actual, expected) in values.as_slice().iter().zip(reference.as_slice()) {
                    assert!(
                        (actual - expected).abs() <= 1e-12 * scale,
                        "{tolerance:e}: {actual} {expected}"
                    );
                }
                match &repeated {
                    None => repeated = Some(values),
                    Some(previous) => assert_eq!(&values, previous),
                }
            }
        }
    }

    /// A slope fit fedemean does not certify (its correction included)
    /// continues with MAP from the original input, reported as the fallback
    /// with fedemean's reason; no other LSMR leg runs. Two LSMR iterations
    /// without preconditioning do not reach 1e-10 on this two-effect slope
    /// design, so fedemean declines at its iteration ceiling, and MAP (its
    /// own budget) certifies the fit bitwise as called directly. Without the
    /// certified fallback fedemean's uncertified report returns.
    #[test]
    fn a_slope_fit_fedemean_does_not_certify_continues_with_map() {
        let (design, weights, original) = boundary_free_design();
        let config = AbsorbConfig {
            solver: FeSolverKind::Lsmr,
            lsmr: LsmrConfig {
                tolerance: 1e-10,
                max_iterations: 2,
                preconditioner: LsmrPreconditioner::Off,
                ..LsmrConfig::default()
            },
            ..AbsorbConfig::default()
        };
        let mut map = original.clone();
        let direct = absorb_map(&design, &weights, &mut map, config.map).unwrap();
        assert!(direct.certified());
        let mut repeated = None;
        for _ in 0..2 {
            let mut values = original.clone();
            let report = absorb_fixed_effects(&design, &weights, &mut values, config).unwrap();
            assert!(report.certified() && report.fallback_used);
            assert_eq!(report.requested_solver, FeSolverKind::Lsmr);
            assert_eq!(report.used_solver, FeSolverKind::Map);
            assert!(
                report.warnings[0].contains("retried MAP")
                    && report.warnings[0].contains("did not certify")
                    && report.warnings[0].contains("within its budget"),
                "{:?}",
                report.warnings
            );
            assert_eq!(values, map);
            assert_eq!(report.certificate, direct.certificate);
            assert_eq!(report.iterations, direct.iterations);
            match &repeated {
                None => repeated = Some(values),
                Some(previous) => assert_eq!(&values, previous),
            }
        }
        let mut values = original.clone();
        let alone = absorb_fixed_effects(
            &design,
            &weights,
            &mut values,
            AbsorbConfig {
                certified_fallback: false,
                ..config
            },
        )
        .unwrap();
        assert!(!alone.certified());
        assert_eq!(alone.used_solver, FeSolverKind::Lsmr);
        assert!(!alone.fallback_used);
    }

    /// Seventeen channels on one effect: fedemean takes them (explicit LSMR
    /// and Auto's LSMR attempt certify on fedemean). A design it does not
    /// take continues: explicit LSMR with MAP, reported as the fallback (or,
    /// without the certified fallback, refused as unsupported), Auto with MAP
    /// as before; the public modified-LSMR entry refuses it.
    #[test]
    fn seventeen_channels_and_ineligible_designs_keep_certified_routes() {
        let (design, weights, original) = super::super::fedemean_slope::tests::seventeen_channels();
        let lsmr = LsmrConfig {
            tolerance: 1e-8,
            ..LsmrConfig::default()
        };
        let explicit = AbsorbConfig {
            solver: FeSolverKind::Lsmr,
            lsmr,
            ..AbsorbConfig::default()
        };
        let mut map = original.clone();
        let direct = absorb_map(&design, &weights, &mut map, explicit.map).unwrap();
        for ineligible in [false, true] {
            super::super::fedemean_slope::force_ineligible_for_test(ineligible);
            let mut values = original.clone();
            let report = absorb_fixed_effects(&design, &weights, &mut values, explicit).unwrap();
            assert!(report.certified(), "{ineligible}");
            assert_eq!(report.requested_solver, FeSolverKind::Lsmr);
            assert_eq!(report.fallback_used, ineligible);
            if ineligible {
                // MAP from the original input, bitwise as called directly.
                assert_eq!(report.used_solver, FeSolverKind::Map);
                assert_eq!(values, map);
                assert_eq!(report.certificate, direct.certificate);
                assert!(report.warnings[0].contains("does not take"));
                let mut public = original.clone();
                let error = super::super::lsmr::absorb_lsmr(&design, &weights, &mut public, lsmr)
                    .unwrap_err();
                assert_eq!(error.code, ErrorCode::Unsupported);
                assert_eq!(public, original);
                let mut refused = original.clone();
                let error = absorb_fixed_effects(
                    &design,
                    &weights,
                    &mut refused,
                    AbsorbConfig {
                        certified_fallback: false,
                        ..explicit
                    },
                )
                .unwrap_err();
                assert_eq!(error.code, ErrorCode::Unsupported);
                assert_eq!(refused, original);
            } else {
                assert_eq!(report.used_solver, FeSolverKind::Lsmr);
            }
            let mut values = original.clone();
            let auto =
                absorb_fixed_effects(&design, &weights, &mut values, AbsorbConfig::default())
                    .unwrap();
            assert!(auto.certified() && !auto.fallback_used, "{ineligible}");
            let expected = if ineligible {
                FeSolverKind::Map
            } else {
                FeSolverKind::Lsmr
            };
            assert_eq!(auto.used_solver, expected);
            assert!(values
                .as_slice()
                .iter()
                .zip(map.as_slice())
                .all(|(a, b)| (a - b).abs() < 1e-6));
        }
        super::super::fedemean_slope::force_ineligible_for_test(false);
    }

    /// Capped memory: the admission charges the held input copy and fereg's
    /// certificate scratch; fedemean admits its own workspace beside them.
    /// At the unbudgeted fit's published peak the fit repeats its bits; one
    /// byte less is a fedemean memory decline (not an allocation error) that
    /// continues with MAP from the original input; below the admission the
    /// explicit request is refused.
    #[test]
    fn capped_memory_keeps_the_published_peak_in_the_budget_and_memory_declines_continue() {
        let (design, weights, original) = boundary_free_design();
        let lsmr = LsmrConfig {
            tolerance: 1e-10,
            preconditioner: LsmrPreconditioner::Schwarz,
            ..LsmrConfig::default()
        };
        let config = AbsorbConfig {
            solver: FeSolverKind::Lsmr,
            lsmr,
            ..AbsorbConfig::default()
        };
        let admitted = generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr).unwrap()
            + owned_fe_matrix_bytes(design.nobs(), 1).unwrap();
        let mut unlimited = original.clone();
        let base = absorb_fixed_effects(&design, &weights, &mut unlimited, config).unwrap();
        assert!(base.certified() && !base.fallback_used);
        assert_eq!(base.used_solver, FeSolverKind::Lsmr);
        let peak = u128::from(base.solver_workspace_peak.bytes().unwrap());
        // fedemean's workspace beside the held copy exceeds fereg's scratch.
        assert!(peak > admitted);
        let run = |usable| {
            let mut values = original.clone();
            absorb_fixed_effects(
                &design,
                &weights,
                &mut values,
                AbsorbConfig {
                    solver_memory_budget: Some(FeSolverMemoryBudget {
                        execution_bytes: 137,
                        usable_bytes: 137 + usable,
                    }),
                    ..config
                },
            )
            .map(|report| (report, values))
        };
        // An ample budget repeats the unbudgeted bits. (The Schwarz build
        // admits its transient storage too, beyond the retained workspace
        // fedemean publishes.)
        let (ample, values) = run(4 * peak).unwrap();
        assert!(ample.certified() && !ample.fallback_used);
        assert_eq!(values, unlimited);
        assert_eq!(ample.solver_workspace_peak, base.solver_workspace_peak);
        let mut map = original.clone();
        absorb_map(&design, &weights, &mut map, config.map).unwrap();
        for usable in [peak - 1, admitted] {
            let (declined, values) = run(usable).unwrap();
            assert!(declined.certified() && declined.fallback_used);
            assert_eq!(declined.used_solver, FeSolverKind::Map);
            assert!(u128::from(declined.solver_workspace_peak.bytes().unwrap()) <= usable);
            assert!(declined.warnings[0].contains("memory limit"));
            assert_eq!(values, map);
        }
        assert_eq!(run(admitted - 1).unwrap_err().code, ErrorCode::Allocation);
        let mut values = original.clone();
        let (declined, route) = super::super::lsmr::absorb_lsmr_with(
            &design,
            &weights,
            &mut values,
            lsmr,
            (lsmr.tolerance, Some(1)),
            None,
        )
        .unwrap()
        .expect("eligible");
        assert_eq!(route, super::super::lsmr::LsmrRoute::Fedemean);
        assert!(!declined.certified());
        assert!(declined
            .warnings
            .iter()
            .any(|w| w.contains("exceeds the memory limit")));
        assert_eq!(values, original);
    }

    /// Two intercept effects and a slope effect, well scaled.
    fn boundary_free_design() -> (FeDesign, Vec<f64>, DenseMatrix) {
        let n = 600;
        let t: Vec<f64> = (0..n)
            .map(|r| (r % 10) as f64 + ((r * 7) % 3) as f64 / 3.0)
            .collect();
        let y: Vec<f64> = (0..n)
            .map(|r| (r as f64 * 0.21).sin() + 0.1 * t[r] * ((r / 20) % 5) as f64)
            .collect();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new((0..n).map(|r| (r / 6) as u32).collect(), 100, true, vec![])
                    .unwrap(),
                EffectTerm::new(
                    (0..n).map(|r| ((r / 6 + r % 6) % 12) as u32).collect(),
                    12,
                    true,
                    vec![t],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        (
            design,
            vec![1.0; n],
            DenseMatrix::from_columns(&[y]).unwrap(),
        )
    }
}
