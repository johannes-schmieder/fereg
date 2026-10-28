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

/// Strict presets certify many-FE exits at a hundredth of the request; the
/// public geometry policy (`prefix_limit` of `None`) accepts at the request
/// and may run its bounded pilot.
fn check_fit(
    design: &Design,
    oracle: &ExplicitOlsResult,
    fit: &CertifiedPreparedOlsFit,
    prefix_limit: Option<usize>,
) {
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
        let tolerance = if design.effects.len() >= 3 && prefix_limit.is_some() {
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
            let Some(prefix_limit) = prefix_limit else {
                assert!(report.pilot_iterations <= 4);
                continue;
            };
            assert!(report.pilot_iterations <= prefix_limit);
            if report.used_solver == FeSolverKind::Lsmr {
                assert_eq!(report.pilot_iterations, prefix_limit);
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

/// Uninjected public Auto uses the many-FE geometry policy for at least three
/// categorical Auto-preconditioned effects and an unseeded policy otherwise.
fn prefix_limit(design: &Design, config: PreparedOlsFitConfig) -> Option<usize> {
    let routing = config.absorb.routing;
    let geometry = routing.is_none_or(|p| {
        p == RoutingPreset::geometry_many() || p == RoutingPreset::geometry_many_windows()
    });
    if geometry && design.effects.len() >= 3 {
        None
    } else if routing.is_none()
        || matches!(routing, Some(p) if p == RoutingPreset::portfolio_unseeded()
        || p == RoutingPreset::incremental_many() || p == RoutingPreset::incremental_two())
    {
        Some(0)
    } else {
        Some(4)
    }
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
    let prefix_limit = prefix_limit(design, config);
    check_fit(design, &oracle, result.fit(), prefix_limit);
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

fn same_discrete_reports(first: &SolveReport, second: &SolveReport) {
    assert_eq!(first.requested_solver, second.requested_solver);
    assert_eq!(first.used_solver, second.used_solver);
    assert_eq!(first.stop_reason, second.stop_reason);
    assert_eq!(first.fallback_used, second.fallback_used);
    assert_eq!(
        first.requested_preconditioner,
        second.requested_preconditioner
    );
    assert_eq!(first.used_preconditioner, second.used_preconditioner);
    assert_eq!(first.pilot_iterations, second.pilot_iterations);
    assert_eq!(first.checkpoint_retries, second.checkpoint_retries);
    assert_eq!(first.warnings, second.warnings);
}

#[test]
fn portfolio_unseeded_matches_strict_qr_and_discrete_decisions_across_workers() {
    use PreparedOlsExecutionMode::{Fast, Low};
    for (effects, deficient, mode, preconditioner) in [
        (3, false, Fast, LsmrPreconditioner::Off),
        (5, true, Low, LsmrPreconditioner::Auto),
        (16, false, Fast, LsmrPreconditioner::Auto),
    ] {
        let design = design(effects, 3, deficient, false);
        let run = |workers| {
            let mut options = config(mode, workers);
            options.absorb.routing = Some(RoutingPreset::portfolio_unseeded());
            options.absorb.lsmr.preconditioner = preconditioner;
            check_case(&design, options)
        };
        let first = run(1);
        let second = run(4);
        assert_eq!(first.fit().finite_rank(), second.fit().finite_rank());
        assert_eq!(
            first.fit().kept_generated_columns(),
            second.fit().kept_generated_columns()
        );
        assert_eq!(
            first.fit().dropped_generated_columns(),
            second.fit().dropped_generated_columns()
        );
        same_discrete_reports(
            first.fit().absorption_report(),
            second.fit().absorption_report(),
        );
        same_discrete_reports(
            first.fit().residual_absorption_report(),
            second.fit().residual_absorption_report(),
        );
        assert_eq!(first.dof(), second.dof());
    }
}

#[test]
fn portfolio_unseeded_preserves_v1_two_fe_and_unaffected_scopes_exactly() {
    use PreparedOlsExecutionMode::{Fast, Low};
    for (effects, slopes, mode, workers) in [
        (0, false, Fast, 1),
        (1, false, Low, 4),
        (2, false, Fast, 1),
        (2, false, Low, 4),
        (3, true, Fast, 1),
    ] {
        let design = design(effects, 3, false, slopes);
        let v1 = config(mode, workers);
        let mut control = v1;
        control.absorb.routing = Some(RoutingPreset::portfolio_unseeded());
        let (expected, expected_rows) = support::native_fit(&design, v1, false).unwrap();
        let (actual, actual_rows) = support::native_fit(&design, control, false).unwrap();
        assert_eq!(expected_rows, actual_rows);
        exact_fit(&expected, &actual);
    }
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

#[test]
fn incremental_families_match_dense_qr_and_existing_portfolio_exactly() {
    use PreparedOlsExecutionMode::{Fast, Low};
    for (effects, mode, deficient, preset) in [
        (2, Fast, false, RoutingPreset::incremental_two()),
        (2, Low, true, RoutingPreset::incremental_two()),
        (3, Fast, false, RoutingPreset::incremental_many()),
        (5, Low, true, RoutingPreset::incremental_many()),
        (16, Fast, false, RoutingPreset::incremental_many()),
    ] {
        let design = design(effects, 3, deficient, false);
        let mut previous: Option<CertifiedPreparedOlsRobust> = None;
        for workers in [1, 4] {
            let mut options = config(mode, workers);
            options.absorb.routing = Some(preset);
            let actual = check_case(&design, options);
            options.absorb.routing = Some(RoutingPreset::portfolio_unseeded());
            let expected = check_case(&design, options);
            exact_fit(actual.fit(), expected.fit());
            assert_eq!(actual.covariance(), expected.covariance());
            assert_eq!(actual.dof(), expected.dof());
            if preset == RoutingPreset::incremental_many() {
                // Public Auto selects the geometry policy; its hidden token
                // must agree exactly.
                options.absorb.routing = None;
                let public = check_case(&design, options);
                options.absorb.routing = Some(RoutingPreset::geometry_many());
                let hidden = check_case(&design, options);
                exact_fit(hidden.fit(), public.fit());
                assert_eq!(hidden.covariance(), public.covariance());
                assert_eq!(hidden.dof(), public.dof());
            }
            if let Some(previous) = previous {
                same_discrete_reports(
                    actual.fit().absorption_report(),
                    previous.fit().absorption_report(),
                );
                same_discrete_reports(
                    actual.fit().residual_absorption_report(),
                    previous.fit().residual_absorption_report(),
                );
                assert_eq!(actual.dof(), previous.dof());
            }
            previous = Some(actual);
        }
    }
}

#[test]
fn incremental_families_preserve_ineligible_science_and_memory_exactly() {
    use PreparedOlsExecutionMode::{Fast, Low};
    for preset in [
        RoutingPreset::incremental_many(),
        RoutingPreset::incremental_two(),
        RoutingPreset::geometry_many(),
    ] {
        for (effects, slopes, preconditioner) in [
            (0, false, LsmrPreconditioner::Auto),
            (1, false, LsmrPreconditioner::Auto),
            (3, true, LsmrPreconditioner::Auto),
            (2, false, LsmrPreconditioner::Off),
            (3, false, LsmrPreconditioner::Diagonal),
            (3, false, LsmrPreconditioner::Schwarz),
            (
                if preset == RoutingPreset::incremental_two() {
                    5
                } else {
                    2
                },
                false,
                LsmrPreconditioner::Auto,
            ),
        ] {
            let design = design(effects, 3, false, slopes);
            for mode in [Fast, Low] {
                let mut options = config(mode, 1);
                options.absorb.lsmr.preconditioner = preconditioner;
                options.absorb.routing = None;
                let (expected, rows) = support::native_fit(&design, options, false).unwrap();
                options.absorb.routing = Some(preset);
                let (actual, actual_rows) = support::native_fit(&design, options, false).unwrap();
                assert_eq!(rows, actual_rows);
                exact_fit(&actual, &expected);
                assert_eq!(
                    actual.solver_workspace_peak(),
                    expected.solver_workspace_peak()
                );
            }
        }
    }
}

/// Independent dense-QR witness for the public Jacobi trial: a mixing design
/// finishes on the diagonal solver, while a weakly connected one escalates to
/// the additive preconditioner. Both stay within the scientific tolerances,
/// repeat exactly at a fixed worker count and agree with the hidden token.
#[test]
fn public_geometry_trial_matches_dense_qr_with_and_without_escalation() {
    for (graph, finished) in [
        (Graph::RandomMobility, LsmrPreconditioner::Diagonal),
        (Graph::AkmWeak, LsmrPreconditioner::Schwarz),
    ] {
        let design = generate(
            72031,
            &DesignSpec {
                rows: 3400,
                levels: vec![200, 60, 12],
                regressors: 2,
                graph,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let (problem, _) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        let (request, loaded) = support::inputs(&design, false);
        let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
        for workers in [1, 4] {
            let options = config(PreparedOlsExecutionMode::Fast, workers);
            let run = |routing| {
                let mut options = options;
                options.absorb.routing = routing;
                fit_prepared_model_robust_with_loaded_moments(
                    &prepared,
                    PreparedOlsRobustConfig {
                        fit: options,
                        ..PreparedOlsRobustConfig::default()
                    },
                    None,
                )
                .unwrap()
            };
            let public = run(None);
            check_fit(&design, &oracle, public.fit(), None);
            let report = public.fit().absorption_report();
            assert_eq!(
                report.used_solver,
                FeSolverKind::Lsmr,
                "{graph:?} {report:?}"
            );
            assert_eq!(
                report.used_preconditioner,
                Some(finished),
                "{graph:?} {report:?}"
            );
            assert_eq!(public.dof().absorbed_rank, oracle.explicit_dummy_rank);
            INFERENCE
                .check(public.statistics().rss, oracle.weighted_rss)
                .unwrap();
            for repeat in [run(None), run(Some(RoutingPreset::geometry_many()))] {
                exact_fit(repeat.fit(), public.fit());
                assert_eq!(repeat.covariance(), public.covariance());
            }
        }
    }
}

/// Independent dense-QR witness for the candidate bounded LSMR session
/// windows (hidden `routing_many_windows_v1`): at most one session per worker,
/// with the Jacobi trial's escalation barrier at the end of each window. On a
/// mixing design that finishes on the diagonal solver and a weak AKM design
/// that escalates, every worker count stays within the scientific tolerances
/// of the explicit-dummy QR, repeats exactly at a fixed worker count, keeps
/// the public fit's discrete decisions and equals it exactly where one window
/// holds every RHS or no RHS escalates.
#[test]
fn bounded_session_windows_match_dense_qr_and_public_discrete_decisions() {
    for graph in [Graph::RandomMobility, Graph::AkmWeak] {
        let design = generate(
            72031,
            &DesignSpec {
                rows: 3400,
                levels: vec![200, 60, 12],
                regressors: 6,
                graph,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let (problem, _) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        let (request, loaded) = support::inputs(&design, false);
        let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
        let rhs = build_prepared_ols_rhs(&prepared).unwrap().values().ncols();
        assert_eq!(rhs, 8);
        for workers in [1, 2, 4, 8] {
            let options = config(PreparedOlsExecutionMode::Fast, workers);
            let run = |routing| {
                let mut options = options;
                options.absorb.routing = routing;
                fit_prepared_model_robust_with_loaded_moments(
                    &prepared,
                    PreparedOlsRobustConfig {
                        fit: options,
                        ..PreparedOlsRobustConfig::default()
                    },
                    None,
                )
                .unwrap()
            };
            let public = run(None);
            let windows = run(Some(RoutingPreset::geometry_many_windows()));
            check_fit(&design, &oracle, windows.fit(), None);
            assert_eq!(windows.dof(), public.dof());
            assert_eq!(windows.dof().absorbed_rank, oracle.explicit_dummy_rank);
            INFERENCE
                .check(windows.statistics().rss, oracle.weighted_rss)
                .unwrap();
            let report = windows.fit().absorption_report();
            assert_eq!(report.used_solver, FeSolverKind::Lsmr, "{graph:?}");
            assert!(!report.fallback_used);
            assert_eq!(
                report.used_preconditioner,
                public.fit().absorption_report().used_preconditioner
            );
            let repeat = run(Some(RoutingPreset::geometry_many_windows()));
            exact_fit(repeat.fit(), windows.fit());
            assert_eq!(repeat.covariance(), windows.covariance());
            // Without escalation every RHS runs the same independent Jacobi
            // recurrence in any window; only a window barrier can change bits.
            // The window's smaller warm-session charge lowers only the
            // reported workspace peak.
            if workers >= rhs || matches!(graph, Graph::RandomMobility) {
                assert_eq!(windows.fit().coefficients(), public.fit().coefficients());
                assert_eq!(windows.fit().residuals(), public.fit().residuals());
                assert_eq!(
                    windows.fit().recovered_fe_coefficients(),
                    public.fit().recovered_fe_coefficients()
                );
                assert_eq!(windows.covariance(), public.covariance());
                for (left, right) in [
                    (
                        windows.fit().absorption_report(),
                        public.fit().absorption_report(),
                    ),
                    (
                        windows.fit().residual_absorption_report(),
                        public.fit().residual_absorption_report(),
                    ),
                ] {
                    same_discrete_reports(left, right);
                    assert_eq!(left.iterations, right.iterations);
                    assert_eq!(left.certificate, right.certificate);
                }
            }
        }
    }
}
