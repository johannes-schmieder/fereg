#![deny(clippy::too_many_lines)]
use fereg_engine::{data::DenseMatrix, threads::ThreadTeam, wls::*};
use fereg_oracle::{
    designs::{generate, DesignSpec, WeightKind},
    explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights},
};
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: [31, 97, 211][seed % 3],
                levels: vec![],
                regressors: 1 + seed % 7,
                collinear: seed % 5 == 0,
                weights: [
                    WeightKind::Unit,
                    WeightKind::Analytic,
                    WeightKind::Frequency,
                    WeightKind::Probability,
                ][seed % 4],
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let matrix = DenseMatrix::from_columns(&generated.regressors).unwrap();
        let slots: Vec<_> = (0..matrix.ncols())
            .map(|column| Slot {
                role: Role::Ordinary,
                reference_norm: (0..matrix.nrows()).fold(0_f64, |norm, row| {
                    norm.hypot(generated.weights[row].sqrt() * matrix[(row, column)])
                }),
            })
            .collect();
        let problem = Problem {
            design: &matrix,
            outcome: &generated.outcome,
            weights: &generated.weights,
            slots: &slots,
            has_absorbed_intercept: false,
        };
        let actual =
            solve(&problem, Policy::default(), &one).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        if seed % 10 == 0 {
            assert_eq!(
                actual,
                solve(&problem, Policy::default(), &eight).unwrap(),
                "seed {seed}: workers"
            );
        }
        let expected = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: generated.outcome.clone(),
            regressors: generated.regressors.clone(),
            effects: vec![],
            weights: OracleWeights::Wls(generated.weights.clone()),
            clusters: vec![],
        })
        .unwrap();
        assert_eq!(
            actual.kept.len(),
            expected.finite_regressor_rank,
            "seed {seed}: rank"
        );
        for row in 0..matrix.nrows() {
            assert!(
                (actual.fitted[row] - expected.fitted_values[row]).abs() < 2e-9,
                "seed {seed}, row {row}: fit"
            );
            assert!(
                (actual.residuals[row] - expected.residuals[row]).abs() < 2e-9,
                "seed {seed}, row {row}: residual"
            );
        }
        if actual.kept.len() == matrix.ncols() {
            for column in 0..matrix.ncols() {
                assert!(
                    (actual.coefficients[column] - expected.coefficients[column].unwrap()).abs()
                        < 2e-9,
                    "seed {seed}: coefficient {column}"
                );
                for right in 0..matrix.ncols() {
                    assert!(
                        (actual.bread[(column, right)] - expected.bread[(column, right)]).abs()
                            < 2e-9,
                        "seed {seed}: bread {column}/{right}"
                    );
                }
            }
        }
        // Nonidentified coefficient coordinates can differ; the fitted-value
        // information variance is invariant to the chosen retained basis.
        for row in 0..matrix.nrows().min(5) {
            let mut observed = 0.;
            let mut reference = 0.;
            for left in 0..matrix.ncols() {
                for right in 0..matrix.ncols() {
                    observed +=
                        matrix[(row, left)] * actual.bread[(left, right)] * matrix[(row, right)];
                }
            }
            for (left, &left_column) in expected.kept_regressors.iter().enumerate() {
                for (right, &right_column) in expected.kept_regressors.iter().enumerate() {
                    reference += matrix[(row, left_column)]
                        * expected.bread[(left, right)]
                        * matrix[(row, right_column)];
                }
            }
            assert!(
                (observed - reference).abs() < 2e-9,
                "seed {seed}: fitted-value bread"
            );
        }
    }
}
#[test]
fn finite_wls_matches_independent_dense_qr() {
    campaign(0, 200);
}
#[test]
#[ignore = "long finite WLS differential campaign"]
fn two_thousand_finite_wls_designs() {
    campaign(200, 2000);
}
