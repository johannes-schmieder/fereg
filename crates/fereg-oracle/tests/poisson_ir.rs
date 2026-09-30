//! Known full-model rays and exact paired-row impossibility proofs; no detector reuse.
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::{DenseMatrix, PreparedWeights, WeightKind},
    ppml::separation::*,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
};
use fereg_oracle::explicit_ols::{fit_explicit_ols, ExplicitOlsProblem, OracleWeights};
#[path = "support/engine.rs"]
mod inputs;
#[path = "support/ir_cases.rs"]
mod ir_cases;
use ir_cases::case;
fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        for separated in [false, true] {
            let c = case(seed, separated);
            let n = c.outcome.len();
            let mut sample = inputs::prepare(&c.effects, &c.outcome);
            sample.weights = PreparedWeights::Provided {
                kind: if seed % 2 == 0 {
                    WeightKind::Frequency
                } else {
                    WeightKind::Probability
                },
                values: (0..n).map(|i| (1 + i % 5) as f64).collect(),
            };
            let ledger = Ledger::new(
                sample,
                Policy {
                    drop_singletons: false,
                    methods: Methods::only(Method::IteratedRectifier),
                },
            )
            .unwrap();
            let raw = if c.columns.is_empty() {
                DenseMatrix::zeros(n, 0).unwrap()
            } else {
                DenseMatrix::from_columns(&c.columns).unwrap()
            };
            let roles = vec![Role::Ordinary; c.columns.len()];
            let mut solvers = vec![Solver::Auto];
            if c.effects.len() >= 2 && c.effects.iter().all(|e| e.slopes().is_empty()) {
                solvers.extend([Solver::Lsmr, Solver::Cmg]);
            }
            for solver in solvers {
                let model = Model {
                    probe: Default::default(),
                    raw: &raw,
                    roles: &roles,
                    controls: Controls::default(),
                    options: Options {
                        solver,
                        ..Options::for_estimator(Estimator::Ppml)
                    },
                };
                let report = ledger
                    .view()
                    .iterated_rectifier(&model, 100, &one)
                    .unwrap_or_else(|e| {
                        panic!("seed={seed}, separated={separated}, solver={solver:?}: {e}")
                    });
                assert_eq!(
                    report.work.projection().calls(),
                    report.iterations + report.span_frames
                );
                if separated {
                    assert_eq!(
                        report.status,
                        Status::Certified,
                        "seed={seed}, solver={solver:?}, fixed_point={:?}, iterations={}",
                        report.fixed_point,
                        report.iterations
                    );
                    let p = report.direction.as_ref().unwrap();
                    let expected: Vec<_> = c
                        .outcome
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &y)| (y == 0.).then_some(i))
                        .collect();
                    assert_eq!(p.rows, expected, "seed={seed}, solver={solver:?}");
                    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                        outcome: p.direction.clone(),
                        regressors: c.columns.clone(),
                        effects: c.effects.clone(),
                        weights: OracleWeights::Unit,
                        clusters: vec![],
                    })
                    .unwrap();
                    let distance = oracle.residuals.iter().fold(0_f64, |n, &v| n.hypot(v))
                        / p.direction.iter().fold(0_f64, |n, &v| n.hypot(v));
                    assert!(
                        distance <= model.controls.projection_tolerance,
                        "seed={seed}, solver={solver:?}, distance={distance:e}"
                    );
                    ledger
                        .view()
                        .certify_direction(p, &model, &one)
                        .unwrap_or_else(|e| panic!("span seed={seed}, solver={solver:?}: {e}"));
                } else {
                    assert_eq!(
                        report.status,
                        Status::NoSeparation,
                        "seed={seed}, solver={solver:?}"
                    );
                    for row in (0..n).step_by(2) {
                        for column in &c.columns {
                            assert_eq!(column[row], column[row + 1]);
                        }
                        for effect in &c.effects {
                            assert_eq!(effect.codes()[row], effect.codes()[row + 1]);
                            for slope in effect.slopes() {
                                assert_eq!(slope[row], slope[row + 1]);
                            }
                        }
                    }
                    let replay = ledger.view().replay_ir(&report, &model, 100, &one).unwrap();
                    assert_eq!(replay.work(), report.work);
                }
                if seed % 37 == 0 {
                    assert_eq!(
                        report,
                        ledger
                            .view()
                            .iterated_rectifier(&model, 100, &eight)
                            .unwrap()
                    );
                }
            }
        }
    }
}
#[test]
fn comprehensive_ir_matches_full_model_rays_and_paired_row_impossibility_proofs() {
    campaign(0, 200);
}
#[test]
#[ignore = "long IR ray and no-separation qualification"]
fn two_thousand_ir_ray_and_paired_row_designs() {
    campaign(200, 2000);
}
