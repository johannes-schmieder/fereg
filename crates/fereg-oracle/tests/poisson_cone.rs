use fereg_engine::{
    data::{DenseMatrix, DropReason, PreparedWeights, WeightKind as EngineWeight},
    ppml::separation::*,
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::Role,
    ErrorCode,
};
use fereg_oracle::{
    designs::{generate, DesignSpec, Graph},
    explicit_ols::{fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights},
};
#[path = "support/engine.rs"]
mod inputs;

fn campaign(start: usize, count: usize) {
    let one = ThreadTeam::new(1).unwrap();
    let eight = ThreadTeam::new(8).unwrap();
    for seed in start..start + count {
        let n = 97 + seed % 37;
        let generated = generate(
            seed as u64,
            &DesignSpec {
                rows: n,
                levels: vec![6, 5, 3][..seed % 4].to_vec(),
                regressors: (seed / 4) % 3,
                graph: if seed % 5 == 0 && seed % 4 >= 2 {
                    Graph::Disconnected
                } else {
                    Graph::DensePanel
                },
                slopes: seed % 3 == 0,
                ..DesignSpec::default()
            },
        )
        .unwrap();
        let y: Vec<_> = (0..n)
            .map(|row| {
                if row % 7 == seed % 7 {
                    0.
                } else {
                    1. + (row % 5) as f64 * 0.25
                }
            })
            .collect();
        let direction: Vec<_> = y
            .iter()
            .enumerate()
            .map(|(row, &y)| {
                if y == 0. {
                    -1. - (row % 3) as f64 * 0.125
                } else {
                    0.
                }
            })
            .collect();
        // A known recession ray requires both the finite and absorbed channels.
        let mut finite = direction.clone();
        for (index, effect) in generated.effects.iter().enumerate() {
            for (row, value) in finite.iter_mut().enumerate() {
                let coefficient = (index + 1 + effect.codes()[row] as usize) as f64 * 0.0625;
                if effect.has_intercept() {
                    *value -= coefficient;
                }
                for slope in effect.slopes() {
                    *value -= coefficient * slope[row];
                }
            }
        }
        let mut columns = generated.regressors.clone();
        columns.push(finite);
        let raw = DenseMatrix::from_columns(&columns).unwrap();
        let roles = vec![Role::Ordinary; columns.len()];
        let mut sample = inputs::prepare(&generated.effects, &y);
        // The cone depends on the model span, never on the prior-weight norm.
        sample.weights = PreparedWeights::Provided {
            kind: if seed % 2 == 0 {
                EngineWeight::Frequency
            } else {
                EngineWeight::Probability
            },
            values: (0..n).map(|row| 1. + (row % 4) as f64).collect(),
        };
        let ledger = Ledger::new(
            sample,
            Policy {
                drop_singletons: false,
                methods: Methods::default(),
            },
        )
        .unwrap();
        let model = Model {
            raw: &raw,
            roles: &roles,
            options: Options::for_estimator(Estimator::Ppml),
            controls: Controls::default(),
        };
        for valid in [true, false] {
            let mut trial = direction.clone();
            if !valid {
                *trial.iter_mut().find(|v| **v < 0.).unwrap() += 0.25;
            }
            let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                outcome: trial.clone(),
                regressors: columns.clone(),
                effects: generated.effects.clone(),
                weights: OracleWeights::Unit,
                clusters: vec![],
            })
            .unwrap_or_else(|e| panic!("oracle seed={seed}, valid={valid}: {e}"));
            let distance = oracle.residuals.iter().fold(0_f64, |a, &v| a.hypot(v))
                / trial.iter().fold(0_f64, |a, &v| a.hypot(v));
            let p = ledger
                .view()
                .direction_proposal(Method::Simplex, 1, trial, Controls::default())
                .unwrap();
            let result = ledger.view().certify_direction(&p, &model, &one);
            if valid {
                assert!(distance < 1e-10, "seed={seed}, distance={distance:e}");
                if seed == 27 {
                    assert!(
                        raw_span_distance(ledger.view(), &p, &model, &one)
                            > model.controls.projection_tolerance
                    );
                }
                let certificate = result.unwrap_or_else(|e| panic!("seed={seed}: {e}"));
                assert!((certificate.span_residual_relative - distance).abs() < 1e-10);
                assert_eq!(
                    p.rows,
                    y.iter()
                        .enumerate()
                        .filter_map(|(row, &v)| (v == 0.).then_some(row))
                        .collect::<Vec<_>>()
                );
                if seed % 19 == 0 {
                    assert_eq!(
                        certificate,
                        ledger.view().certify_direction(&p, &model, &eight).unwrap()
                    );
                }
            } else {
                assert!(distance > 1e-4, "seed={seed}, distance={distance:e}");
                assert_eq!(
                    result.unwrap_err().code,
                    ErrorCode::NonConvergence,
                    "seed={seed}"
                );
            }
        }
    }
}

fn raw_span_distance(
    view: View<'_>,
    proposal: &DirectionProposal,
    model: &Model<'_>,
    team: &ThreadTeam,
) -> f64 {
    use fereg_engine::{
        fe::{Design, Plan, SchurSession, Weights},
        wls,
    };
    let normalized: Vec<_> = proposal
        .direction
        .iter()
        .map(|v| v / proposal.certificate.direction_scale)
        .collect();
    let design = Design::new(view.sample(), Weights::Unit).unwrap();
    let options = Options {
        tolerance: model.controls.projection_tolerance,
        ..model.options
    };
    // Preserve the original unrefined recurrence as an executable witness:
    // local FE certification alone did not establish full-model distance.
    let mut columns = vec![normalized.clone()];
    for column in 0..model.raw.ncols() {
        columns.push(model.raw.column(column).unwrap());
    }
    let rhs = DenseMatrix::from_columns(&columns).unwrap();
    let plan = Plan::new(&design, Estimator::Ppml, &options).unwrap();
    let mut projection = SchurSession::new(&design, &plan, team)
        .unwrap()
        .solve(&rhs, team)
        .unwrap();
    design
        .normalize_coefficients(&mut projection.coefficients, options.tolerance, team)
        .unwrap();
    let residuals = design
        .reconstruct(&rhs, &projection.coefficients, team)
        .unwrap();
    design
        .certify(
            &rhs,
            &projection.coefficients,
            &residuals,
            options.tolerance,
            team,
        )
        .unwrap();
    let within = DenseMatrix::from_columns(
        &(1..rhs.ncols())
            .map(|c| residuals.column(c).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let slots: Vec<_> = columns[1..]
        .iter()
        .zip(model.roles)
        .map(|(values, &role)| wls::Slot {
            role,
            reference_norm: values.iter().fold(0_f64, |norm, &v| norm.hypot(v)),
        })
        .collect();
    let fit = wls::solve(
        &wls::Problem {
            design: &within,
            outcome: &residuals.column(0).unwrap(),
            weights: Weights::Unit,
            slots: &slots,
            has_absorbed_intercept: true,
        },
        wls::Policy {
            score_tolerance: model.controls.projection_tolerance,
            ..wls::Policy::default()
        },
        team,
    )
    .unwrap();
    let mut alpha = DenseMatrix::zeros(design.layout().total, 1).unwrap();
    for row in 0..alpha.nrows() {
        alpha[(row, 0)] = std::iter::once(projection.coefficients[(row, 0)])
            .chain(
                fit.coefficients
                    .iter()
                    .enumerate()
                    .map(|(c, &b)| -b * projection.coefficients[(row, c + 1)]),
            )
            .sum::<f64>();
    }
    design
        .normalize_coefficients(&mut alpha, options.tolerance, team)
        .unwrap();
    let target: Vec<_> = normalized
        .iter()
        .enumerate()
        .map(|(row, &y)| {
            y - fit
                .coefficients
                .iter()
                .enumerate()
                .map(|(c, &b)| model.raw[(row, c)] * b)
                .sum::<f64>()
        })
        .collect();
    let target = DenseMatrix::from_columns(&[target]).unwrap();
    let residuals = design.reconstruct(&target, &alpha, team).unwrap();
    design
        .certify(&target, &alpha, &residuals, options.tolerance, team)
        .unwrap();
    residuals.as_slice().iter().fold(0_f64, |a, &v| a.hypot(v))
        / normalized.iter().fold(0_f64, |a, &v| a.hypot(v))
}

#[test]
fn strict_span_verification_does_not_substitute_a_passing_fe_score() {
    campaign(27, 1);
}

#[test]
fn recession_span_matches_independent_explicit_dummy_qr() {
    campaign(0, 200);
}
#[test]
#[ignore = "long numerical separation cone differential"]
fn long_recession_span_differential() {
    campaign(200, 2000);
}

#[test]
fn numerical_deletion_and_its_full_cascade_publish_one_generation() {
    let effects = vec![
        DummyEffect::new(vec![0, 0, 1, 1, 1, 2, 2, 2, 2], 3, true, vec![]).unwrap(),
        DummyEffect::new(vec![0, 1, 0, 2, 2, 1, 1, 2, 2], 3, true, vec![]).unwrap(),
    ];
    let sample = inputs::prepare(&effects, &[0., 1., 1., 0., 0., 1., 1., 1., 1.]);
    let mut ledger = Ledger::new(
        sample,
        Policy {
            drop_singletons: true,
            methods: Methods::default(),
        },
    )
    .unwrap();
    assert!(ledger.prune_initial_singletons().unwrap().is_none());
    let exact = ledger.view().exact_proposal().unwrap();
    assert!(ledger.accept_exact(&exact).unwrap().is_none());
    let p = ledger
        .view()
        .direction_proposal(
            Method::Simplex,
            1,
            vec![-1., 0., 0., 0., 0., 0., 0., 0., 0.],
            Controls::default(),
        )
        .unwrap();
    let raw = DenseMatrix::from_columns(&[p.direction.clone()]).unwrap();
    let model = Model {
        raw: &raw,
        roles: &[Role::Ordinary],
        options: Options::for_estimator(Estimator::Ppml),
        controls: Controls::default(),
    };
    let result = ledger
        .accept_direction(&p, &model, &ThreadTeam::new(1).unwrap())
        .unwrap();
    assert_eq!(
        (
            result.commit.generation,
            result.commit.primary,
            result.commit.singletons,
            result.commit.exact_followup
        ),
        (1, 1, 2, 2)
    );
    let out = ledger.view().sample();
    assert_eq!(out.retained_rows(), &[5, 6, 7, 8]);
    assert_eq!(
        &out.drop_reasons[..5],
        &[
            DropReason::SimplexSeparation,
            DropReason::Singleton,
            DropReason::Singleton,
            DropReason::FixedEffectSeparation,
            DropReason::FixedEffectSeparation
        ]
    );
    out.topology.as_ref().unwrap().validate(out).unwrap();
}

#[test]
fn numerical_cascade_cannot_commit_when_it_eliminates_all_positive_outcomes() {
    let effects = vec![
        DummyEffect::new(vec![0, 0, 1, 1], 2, true, vec![]).unwrap(),
        DummyEffect::new(vec![0, 1, 0, 1], 2, true, vec![]).unwrap(),
    ];
    let sample = inputs::prepare(&effects, &[0., 1., 1., 1.]);
    let mut ledger = Ledger::new(
        sample,
        Policy {
            drop_singletons: true,
            methods: Methods::default(),
        },
    )
    .unwrap();
    let p = ledger
        .view()
        .direction_proposal(
            Method::Simplex,
            1,
            vec![-1., 0., 0., 0.],
            Controls::default(),
        )
        .unwrap();
    let raw = DenseMatrix::from_columns(&[p.direction.clone()]).unwrap();
    let model = Model {
        raw: &raw,
        roles: &[Role::Ordinary],
        options: Options::for_estimator(Estimator::Ppml),
        controls: Controls::default(),
    };
    let digest = ledger.trace_digest();
    assert_eq!(
        ledger
            .accept_direction(&p, &model, &ThreadTeam::new(1).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::InsufficientObservations
    );
    assert_eq!(ledger.view().generation(), 0);
    assert_eq!(ledger.view().sample().n_active(), 4);
    assert_eq!(ledger.trace_digest(), digest);
}
