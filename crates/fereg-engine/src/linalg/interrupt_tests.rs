//! Cancellation must leave immutable factors reusable, including rank loss.
use super::WeightedQr;
use crate::{
    data::{DenseMatrix, Weights},
    execution::Poll,
    ErrorCode, Result,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn matrix() -> DenseMatrix {
    DenseMatrix::from_columns(&[
        (0..2053).map(|row| (row as f64 * 0.31).sin()).collect(),
        (0..2053).map(|row| (row as f64 * 0.2).cos()).collect(),
        (0..2053).map(|row| (row % 13) as f64 - 6.).collect(),
        vec![1.; 2053],
        (0..2053)
            .map(|row| 2. * (row as f64 * 0.31).sin())
            .collect(),
    ])
    .unwrap()
}
fn snapshot(factor: WeightedQr) -> Vec<u64> {
    let mut bits = factor
        .permutation
        .iter()
        .map(|&i| i as u64)
        .collect::<Vec<_>>();
    bits.extend(
        factor
            .qr
            .as_slice()
            .iter()
            .chain(&factor.taus)
            .chain(&factor.scaling.maxima)
            .chain(&factor.scaling.norms)
            .chain(&factor.scaling.roots)
            .map(|x| x.to_bits()),
    );
    bits.push(factor.scaling.root_scale.to_bits());
    bits
}
fn every_checkpoint<T: std::fmt::Debug + PartialEq>(run: impl Fn(Poll<'_>) -> Result<T>) {
    let expected = run(None).unwrap();
    let count = AtomicUsize::new(0);
    let count_poll = || {
        count.fetch_add(1, Ordering::Relaxed);
        true
    };
    assert_eq!(run(Some(&count_poll)).unwrap(), expected);
    let total = count.load(Ordering::Relaxed);
    assert!(total > 2);
    for stop in 1..=total {
        count.store(0, Ordering::Relaxed);
        let cancel = || count.fetch_add(1, Ordering::Relaxed) + 1 != stop;
        let error = run(Some(&cancel)).unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(count.load(Ordering::Relaxed), stop);
        assert_eq!(run(None).unwrap(), expected);
    }
}

#[test]
fn weighted_rank_revealing_qr_aborts_every_dense_checkpoint_without_a_factor() {
    let weights = (0..2053)
        .map(|row| 0.25 + (row % 7) as f64 / 3.)
        .collect::<Vec<_>>();
    every_checkpoint(|poll| {
        let factor = WeightedQr::factor(matrix(), Weights::Provided(&weights), 1e-12, poll)?;
        assert_eq!(factor.rank(), 4);
        Ok(snapshot(factor))
    });
}
#[test]
fn qr_rhs_normal_and_inverse_actions_abort_without_mutating_the_factor() {
    let design = matrix();
    let weights = (0..2053)
        .map(|row| 0.25 + (row % 7) as f64 / 3.)
        .collect::<Vec<_>>();
    let factor = WeightedQr::factor(matrix(), Weights::Provided(&weights), 1e-12, None).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..2053).map(|row| 1.2 - 0.7 * design[(row, 0)]).collect(),
        (0..2053)
            .map(|row| -0.4 * design[(row, 2)] + design[(row, 1)])
            .collect(),
    ])
    .unwrap();
    every_checkpoint(|poll| {
        factor.solve(
            DenseMatrix::from_columns(&[rhs.column(0)?, rhs.column(1)?])?,
            poll,
        )
    });
    every_checkpoint(|poll| factor.scaled_inverse(poll));
    let mut normal = DenseMatrix::zeros(design.ncols(), rhs.ncols()).unwrap();
    for column in 0..design.ncols() {
        for response in 0..rhs.ncols() {
            normal[(column, response)] = crate::threads::compensated_sum(
                (0..design.nrows())
                    .map(|row| weights[row] * design[(row, column)] * rhs[(row, response)]),
            );
        }
    }
    every_checkpoint(|poll| factor.solve_normal(&normal, poll));
    let fitted = factor.solve(rhs, None).unwrap();
    for row in 0..design.nrows() {
        let recovered = crate::threads::compensated_sum(
            factor
                .kept()
                .iter()
                .map(|&column| design[(row, column)] * fitted[(column, 0)]),
        );
        assert!((recovered - (1.2 - 0.7 * design[(row, 0)])).abs() < 2e-14);
    }
}
