#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    ppml::irls::{fit_final_sample, Problem},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::Role,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph},
    explicit_ols::DummyEffect,
    explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlWeights},
};
#[path = "support/engine.rs"]
mod engine_inputs;
fn explicit_columns(effects: &[DummyEffect], n: usize) -> Vec<Vec<f64>> {
    let mut columns = Vec::new();
    for effect in effects {
        for level in 0..effect.n_levels() {
            for basis in effect
                .has_intercept()
                .then(|| vec![1.; n])
                .into_iter()
                .chain(effect.slopes().iter().cloned())
            {
                columns.push(
                    (0..n)
                        .map(|i| {
                            if effect.codes()[i] as usize == level {
                                basis[i]
                            } else {
                                0.
                            }
                        })
                        .collect(),
                );
            }
        }
    }
    columns
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a:.17e} != {b:.17e}"
    );
}
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let n = 97 + seed % 37;
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: n,
                levels: vec![3, 2, 4][..seed % 4].to_vec(),
                regressors: 1 + seed % 3,
                slopes: seed % 3 == 0,
                graph: if seed % 4 >= 2 && seed % 5 == 0 {
                    Graph::Disconnected
                } else {
                    Graph::DensePanel
                },
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let y: Vec<_> = (0..n).map(|i| ((i * 11 + seed) % 19) as f64 / 2.).collect();
        let offset: Vec<_> = (0..n)
            .map(|i| ((i + seed) % 7) as f64 * 0.1 - 0.3)
            .collect();
        let values: Vec<_> = (0..n).map(|i| (1 + (i + seed) % 5) as f64).collect();
        let (raw_weights, oracle_weights) = match seed % 3 {
            0 => (PreparedWeights::None, PpmlWeights::Unit),
            1 => (
                PreparedWeights::Provided {
                    kind: WeightKind::Frequency,
                    values: values.clone(),
                },
                PpmlWeights::Frequency(values.iter().map(|v| *v as u32).collect()),
            ),
            _ => (
                PreparedWeights::Provided {
                    kind: WeightKind::Probability,
                    values: values.clone(),
                },
                PpmlWeights::Probability(values),
            ),
        };
        let mut sample = engine_inputs::prepare(&generated.effects, &y);
        sample.weights = raw_weights;
        sample.adjustment = PreparedAdjustment::Offset(offset.clone());
        let q = ReferenceWeights::new(&sample.weights, n, Estimator::Ppml).unwrap();
        let mut columns = if generated.effects.is_empty() {
            vec![vec![1.; n]]
        } else {
            vec![]
        };
        columns.extend(generated.regressors);
        let raw = DenseMatrix::from_columns(&columns).unwrap();
        columns.extend(explicit_columns(&generated.effects, n));
        let oracle = fit_explicit_ppml(&PpmlProblem {
            design: (0..n)
                .map(|row| columns.iter().map(|c| c[row]).collect())
                .collect(),
            outcome: y,
            offset,
            weights: oracle_weights,
            clusters: vec![],
        })
        .unwrap_or_else(|e| panic!("oracle seed {seed}: {e}"));
        let roles = vec![Role::Ordinary; raw.ncols()];
        let problem = Problem {
            sample: &sample,
            raw: &raw,
            roles: &roles,
        };
        let options = Options::for_estimator(Estimator::Ppml);
        let fit = fit_final_sample(&problem, options, &one)
            .unwrap_or_else(|e| panic!("engine seed {seed}: {e}"));
        close(fit.evaluation.objective.deviance, oracle.deviance, seed);
        for row in 0..n {
            close(fit.point.eta[row], oracle.eta[row], seed);
            close(fit.evaluation.mean[row], oracle.mean[row], seed);
            assert_eq!(
                fit.hessian_weights[row],
                q.value(row) * fit.evaluation.mean[row]
            );
            assert_eq!(fit.evaluation.mean[row], fit.point.eta[row].exp());
        }
        for col in 0..raw.ncols() {
            close(fit.point.finite[col], oracle.coefficients[col], seed);
            for other in 0..raw.ncols() {
                close(
                    fit.linear.finite.bread[(col, other)],
                    oracle.bread[col][other],
                    seed,
                );
            }
        }
        assert!(fit.scores.passed);
        assert!(fit.diagnostics.accepted_iterations >= 2);
        assert!(fit.diagnostics.strict_checks >= 2);
        if seed % 19 == 0 {
            assert_eq!(fit, fit_final_sample(&problem, options, &eight).unwrap());
        }
    }
}
#[test]
fn cold_irls_matches_independent_explicit_dummy_newton() {
    campaign(0, 200);
}
#[test]
#[ignore = "long cold PPML IRLS campaign"]
fn two_thousand_cold_irls_fits() {
    campaign(200, 2000);
}
