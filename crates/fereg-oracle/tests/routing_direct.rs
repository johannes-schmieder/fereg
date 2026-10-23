#![deny(clippy::too_many_lines)]
//! Actual prepared direct admission with independent full-dummy QR science.
mod support;
#[path = "support/routing_direct.rs"]
mod witness;

use fereg_core::fe::RoutingPreset;
use fereg_core::{
    build_prepared_ols_rhs, fit_prepared_model_robust_with_loaded_moments, prepare_model,
    PreparedOlsExecutionMode, PreparedOlsFitConfig, PreparedOlsRobustConfig, ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::explicit_ols::fit_explicit_ols;

fn config(mode: PreparedOlsExecutionMode, workers: usize) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = mode;
    config.execution.poolsize = 63;
    config.execution.threads = threads;
    config.absorb.lsmr.threads = threads;
    config.absorb.routing = Some(RoutingPreset::ablation("direct").unwrap());
    config.absorb.retain_coefficients = true;
    config.absorb.map.tolerance = 1e-12;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-12);
    config
}

fn assert_unrouted_requires_iterations(design: &fereg_oracle::designs::Design, workers: usize) {
    let mut baseline = config(PreparedOlsExecutionMode::Fast, workers);
    baseline.absorb.routing = None;
    let (fit, rows) = support::native_fit(design, baseline, false).unwrap();
    assert_eq!(rows, (0..128).collect::<Vec<_>>());
    assert!(fit.absorption_report().certified());
    assert!(fit.absorption_report().iterations > 0);
}

fn assert_packed_rhs(prepared: &fereg_core::PreparedOlsData) {
    let rhs = build_prepared_ols_rhs(prepared).unwrap();
    // 63 finite controls plus the generated reported intercept and outcome.
    assert_eq!((rhs.values().nrows(), rhs.values().ncols()), (128, 65));
    assert!(rhs
        .values()
        .as_slice()
        .chunks_exact(65)
        .all(|row| row[64] == 1.));
}

#[test]
fn direct_full_rhs_admission_and_low_chunks_match_dense_qr_with_saved_gauge() {
    for weighted in [false, true] {
        let design = witness::design(weighted);
        let (problem, rows) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        assert_eq!(oracle.finite_regressor_rank, 63);
        assert_eq!(oracle.explicit_dummy_rank, if weighted { 40 } else { 33 });
        assert_eq!(rows, (0..128).collect::<Vec<_>>());
        let channels = witness::saved_channels(&design, &oracle);
        let constant = witness::reported_constant(&design, &oracle);
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for workers in [1, 4] {
                let config = config(mode, workers);
                let (request, loaded) = support::inputs(&design, false);
                let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
                assert_packed_rhs(&prepared);
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
                assert!(fit.absorption_report().certified());
                assert!(fit.residual_absorption_report().certified());
                if mode == PreparedOlsExecutionMode::Fast {
                    // The identical incumbent requires a retained-firm solve;
                    // only the admitted direct route finishes in zero iterations.
                    assert_unrouted_requires_iterations(&design, workers);
                    assert_eq!(fit.absorption_report().iterations, 0);
                }
                assert_eq!(fit.finite_rank(), 63);
                assert_eq!(fit.kept_generated_columns(), oracle.kept_regressors);
                assert!(fit.dropped_generated_columns().is_empty());
                for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
                    COEFFICIENT
                        .check(actual.value.unwrap(), expected.unwrap())
                        .unwrap();
                }
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
                    .map(|value| value - constant)
                    .collect();
                let saved = prepared
                    .recover_absorbed_effects_fast(
                        &target,
                        &[true, true],
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
                for (actual, expected) in saved.channels().iter().zip(&channels) {
                    PREDICTION
                        .check_slice(actual.as_ref().unwrap(), expected)
                        .unwrap();
                }
            }
        }
    }
}
