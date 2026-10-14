#![deny(clippy::too_many_lines)]
//! Prefix-to-LSMR public-caller witnesses against independent weighted QR.
mod support;

use fereg_core::fe::{
    CategoricalPrefix, LsmrPreconditioner, MapConfig, RoutingPreset, SolveReport,
};
use fereg_core::{
    build_prepared_ols_rhs, fit_prepared_model_robust_with_loaded_moments, prepare_model,
    FeSolverKind, PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsRobustConfig,
    ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{generate, Design, DesignSpec, Graph, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsResult};

fn config(
    mode: PreparedOlsExecutionMode,
    workers: usize,
    prefix: Option<CategoricalPrefix>,
    preconditioner: LsmrPreconditioner,
) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = mode;
    config.execution.poolsize = 3;
    config.execution.threads = threads;
    config.absorb.solver = FeSolverKind::Auto;
    config.absorb.routing = prefix.map(RoutingPreset::prefix);
    config.absorb.lsmr.threads = threads;
    config.absorb.lsmr.preconditioner = preconditioner;
    config.absorb.lsmr.max_iterations = 128;
    config.absorb.map.tolerance = 1e-12;
    config.absorb.map.max_iterations = 256;
    config.absorb.retain_coefficients = true;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-12);
    config
}

fn design(effects: usize, regressors: usize) -> Design {
    generate(
        7381,
        &DesignSpec {
            rows: 288,
            levels: [6, 6, 4, 4, 4][..effects].to_vec(),
            regressors,
            graph: Graph::Disconnected,
            weights: WeightKind::Analytic,
            ..DesignSpec::default()
        },
    )
    .unwrap()
}

fn weak_design() -> Design {
    let mut design = generate(
        7381,
        &DesignSpec {
            rows: 544,
            levels: vec![16, 16, 6],
            regressors: 3,
            graph: Graph::Disconnected,
            weights: WeightKind::Analytic,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    // Two weak rings. Sixteen home cells and one move per worker preserve
    // slowly decaying modes after either bounded prefix, while a third factor
    // crosses the cells inside each connected component.
    let codes: [Vec<u32>; 3] = std::array::from_fn(|effect| {
        (0..544)
            .map(|row| {
                let component = row / 272;
                let cell = (row % 272) / 2;
                let worker = cell / 17;
                match effect {
                    0 => (8 * component + worker) as u32,
                    1 => (8 * component + (worker + usize::from(cell % 17 == 16)) % 8) as u32,
                    _ => (3 * component + (row * 7 + row / 11) % 3) as u32,
                }
            })
            .collect()
    });
    design.effects = codes
        .into_iter()
        .zip([16, 16, 6])
        .map(|(codes, levels)| DummyEffect::new(codes, levels, true, vec![]).unwrap())
        .collect();
    for row in 0..544 {
        design.outcome[row] += design
            .effects
            .iter()
            .enumerate()
            .map(|(effect, term)| ((effect + 1) as f64 * f64::from(term.codes()[row])).cos())
            .sum::<f64>();
    }
    design
}

fn constant(design: &Design, oracle: &ExplicitOlsResult) -> f64 {
    design
        .weights
        .iter()
        .zip(&oracle.absorbed_fitted_values)
        .map(|(weight, fitted)| weight * fitted)
        .sum::<f64>()
        / design.weights.iter().sum::<f64>()
}

fn check_saved_gauge(design: &Design, channels: &[Option<Vec<f64>>]) {
    assert_eq!(channels.len(), design.effects.len());
    let split = design.effects[0].n_levels() / 2;
    for channel in channels.iter().skip(1) {
        let channel = channel.as_ref().unwrap();
        for component in 0..2 {
            let (sum, mass) = design
                .weights
                .iter()
                .enumerate()
                .filter(|(row, _)| {
                    usize::from(design.effects[0].codes()[*row] as usize >= split) == component
                })
                .fold((0., 0.), |(sum, mass), (row, weight)| {
                    (sum + weight * channel[row], mass + weight)
                });
            assert!((sum / mass).abs() < 1e-9);
        }
    }
}

fn check_prefix_route(report: &SolveReport, config: PreparedOlsFitConfig) {
    let Some(preset) = config.absorb.routing else {
        return;
    };
    let (prefix, cap) = [
        (CategoricalPrefix::None, 0),
        (CategoricalPrefix::Map4, 4),
        (CategoricalPrefix::CompactIt4, 8),
    ]
    .into_iter()
    .find(|(prefix, _)| preset == RoutingPreset::prefix(*prefix))
    .expect("this witness uses only categorical prefix presets");
    assert!(report.iterations <= config.absorb.lsmr.max_iterations);
    assert!(report.certificate.relative_infinity_norm <= config.absorb.map.tolerance);
    if report.used_solver == FeSolverKind::Lsmr {
        assert_eq!(report.pilot_iterations, cap, "{report:?}");
        assert!(report.iterations > cap, "{report:?}");
        assert_eq!(
            report.requested_preconditioner,
            Some(config.absorb.lsmr.preconditioner)
        );
    } else {
        let early_family = match prefix {
            CategoricalPrefix::Map4 => FeSolverKind::Map,
            CategoricalPrefix::CompactIt4 => FeSolverKind::IronsTuck,
            CategoricalPrefix::None => panic!("zero prefix must reach LSMR: {report:?}"),
        };
        assert_eq!(report.used_solver, early_family, "{report:?}");
        assert_eq!(report.iterations, report.pilot_iterations);
        assert!(report.pilot_iterations > 0 && report.pilot_iterations <= cap);
        assert_eq!(report.requested_preconditioner, None);
        assert_eq!(report.used_preconditioner, None);
        assert!(!report.fallback_used);
    }
}

fn check_case(
    design: &Design,
    oracle: &ExplicitOlsResult,
    config: PreparedOlsFitConfig,
) -> SolveReport {
    let (request, loaded) = support::inputs(design, false);
    let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
    assert_eq!(prepared.sample().n_active(), design.outcome.len());
    let width = build_prepared_ols_rhs(&prepared).unwrap().values().ncols();
    assert_eq!(width, design.regressors.len() + 2);
    let result = fit_prepared_model_robust_with_loaded_moments(
        &prepared,
        PreparedOlsRobustConfig {
            fit: config,
            ..PreparedOlsRobustConfig::default()
        },
        None,
    )
    .unwrap();
    let fit = result.fit();
    check_prefix_route(fit.absorption_report(), config);
    if !config.absorb.retain_coefficients {
        assert!(fit.recovered_fe_coefficients().is_none());
    }
    for report in [fit.absorption_report(), fit.residual_absorption_report()] {
        assert!(report.certified());
        assert_eq!(report.requested_solver, FeSolverKind::Auto);
    }
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_eq!(fit.kept_generated_columns(), oracle.kept_regressors);
    assert!(fit.dropped_generated_columns().is_empty());
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        COEFFICIENT
            .check(actual.value.unwrap(), expected.unwrap())
            .unwrap();
    }
    let intercept = constant(design, oracle);
    COEFFICIENT
        .check(fit.coefficients().last().unwrap().value.unwrap(), intercept)
        .unwrap();
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap();
    INFERENCE
        .check(result.statistics().rss, oracle.weighted_rss)
        .unwrap();
    assert_eq!(result.dof().absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(
        result.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    INFERENCE
        .check_slice(
            result.covariance().values(),
            oracle
                .normalized_reported_intercept_hc1_vce
                .as_ref()
                .unwrap()
                .as_slice(),
        )
        .unwrap();
    let target: Vec<_> = oracle
        .absorbed_fitted_values
        .iter()
        .map(|value| value - intercept)
        .collect();
    // Saved-effect recovery is a separate projection action. Preserve its
    // ordinary bounded allowance on weak rings; the fit above still has the
    // deliberately smaller 128-step shared prefix/LSMR allowance.
    let recovery = MapConfig {
        max_iterations: MapConfig::default().max_iterations,
        ..config.absorb.map
    };
    let saved = prepared
        .recover_absorbed_effects_fast(
            &target,
            &vec![true; design.effects.len()],
            fit.recovered_fe_coefficients(),
            false,
            true,
            config.absorb.lsmr,
            recovery,
            None,
        )
        .unwrap();
    PREDICTION
        .check_slice(saved.absorbed_sum(), &target)
        .unwrap();
    check_saved_gauge(design, saved.channels());
    fit.absorption_report().clone()
}

#[test]
fn categorical_prefixes_preserve_original_weighted_model_and_inference() {
    let design = design(3, 3);
    let (problem, _) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    assert_eq!(oracle.finite_regressor_rank, 3);
    // Public auto and each independently selected prefix share the same truth.
    for prefix in [
        None,
        Some(CategoricalPrefix::None),
        Some(CategoricalPrefix::Map4),
        Some(CategoricalPrefix::CompactIt4),
    ] {
        for preconditioner in [
            LsmrPreconditioner::Off,
            LsmrPreconditioner::Diagonal,
            LsmrPreconditioner::Auto,
        ] {
            for mode in [
                PreparedOlsExecutionMode::Fast,
                PreparedOlsExecutionMode::Low,
            ] {
                for workers in [1, 4] {
                    check_case(
                        &design,
                        &oracle,
                        config(mode, workers, prefix, preconditioner),
                    );
                }
            }
        }
    }
    let default = config(
        PreparedOlsExecutionMode::Fast,
        1,
        None,
        LsmrPreconditioner::Auto,
    );
    let (fit, rows) = support::native_fit(&design, default, false).unwrap();
    assert_eq!(rows, (0..design.outcome.len()).collect::<Vec<_>>());
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap();
}

#[test]
fn categorical_prefixes_cross_rhs_windows_and_many_effects() {
    for (effects, regressors) in [(3, 63), (5, 3)] {
        let design = design(effects, regressors);
        let (problem, _) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        assert_eq!(oracle.finite_regressor_rank, regressors);
        for prefix in [
            None,
            Some(CategoricalPrefix::None),
            Some(CategoricalPrefix::Map4),
            Some(CategoricalPrefix::CompactIt4),
        ] {
            let report = check_case(
                &design,
                &oracle,
                config(
                    PreparedOlsExecutionMode::Fast,
                    4,
                    prefix,
                    LsmrPreconditioner::Auto,
                ),
            );
            if effects == 3 && prefix == Some(CategoricalPrefix::CompactIt4) {
                assert_eq!(report.used_solver, FeSolverKind::IronsTuck);
                assert_eq!(report.pilot_iterations, 8);
            }
        }
    }
}

#[test]
fn weak_prefixes_reach_lsmr_with_retained_and_unretained_coefficients() {
    let design = weak_design();
    let (problem, _) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    assert_eq!(oracle.finite_regressor_rank, 3);
    for prefix in [CategoricalPrefix::Map4, CategoricalPrefix::CompactIt4] {
        for preconditioner in [
            LsmrPreconditioner::Off,
            LsmrPreconditioner::Diagonal,
            LsmrPreconditioner::Auto,
        ] {
            for mode in [
                PreparedOlsExecutionMode::Fast,
                PreparedOlsExecutionMode::Low,
            ] {
                for workers in [1, 4] {
                    let report = check_case(
                        &design,
                        &oracle,
                        config(mode, workers, Some(prefix), preconditioner),
                    );
                    // This designated unresolved fixture must exercise the seed
                    // handoff, including multiple Low projection actions.
                    assert_eq!(report.used_solver, FeSolverKind::Lsmr, "{report:?}");
                }
            }
        }
        let mut unretained = config(
            PreparedOlsExecutionMode::Fast,
            4,
            Some(prefix),
            LsmrPreconditioner::Auto,
        );
        unretained.absorb.retain_coefficients = false;
        let report = check_case(&design, &oracle, unretained);
        assert_eq!(report.used_solver, FeSolverKind::Lsmr, "{report:?}");
    }
}
