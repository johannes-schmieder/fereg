#![deny(clippy::too_many_lines)]
//! Independent full-dummy witnesses for the experimental two-way Schur route.
mod support;

use fereg_core::{
    fit_prepared_model_robust_with_loaded_moments,
    fit_prepared_model_unadjusted_with_loaded_moments, prepare_model, CertifiedPreparedOlsFit,
    FeSolverKind, PreparedOlsData, PreparedOlsExecutionMode, PreparedOlsFitConfig,
    PreparedOlsRobustConfig, PreparedOlsUnadjustedConfig, ThreadConfig,
};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsResult};

/// Each moving component contains two workers of every distinct-employer
/// degree 1..7. Unequal repeated cells exercise sums of weights, not row degree.
/// The last component has one firm and two workers with repeated observations;
/// its firm is isolated after eliminating its workers but its rows remain.
fn degree_design(weight_kind: WeightKind) -> Design {
    let mut worker = Vec::new();
    let mut firm = Vec::new();
    let mut next_worker = 0;
    for component in 0..2 {
        for degree in 1..=7 {
            for replica in 0..2 {
                for employer in 0..degree {
                    for _ in 0..2 + (degree + employer + replica) % 3 {
                        worker.push(next_worker);
                        firm.push(component * 7 + (employer + replica) % 7);
                    }
                }
                next_worker += 1;
            }
        }
    }
    for id in next_worker..next_worker + 2 {
        worker.extend([id; 5]);
        firm.extend([14; 5]);
    }
    let n = worker.len();
    let x: Vec<_> = (0..n).map(|r| (0.37 * (r % 41) as f64).sin()).collect();
    let z: Vec<_> = (0..n).map(|r| (0.21 * (r % 37) as f64).cos()).collect();
    let outcome = (0..n)
        .map(|r| {
            1.3 * x[r] - 0.7 * z[r] + 0.13 * f64::from(worker[r]) - 0.19 * f64::from(firm[r])
                + 0.07 * (0.83 * (r % 53) as f64).sin()
        })
        .collect();
    let weights = (0..n)
        .map(|r| match weight_kind {
            WeightKind::Unit => 1.,
            WeightKind::Frequency => 1. + (r % 4) as f64,
            WeightKind::Analytic | WeightKind::Probability => 0.25 + (r % 11) as f64 / 4.,
        })
        .collect();
    Design {
        outcome,
        regressors: vec![x, z],
        effects: vec![
            DummyEffect::new(worker, (next_worker + 2) as usize, true, vec![]).unwrap(),
            DummyEffect::new(firm, 15, true, vec![]).unwrap(),
        ],
        weights,
        weight_kind,
        missing: vec![false; n],
        clusters: vec![],
    }
}

fn config(cutoff: usize, mode: PreparedOlsExecutionMode, workers: usize) -> PreparedOlsFitConfig {
    let threads = ThreadConfig::from_requested(workers, true).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.execution.mode = mode;
    config.execution.poolsize = 1;
    config.execution.threads = threads;
    config.absorb.lsmr.threads = threads;
    config.absorb.solver = FeSolverKind::CmgSchur;
    config.absorb.cmg_schur_degree = cutoff;
    config.absorb.retain_coefficients = true;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-14);
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
    let fitted: Vec<_> = design
        .outcome
        .iter()
        .zip(fit.residuals())
        .map(|(y, r)| y - r)
        .collect();
    PREDICTION
        .check_slice(&fitted, &oracle.fitted_values)
        .unwrap();
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    assert_eq!(fit.absorption_report().used_solver, FeSolverKind::CmgSchur);
    assert_eq!(
        fit.residual_absorption_report().used_solver,
        FeSolverKind::CmgSchur
    );
    assert!(!fit.absorption_report().fallback_used);
    assert!(!fit.residual_absorption_report().fallback_used);
}

fn check_saved_fe(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    oracle: &ExplicitOlsResult,
    design: &Design,
    config: PreparedOlsFitConfig,
) {
    if config.execution.mode == PreparedOlsExecutionMode::Fast {
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
        .check_slice(saved.absorbed_sum(), &oracle.absorbed_fitted_values)
        .unwrap();
    let channel_sum: Vec<_> = saved.channels()[0]
        .as_ref()
        .unwrap()
        .iter()
        .zip(saved.channels()[1].as_ref().unwrap())
        .map(|(a, b)| a + b)
        .collect();
    PREDICTION
        .check_slice(&channel_sum, &oracle.absorbed_fitted_values)
        .unwrap();
}

fn check_inference(
    design: &Design,
    prepared: &PreparedOlsData,
    oracle: &ExplicitOlsResult,
    config: PreparedOlsFitConfig,
) {
    let robust_config = PreparedOlsRobustConfig {
        fit: config,
        ..PreparedOlsRobustConfig::default()
    };
    let robust =
        fit_prepared_model_robust_with_loaded_moments(prepared, robust_config, None).unwrap();
    check_fit(robust.fit(), oracle, design);
    check_saved_fe(prepared, robust.fit(), oracle, design, config);
    assert_eq!(
        robust.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    assert_eq!(robust.dof().absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(
        robust.weights().effective_observations,
        oracle.effective_observations
    );
    INFERENCE
        .check(robust.statistics().rss, oracle.weighted_rss)
        .unwrap();
    INFERENCE
        .check_slice(
            robust.covariance().values(),
            oracle
                .normalized_reported_intercept_hc1_vce
                .as_ref()
                .unwrap()
                .as_slice(),
        )
        .unwrap();
    let again =
        fit_prepared_model_robust_with_loaded_moments(prepared, robust_config, None).unwrap();
    // Timings are deliberately excluded; scientific fields repeat exactly.
    assert_eq!(robust.fit().coefficients(), again.fit().coefficients());
    assert_eq!(robust.fit().residuals(), again.fit().residuals());
    assert_eq!(
        robust.fit().recovered_fe_coefficients(),
        again.fit().recovered_fe_coefficients()
    );
    assert_eq!(robust.covariance(), again.covariance());
    assert_eq!(robust.dof(), again.dof());
    assert_eq!(
        robust.fit().absorption_report().iterations,
        again.fit().absorption_report().iterations
    );
    if design.weight_kind != WeightKind::Probability {
        let iid = fit_prepared_model_unadjusted_with_loaded_moments(
            prepared,
            PreparedOlsUnadjustedConfig {
                fit: config,
                ..PreparedOlsUnadjustedConfig::default()
            },
            None,
        )
        .unwrap();
        check_fit(iid.fit(), oracle, design);
        assert_eq!(iid.dof().residual_dof, oracle.residual_degrees_of_freedom);
        for i in 0..design.regressors.len() {
            for j in 0..design.regressors.len() {
                INFERENCE
                    .check(
                        iid.covariance().values()[i * iid.covariance().dimension() + j],
                        oracle.iid_vce[(i, j)],
                    )
                    .unwrap();
            }
        }
    }
}

#[test]
fn every_cutoff_matches_dense_dummy_ols_on_weighted_disconnected_degree_one_through_seven() {
    for weight in [
        WeightKind::Unit,
        WeightKind::Analytic,
        WeightKind::Frequency,
        WeightKind::Probability,
    ] {
        let design = degree_design(weight);
        let (problem, expected_rows) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        let (request, loaded) = support::inputs(&design, false);
        let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
        assert_eq!(
            prepared
                .sample()
                .original_row_by_active
                .iter()
                .collect::<Vec<_>>(),
            expected_rows
        );
        assert_eq!(expected_rows.len(), design.outcome.len());
        for cutoff in [4, 5, 6] {
            for mode in [
                PreparedOlsExecutionMode::Fast,
                PreparedOlsExecutionMode::Low,
            ] {
                for workers in [1, 4] {
                    check_inference(&design, &prepared, &oracle, config(cutoff, mode, workers));
                }
            }
        }
        // The coefficient-only public entry must agree with the same oracle
        // and preserve the same prepared sample as both inference pipelines.
        let (fit, rows) =
            support::native_fit(&design, config(5, PreparedOlsExecutionMode::Fast, 1), false)
                .unwrap();
        assert_eq!(rows, expected_rows);
        check_fit(&fit, &oracle, &design);
    }
}
