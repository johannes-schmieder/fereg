use fereg_engine::{
    data::{DenseMatrix, PreparedWeights, ReferenceWeights, WeightKind, Weights},
    fe::Design,
    spec::Estimator,
    threads::ThreadTeam,
    wls::{self, Policy, Problem, Role, Slot},
};
mod support;

#[test]
fn borrowed_reference_frame_matches_materialized_fwl_across_workers() {
    let mut sample = support::prepared(&[7], false, 301, 0);
    let n = sample.n_active();
    let rhs = DenseMatrix::from_columns(&[
        (0..n)
            .map(|i| (i as f64 * 0.13).sin() + (i % 7) as f64)
            .collect(),
        (0..n).map(|i| (i as f64 * 0.31).cos()).collect(),
        (0..n).map(|i| (i % 11) as f64 - 3.).collect(),
    ])
    .unwrap();
    for kind in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Analytic),
        Some(WeightKind::Probability),
    ] {
        sample.weights = kind.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
            kind,
            values: (0..n)
                .map(|i| {
                    (1 + i % 5) as f64
                        * if kind == WeightKind::Frequency {
                            1.
                        } else {
                            1e250
                        }
                })
                .collect(),
        });
        let reference = ReferenceWeights::new(&sample.weights, n, Estimator::Ols).unwrap();
        let q: Vec<_> = (0..n).map(|i| reference.value(i)).collect();
        let mut baseline = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let views = if kind.is_none() {
                vec![
                    Weights::Provided(&q),
                    Weights::Reference(reference),
                    Weights::Unit,
                ]
            } else {
                vec![Weights::Provided(&q), Weights::Reference(reference)]
            };
            for weights in views {
                let candidate = Design::new(&sample, weights)
                    .unwrap()
                    .direct_candidate(&rhs, &team)
                    .unwrap();
                let finite = DenseMatrix::from_columns(&[
                    candidate.residuals.column(1).unwrap(),
                    candidate.residuals.column(2).unwrap(),
                ])
                .unwrap();
                let outcome = candidate.residuals.column(0).unwrap();
                let slots: Vec<_> = (1..=2)
                    .map(|column| Slot {
                        role: Role::Ordinary,
                        reference_norm: (0..n).fold(0_f64, |norm, row| {
                            norm.hypot(weights.at(row).sqrt() * rhs[(row, column)])
                        }),
                    })
                    .collect();
                let fit = wls::solve(
                    &Problem {
                        design: &finite,
                        outcome: &outcome,
                        weights,
                        slots: &slots,
                        has_absorbed_intercept: true,
                    },
                    Policy::default(),
                    &team,
                )
                .unwrap();
                let result = (candidate, fit);
                if let Some(expected) = &baseline {
                    assert_eq!(&result, expected);
                } else {
                    baseline = Some(result);
                }
            }
        }
    }
}

#[test]
fn shared_weights_reject_invalid_or_misaligned_frames_before_indexing() {
    let sample = support::prepared(&[2], false, 2, 0);
    let reference = ReferenceWeights::new(&PreparedWeights::None, 3, Estimator::Ols).unwrap();
    let matrix = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let slots = [Slot {
        role: Role::Ordinary,
        reference_norm: 2.,
    }];
    let team = ThreadTeam::new(1).unwrap();
    for weights in [
        Weights::Reference(reference),
        Weights::Provided(&[1.; 3]),
        Weights::Provided(&[1., 0., 1., 1.]),
        Weights::Provided(&[1., -1., 1., 1.]),
        Weights::Provided(&[1., f64::NAN, 1., 1.]),
        Weights::Provided(&[1., f64::INFINITY, 1., 1.]),
    ] {
        assert!(Design::new(&sample, weights).is_err());
        assert!(wls::solve(
            &Problem {
                design: &matrix,
                outcome: &[1.; 4],
                weights,
                slots: &slots,
                has_absorbed_intercept: false,
            },
            Policy::default(),
            &team
        )
        .is_err());
    }
}
