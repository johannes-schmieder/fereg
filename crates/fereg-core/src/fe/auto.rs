use super::certificate::CertificateContext;
use super::map::absorb_map_with_context;
use super::projection::ParallelProjectionPlan;
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_accelerated_map, absorb_lsmr, absorb_map, FeDesign, FeSolverKind, LsmrConfig, MapConfig,
    SolveReport, SolveStopReason,
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

/// Smallest recurrence target explicit LSMR chases below its certificate.
const EXPLICIT_LSMR_RECURRENCE_FLOOR: f64 = 1e-15;

/// Explicit `solver(lsmr)` iterates to a hundredth of the requested tolerance,
/// floored at 1e-15 and never looser than the request, while the independent
/// FE certificate stays at the request (scoped decisions of 2026-10-10: an
/// equal-tolerance stop can leave a certificate just above the request).
/// Unlike [`direct_lsmr_tolerance`] it keeps a margin at the 1e-14 final
/// residual projection, because explicit LSMR has no MAP fallback.
pub(crate) fn explicit_lsmr_recurrence(config: LsmrConfig) -> LsmrConfig {
    LsmrConfig {
        tolerance: (config.tolerance * 1e-2)
            .max(EXPLICIT_LSMR_RECURRENCE_FLOOR)
            .min(config.tolerance),
        ..config
    }
}

/// Unified fixed-effect absorption configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsorbConfig {
    pub solver: FeSolverKind,
    /// Maximum distinct-neighbor degree eliminated in the experimental CMG graph.
    pub cmg_schur_degree: usize,
    pub map: MapConfig,
    pub lsmr: LsmrConfig,
    /// Whether `Auto` may retry the other certified solver after a failed certificate.
    pub certified_fallback: bool,
    /// Internal joint execution-plus-solver workspace budget. This is populated
    /// by the existing memory-limit seam and is not public estimator syntax.
    pub solver_memory_budget: Option<FeSolverMemoryBudget>,
    /// Retain coefficient-space solver products for zero-extra-solve saved-FE
    /// recovery. Callers enable this only when saved channels are requested.
    pub retain_coefficients: bool,
}

impl Default for AbsorbConfig {
    fn default() -> Self {
        Self {
            solver: FeSolverKind::Auto,
            cmg_schur_degree: 5,
            map: MapConfig::default(),
            lsmr: LsmrConfig::default(),
            certified_fallback: true,
            solver_memory_budget: None,
            retain_coefficients: false,
        }
    }
}

/// Residualize with explicit solver selection or deterministic automatic routing.
pub fn absorb_fixed_effects(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
    match config.solver {
        FeSolverKind::Map => {
            let peak = admit_generic_solver(design, values, config, FeSolverKind::Map, false)?;
            let mut report = absorb_map(design, weights, values, config.map)?;
            report.solver_workspace_peak = peak;
            Ok(report)
        }
        FeSolverKind::Lsmr => {
            let attempted_peak = admit_generic_solver(
                design,
                values,
                config,
                FeSolverKind::Lsmr,
                config.certified_fallback,
            )?;
            let original = config
                .certified_fallback
                .then(|| {
                    fallible_matrix_copy(values, "unable to reserve explicit LSMR fallback matrix")
                })
                .transpose()?;
            let mut report = super::lsmr::absorb_lsmr_certified_at(
                design,
                weights,
                values,
                explicit_lsmr_recurrence(config.lsmr),
                config.lsmr.tolerance,
            )?;
            report.solver_workspace_peak = attempted_peak;
            if report.certified() || !config.certified_fallback {
                return Ok(report);
            }
            if let Some(original) = original {
                *values = original;
            }
            let fallback_peak =
                admit_generic_solver(design, values, config, FeSolverKind::Map, false)?;
            let mut fallback = absorb_map(design, weights, values, config.map)?;
            fallback.solver_workspace_peak = attempted_peak.max(fallback_peak);
            fallback.requested_solver = FeSolverKind::Lsmr;
            fallback.fallback_used = true;
            fallback.warnings.insert(
                0,
                format!(
                    "explicit LSMR path did not certify after {} iterations; retried MAP",
                    report.iterations
                ),
            );
            Ok(fallback)
        }
        FeSolverKind::Cg | FeSolverKind::IronsTuck | FeSolverKind::Squarem => {
            let peak = admit_generic_solver(design, values, config, config.solver, false)?;
            let mut report =
                absorb_accelerated_map(design, weights, values, config.map, config.solver)?;
            report.solver_workspace_peak = peak;
            Ok(report)
        }
        FeSolverKind::Cmg | FeSolverKind::CmgSchur => Err(FeregError::new(
            ErrorCode::Unsupported,
            "CMG solvers require a prepared intercept-only categorical topology",
        )),
        FeSolverKind::Auto => absorb_auto(design, weights, values, config),
    }
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
        match admit_generic_solver(design, values, config, FeSolverKind::Lsmr, true) {
            Ok(admitted) => {
                peak = admitted;
                let mut candidate =
                    fallible_matrix_copy(values, "unable to reserve general FE candidate")?;
                let policy = LsmrConfig {
                    tolerance: direct_lsmr_tolerance(config.map.tolerance),
                    max_iterations: config
                        .lsmr
                        .max_iterations
                        .min(config.map.max_iterations / 2),
                    ..config.lsmr
                };
                let mut report = absorb_lsmr(design, weights, &mut candidate, policy)?;
                // absorb_lsmr already certified these residuals against the
                // same reference norms (weighted_column_norms of the untouched
                // inputs); only the public pass threshold differs.
                report.certificate.passed =
                    report.certificate.relative_infinity_norm <= config.map.tolerance;
                report.requested_solver = FeSolverKind::Auto;
                report.solver_workspace_peak = peak;
                if report.certificate.passed {
                    report.stop_reason = SolveStopReason::Certified;
                    *values = candidate;
                    return Ok(report);
                }
                prior = Some(report);
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
    let admitted = admit_generic_solver(design, values, config, FeSolverKind::Map, false)?;
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

fn admit_generic_solver(
    design: &FeDesign,
    values: &DenseMatrix,
    config: AbsorbConfig,
    solver: FeSolverKind,
    preserve_input: bool,
) -> Result<SolverWorkspacePeak> {
    let workspace = generic_fe_solver_bytes(design, values.ncols(), solver, config.lsmr)?;
    let preserved = if preserve_input {
        owned_fe_matrix_bytes(values.nrows(), values.ncols())?
    } else {
        0
    };
    let required = workspace.checked_add(preserved).ok_or_else(|| {
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
        let attempt = generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr, config.lsmr).unwrap()
            + owned_fe_matrix_bytes(6, 1).unwrap();
        let fallback = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map, config.lsmr).unwrap();
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
        let required = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map, config.lsmr).unwrap();
        let cache = CertificateContext::new(&design, &weights)
            .cache_bytes()
            .unwrap();
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
            assert_eq!(
                report.solver_workspace_peak.bytes(),
                Some(if usable < required + cache {
                    required
                } else {
                    required + cache
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
        let _map = generic_fe_solver_bytes(&design, 8, FeSolverKind::Map, config.lsmr).unwrap();
        let cg = generic_fe_solver_bytes(&design, 8, FeSolverKind::Map, config.lsmr).unwrap();
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
                let _map =
                    generic_fe_solver_bytes(&design, 8, FeSolverKind::Map, config.lsmr).unwrap();
                let cg =
                    generic_fe_solver_bytes(&design, 8, FeSolverKind::Map, config.lsmr).unwrap();
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
                let required =
                    generic_fe_solver_bytes(&design, 1, FeSolverKind::Map, config.lsmr).unwrap();
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
    fn explicit_owned_lsmr_tightens_the_recurrence_and_certifies_the_request() {
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
            explicit_lsmr_recurrence(lsmr),
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
                            let reserve = generic_fe_solver_bytes(&design, 1, solver, config.lsmr)
                                .unwrap()
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
                                    assert_eq!(
                                        report.solver_workspace_peak.bytes(),
                                        Some(reserve as u64)
                                    );
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
        let reserve = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map, config.lsmr).unwrap();
        assert!(
            reserve < generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr, config.lsmr).unwrap()
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
        let cg_bytes = generic_fe_solver_bytes(&design, 1, FeSolverKind::Map, config.lsmr).unwrap();
        let lsmr_bytes =
            generic_fe_solver_bytes(&design, 1, FeSolverKind::Lsmr, config.lsmr).unwrap();
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
