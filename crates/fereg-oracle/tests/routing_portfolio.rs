#![deny(clippy::too_many_lines)]
//! Portfolio v1 estimation witnesses against independent explicit-dummy QR.
mod support;

use fereg_core::fe::{LsmrPreconditioner, RoutingPreset, SolveReport};
use fereg_core::{
    build_prepared_ols_rhs, fit_prepared_model_robust_with_loaded_moments, prepare_model,
    CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust, ErrorCode, FeSolverKind, PreparedOlsData,
    PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsRobustConfig, ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{generate, Design, DesignSpec, Graph, WeightKind};
use fereg_oracle::explicit_dummy::{
    design::ExplicitFeDesign, fe_remaining_projection_with_reference,
};
use fereg_oracle::explicit_ols::{fit_explicit_ols, ExplicitOlsResult, OracleMatrix};

fn design(effects: usize, regressors: usize, rank_deficient: bool, slopes: bool) -> Design {
    let mut design = generate(
        19431,
        &DesignSpec {
            rows: 288,
            levels: (0..effects).map(|j| if j < 2 { 6 } else { 4 }).collect(),
            regressors,
            graph: Graph::Disconnected,
            weights: WeightKind::Analytic,
            slopes,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    // An exact zero column has a unique omission decision. Duplicate columns
    // would allow the independent pivoted QR to retain a different valid slot.
    if rank_deficient {
        design.regressors.push(vec![0.0; design.outcome.len()]);
    }
    design
}

fn config(mode: PreparedOlsExecutionMode, workers: usize) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = mode;
    config.execution.poolsize = 3;
    config.execution.threads = threads;
    config.absorb.routing = Some(RoutingPreset::portfolio_v1());
    config.absorb.lsmr.threads = threads;
    config.absorb.lsmr.max_iterations = 128;
    config.absorb.map.tolerance = 1e-12;
    config.absorb.map.max_iterations = 512;
    config.absorb.retain_coefficients = true;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-12);
    config
}

fn scientific_report(report: &SolveReport) -> SolveReport {
    let mut report = report.clone();
    report.setup_seconds = 0.0;
    report.solve_seconds = 0.0;
    report.certification_seconds = 0.0;
    report
}

fn exact_fit(first: &CertifiedPreparedOlsFit, second: &CertifiedPreparedOlsFit) {
    assert_eq!(first.coefficients(), second.coefficients());
    assert_eq!(first.residuals(), second.residuals());
    assert_eq!(first.finite_rank(), second.finite_rank());
    assert_eq!(
        first.kept_generated_columns(),
        second.kept_generated_columns()
    );
    assert_eq!(
        first.dropped_generated_columns(),
        second.dropped_generated_columns()
    );
    assert_eq!(
        first.recovered_fe_coefficients(),
        second.recovered_fe_coefficients()
    );
    assert_eq!(
        scientific_report(first.absorption_report()),
        scientific_report(second.absorption_report())
    );
    assert_eq!(
        scientific_report(first.residual_absorption_report()),
        scientific_report(second.residual_absorption_report())
    );
}

fn check_fit(design: &Design, oracle: &ExplicitOlsResult, fit: &CertifiedPreparedOlsFit) {
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_eq!(fit.kept_generated_columns(), oracle.kept_regressors);
    assert_eq!(fit.dropped_generated_columns(), oracle.dropped_regressors);
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        assert_eq!(actual.value.is_some(), expected.is_some());
        if let (Some(actual), Some(expected)) = (actual.value, *expected) {
            COEFFICIENT.check(actual, expected).unwrap();
        }
    }
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap();
    let explicit = ExplicitFeDesign::new(design.outcome.len(), design.effects.clone()).unwrap();
    let residual =
        OracleMatrix::from_row_major(design.outcome.len(), 1, fit.residuals().to_vec()).unwrap();
    let original =
        OracleMatrix::from_row_major(design.outcome.len(), 1, design.outcome.clone()).unwrap();
    let audit =
        fe_remaining_projection_with_reference(&explicit, &design.weights, &residual, &original)
            .unwrap();
    assert!(audit.maximum_relative_two_norm < 1e-10, "{audit:?}");
    for report in [fit.absorption_report(), fit.residual_absorption_report()] {
        assert!(report.certified(), "{report:?}");
        assert_eq!(report.requested_solver, FeSolverKind::Auto);
        let tolerance = if design.effects.len() >= 3 {
            1e-14
        } else {
            1e-12
        };
        assert!(
            report.certificate.relative_infinity_norm <= tolerance,
            "{report:?}"
        );
        assert!(report.iterations <= 512);
        if design.effects.len() >= 3 {
            assert!(report.pilot_iterations <= 4);
            if report.used_solver == FeSolverKind::Lsmr {
                assert_eq!(report.pilot_iterations, 4);
                assert!(report.iterations <= 128);
            }
        } else {
            assert_eq!(report.pilot_iterations, 0);
        }
    }
}

fn check_saved_effects(
    design: &Design,
    oracle: &ExplicitOlsResult,
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    config: PreparedOlsFitConfig,
) {
    let intercept = design
        .weights
        .iter()
        .zip(&oracle.absorbed_fitted_values)
        .map(|(w, fit)| w * fit)
        .sum::<f64>()
        / design.weights.iter().sum::<f64>();
    COEFFICIENT
        .check(fit.coefficients().last().unwrap().value.unwrap(), intercept)
        .unwrap();
    let target: Vec<_> = oracle
        .absorbed_fitted_values
        .iter()
        .map(|value| value - intercept)
        .collect();
    let saved = prepared
        .recover_absorbed_effects_fast(
            &target,
            &vec![true; design.effects.len()],
            fit.recovered_fe_coefficients(),
            false,
            true,
            config.absorb.lsmr,
            config.absorb.map,
            None,
        )
        .unwrap();
    PREDICTION
        .check_slice(saved.absorbed_sum(), &target)
        .unwrap();
    let mut sum = vec![0.0; design.outcome.len()];
    for (effect, channel) in saved.channels().iter().enumerate() {
        let channel = channel.as_ref().unwrap();
        for (total, value) in sum.iter_mut().zip(channel) {
            *total += value;
        }
        if effect == 0 {
            continue;
        }
        for component in 0..2 {
            let (moment, mass) = design
                .weights
                .iter()
                .enumerate()
                .filter(|(row, _)| usize::from(design.effects[0].codes()[*row] >= 3) == component)
                .fold((0.0, 0.0), |(moment, mass), (row, weight)| {
                    (moment + weight * channel[row], mass + weight)
                });
            assert!((moment / mass).abs() < 1e-9);
        }
    }
    PREDICTION.check_slice(&sum, &target).unwrap();
}

fn check_case(design: &Design, config: PreparedOlsFitConfig) -> CertifiedPreparedOlsRobust {
    let (problem, rows) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    let (request, loaded) = support::inputs(design, false);
    let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
    assert_eq!(
        prepared
            .sample()
            .original_row_by_active
            .iter()
            .collect::<Vec<_>>(),
        rows
    );
    assert_eq!(rows.len(), design.outcome.len());
    assert_eq!(
        build_prepared_ols_rhs(&prepared).unwrap().values().ncols(),
        design.regressors.len() + 2
    );
    let run = || {
        fit_prepared_model_robust_with_loaded_moments(
            &prepared,
            PreparedOlsRobustConfig {
                fit: config,
                ..PreparedOlsRobustConfig::default()
            },
            None,
        )
        .unwrap()
    };
    let result = run();
    check_fit(design, &oracle, result.fit());
    check_saved_effects(design, &oracle, &prepared, result.fit(), config);
    assert_eq!(result.dof().absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(
        result.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    INFERENCE
        .check(result.statistics().rss, oracle.weighted_rss)
        .unwrap();
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
    let again = run();
    exact_fit(result.fit(), again.fit());
    assert_eq!(result.covariance(), again.covariance());
    assert_eq!(result.dof(), again.dof());
    result
}

#[test]
fn portfolio_routing_matches_dense_qr_across_weighted_disconnected_and_wide_designs() {
    use PreparedOlsExecutionMode::{Fast, Low};
    // Representative cases, not a Cartesian product: 65 RHS crosses the
    // existing LSMR window; the Low pool has both full blocks and tail blocks.
    for (effects, regressors, deficient, mode, workers, preconditioner) in [
        (2, 3, false, Fast, 1, LsmrPreconditioner::Auto),
        (2, 3, false, Low, 4, LsmrPreconditioner::Auto),
        (3, 3, false, Fast, 1, LsmrPreconditioner::Off),
        (3, 3, false, Low, 4, LsmrPreconditioner::Auto),
        (5, 3, true, Low, 1, LsmrPreconditioner::Diagonal),
        (8, 3, false, Fast, 4, LsmrPreconditioner::Auto),
        (3, 63, false, Fast, 4, LsmrPreconditioner::Auto),
        (3, 63, false, Low, 1, LsmrPreconditioner::Auto),
    ] {
        let mut options = config(mode, workers);
        options.absorb.lsmr.preconditioner = preconditioner;
        check_case(&design(effects, regressors, deficient, false), options);
    }
}

#[test]
fn portfolio_routing_leaves_zero_one_and_slope_models_exact_and_rejects_explicit_solvers() {
    use PreparedOlsExecutionMode::{Fast, Low};
    for (effects, slopes, mode, workers) in
        [(0, false, Fast, 1), (1, false, Low, 4), (3, true, Fast, 1)]
    {
        let design = design(effects, 3, false, slopes);
        let portfolio = config(mode, workers);
        let incumbent = PreparedOlsFitConfig {
            absorb: fereg_core::fe::AbsorbConfig {
                routing: None,
                ..portfolio.absorb
            },
            ..portfolio
        };
        let (expected, expected_rows) = support::native_fit(&design, incumbent, false).unwrap();
        let (actual, actual_rows) = support::native_fit(&design, portfolio, false).unwrap();
        assert_eq!(expected_rows, actual_rows);
        exact_fit(&expected, &actual);
    }
    let design = design(3, 3, false, false);
    for solver in [
        FeSolverKind::Map,
        FeSolverKind::Lsmr,
        FeSolverKind::IronsTuck,
        FeSolverKind::Cmg,
        FeSolverKind::CmgSchur,
    ] {
        let mut options = config(Fast, 1);
        options.absorb.solver = solver;
        let error = support::native_fit(&design, options, false).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(error
            .message
            .contains("routing qualification requires automatic solver selection"));
    }
}
