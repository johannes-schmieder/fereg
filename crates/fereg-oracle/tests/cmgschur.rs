#![deny(clippy::too_many_lines)]
//! Independent full-dummy witnesses for the experimental two-way Schur route.
#[path = "support/cmgschur_sampling.rs"]
mod sampled;
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

/// A separate, algebraic witness for already-absorbed columns: every nonzero
/// pair shares worker and firm, and w[a] * x[a] + w[b] * x[b] is exactly zero.
/// Dyadic weights, amplitudes and values avoid an iterative projection oracle.
fn cell_contrast(design: &Design, seed: usize) -> Vec<f64> {
    let mut values = vec![0.; design.outcome.len()];
    let worker = design.effects[0].codes();
    let firm = design.effects[1].codes();
    let mut first = 0;
    let mut cell = 0_usize;
    while first < values.len() {
        let mut end = first + 1;
        while end < values.len() && worker[end] == worker[first] && firm[end] == firm[first] {
            end += 1;
        }
        assert!(end >= first + 2);
        let hash = ((cell + 1) as u64)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add((seed as u64 + 1).wrapping_mul(1_442_695_040_888_963_407));
        let amplitude = (((hash >> (seed % 13 + 17)) & 255) as f64 - 127.) / 16.;
        values[first] = design.weights[first + 1] * amplitude;
        values[first + 1] = -design.weights[first] * amplitude;
        assert_eq!(
            design.weights[first] * values[first] + design.weights[first + 1] * values[first + 1],
            0.
        );
        first = end;
        cell += 1;
    }
    values
}

fn tiled_design(weight: WeightKind, controls: usize, all_orthogonal: bool) -> Design {
    let mut design = degree_design(weight);
    let rows = design.outcome.len();
    let active = design.regressors[0].clone();
    let mut columns: Vec<Vec<f64>> = Vec::new();
    for column in 0..controls {
        columns.push(match column {
            0 => cell_contrast(&design, 0),
            2 => vec![0.; rows],
            3 => columns[1].clone(),
            4 if all_orthogonal => vec![0.; rows],
            4 => (0..rows)
                .map(|r| {
                    f64::from(design.effects[0].codes()[r])
                        + 2. * f64::from(design.effects[1].codes()[r])
                })
                .collect(),
            _ if all_orthogonal => cell_contrast(&design, column),
            1 => active.clone(),
            _ => (0..rows)
                .map(|r| {
                    (((r * (column * 13 + 7) + column * 31) % 157) as f64 * 0.17).sin()
                        + f64::from(design.effects[1].codes()[r]) / (column + 1) as f64
                })
                .collect(),
        });
    }
    if all_orthogonal {
        design.outcome = cell_contrast(&design, 37);
    }
    for (column, values) in columns.iter().enumerate() {
        for (y, value) in design.outcome.iter_mut().zip(values) {
            *y += value * (column + 1) as f64 / 8.;
        }
    }
    design.regressors = columns;
    design
}

fn check_tiled_fit(fit: &CertifiedPreparedOlsFit, oracle: &ExplicitOlsResult, design: &Design) {
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_eq!(fit.kept_generated_columns(), oracle.kept_regressors);
    assert_eq!(fit.dropped_generated_columns(), oracle.dropped_regressors);
    for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
        assert_eq!(actual.value.is_some(), expected.is_some());
        if let (Some(actual), Some(expected)) = (actual.value, expected) {
            COEFFICIENT.check(actual, *expected).unwrap();
        }
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
    for report in [fit.absorption_report(), fit.residual_absorption_report()] {
        assert!(report.certified());
        assert_eq!(report.used_solver, FeSolverKind::CmgSchur);
        assert!(!report.fallback_used);
    }
}

fn check_tiled_saved_fe(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    oracle: &ExplicitOlsResult,
    design: &Design,
    config: PreparedOlsFitConfig,
) {
    if config.execution.mode == PreparedOlsExecutionMode::Fast {
        assert!(fit.recovered_fe_coefficients().is_some());
    }
    let finite: Vec<f64> = (0..design.outcome.len())
        .map(|r| {
            design
                .regressors
                .iter()
                .zip(fit.coefficients())
                .map(|(x, b)| x[r] * b.value.unwrap_or(0.))
                .sum()
        })
        .collect();
    let target: Vec<_> = design
        .outcome
        .iter()
        .zip(fit.residuals())
        .zip(&finite)
        .map(|((y, e), xb)| y - e - xb)
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
        .or_else(|error| {
            // For the four-control wholly orthogonal fixture, all four routes
            // (auto/cmg/lsmr/cmgschur) produce an extracted FE target of at most
            // 8.89e-16 (unit) or 3.56e-15 (analytic). Its subtraction noise is
            // outside the FE span. Follow the native saved-output caller's
            // existing model-scale recovery; retain independent QR checks.
            assert_eq!(error.code, fereg_core::ErrorCode::InvalidData);
            assert!(error.message.starts_with(
                "categorical recovery target is not certified to lie in the absorbed span:"
            ));
            let raw: Vec<_> = design
                .outcome
                .iter()
                .zip(&finite)
                .map(|(y, xb)| y - xb)
                .collect();
            let scale = design
                .outcome
                .iter()
                .zip(&finite)
                .zip(fit.residuals())
                .map(|((y, xb), e)| y.abs() + xb.abs() + e.abs())
                .fold(0.0_f64, f64::max);
            prepared.recover_model_effects(
                &raw,
                fit.residuals(),
                scale,
                &[true, true],
                false,
                true,
                fereg_core::MapConfig {
                    tolerance: 1e-12,
                    max_iterations: 20_000,
                    check_every: 1,
                    symmetric: true,
                },
                None,
            )
        })
        .unwrap();
    PREDICTION
        .check_slice(saved.absorbed_sum(), &oracle.absorbed_fitted_values)
        .unwrap();
    let sum: Vec<_> = saved.channels()[0]
        .as_ref()
        .unwrap()
        .iter()
        .zip(saved.channels()[1].as_ref().unwrap())
        .map(|(a, b)| a + b)
        .collect();
    PREDICTION
        .check_slice(&sum, &oracle.absorbed_fitted_values)
        .unwrap();
}

fn check_tiled_inference(
    prepared: &PreparedOlsData,
    design: &Design,
    oracle: &ExplicitOlsResult,
    config: PreparedOlsFitConfig,
) {
    let robust_config = PreparedOlsRobustConfig {
        fit: config,
        ..PreparedOlsRobustConfig::default()
    };
    let run = || {
        fit_prepared_model_robust_with_loaded_moments(prepared, robust_config, None).unwrap_or_else(
            |error| {
                panic!(
                    "controls {}, {:?}, cutoff {}, {:?}, threads {}: {error}",
                    design.regressors.len(),
                    design.weight_kind,
                    config.absorb.cmg_schur_degree,
                    config.execution.mode,
                    config.execution.threads.resolved()
                )
            },
        )
    };
    let result = run();
    check_tiled_fit(result.fit(), oracle, design);
    check_tiled_saved_fe(prepared, result.fit(), oracle, design, config);
    assert_eq!(
        result.dof().residual_dof,
        oracle.residual_degrees_of_freedom
    );
    assert_eq!(result.dof().absorbed_rank, oracle.explicit_dummy_rank);
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
    assert_eq!(result.fit().coefficients(), again.fit().coefficients());
    assert_eq!(result.fit().residuals(), again.fit().residuals());
    assert_eq!(
        result.fit().recovered_fe_coefficients(),
        again.fit().recovered_fe_coefficients()
    );
    assert_eq!(
        result.fit().kept_generated_columns(),
        again.fit().kept_generated_columns()
    );
    assert_eq!(
        result.fit().dropped_generated_columns(),
        again.fit().dropped_generated_columns()
    );
    assert_eq!(result.covariance(), again.covariance());
    assert_eq!(result.dof(), again.dof());
    assert_eq!(
        result.fit().absorption_report().iterations,
        again.fit().absorption_report().iterations
    );
    assert_eq!(
        result.fit().residual_absorption_report().iterations,
        again.fit().residual_absorption_report().iterations
    );
}

fn check_tiled_design(design: &Design, cutoffs: &[usize]) {
    let (problem, rows) = design.oracle_problem(false).unwrap();
    let mut oracle = fit_explicit_ols(&problem).unwrap();
    if design.regressors.len() > 3 && oracle.coefficients[3].is_some() {
        // Independent pivoted QR can choose either identical column. Express
        // its identified coefficient in the original earlier slot before
        // checking native omission decisions. Slot 2 is zero, so this relabel
        // cannot reorder the compact coefficient or covariance coordinates.
        assert_eq!(design.regressors[1], design.regressors[3]);
        assert!(oracle.coefficients[1].is_none());
        assert!(oracle.dropped_regressors.contains(&2));
        oracle.coefficients.swap(1, 3);
        let kept = oracle
            .kept_regressors
            .iter()
            .position(|&column| column == 3)
            .unwrap();
        let dropped = oracle
            .dropped_regressors
            .iter()
            .position(|&column| column == 1)
            .unwrap();
        oracle.kept_regressors[kept] = 1;
        oracle.dropped_regressors[dropped] = 3;
        oracle.dropped_regressors.sort_unstable();
    }
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
    let controls = design.regressors.len();
    let dropped: Vec<_> = [2, 3, 4]
        .into_iter()
        .filter(|&column| column < controls)
        .collect();
    assert_eq!(oracle.dropped_regressors, dropped);
    assert_eq!(oracle.finite_regressor_rank, controls - dropped.len());
    for &cutoff in cutoffs {
        for mode in [
            PreparedOlsExecutionMode::Fast,
            PreparedOlsExecutionMode::Low,
        ] {
            for workers in [1, 4] {
                let mut config = config(cutoff, mode, workers);
                // Fast projects the complete RHS bank. Low independently tests
                // complete four-column scratch blocks and their final tails.
                config.execution.poolsize = 4;
                check_tiled_inference(&prepared, design, &oracle, config);
            }
        }
    }
}

#[test]
fn rhs_tile_tails_and_inactive_columns_preserve_dense_qr_rank_and_inference() {
    for weight in [WeightKind::Unit, WeightKind::Analytic] {
        // Including the outcome, these produce RHS widths 3,4,5,6,9,10.
        for controls in [2, 3, 4, 5, 8, 9] {
            check_tiled_design(&tiled_design(weight, controls, false), &[4, 5, 6]);
        }
    }
}

#[test]
fn wholly_fe_orthogonal_rhs_banks_match_dense_qr_and_repeat() {
    for weight in [WeightKind::Unit, WeightKind::Analytic] {
        for controls in [4, 9] {
            check_tiled_design(&tiled_design(weight, controls, true), &[5]);
        }
    }
}
