use super::*;
use crate::{
    data::{PreparedWeights, ReferenceWeights, WeightKind},
    execution::interruption_tests::every_checkpoint,
    spec::Estimator,
};

#[test]
fn reference_null_and_log_fallback_cancel_every_checkpoint_without_losing_tiny_outcomes() {
    let n = 2053;
    let regular: Vec<_> = (0..n).map(|i| [0., 0.5, 1., 2., 4.][i % 5]).collect();
    let mut tiny = vec![0.; n];
    tiny[n - 1] = 1e300;
    let mut tiny_weights = vec![1.; n];
    tiny_weights[n - 1] = f64::from_bits(1);
    for (outcome, raw, fallback) in [
        (
            regular,
            PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values: (0..n).map(|i| (1 + i % 3) as f64).collect(),
            },
            false,
        ),
        (
            tiny,
            PreparedWeights::Provided {
                kind: WeightKind::Probability,
                values: tiny_weights,
            },
            true,
        ),
    ] {
        let q = ReferenceWeights::new(&raw, n, Estimator::Ppml).unwrap();
        let offset = vec![700.; n];
        let family = Family::new(&outcome, q, Some(&offset)).unwrap();
        assert_eq!(family.scaled_null_mean(None).unwrap().is_none(), fallback);
        let null = family.null_objective().unwrap();
        if fallback {
            // The representable normalized subnormal weight may round. The null
            // experiment uses the accepted computational weights, not raw ratios.
            let expected = q.value(n - 1) * 1e300 / (0..n).map(|row| q.value(row)).sum::<f64>();
            assert!(null.mean > 0. && null.log_likelihood.is_finite());
            assert!((null.mean / expected - 1.).abs() < 2e-13);
        }
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let input = team.install(|| {
                every_checkpoint(|poll| {
                    Family::new_polled(&outcome, q, Some(&offset), poll).map(|f| f.terms)
                })
            });
            let likelihood =
                team.install(|| every_checkpoint(|poll| family.null_objective_polled(poll)));
            assert!(input >= 4 && likelihood > 5);
            assert_eq!(
                *reference.get_or_insert((input, likelihood)),
                (input, likelihood)
            );
        }
        assert_eq!(
            Family::new(&outcome, q, None)
                .unwrap()
                .null_objective()
                .unwrap(),
            null
        );
    }
}
