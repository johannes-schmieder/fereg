#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, Engine, Weights},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::{self, Policy, Role},
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph, WeightKind},
    explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod engine_inputs;
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a:.17e} != {b:.17e}"
    );
}
fn campaign(start: usize, count: usize) {
    for solver in [Solver::Auto, Solver::Lsmr] {
        campaign_solver(start, count, solver);
    }
}
fn campaign_solver(start: usize, count: usize, solver: Solver) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: 97 + seed % 37,
                levels: vec![6, 5, 3][..seed % 4].to_vec(),
                regressors: (seed / 4) % 4,
                graph: if seed % 4 >= 2 && seed % 5 == 0 {
                    Graph::Disconnected
                } else {
                    Graph::DensePanel
                },
                slopes: seed % 3 == 0,
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
        let sample = engine_inputs::prepare(&generated.effects, &generated.outcome);
        let n = sample.n_active();
        let raw = if generated.regressors.is_empty() {
            DenseMatrix::zeros(n, 0).unwrap()
        } else {
            DenseMatrix::from_columns(&generated.regressors).unwrap()
        };
        let mut basis = None;
        // Changes represent distinct IRLS/Hessian frames over the same immutable sample.
        for frame in 0..2 {
            let weights: Vec<_> = generated
                .weights
                .iter()
                .enumerate()
                .map(|(i, w)| w * (1. + frame as f64 * (i % 7) as f64 * 0.17))
                .collect();
            let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
            let options = Options {
                solver,
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ols)
            };
            let mut engine = Engine::new(&design, Estimator::Ols, &options, &one).unwrap();
            let roles = vec![Role::Ordinary; raw.ncols()];
            let actual = wls::solve_absorbed_with_basis(
                &mut engine,
                &raw,
                &generated.outcome,
                &roles,
                Policy::default(),
                basis.as_ref(),
                &one,
            )
            .unwrap_or_else(|e| panic!("seed {seed}, frame {frame}: {e}"));
            if let Some(basis) = &basis {
                assert!(basis.matches(&actual.finite));
                let fresh = wls::solve_absorbed(
                    &mut engine,
                    &raw,
                    &generated.outcome,
                    &roles,
                    Policy::default(),
                    &one,
                )
                .unwrap();
                assert!(
                    basis.matches(&fresh.finite),
                    "canonical basis changed at seed {seed}"
                );
            }
            let expected = fit_explicit_ols(&ExplicitOlsProblem {
                outcome: generated.outcome.clone(),
                regressors: generated.regressors.clone(),
                effects: generated.effects.clone(),
                weights: OracleWeights::Wls(weights.clone()),
                clusters: vec![],
            })
            .unwrap_or_else(|e| panic!("oracle seed {seed}, frame {frame}: {e}"));
            for (column, beta) in actual.finite.coefficients.iter().enumerate() {
                close(*beta, expected.coefficients[column].unwrap_or(0.), seed);
            }
            for (&a, &b) in actual.residuals.iter().zip(&expected.residuals) {
                close(a, b, seed);
            }
            for (i, &left) in expected.kept_regressors.iter().enumerate() {
                for (j, &right) in expected.kept_regressors.iter().enumerate() {
                    close(
                        actual.finite.bread[(left, right)],
                        expected.bread[(i, j)],
                        seed,
                    );
                }
            }
            assert!(actual.relative_original_finite_score <= 5e-10);
            assert_eq!(actual.projection_diagnostics.len(), raw.ncols() + 1);
            assert!(actual.recovery_diagnostics.len() <= 1);
            if seed % 19 == 0 {
                let mut engine = Engine::new(&design, Estimator::Ols, &options, &eight).unwrap();
                let parallel = wls::solve_absorbed_with_basis(
                    &mut engine,
                    &raw,
                    &generated.outcome,
                    &roles,
                    Policy::default(),
                    basis.as_ref(),
                    &eight,
                )
                .unwrap();
                assert_eq!(actual.finite, parallel.finite);
                assert_eq!(actual.residuals, parallel.residuals);
                assert_eq!(actual.absorbed_coefficients, parallel.absorbed_coefficients);
                assert_eq!(
                    actual.projection_diagnostics,
                    parallel.projection_diagnostics
                );
                assert_eq!(actual.recovery_diagnostics, parallel.recovery_diagnostics);
            }
            if basis.is_none() {
                basis = Some(wls::FrozenBasis::from_fit(&actual.finite).unwrap());
            }
        }
    }
}
#[test]
fn shared_absorbed_frames_match_independent_dense_dummy_wls() {
    campaign(0, 200);
}
#[test]
#[ignore = "long shared absorbed WLS frame campaign"]
fn two_thousand_absorbed_designs_with_changing_weights() {
    campaign(200, 2000);
}
