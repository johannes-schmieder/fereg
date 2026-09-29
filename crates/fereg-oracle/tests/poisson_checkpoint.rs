#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*,
    fe::{Design, Engine, Weights},
    ppml::{
        checkpoint::{Checkpoint, Decision, Stage},
        family::Family,
        step::Point,
    },
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{self, FrozenBasis, Policy, Role},
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph},
    explicit_ols::DummyEffect,
    explicit_ppml::{fit_explicit_ppml, PpmlProblem, PpmlResult, PpmlWeights},
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
fn oracle_point(
    fe: &Design<'_>,
    raw: &DenseMatrix,
    effects: &[DummyEffect],
    fit: &PpmlResult,
) -> Point {
    let mut alpha = DenseMatrix::zeros(fe.layout().total, 1).unwrap();
    let mut base = raw.ncols();
    for (index, effect) in effects.iter().enumerate() {
        let layout = &fe.layout().effects[index];
        for level in 0..layout.levels {
            let representative = fe
                .topology()
                .effect_partition(index)
                .unwrap()
                .membership()
                .members(level)
                .unwrap()[0];
            let original = effect.codes()[representative] as usize;
            for channel in 0..layout.channels {
                alpha[(layout.offset + channel * layout.levels + level, 0)] =
                    fit.coefficients[base + original * layout.channels + channel];
            }
        }
        base += effect.n_levels() * layout.channels;
    }
    Point {
        finite: fit.coefficients[..raw.ncols()].to_vec(),
        absorbed: alpha,
        eta: fit.eta.clone(),
    }
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
        let y: Vec<_> = (0..n)
            .map(|i| (1 + (i * 11 + seed) % 19) as f64 / 2.)
            .collect();
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
        let sample = engine_inputs::prepare(&generated.effects, &y);
        let q = ReferenceWeights::new(&raw_weights, n, Estimator::Ppml).unwrap();
        let fe = Design::new(&sample, Weights::Reference(q)).unwrap();
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
            outcome: y.clone(),
            offset: offset.clone(),
            weights: oracle_weights,
            clusters: vec![],
        })
        .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let accepted = oracle_point(&fe, &raw, &generated.effects, &oracle);
        let roles = vec![Role::Ordinary; raw.ncols()];
        let options = Options {
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ppml)
        };
        let mut engine = Engine::new(&fe, Estimator::Ppml, &options, &one).unwrap();
        let linear =
            wls::solve_absorbed(&mut engine, &raw, &y, &roles, Policy::default(), &one).unwrap();
        let basis = FrozenBasis::from_fit(&linear.finite).unwrap();
        let family = Family::new(&y, q, Some(&offset)).unwrap();
        let evaluation = family.evaluate(&accepted.eta, &one).unwrap();
        let checkpoint = Checkpoint {
            family: &family,
            design: &fe,
            raw: &raw,
            roles: &roles,
            basis: &basis,
            options: Options::for_estimator(Estimator::Ppml),
        };
        for stage in [Stage::Fisher, Stage::FinalHessian] {
            let report = checkpoint
                .run(&accepted, &evaluation, stage, &one)
                .unwrap_or_else(|e| panic!("seed {seed}, stage {stage:?}: {e}"));
            assert_eq!(
                report.decision,
                Decision::Stable,
                "seed {seed}, stage {stage:?}: {:?}",
                report.candidate.as_ref().map(|c| c.movement)
            );
            let candidate = report.candidate.as_ref().unwrap();
            for row in 0..n {
                close(candidate.point.eta[row], oracle.eta[row], seed);
                assert_eq!(
                    candidate.hessian_weights[row],
                    q.value(row) * oracle.eta[row].exp()
                );
            }
            for col in 0..raw.ncols() {
                close(candidate.point.finite[col], oracle.coefficients[col], seed);
                for other in 0..raw.ncols() {
                    close(
                        candidate.linear.finite.bread[(col, other)],
                        oracle.bread[col][other],
                        seed,
                    );
                }
            }
            if seed % 19 == 0 {
                assert_eq!(
                    report,
                    checkpoint
                        .run(&accepted, &evaluation, stage, &eight)
                        .unwrap()
                );
            }
        }
    }
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() < 2e-9 * a.abs().max(b.abs()).max(1.),
        "seed {seed}: {a} != {b}"
    );
}
#[test]
fn strict_checkpoints_match_independent_dummy_newton_states() {
    campaign(0, 200);
}
#[test]
#[ignore = "long PPML strict-checkpoint campaign"]
fn two_thousand_strict_checkpoint_states() {
    campaign(200, 2000);
}
