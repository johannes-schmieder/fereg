#![deny(clippy::too_many_lines)]
//! Independent dense-QR witnesses through the explicit prepared IT caller.
mod support;

use fereg_core::{
    fit_prepared_model_coefficients, prepare_model, CertifiedPreparedOlsFit, ErrorCode,
    FeSolverKind, PreparedOlsExecutionMode, PreparedOlsFitConfig, ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_dummy::{
    design::ExplicitFeDesign, fe_remaining_projection_with_reference,
};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsResult, OracleMatrix};

fn disconnected_design(effects: usize, slopes: bool) -> Design {
    let sizes = [3, 3, 2, 2, 2];
    let mut codes = vec![Vec::new(); effects];
    for component in 0..2 {
        for cell in 0..72 {
            // Unequal cell multiplicities and nonseparable weights prevent a
            // Cartesian-product shortcut from replacing the iterative caller.
            for _ in 0..2 + cell % 3 {
                let mut index = cell;
                for (effect, values) in codes.iter_mut().enumerate() {
                    values.push((component * sizes[effect] + index % sizes[effect]) as u32);
                    index /= sizes[effect];
                }
            }
        }
    }
    let n = codes[0].len();
    let x: Vec<_> = (0..n)
        .map(|r| ((17 * r % 101) as f64 * 0.13).sin())
        .collect();
    let z: Vec<_> = (0..n)
        .map(|r| ((31 * r % 97) as f64 * 0.17).cos())
        .collect();
    let slope: Vec<_> = (0..n).map(|r| ((r * 19 % 43) as f64 - 21.) / 8.).collect();
    let outcome = (0..n)
        .map(|r| {
            1.2 * x[r] - 0.6 * z[r]
                + codes
                    .iter()
                    .enumerate()
                    .map(|(e, c)| (e + 1) as f64 * f64::from(c[r]) / 7.)
                    .sum::<f64>()
                + if slopes {
                    slope[r] * (0.2 + f64::from(codes[0][r]) / 5.)
                } else {
                    0.
                }
                + 0.04 * ((r * 11 % 71) as f64).sin()
        })
        .collect();
    Design {
        outcome,
        regressors: vec![x, z],
        effects: codes
            .into_iter()
            .enumerate()
            .map(|(e, c)| {
                DummyEffect::new(
                    c,
                    2 * sizes[e],
                    true,
                    if slopes && e == 0 {
                        vec![slope.clone()]
                    } else {
                        vec![]
                    },
                )
                .unwrap()
            })
            .collect(),
        weight_kind: WeightKind::Analytic,
        weights: (0..n).map(|r| 0.25 + (r * 13 % 19) as f64 / 8.).collect(),
        missing: vec![false; n],
        clusters: vec![],
    }
}

fn config(mode: PreparedOlsExecutionMode, workers: usize) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = mode;
    config.execution.poolsize = 1;
    config.execution.threads = threads;
    config.absorb.lsmr.threads = threads;
    config.absorb.solver = FeSolverKind::IronsTuck;
    config.absorb.map.tolerance = 1e-12;
    config.absorb.retain_coefficients = true;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-12);
    config
}

fn check_fit(fit: &CertifiedPreparedOlsFit, oracle: &ExplicitOlsResult, design: &Design) {
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_eq!(fit.kept_generated_columns(), oracle.kept_regressors);
    assert_eq!(fit.dropped_generated_columns(), oracle.dropped_regressors);
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        COEFFICIENT
            .check(actual.value.unwrap(), expected.unwrap())
            .unwrap();
    }
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap();
    for report in [fit.absorption_report(), fit.residual_absorption_report()] {
        assert!(report.certified());
        assert_eq!(report.requested_solver, FeSolverKind::IronsTuck);
        assert_eq!(report.used_solver, FeSolverKind::IronsTuck);
        assert!(!report.fallback_used);
    }
    let explicit = ExplicitFeDesign::new(design.outcome.len(), design.effects.clone()).unwrap();
    let residual =
        OracleMatrix::from_row_major(design.outcome.len(), 1, fit.residuals().to_vec()).unwrap();
    let original =
        OracleMatrix::from_row_major(design.outcome.len(), 1, design.outcome.clone()).unwrap();
    let audit =
        fe_remaining_projection_with_reference(&explicit, &design.weights, &residual, &original)
            .unwrap();
    assert!(audit.maximum_relative_two_norm < 1e-10, "{audit:?}");
}

fn check_saved_effects(
    design: &Design,
    oracle: &ExplicitOlsResult,
    config: PreparedOlsFitConfig,
) -> CertifiedPreparedOlsFit {
    let (request, loaded) = support::inputs(design, false);
    let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
    let fit = fit_prepared_model_coefficients(&prepared, config).unwrap();
    assert_eq!(
        prepared
            .sample()
            .original_row_by_active
            .iter()
            .collect::<Vec<_>>(),
        (0..design.outcome.len()).collect::<Vec<_>>()
    );
    check_fit(&fit, oracle, design);
    if config.absorb.it_coefficients
        && config.execution.mode == PreparedOlsExecutionMode::Fast
        && design
            .effects
            .iter()
            .all(|effect| effect.slopes().is_empty())
    {
        // Retaining coefficients requires the new compact categorical plan.
        assert!(fit.recovered_fe_coefficients().is_some());
    }
    let target: Vec<_> = design
        .outcome
        .iter()
        .zip(fit.residuals())
        .enumerate()
        .map(|(r, (y, residual))| {
            y - residual
                - design
                    .regressors
                    .iter()
                    .zip(fit.coefficients())
                    .map(|(x, b)| x[r] * b.value.unwrap())
                    .sum::<f64>()
        })
        .collect();
    let channels = design
        .effects
        .iter()
        .map(|effect| usize::from(effect.has_intercept()) + effect.slopes().len())
        .sum();
    let saved = prepared
        .recover_absorbed_effects_fast(
            &target,
            &vec![true; channels],
            fit.recovered_fe_coefficients(),
            false,
            true,
            config.absorb.lsmr,
            config.absorb.map,
            None,
        )
        .unwrap();
    PREDICTION
        .check_slice(saved.absorbed_sum(), &oracle.absorbed_fitted_values)
        .unwrap();
    PREDICTION
        .check_slice(saved.absorbed_sum(), &target)
        .unwrap();
    fit
}

#[test]
fn categorical_it_multiple_rhs_disconnected_weights_match_dense_qr_and_repeat() {
    for effects in [2, 3, 5] {
        let design = disconnected_design(effects, false);
        let (problem, rows) = design.oracle_problem(false).unwrap();
        assert_eq!(rows.len(), design.outcome.len());
        let oracle = fit_explicit_ols(&problem).unwrap();
        let mut projection = problem;
        projection.regressors.clear();
        let within = fit_explicit_ols(&projection).unwrap();
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for (workers, compact) in [(1, false), (4, false), (1, true), (4, true)] {
                let mut config = config(mode, workers);
                config.absorb.it_coefficients = compact;
                let fit = check_saved_effects(&design, &oracle, config);
                INFERENCE
                    .check(
                        fit.initial_absorbed_outcome_sum_of_squares(),
                        within.weighted_rss,
                    )
                    .unwrap();
                if effects == 5 && mode == PreparedOlsExecutionMode::Fast {
                    let again = check_saved_effects(&design, &oracle, config);
                    assert_eq!(fit.coefficients(), again.coefficients());
                    assert_eq!(fit.residuals(), again.residuals());
                    assert_eq!(
                        fit.recovered_fe_coefficients(),
                        again.recovered_fe_coefficients()
                    );
                    assert_eq!(
                        fit.absorption_report().iterations,
                        again.absorption_report().iterations
                    );
                    assert_eq!(
                        fit.residual_absorption_report().iterations,
                        again.residual_absorption_report().iterations
                    );
                }
            }
        }
    }
}

#[test]
fn generic_slope_it_remains_certified_and_explicit_cmg_stays_unsupported() {
    let design = disconnected_design(2, true);
    let (problem, _) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        check_saved_effects(&design, &oracle, config(mode, 1));
    }
    for solver in [FeSolverKind::Cmg, FeSolverKind::CmgSchur] {
        let mut config = config(PreparedOlsExecutionMode::Fast, 1);
        config.absorb.solver = solver;
        let error = support::native_fit(&design, config, false).unwrap_err();
        assert_eq!(error.code, ErrorCode::Unsupported);
    }
}
