#![deny(clippy::too_many_lines)]
use fereg_oracle::compare::{COEFFICIENT, INFERENCE, PREDICTION};
use fereg_oracle::explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlWeights};

fn intercept() -> PpmlProblem {
    PpmlProblem {
        design: vec![vec![1.0]; 4],
        outcome: vec![1.0, 3.0, 2.0, 6.0],
        offset: vec![],
        weights: PpmlWeights::Unit,
        clusters: vec![],
    }
}

#[test]
fn intercept_matches_hand_computed_mean_hessian_and_robust_variance() {
    let fit = fit_explicit_ppml(&intercept()).unwrap();
    COEFFICIENT
        .check(fit.coefficients[0], 3.0_f64.ln())
        .unwrap();
    PREDICTION.check_slice(&fit.mean, &[3.0; 4]).unwrap();
    INFERENCE.check(fit.hessian[0][0], 12.0).unwrap();
    INFERENCE.check(fit.covariance[0][0], 7.0 / 54.0).unwrap();
    assert!(fit.score[0].abs() < 1e-8);
}

#[test]
fn frequency_weights_match_literal_replication_including_covariance() {
    let mut weighted = intercept();
    let frequencies = vec![1, 3, 2, 4];
    weighted.weights = PpmlWeights::Frequency(frequencies.clone());
    weighted.design = vec![
        vec![1.0, -1.0],
        vec![1.0, 0.4],
        vec![1.0, 0.2],
        vec![1.0, 1.5],
    ];
    let rows = frequencies
        .iter()
        .enumerate()
        .flat_map(|(i, &f)| std::iter::repeat_n(i, f as usize))
        .collect::<Vec<_>>();
    let mut expanded = weighted.clone();
    expanded.design = rows.iter().map(|&i| weighted.design[i].clone()).collect();
    expanded.outcome = rows.iter().map(|&i| weighted.outcome[i]).collect();
    expanded.weights = PpmlWeights::Unit;
    for clustered in [false, true] {
        if clustered {
            weighted.clusters = vec![vec![0, 0, 1, 1]];
            expanded.clusters = vec![rows.iter().map(|&i| weighted.clusters[0][i]).collect()];
        }
        let a = fit_explicit_ppml(&weighted).unwrap();
        let b = fit_explicit_ppml(&expanded).unwrap();
        COEFFICIENT
            .check_slice(&a.coefficients, &b.coefficients)
            .unwrap();
        for (a, b) in a.covariance.iter().zip(&b.covariance) {
            INFERENCE.check_slice(a, b).unwrap();
        }
        INFERENCE.check(a.deviance, b.deviance).unwrap();
    }
}

#[test]
fn probability_weights_are_invariant_to_common_scale() {
    let mut problem = intercept();
    let mut previous = None;
    for scale in [1.0, 1e-200, 1e200] {
        problem.weights =
            PpmlWeights::Probability(vec![scale, 2.0 * scale, 0.5 * scale, 3.0 * scale]);
        let fit = fit_explicit_ppml(&problem).unwrap();
        if let Some((b, v)) = previous {
            COEFFICIENT.check(fit.coefficients[0], b).unwrap();
            INFERENCE.check(fit.covariance[0][0], v).unwrap();
        }
        previous = Some((fit.coefficients[0], fit.covariance[0][0]));
    }
}

#[test]
fn explicit_dummies_slopes_offsets_and_omissions_recover_known_fractional_means() {
    let design = (0..48)
        .map(|i| {
            let x = ((i * 17) % 29) as f64 / 29.0;
            vec![
                1.0,
                x,
                f64::from(i % 3 == 0),
                f64::from(i % 3 == 1),
                f64::from(i % 2 == 0),
                f64::from(i % 3 == 0) * x,
                2.0 * x,
            ]
        })
        .collect::<Vec<_>>();
    let truth = [0.2, 0.4, -0.3, 0.1, 0.2, -0.15, 0.0];
    let offset = (0..48)
        .map(|i| (1.0 + (i % 5) as f64).ln())
        .collect::<Vec<_>>();
    let mean = design
        .iter()
        .zip(&offset)
        .map(|(row, offset)| {
            (row.iter().zip(truth).map(|(x, b)| x * b).sum::<f64>() + offset).exp()
        })
        .collect::<Vec<_>>();
    let problem = PpmlProblem {
        design,
        outcome: mean.clone(),
        offset,
        weights: PpmlWeights::Unit,
        clusters: vec![],
    };
    let fit = fit_explicit_ppml(&problem).unwrap();
    assert_eq!(fit.omitted_columns, vec![6]);
    COEFFICIENT.check_slice(&fit.coefficients, &truth).unwrap();
    PREDICTION.check_slice(&fit.mean, &mean).unwrap();
    assert!(fit.deviance.abs() < 1e-9);
}

#[test]
fn multiway_raw_covariance_matches_hand_inclusion_exclusion() {
    let mut problem = intercept();
    problem.clusters = vec![vec![0, 0, 1, 1], vec![0, 1, 0, 1]];
    let fit = fit_explicit_ppml(&problem).unwrap();
    // Cluster score squares: first 8, second 18, intersection 14. Bread=1/12.
    INFERENCE
        .check(fit.covariance[0][0], (8.0 + 18.0 - 14.0) * 2.0 / 144.0)
        .unwrap();
}

#[test]
fn zero_outcomes_are_allowed_but_invalid_and_unidentified_inputs_fail() {
    let mut problem = intercept();
    problem.outcome[0] = 0.0;
    assert!(fit_explicit_ppml(&problem).is_ok());
    for outcome in [vec![0.0; 4], vec![-1.0, 3.0, 2.0, 6.0], vec![f64::NAN; 4]] {
        problem.outcome = outcome;
        assert!(fit_explicit_ppml(&problem).is_err());
    }
    problem = intercept();
    problem.clusters = vec![vec![0; 4]];
    assert!(fit_explicit_ppml(&problem).is_err());
    problem.clusters.clear();
    problem.offset = vec![0.0];
    assert!(fit_explicit_ppml(&problem).is_err());
}
