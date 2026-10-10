#![deny(clippy::too_many_lines)]
mod support;
use fereg_core::{FeSolverKind, PreparedOlsExecutionMode, PreparedOlsFitConfig};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{generate, Design, DesignSpec, Graph, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, OracleWeights};
use support::native_fit;
#[path = "support/counterexample.rs"]
mod regression;
use regression::counterexample;

#[test]
fn default_routes_match_seeded_dense_ols_oracles() {
    check_seeds(0..96);
}

#[test]
#[ignore = "long differential suite: 2,000 independent seeded designs"]
fn default_routes_long_differential_suite() {
    check_seeds(0..2000);
}

fn check_seeds(seeds: std::ops::Range<u64>) {
    for seed in seeds {
        let graphs = [
            Graph::DensePanel,
            Graph::AkmStrong,
            Graph::AkmWeak,
            Graph::Chain,
            Graph::Barbell,
            Graph::Disconnected,
        ];
        let weights = [
            WeightKind::Unit,
            WeightKind::Analytic,
            WeightKind::Frequency,
            WeightKind::Probability,
        ];
        let q = (seed as usize / 4) % 4;
        let spec = DesignSpec {
            rows: 96 + (seed as usize % 4) * 24,
            levels: vec![4 + (seed as usize % 3); q],
            regressors: 1 + (seed as usize / 3) % 3,
            graph: graphs[(seed as usize / 16) % 6],
            weights: weights[seed as usize % 4],
            slopes: seed % 7 == 0,
            missing_fraction: if seed % 5 == 0 { 0.06 } else { 0.0 },
            singleton: seed % 3 == 0,
            collinear: seed % 11 == 0,
            ..DesignSpec::default()
        };
        let design = generate(seed, &spec).unwrap();
        compare_native(
            &design,
            PreparedOlsFitConfig::default(),
            &format!("seed {seed}, {spec:?}"),
        );
    }
}

fn compare_native(design: &Design, config: PreparedOlsFitConfig, context: &str) {
    let (problem, rows) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap_or_else(|e| panic!("oracle {context}: {e}"));
    let (fit, prepared) =
        native_fit(design, config, false).unwrap_or_else(|e| panic!("native {context}: {e}"));
    assert_eq!(prepared, rows, "{context}");
    PREDICTION
        .check_slice(fit.residuals(), &oracle.residuals)
        .unwrap_or_else(|e| panic!("residual {context}: {e}"));
    let fitted = rows
        .iter()
        .zip(fit.residuals())
        .map(|(&i, &r)| design.outcome[i] - r)
        .collect::<Vec<_>>();
    PREDICTION
        .check_slice(&fitted, &oracle.fitted_values)
        .unwrap_or_else(|e| panic!("fitted {context}: {e}"));
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank, "{context}");
    // Duplicate regressors identify only their combined contribution, not individual slots.
    if oracle.coefficients.iter().all(Option::is_some) {
        for (actual, expected) in fit.coefficients().iter().zip(&oracle.coefficients) {
            COEFFICIENT
                .check(actual.value.unwrap(), expected.unwrap())
                .unwrap_or_else(|e| panic!("coefficient {context}: {e}"));
        }
    }
    let q = match problem.weights {
        OracleWeights::Unit => vec![1.0; rows.len()],
        OracleWeights::Wls(w) => w,
        OracleWeights::Frequency(w) => w.into_iter().map(f64::from).collect(),
    };
    let rss = fit
        .residuals()
        .iter()
        .zip(q)
        .map(|(&r, w)| r * r * w)
        .sum::<f64>();
    INFERENCE
        .check(rss, oracle.weighted_rss)
        .unwrap_or_else(|e| panic!("RSS {context}: {e}"));
}

#[test]
fn repaired_cmg_and_retained_lsmr_match_the_800_row_regression() {
    for mode in [
        PreparedOlsExecutionMode::Fast,
        PreparedOlsExecutionMode::Low,
    ] {
        for solver in [FeSolverKind::Auto, FeSolverKind::Cmg, FeSolverKind::Lsmr] {
            let mut config = PreparedOlsFitConfig::default();
            config.execution.mode = mode;
            config.execution.poolsize = 1;
            config.absorb.solver = solver;
            compare_native(
                &counterexample(),
                config,
                &format!("CMG regression {mode:?} {solver:?}"),
            );
        }
    }
}

/// SPEC section 20 witness: the FE moment certificate alone can accept a wrong
/// CG result. CG remains rejected at the public ado boundary (`solver(cg)`).
#[test]
fn cg_counterexample_shows_moment_certification_alone_is_insufficient() {
    let design = counterexample();
    let (problem, _) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Cg;
    let (fit, _) = native_fit(&design, config, false).unwrap();
    assert!(fit.residual_absorption_report().certified());
    assert!(
        COEFFICIENT
            .check(
                fit.coefficients()[0].value.unwrap(),
                oracle.coefficients[0].unwrap()
            )
            .is_err(),
        "the CG counterexample no longer separates certification from accuracy; replace this SPEC section 20 witness"
    );
}

/// Regression (2026-10-10): saved outputs run the final residual projection at
/// 1e-14. When explicit LSMR stopped its recurrence at that same tolerance,
/// this 100,000-row AKM design failed its residual certificate (3 of 120
/// scanned seeds did); the 1e-15 recurrence floor keeps a margin.
#[test]
fn explicit_lsmr_final_residual_projection_keeps_a_margin() {
    let design = generate(
        20261013,
        &DesignSpec {
            rows: 100_000,
            levels: vec![10_000, 1_000],
            regressors: 3,
            graph: Graph::AkmStrong,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Lsmr;
    config.absorb.map.tolerance = 1e-8;
    config.absorb.map.max_iterations = 16_000;
    config.absorb.lsmr.tolerance = 1e-8;
    config.absorb.lsmr.max_iterations = 16_000;
    config.allow_final_fe_recovery = true;
    config.final_residual_tolerance = Some(1e-14);
    let (fit, _) = native_fit(&design, config, false)
        .expect("explicit LSMR certifies the 1e-14 final residual");
    assert!(fit.residual_absorption_report().certified());
}

#[test]
fn cmg_rejects_unsupported_slopes_while_lsmr_remains_available() {
    let design = generate(
        92,
        &DesignSpec {
            slopes: true,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Cmg;
    assert!(native_fit(&design, config, false).is_err());
    config.absorb.solver = FeSolverKind::Lsmr;
    compare_native(&design, config, "explicit LSMR slopes");
}

#[test]
fn inference_matches_independent_dense_outputs_on_exact_dof_designs() {
    use fereg_core::{
        fit_prepared_model_robust_with_loaded_moments,
        fit_prepared_model_unadjusted_with_loaded_moments, prepare_model, PreparedOlsRobustConfig,
        PreparedOlsUnadjustedConfig,
    };
    for seed in 0..32 {
        let kind = [
            WeightKind::Unit,
            WeightKind::Analytic,
            WeightKind::Frequency,
            WeightKind::Probability,
        ][seed as usize % 4];
        let spec = DesignSpec {
            levels: vec![5; 1 + (seed as usize / 4) % 2],
            weights: kind,
            graph: if seed % 3 == 0 {
                Graph::Disconnected
            } else {
                Graph::DensePanel
            },
            ..DesignSpec::default()
        };
        let design = generate(seed, &spec).unwrap();
        let (problem, _) = design.oracle_problem(false).unwrap();
        let oracle = fit_explicit_ols(&problem).unwrap();
        let (request, loaded) = support::inputs(&design, false);
        let prepared = prepare_model(&request, &[], &loaded, true).unwrap();
        let robust = fit_prepared_model_robust_with_loaded_moments(
            &prepared,
            PreparedOlsRobustConfig::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            robust.dof().residual_dof,
            oracle.residual_degrees_of_freedom
        );
        assert_eq!(
            robust.weights().effective_observations,
            oracle.effective_observations
        );
        INFERENCE
            .check(robust.statistics().rss, oracle.weighted_rss)
            .unwrap();
        let target = oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .unwrap();
        INFERENCE
            .check_slice(robust.covariance().values(), target.as_slice())
            .unwrap();
        if kind != WeightKind::Probability {
            let iid = fit_prepared_model_unadjusted_with_loaded_moments(
                &prepared,
                PreparedOlsUnadjustedConfig::default(),
                None,
            )
            .unwrap();
            assert_eq!(iid.dof().residual_dof, oracle.residual_degrees_of_freedom);
            for i in 0..spec.regressors {
                for j in 0..spec.regressors {
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
}
