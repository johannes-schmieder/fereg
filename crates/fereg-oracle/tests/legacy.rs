#![deny(clippy::too_many_lines)]
mod support;
use fereg_core::{FeSolverKind, PreparedOlsExecutionMode, PreparedOlsFitConfig};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{generate, Design, DesignSpec, Graph, WeightKind};
use fereg_oracle::explicit_ols::{fit_explicit_ols, DummyEffect, OracleWeights};
use support::legacy_fit;

#[test]
fn legacy_default_routes_match_seeded_dense_ols_oracles() {
    check_seeds(0..96);
}

#[test]
#[ignore = "long differential suite: 2,000 independent seeded designs"]
fn legacy_default_routes_long_differential_suite() {
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
        compare_legacy(
            &design,
            PreparedOlsFitConfig::default(),
            &format!("seed {seed}, {spec:?}"),
        );
    }
}

fn compare_legacy(design: &Design, config: PreparedOlsFitConfig, context: &str) {
    let (problem, rows) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap_or_else(|e| panic!("oracle {context}: {e}"));
    let (fit, prepared) =
        legacy_fit(design, config, false).unwrap_or_else(|e| panic!("legacy {context}: {e}"));
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

fn counterexample() -> Design {
    let rows = 800;
    let worker = (0..rows).map(|i| (i / 8) as u32).collect::<Vec<_>>();
    let firm = (0..rows)
        .map(|i| ((i / 8 + i % 8) % 10) as u32)
        .collect::<Vec<_>>();
    let x = (0..rows)
        .map(|i| ((i * 37 + 11) % 997) as f64 / 97.0 - 5.0)
        .collect::<Vec<_>>();
    let outcome = (0..rows)
        .map(|i| {
            0.7 * x[i]
                + (f64::from(worker[i]) * 0.13).sin()
                + (f64::from(firm[i]) * 0.29).cos()
                + (((i * 53 + 7) % 101) as f64 - 50.0) * 0.001
        })
        .collect();
    Design {
        outcome,
        regressors: vec![x],
        effects: vec![
            DummyEffect::new(worker, 100, true, vec![]).unwrap(),
            DummyEffect::new(firm, 10, true, vec![]).unwrap(),
        ],
        weight_kind: WeightKind::Unit,
        weights: vec![1.0; rows],
        missing: vec![false; rows],
        clusters: vec![],
    }
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
            compare_legacy(
                &counterexample(),
                config,
                &format!("CMG regression {mode:?} {solver:?}"),
            );
        }
    }
}

#[test]
#[ignore = "diagnostic legacy-only CG defect; CG is rejected by the public ado boundary"]
fn legacy_cg_counterexample_detects_false_certification() {
    let design = counterexample();
    let (problem, _) = design.oracle_problem(false).unwrap();
    let oracle = fit_explicit_ols(&problem).unwrap();
    let mut config = PreparedOlsFitConfig::default();
    config.absorb.solver = FeSolverKind::Cg;
    let (fit, _) = legacy_fit(&design, config, false).unwrap();
    assert!(fit.residual_absorption_report().certified());
    assert!(
        COEFFICIENT
            .check(
                fit.coefficients()[0].value.unwrap(),
                oracle.coefficients[0].unwrap()
            )
            .is_err(),
        "update/remove this legacy-only diagnostic once the known CG defect is gone"
    );
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
    assert!(legacy_fit(&design, config, false).is_err());
    config.absorb.solver = FeSolverKind::Lsmr;
    compare_legacy(&design, config, "explicit LSMR slopes");
}

#[test]
fn legacy_inference_matches_independent_dense_outputs_on_exact_dof_designs() {
    use fereg_core::{
        fit_prepared_categorical_ols_robust_map, fit_prepared_categorical_ols_unadjusted_map,
        prepare_categorical_ols_data, PreparedOlsRobustConfig, PreparedOlsUnadjustedConfig,
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
        let prepared = prepare_categorical_ols_data(&request, &loaded).unwrap();
        let robust =
            fit_prepared_categorical_ols_robust_map(&prepared, PreparedOlsRobustConfig::default())
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
            let iid = fit_prepared_categorical_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig::default(),
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
