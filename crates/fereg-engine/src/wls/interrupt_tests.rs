//! Cancellation is fatal in both finite solvers and frozen-basis validation.
use super::{
    solve_with_basis_polled, FrozenBasis, Method, Policy, Problem, Role, Slot, SolveOverride,
};
use crate::{
    data::{DenseMatrix, Weights},
    threads::ThreadTeam,
    ErrorCode,
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn finite_cholesky_and_qr_abort_every_checkpoint_without_changing_the_accepted_fit() {
    // Balanced binary columns make the hand-calculated input and coefficients
    // exact; the separate tall weighted QR fixture retains an irregular tail.
    let raw = DenseMatrix::from_columns(&[
        vec![1.; 4096],
        (0..4096)
            .map(|row| if row % 2 == 0 { 1. } else { -1. })
            .collect(),
        (0..4096)
            .map(|row| if row % 4 < 2 { 1. } else { -1. })
            .collect(),
        (0..4096)
            .map(|row| {
                if row % 4 == 0 || row % 4 == 3 {
                    1.
                } else {
                    -1.
                }
            })
            .collect(),
    ])
    .unwrap();
    let outcome = (0..raw.nrows())
        .map(|row| 1.25 - 0.75 * raw[(row, 1)] + 0.5 * raw[(row, 2)] + 0.25 * raw[(row, 3)])
        .collect::<Vec<_>>();
    let slots = (0..raw.ncols())
        .map(|column| Slot {
            role: Role::Ordinary,
            reference_norm: (0..raw.nrows())
                .fold(0_f64, |norm, row| norm.hypot(raw[(row, column)])),
        })
        .collect::<Vec<_>>();
    let problem = Problem {
        design: &raw,
        outcome: &outcome,
        weights: Weights::Unit,
        slots: &slots,
        has_absorbed_intercept: false,
    };
    for override_method in [SolveOverride::Cholesky, SolveOverride::Qr] {
        let policy = Policy {
            override_method,
            ..Policy::default()
        };
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let baseline = super::solve(&problem, policy, &team).unwrap();
            assert_eq!(
                baseline.method,
                if override_method == SolveOverride::Qr {
                    Method::ColumnPivotedQr
                } else {
                    Method::GuardedCholesky
                }
            );
            for (&actual, expected) in baseline.coefficients.iter().zip([1.25, -0.75, 0.5, 0.25]) {
                assert!(
                    (actual - expected).abs() < 2e-14,
                    "{override_method:?}, T{workers}: {actual:.17e} versus {expected:.17e}"
                );
            }
            let frozen = FrozenBasis::from_fit(&baseline).unwrap();
            for basis in [None, Some(&frozen)] {
                let count = AtomicUsize::new(0);
                let observe = || {
                    count.fetch_add(1, Ordering::Relaxed);
                    true
                };
                let expected =
                    solve_with_basis_polled(&problem, policy, basis, &team, None).unwrap();
                assert_eq!(
                    solve_with_basis_polled(&problem, policy, basis, &team, Some(&observe))
                        .unwrap(),
                    expected
                );
                let total = count.load(Ordering::Relaxed);
                assert!(total > 20);
                for stop in 1..=total {
                    count.store(0, Ordering::Relaxed);
                    // Sibling moment workers share a persistent cancellation
                    // signal; a later request cannot reopen the operation.
                    let cancel = || {
                        count
                            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                                (value < stop).then_some(value + 1)
                            })
                            .is_ok_and(|previous| previous + 1 < stop)
                    };
                    let error =
                        solve_with_basis_polled(&problem, policy, basis, &team, Some(&cancel))
                            .unwrap_err();
                    assert_eq!(error.code, ErrorCode::Interrupted);
                    assert_eq!(count.load(Ordering::Relaxed), stop);
                    assert_eq!(super::solve(&problem, policy, &team).unwrap(), baseline);
                }
            }
            if let Some(reference) = &reference {
                assert_eq!(reference, &baseline);
            } else {
                reference = Some(baseline);
            }
        }
    }
}
