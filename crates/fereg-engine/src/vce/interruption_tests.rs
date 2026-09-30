//! Cancellation must discard covariance work without changing its science.
use super::{psd, Covariance, Kind, Problem};
use crate::{
    data::{DenseMatrix, PreparedWeights, ReferenceWeights, WeightKind},
    dof::ResidualDof,
    execution::Poll,
    factor::CategoricalCodeView,
    spec::Estimator,
    threads::ThreadTeam,
    ErrorCode, Result,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn every_checkpoint<T: std::fmt::Debug + PartialEq>(
    mut run: impl FnMut(Poll<'_>) -> Result<T>,
) -> usize {
    let expected = run(None).unwrap();
    let count = AtomicUsize::new(0);
    let observe = || {
        count.fetch_add(1, Ordering::Relaxed);
        true
    };
    assert_eq!(run(Some(&observe)).unwrap(), expected);
    let total = count.load(Ordering::Relaxed);
    assert!(total > 0);
    for stop in 1..=total {
        count.store(0, Ordering::Relaxed);
        let cancel = || {
            count
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    (value < stop).then_some(value + 1)
                })
                .is_ok_and(|previous| previous + 1 < stop)
        };
        assert_eq!(run(Some(&cancel)).unwrap_err().code, ErrorCode::Interrupted);
        assert_eq!(count.load(Ordering::Relaxed), stop);
        assert_eq!(run(None).unwrap(), expected);
    }
    total
}

#[test]
fn iid_robust_and_multiway_replays_abort_every_checkpoint_with_fixed_worker_results() {
    // More than two fixed reduction blocks, with an odd tail and heterogeneous
    // frequency/probability weights; the covariance replay re-sums raw rows.
    let n = 2053;
    let mut design = DenseMatrix::zeros(n, 3).unwrap();
    for row in 0..n {
        design[(row, 0)] = 1.;
        design[(row, 1)] = (row % 7) as f64 / 4. - 0.5;
        design[(row, 2)] = (row % 11) as f64 / 8. - 0.25;
    }
    let mut bread = DenseMatrix::zeros(3, 3).unwrap();
    for i in 0..3 {
        bread[(i, i)] = 0.25 + i as f64 / 8.;
    }
    let residual: Vec<_> = (0..n).map(|i| (i % 13) as f64 / 16. - 0.375).collect();
    let first: Vec<_> = (0..n).map(|i| (i % 5) as u32).collect();
    let second: Vec<_> = (0..n).map(|i| ((i / 5) % 7) as u32).collect();
    let all = [
        CategoricalCodeView::new(&first, 5),
        CategoricalCodeView::new(&second, 7),
    ];
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        for weight in [
            None,
            Some(WeightKind::Frequency),
            Some(WeightKind::Analytic),
            Some(WeightKind::Probability),
        ] {
            if estimator == Estimator::Ppml && weight == Some(WeightKind::Analytic) {
                continue;
            }
            let source = weight.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
                kind,
                values: (0..n).map(|i| (1 + i % 3) as f64).collect(),
            });
            let weights = ReferenceWeights::new(&source, n, estimator).unwrap();
            for kind in [Kind::Iid, Kind::Robust, Kind::Cluster] {
                if kind == Kind::Iid
                    && (estimator == Estimator::Ppml || weight == Some(WeightKind::Probability))
                {
                    continue;
                }
                let clusters = if kind == Kind::Cluster {
                    &all[..]
                } else {
                    &all[..0]
                };
                let counts = if clusters.is_empty() {
                    vec![]
                } else {
                    vec![5, 7]
                };
                let problem = Problem {
                    design: &design,
                    bread: &bread,
                    residual: &residual,
                    weights,
                    clusters,
                    dof: ResidualDof::new(estimator, weights.effective(), 3, 0, 0, &counts)
                        .unwrap(),
                };
                let mut reference = None;
                for workers in [1, 8] {
                    let team = ThreadTeam::new(workers).unwrap();
                    let total = every_checkpoint(|poll| {
                        super::estimate_polled(&problem, kind, 1e-10, &team, poll)
                    });
                    let covariance = super::estimate(&problem, kind, 1e-10, &team).unwrap();
                    let state = (total, covariance);
                    if let Some(reference) = &reference {
                        assert_eq!(reference, &state);
                    } else {
                        reference = Some(state);
                    }
                }
            }
        }
    }
}

#[test]
fn psd_repair_inspection_scatter_and_wald_preserve_policies_on_abort() {
    let indefinite = [1., 2., 0., 2., 1., 0., 0., 0., 0.];
    every_checkpoint(|poll| psd::apply_psd_policy_polled(&indefinite, 3, 2, 1e-10, poll));
    let (values, report) = psd::apply_psd_policy(&indefinite, 3, 2, 1e-10).unwrap();
    assert_eq!(report.clipped_eigenvalues, 1);
    assert_eq!(report.final_rank, 1);
    assert_eq!(report.raw_minimum_eigenvalue, -1.);
    assert!((values[0] - 1.5).abs() < 1e-12);
    every_checkpoint(|poll| {
        psd::inspect_published_covariance_polled(&values, 3, 2, 1e-10, poll).map(|(v, r)| {
            (
                v,
                r.minimum_eigenvalue,
                r.rank,
                r.reconstruction_relative_error,
                r.orthogonality_relative_error,
            )
        })
    });
    let covariance = Covariance {
        values: DenseMatrix::from_row_major(3, 3, values).unwrap(),
        standard_errors: vec![1.5_f64.sqrt(), 1.5_f64.sqrt(), 0.],
        correction: 1.,
        cluster_counts: vec![5, 7],
        subset_counts: vec![5, 7, 35],
        reconstruction_error: 0.,
        psd: report,
    };
    every_checkpoint(|poll| {
        covariance
            .clone()
            .scatter_polled(7, &[0, 3, 6], poll)
            .map(|v| {
                (
                    v.values,
                    v.standard_errors
                        .iter()
                        .map(|x| x.to_bits())
                        .collect::<Vec<_>>(),
                )
            })
    });
    let matrix =
        DenseMatrix::from_row_major(3, 3, vec![2., 0.5, 0., 0.5, 1., 0., 0., 0., 0.]).unwrap();
    for estimator in [Estimator::Ols, Estimator::Ppml] {
        for tested in [&[0, 1][..], &[0, 1, 2][..], &[][..]] {
            every_checkpoint(|poll| {
                super::wald_polled(&[2., 3., 0.], &matrix, tested, estimator, 1e-10, poll)
            });
        }
        let full = super::wald(&[2., 3., 0.], &matrix, &[0, 1], estimator, 1e-10).unwrap();
        let expected = 64. / 7. / if estimator == Estimator::Ols { 2. } else { 1. };
        assert!((full.statistic.unwrap() - expected).abs() < 1e-12);
        assert_eq!(
            super::wald(&[2., 3., 0.], &matrix, &[0, 1, 2], estimator, 1e-10)
                .unwrap()
                .statistic,
            None
        );
    }
}
