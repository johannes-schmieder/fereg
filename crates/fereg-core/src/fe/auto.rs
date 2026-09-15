use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_accelerated_map, absorb_lsmr, absorb_map, certify_residuals_with_reference_norms,
    weighted_column_norms, FeDesign, FeSolverKind, LsmrConfig, MapConfig, SolveReport,
    SolveStopReason,
};
use crate::memory::SolverWorkspacePeak;
use crate::memory::{generic_fe_solver_bytes, owned_fe_matrix_bytes, FeSolverMemoryBudget};

/// Frozen automatic-routing threshold selected from the registered AKM matrix.
///
/// The ratio is the independently certified FE moment after four symmetric
/// MAP sweeps divided by its initial value. Values at or above 0.8% identify
/// the slow-mixing graphs where the bounded LSMR setup cost paid for itself.
pub(crate) const AUTO_LSMR_MIN_PILOT_CONTRACTION: f64 = 0.008;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoLsmrRoute {
    None,
    Direct,
    CgPolish,
}

pub(crate) fn auto_selects_lsmr(pilot_contraction: f64, total_fe_coefficients: usize) -> bool {
    auto_lsmr_route(pilot_contraction, total_fe_coefficients) == AutoLsmrRoute::Direct
}

pub(crate) fn auto_lsmr_route(
    pilot_contraction: f64,
    _total_fe_coefficients: usize,
) -> AutoLsmrRoute {
    if pilot_contraction < AUTO_LSMR_MIN_PILOT_CONTRACTION {
        AutoLsmrRoute::None
    } else {
        AutoLsmrRoute::Direct
    }
}

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

/// Qualification-only comparison route: use LSMR as a bulk reducer, then let
/// comparator-equivalent CG polish and certify the observation residual.
///
/// The explicit cap argument lets focused tests exercise both sides of the
/// route on a bounded graph. Runtime callers always pass the frozen cap.
pub(crate) fn auto_lsmr_route_at_cap(
    pilot_contraction: f64,
    total_fe_coefficients: usize,
    direct_lsmr_cap: usize,
) -> AutoLsmrRoute {
    if pilot_contraction < AUTO_LSMR_MIN_PILOT_CONTRACTION {
        AutoLsmrRoute::None
    } else if total_fe_coefficients <= direct_lsmr_cap {
        AutoLsmrRoute::Direct
    } else {
        AutoLsmrRoute::CgPolish
    }
}

/// Unified fixed-effect absorption configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsorbConfig {
    pub solver: FeSolverKind,
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
            let mut report = absorb_lsmr(design, weights, values, config.lsmr)?;
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
        FeSolverKind::Cmg => Err(FeregError::new(
            ErrorCode::Unsupported,
            "solver(cmg) requires a prepared intercept-only categorical topology",
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
    // Admit the cheap pilot first. An unused LSMR/CG workspace must not prevent
    // a MAP certificate from closing an otherwise affordable request.
    let mut attempted_peak =
        admit_generic_solver(design, values, config, FeSolverKind::Map, false)?;
    let reference_norms = weighted_column_norms(values, weights)?;
    let initial = certify_residuals_with_reference_norms(
        design,
        weights,
        values,
        &reference_norms,
        config.map.tolerance,
    )?;
    let pilot_budget = config.map.max_iterations.min(4);
    let mut pilot = absorb_map(
        design,
        weights,
        values,
        MapConfig {
            max_iterations: pilot_budget,
            check_every: 1,
            ..config.map
        },
    )?;
    let contraction = if initial.relative_two_norm == 0.0 {
        0.0
    } else {
        pilot.certificate.relative_two_norm / initial.relative_two_norm
    };
    if !contraction.is_finite() || contraction < 0.0 {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "automatic MAP pilot contraction is invalid",
        ));
    }
    pilot.solver_workspace_peak = attempted_peak;
    pilot.requested_solver = FeSolverKind::Auto;
    pilot.pilot_iterations = pilot.iterations;
    pilot.pilot_contraction = contraction;
    if pilot.certified() {
        return Ok(pilot);
    }
    if pilot_budget == config.map.max_iterations {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "automatic MAP pilot exhausted the solver budget without certification",
        ));
    }

    let mut remaining_iterations = config.map.max_iterations - pilot_budget;
    let mut lsmr_failure = None;
    let mut attempted_lsmr = None;
    if auto_selects_lsmr(contraction, design.total_coefficients()) {
        let lsmr_config = LsmrConfig {
            tolerance: direct_lsmr_tolerance(config.map.tolerance),
            max_iterations: config.lsmr.max_iterations.min(remaining_iterations),
            ..config.lsmr
        };
        let attempt: Result<(SolveReport, DenseMatrix)> = (|| {
            let peak = admit_generic_solver(design, values, config, FeSolverKind::Lsmr, true)?;
            // Keep admitted work even if construction/solving errors before a
            // report is returned. An admission rejection itself allocates none
            // of this workspace and must not charge the rejected envelope.
            attempted_peak = attempted_peak.max(peak);
            let mut lsmr_values =
                fallible_matrix_copy(values, "unable to reserve automatic LSMR fallback matrix")?;
            let report = absorb_lsmr(design, weights, &mut lsmr_values, lsmr_config)?;
            Ok((report, lsmr_values))
        })();
        match attempt {
            Ok((mut lsmr, lsmr_values)) => {
                lsmr.certificate = certify_residuals_with_reference_norms(
                    design,
                    weights,
                    &lsmr_values,
                    &reference_norms,
                    lsmr_config.tolerance,
                )?;
                if lsmr.certificate.passed {
                    lsmr.stop_reason = SolveStopReason::Certified;
                }
                if lsmr.certified() {
                    lsmr.solver_workspace_peak = attempted_peak;
                    lsmr.requested_solver = FeSolverKind::Auto;
                    lsmr.iterations = lsmr.iterations.saturating_add(pilot.iterations);
                    lsmr.pilot_iterations = pilot.iterations;
                    lsmr.pilot_contraction = contraction;
                    lsmr.setup_seconds += pilot.setup_seconds;
                    lsmr.solve_seconds += pilot.solve_seconds;
                    *values = lsmr_values;
                    return Ok(lsmr);
                }
                remaining_iterations = remaining_iterations
                    .checked_sub(lsmr.iterations)
                    .filter(|remaining| *remaining > 0)
                    .ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::InvalidData,
                            "automatic LSMR continuation exhausted the shared iteration budget before CG polish",
                        )
                    })?;
                *values = lsmr_values;
                attempted_lsmr = Some((
                    lsmr.iterations,
                    lsmr.setup_seconds,
                    lsmr.solve_seconds,
                    lsmr.requested_preconditioner,
                    lsmr.used_preconditioner,
                ));
                lsmr_failure = Some(format!(
                    "automatic LSMR path did not certify after {} iterations; continued CG from the LSMR candidate",
                    lsmr.iterations
                ));
            }
            Err(error) => {
                lsmr_failure = Some(format!("automatic LSMR path failed ({error}); retried CG"));
            }
        }
    }

    // A failed/rejected LSMR attempt has released its solver and transport.
    // Recheck the actual continuation instead of charging two live solvers.
    let continuation_peak = admit_generic_solver(design, values, config, FeSolverKind::Cg, false)?;
    attempted_peak = attempted_peak.max(continuation_peak);
    let mut cg = absorb_accelerated_map(
        design,
        weights,
        values,
        MapConfig {
            max_iterations: remaining_iterations,
            ..config.map
        },
        FeSolverKind::Cg,
    )?;
    if !cg.certified() {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "automatic CG continuation exhausted the solver budget without certification",
        ));
    }
    cg.solver_workspace_peak = attempted_peak;
    cg.requested_solver = FeSolverKind::Auto;
    cg.iterations = cg.iterations.saturating_add(pilot.iterations);
    cg.pilot_iterations = pilot.iterations;
    cg.pilot_contraction = contraction;
    cg.setup_seconds += pilot.setup_seconds;
    cg.solve_seconds += pilot.solve_seconds;
    if let Some((iterations, setup, solve, requested, used)) = attempted_lsmr {
        cg.iterations = cg.iterations.saturating_add(iterations);
        cg.setup_seconds += setup;
        cg.solve_seconds += solve;
        cg.requested_preconditioner = requested;
        cg.used_preconditioner = used;
    }
    if let Some(warning) = lsmr_failure {
        cg.fallback_used = true;
        cg.warnings.insert(0, warning);
    }
    Ok(cg)
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
    fn auto_lsmr_threshold_is_frozen_and_inclusive() {
        assert!(!auto_selects_lsmr(
            AUTO_LSMR_MIN_PILOT_CONTRACTION - f64::EPSILON,
            65_536,
        ));
        assert!(auto_selects_lsmr(AUTO_LSMR_MIN_PILOT_CONTRACTION, 65_536,));
        assert!(auto_selects_lsmr(1.0, 10_000_000));
        assert_eq!(
            auto_lsmr_route(AUTO_LSMR_MIN_PILOT_CONTRACTION - f64::EPSILON, 65_537,),
            AutoLsmrRoute::None,
        );
        assert_eq!(
            auto_lsmr_route(AUTO_LSMR_MIN_PILOT_CONTRACTION, 65_536,),
            AutoLsmrRoute::Direct,
        );
        assert_eq!(
            auto_lsmr_route(AUTO_LSMR_MIN_PILOT_CONTRACTION, 65_537,),
            AutoLsmrRoute::Direct,
        );
        assert_eq!(
            auto_lsmr_route_at_cap(AUTO_LSMR_MIN_PILOT_CONTRACTION, 65_537, 65_536,),
            AutoLsmrRoute::CgPolish,
        );
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
    fn auto_fails_closed_when_the_pilot_exhausts_the_total_budget() {
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
        assert_eq!(error.code, ErrorCode::InvalidData);
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
    fn generic_auto_rechecks_cg_when_slope_lsmr_does_not_fit() {
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
        let cg_bytes = generic_fe_solver_bytes(&design, 1, FeSolverKind::Cg, config.lsmr).unwrap();
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
                assert_eq!(report.used_solver, FeSolverKind::Cg);
                assert!(report.fallback_used);
                assert!(report.warnings[0].contains("workspace bytes"));
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
