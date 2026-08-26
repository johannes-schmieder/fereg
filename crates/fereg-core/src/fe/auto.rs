use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    absorb_accelerated_map, absorb_lsmr, absorb_map, certify_residuals_with_reference_norms,
    weighted_column_norms, FeDesign, FeSolverKind, LsmrConfig, MapConfig, SolveReport,
};

/// Frozen automatic-routing threshold selected from the registered AKM matrix.
///
/// The ratio is the independently certified FE moment after four symmetric
/// MAP sweeps divided by its initial value. Values at or above 0.8% identify
/// the slow-mixing graphs where the bounded LSMR setup cost paid for itself.
pub(crate) const AUTO_LSMR_MIN_PILOT_CONTRACTION: f64 = 0.008;
/// Largest FE coefficient axis on which the registered residual-parity and
/// setup-cost evidence supports automatic LSMR routing.
pub(crate) const AUTO_LSMR_MAX_COEFFICIENTS: usize = 65_536;

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
    total_fe_coefficients: usize,
) -> AutoLsmrRoute {
    auto_lsmr_route_at_cap(
        pilot_contraction,
        total_fe_coefficients,
        AUTO_LSMR_MAX_COEFFICIENTS,
    )
}

/// Development-only large-axis route: use LSMR as a bulk reducer, then let
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
}

impl Default for AbsorbConfig {
    fn default() -> Self {
        Self {
            solver: FeSolverKind::Auto,
            map: MapConfig::default(),
            lsmr: LsmrConfig::default(),
            certified_fallback: true,
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
        FeSolverKind::Map => absorb_map(design, weights, values, config.map),
        FeSolverKind::Lsmr => {
            let original = config.certified_fallback.then(|| values.clone());
            let report = absorb_lsmr(design, weights, values, config.lsmr)?;
            if report.certified() || !config.certified_fallback {
                return Ok(report);
            }
            if let Some(original) = original {
                *values = original;
            }
            let mut fallback = absorb_map(design, weights, values, config.map)?;
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
            absorb_accelerated_map(design, weights, values, config.map, config.solver)
        }
        FeSolverKind::Auto => absorb_auto(design, weights, values, config),
    }
}

fn absorb_auto(
    design: &FeDesign,
    weights: &[f64],
    values: &mut DenseMatrix,
    config: AbsorbConfig,
) -> Result<SolveReport> {
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

    let remaining_iterations = config.map.max_iterations - pilot_budget;
    let mut lsmr_failure = None;
    if auto_selects_lsmr(contraction, design.total_coefficients()) {
        let mut lsmr_values =
            fallible_matrix_copy(values, "unable to reserve automatic LSMR fallback matrix")?;
        let lsmr_config = LsmrConfig {
            tolerance: config.map.tolerance,
            max_iterations: config.lsmr.max_iterations.min(remaining_iterations),
            ..config.lsmr
        };
        match absorb_lsmr(design, weights, &mut lsmr_values, lsmr_config) {
            Ok(mut lsmr) if lsmr.certified() => {
                lsmr.requested_solver = FeSolverKind::Auto;
                lsmr.iterations = lsmr.iterations.saturating_add(pilot.iterations);
                lsmr.pilot_iterations = pilot.iterations;
                lsmr.pilot_contraction = contraction;
                lsmr.setup_seconds += pilot.setup_seconds;
                lsmr.solve_seconds += pilot.solve_seconds;
                *values = lsmr_values;
                return Ok(lsmr);
            }
            Ok(lsmr) => {
                lsmr_failure = Some(format!(
                    "automatic LSMR path did not certify after {} iterations; retried CG",
                    lsmr.iterations
                ));
            }
            Err(error) => {
                lsmr_failure = Some(format!("automatic LSMR path failed ({error}); retried CG"));
            }
        }
    }

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
    cg.requested_solver = FeSolverKind::Auto;
    cg.iterations = cg.iterations.saturating_add(pilot.iterations);
    cg.pilot_iterations = pilot.iterations;
    cg.pilot_contraction = contraction;
    cg.setup_seconds += pilot.setup_seconds;
    cg.solve_seconds += pilot.solve_seconds;
    if let Some(warning) = lsmr_failure {
        cg.fallback_used = true;
        cg.warnings.insert(0, warning);
    }
    Ok(cg)
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
            AUTO_LSMR_MAX_COEFFICIENTS,
        ));
        assert!(auto_selects_lsmr(
            AUTO_LSMR_MIN_PILOT_CONTRACTION,
            AUTO_LSMR_MAX_COEFFICIENTS,
        ));
        assert!(!auto_selects_lsmr(1.0, AUTO_LSMR_MAX_COEFFICIENTS + 1,));
        assert_eq!(
            auto_lsmr_route(
                AUTO_LSMR_MIN_PILOT_CONTRACTION - f64::EPSILON,
                AUTO_LSMR_MAX_COEFFICIENTS + 1,
            ),
            AutoLsmrRoute::None,
        );
        assert_eq!(
            auto_lsmr_route(AUTO_LSMR_MIN_PILOT_CONTRACTION, AUTO_LSMR_MAX_COEFFICIENTS,),
            AutoLsmrRoute::Direct,
        );
        assert_eq!(
            auto_lsmr_route(
                AUTO_LSMR_MIN_PILOT_CONTRACTION,
                AUTO_LSMR_MAX_COEFFICIENTS + 1,
            ),
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
