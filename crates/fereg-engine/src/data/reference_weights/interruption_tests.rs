use super::*;
use crate::{execution::interruption_tests::every_checkpoint, threads::ThreadTeam};

fn signature(q: ReferenceWeights<'_>) -> (Vec<u64>, usize, Option<u64>, Option<u64>) {
    (
        (0..q.nobs()).map(|i| q.value(i).to_bits()).collect(),
        q.effective(),
        q.raw_sum.map(f64::to_bits),
        q.log_raw_sum.map(f64::to_bits),
    )
}

#[test]
fn reference_scales_and_weighted_means_cancel_every_checkpoint_and_retry_identically() {
    let n = 4099;
    for kind in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Analytic),
        Some(WeightKind::Probability),
    ] {
        let raw = kind.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
            kind,
            values: (0..n).map(|i| (1 + i % 7) as f64).collect(),
        });
        let values: Vec<_> = (0..n).map(|i| ((i * 7) % 31) as f64 / 8. - 2.).collect();
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let scales = team.install(|| {
                every_checkpoint(|poll| {
                    ReferenceWeights::new_polled(&raw, n, Estimator::Ols, poll).map(signature)
                })
            });
            let q = ReferenceWeights::new(&raw, n, Estimator::Ols).unwrap();
            let means = team.install(|| every_checkpoint(|poll| q.mean_polled(&values, poll)));
            assert!(means > 10);
            assert_eq!(*reference.get_or_insert((scales, means)), (scales, means));
            assert_eq!(
                q.mean(&values).unwrap().to_bits(),
                q.mean_by(|i| values[i]).unwrap().to_bits()
            );
        }
    }
    let tiny = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: vec![1., f64::from_bits(1)],
    };
    every_checkpoint(|poll| {
        let q = ReferenceWeights::new_polled(&tiny, 2, Estimator::Ppml, poll)?;
        Ok(q.mean_polled(&[0., 1e300], poll)?.to_bits())
    });
    let huge = PreparedWeights::Provided {
        kind: WeightKind::Analytic,
        values: vec![f64::MAX; n],
    };
    every_checkpoint(|poll| {
        let q = ReferenceWeights::new_polled(&huge, n, Estimator::Ols, poll)?;
        assert_eq!(q.raw_sum, None);
        assert_eq!(q.mean_polled(&vec![0.; n], poll)?, 0.);
        Ok(signature(q))
    });
}

#[test]
fn mean_value_failure_and_malformed_weights_keep_the_original_error() {
    let q = ReferenceWeights::new(&PreparedWeights::None, 4099, Estimator::Ols).unwrap();
    let failure = FeregError::new(ErrorCode::NonConvergence, "nested reported mean failed");
    assert_eq!(
        q.try_mean_by_polled(
            |i| {
                if i == 1025 {
                    Err(failure.clone())
                } else {
                    Ok(1.)
                }
            },
            None
        )
        .unwrap_err(),
        failure
    );
    let observe = || true;
    for raw in [
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values: vec![1., 1.5],
        },
        PreparedWeights::Provided {
            kind: WeightKind::Analytic,
            values: vec![1., 0.],
        },
        PreparedWeights::Provided {
            kind: WeightKind::Importance,
            values: vec![1., 1.],
        },
    ] {
        for estimator in [Estimator::Ols, Estimator::Ppml] {
            assert_eq!(
                ReferenceWeights::new(&raw, 2, estimator).unwrap_err(),
                ReferenceWeights::new_polled(&raw, 2, estimator, Some(&observe)).unwrap_err()
            );
        }
    }
    assert_eq!(
        q.mean(&[1.]).unwrap_err(),
        q.mean_polled(&[1.], Some(&observe)).unwrap_err()
    );
}
