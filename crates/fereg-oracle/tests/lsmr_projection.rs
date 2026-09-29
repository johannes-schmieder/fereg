#![deny(clippy::too_many_lines)]
#[path = "support/outputs.rs"]
mod output_checks;
use fereg_engine::{
    data::*,
    fe::{Backend, Design, Engine, StopReason, Weights},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph, WeightKind},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod engine_inputs;
use engine_inputs::prepare;
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: [120, 191][seed % 2],
                levels: if seed % 3 == 0 {
                    vec![8, 5, 3]
                } else {
                    vec![9, 7]
                },
                graph: [
                    Graph::DensePanel,
                    Graph::AkmStrong,
                    Graph::AkmWeak,
                    Graph::Chain,
                    Graph::Barbell,
                    Graph::Disconnected,
                ][seed % 6],
                slopes: seed % 4 == 0,
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
        let mut effects = generated.effects.clone();
        if seed % 4 == 0 {
            let effect = &effects[0];
            let slope = effect.slopes()[0].clone();
            effects[0] = DummyEffect::new(
                effect.codes().to_vec(),
                effect.n_levels(),
                seed % 8 == 0,
                vec![
                    slope.clone(),
                    slope.iter().map(|v| 2. * v).collect(),
                    vec![0.; slope.len()],
                ],
            )
            .unwrap();
        }
        if seed % 7 == 0 {
            effects.push(effects[0].clone());
        }
        let sample = prepare(&effects, &generated.outcome);
        let design = Design::new(&sample, Weights::Provided(&generated.weights)).unwrap();
        let options = Options {
            solver: Solver::Lsmr,
            tolerance: 1e-10,
            iterations: 1000,
            ..Options::for_estimator(Estimator::Ols)
        };
        let columns = vec![
            generated.outcome.clone(),
            generated.regressors[0].clone(),
            vec![0.; sample.n_active()],
        ];
        let rhs = DenseMatrix::from_columns(&columns).unwrap();
        let result = Engine::new(&design, Estimator::Ols, &options, &one)
            .unwrap()
            .project(&rhs, &one)
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert!(result
            .diagnostics
            .iter()
            .all(|d| d.stop == StopReason::Converged
                && d.used == Backend::Lsmr
                && d.attempts.len() == 1
                && d.iterations <= options.iterations));
        assert_eq!(result.diagnostics[2].iterations, 0);
        if seed % 10 == 0 {
            assert_eq!(
                result,
                Engine::new(&design, Estimator::Ols, &options, &eight)
                    .unwrap()
                    .project(&rhs, &eight)
                    .unwrap(),
                "seed {seed}: worker identity"
            );
        }
        let residuals = &result.residuals;
        for (column, outcome) in columns.into_iter().take(2).enumerate() {
            let expected = fit_explicit_ols(&ExplicitOlsProblem {
                outcome,
                regressors: vec![],
                effects: effects.clone(),
                weights: OracleWeights::Wls(generated.weights.clone()),
                clusters: vec![],
            })
            .unwrap_or_else(|e| panic!("oracle seed {seed}: {e}"));
            if column == 0 {
                output_checks::check(
                    &design,
                    &result.coefficients,
                    residuals,
                    &expected.residuals,
                    &one,
                );
            }
            for (row, &expected) in expected.residuals.iter().enumerate() {
                let actual = residuals[(row, column)];
                assert!(
                    (actual - expected).abs() < 2e-9 * expected.abs().max(1.),
                    "seed {seed}, row {row}, RHS {column}: {actual} != {expected}"
                );
            }
        }
    }
}
#[test]
fn lsmr_projection_matches_independent_dense_dummy_qr() {
    campaign(0, 200);
}
#[test]
#[ignore = "long retained LSMR weighted-span campaign"]
fn two_thousand_lsmr_projection_designs() {
    campaign(200, 2000);
}
