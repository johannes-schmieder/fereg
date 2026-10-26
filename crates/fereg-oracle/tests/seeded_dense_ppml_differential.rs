#![deny(clippy::too_many_lines)]
//! Public prepared PPML against the independent dense Poisson oracle.
//!
//! `fereg_oracle::explicit_ppml` materializes every fixed-effect dummy and
//! solves the full Poisson score equations by damped Newton steps; it never
//! calls production IRLS, FE absorption, rank or inference code. These tests
//! replace the former in-crate dense comparators, which reused production
//! absorption and rank solvers.
#[allow(dead_code)]
mod support;

use fereg_core::ppml::{fit_prepared_ppml, CertifiedPpmlResult};
use fereg_core::{ErrorCode, SeparationConfig};
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::designs::{generate, Design, DesignSpec, Graph, WeightKind};
use fereg_oracle::explicit_ppml::{fit_explicit_ppml, PpmlProblem};

/// Compare finite coefficients, their covariance block and fitted means.
fn assert_matches_oracle(
    fit: &CertifiedPpmlResult,
    oracle_problem: &PpmlProblem,
    oracle_rows: &[usize],
    regressors: usize,
    context: &str,
) {
    let oracle = fit_explicit_ppml(oracle_problem).unwrap_or_else(|e| panic!("{context}: {e}"));
    let values = fit.coefficients().values();
    for (column, (&actual, &expected)) in values
        .iter()
        .zip(&oracle.coefficients)
        .take(regressors)
        .enumerate()
    {
        COEFFICIENT
            .check(actual, expected)
            .unwrap_or_else(|e| panic!("{context} beta[{column}]: {e}"));
    }
    let covariance = &fit.inference().vce.covariance;
    let dimension = values.len();
    assert_eq!(covariance.len(), dimension * dimension, "{context}");
    for row in 0..regressors {
        for col in 0..regressors {
            INFERENCE
                .check(
                    covariance[row * dimension + col],
                    oracle.covariance[row][col],
                )
                .unwrap_or_else(|e| panic!("{context} V[{row},{col}]: {e}"));
        }
    }
    let output = fit.output();
    assert_eq!(output.original_rows(), oracle_rows, "{context} sample");
    PREDICTION
        .check_slice(output.mean(), &oracle.mean)
        .unwrap_or_else(|e| panic!("{context} mean: {e}"));
}

fn fit_design(design: &Design, context: &str) -> CertifiedPpmlResult {
    let (request, loaded) =
        support::ppml::request(design, None, false, SeparationConfig::default());
    fit_prepared_ppml(request, loaded).unwrap_or_else(|e| panic!("{context}: {e}"))
}

#[test]
fn seeded_designs_match_dense_poisson_oracle() {
    let graphs = [Graph::DensePanel, Graph::AkmStrong, Graph::Chain];
    let weights = [
        WeightKind::Unit,
        WeightKind::Frequency,
        WeightKind::Probability,
    ];
    for seed in 0..24_u64 {
        let effects = (seed % 3) as usize;
        let spec = DesignSpec {
            rows: 96,
            levels: vec![6, 5][..effects].to_vec(),
            regressors: 2,
            graph: graphs[(seed as usize / 3) % graphs.len()],
            weights: weights[(seed as usize / 2) % weights.len()],
            clusters: usize::from(seed % 4 == 3),
            poisson: true,
            ..DesignSpec::default()
        };
        let design = generate(seed, &spec).expect("design");
        let context = format!("seed {seed} effects {effects}");
        let fit = fit_design(&design, &context);
        let (problem, rows) = design.poisson_problem(false).expect("oracle problem");
        assert_matches_oracle(&fit, &problem, &rows, design.regressors.len(), &context);
    }
}

/// Long chains and weak AKM mobility keep PPML Auto's diagonal joint
/// recurrence past its block checkpoint, so ordinary frames and the strict
/// reprojection reuse one CMG hierarchy. The dense oracle stays authoritative.
#[test]
fn weak_two_fe_graphs_reusing_cmg_match_dense_poisson_oracle() {
    for (seed, graph, levels, rows, weights) in [
        (7, Graph::Chain, vec![300, 150], 1200, WeightKind::Unit),
        (
            8,
            Graph::AkmWeak,
            vec![200, 60],
            3400,
            WeightKind::Probability,
        ),
        (9, Graph::Chain, vec![240, 120], 960, WeightKind::Frequency),
    ] {
        let spec = DesignSpec {
            rows,
            levels,
            regressors: 2,
            graph,
            weights,
            poisson: true,
            ..DesignSpec::default()
        };
        let design = generate(seed, &spec).expect("design");
        let context = format!("weak seed {seed}");
        let fit = fit_design(&design, &context);
        let (problem, rows) = design.poisson_problem(false).expect("oracle problem");
        assert_matches_oracle(&fit, &problem, &rows, design.regressors.len(), &context);
    }
}

#[test]
fn fractional_outcomes_match_dense_poisson_oracle() {
    let spec = DesignSpec {
        levels: vec![6],
        poisson: true,
        ..DesignSpec::default()
    };
    let mut design = generate(11, &spec).expect("design");
    for value in &mut design.outcome {
        *value *= 0.37;
    }
    assert!(design.outcome.iter().any(|y| y.fract() != 0.0));
    let fit = fit_design(&design, "fractional");
    let (problem, rows) = design.poisson_problem(false).expect("oracle problem");
    assert_matches_oracle(&fit, &problem, &rows, design.regressors.len(), "fractional");
}

#[test]
fn exposure_enters_once_as_a_log_offset() {
    let spec = DesignSpec {
        levels: vec![5],
        poisson: true,
        ..DesignSpec::default()
    };
    let design = generate(5, &spec).expect("design");
    let exposure: Vec<f64> = (0..design.outcome.len())
        .map(|row| 0.5 + (row % 7) as f64 * 0.25)
        .collect();
    let (request, loaded) =
        support::ppml::request(&design, Some(&exposure), false, SeparationConfig::default());
    let fit = fit_prepared_ppml(request, loaded).expect("exposure fit");
    let (mut problem, rows) = design.poisson_problem(false).expect("oracle problem");
    problem.offset = rows.iter().map(|&row| exposure[row].ln()).collect();
    assert_matches_oracle(&fit, &problem, &rows, design.regressors.len(), "exposure");
}

#[test]
fn negative_and_all_zero_outcomes_fail_closed() {
    let spec = DesignSpec {
        levels: vec![4],
        poisson: true,
        ..DesignSpec::default()
    };
    let mut negative = generate(3, &spec).expect("design");
    negative.outcome[7] = -1.0;
    let (request_negative, loaded_negative) =
        support::ppml::request(&negative, None, false, SeparationConfig::default());
    let error = fit_prepared_ppml(request_negative, loaded_negative).expect_err("negative");
    assert_eq!(error.code, ErrorCode::InvalidData, "{error}");

    let mut zero = generate(3, &spec).expect("design");
    zero.outcome.iter_mut().for_each(|value| *value = 0.0);
    let (request_zero, loaded_zero) =
        support::ppml::request(&zero, None, false, SeparationConfig::default());
    let error = fit_prepared_ppml(request_zero, loaded_zero).expect_err("all zero");
    assert_eq!(error.code, ErrorCode::InsufficientObservations, "{error}");
}
