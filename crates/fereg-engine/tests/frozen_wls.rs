use fereg_engine::{
    data::{DenseMatrix, Weights},
    threads::ThreadTeam,
    wls::*,
    ErrorCode,
};
fn problem<'a>(x: &'a DenseMatrix, y: &'a [f64], slots: &'a [Slot]) -> Problem<'a> {
    Problem {
        design: x,
        outcome: y,
        weights: Weights::Unit,
        slots,
        has_absorbed_intercept: true,
    }
}
fn ordinary(norm: f64) -> Slot {
    Slot {
        role: Role::Ordinary,
        reference_norm: norm,
    }
}
#[test]
fn frozen_coordinates_survive_same_rank_canonical_pivot_changes() {
    let x = DenseMatrix::from_columns(&[vec![-1., 0., 1.], vec![-1., 0., 1.]]).unwrap();
    let y = [-2., 0.5, 2.];
    let slots = [ordinary(2.), ordinary(4.)];
    let team = ThreadTeam::new(1).unwrap();
    let original = solve(&problem(&x, &y, &slots), Policy::default(), &team).unwrap();
    let basis = FrozenBasis::from_fit(&original).unwrap();
    assert_eq!(basis.kept(), &[0]);
    let changed = [ordinary(4.), ordinary(2.)];
    let next = problem(&x, &y, &changed);
    let canonical = solve(&next, Policy::default(), &team).unwrap();
    assert_eq!(canonical.kept, vec![1]);
    assert!(!basis.matches(&canonical));
    for method in [
        SolveOverride::Automatic,
        SolveOverride::Qr,
        SolveOverride::Cholesky,
    ] {
        let policy = Policy {
            override_method: method,
            ..Policy::default()
        };
        let fit = solve_with_basis(&next, policy, Some(&basis), &team).unwrap();
        assert!(basis.matches(&fit));
        assert!((fit.coefficients[0] - 2.).abs() < 1e-14);
        assert_eq!(fit.coefficients[1], 0.);
        assert!((fit.bread[(0, 0)] - 0.5).abs() < 1e-14);
        assert_eq!(fit.bread[(1, 1)], 0.);
        for workers in [2, 4, 8] {
            assert_eq!(
                fit,
                solve_with_basis(
                    &next,
                    policy,
                    Some(&basis),
                    &ThreadTeam::new(workers).unwrap()
                )
                .unwrap()
            );
        }
    }
}
#[test]
fn positive_working_weights_cannot_silently_remove_a_frozen_direction() {
    let x = DenseMatrix::from_columns(&[vec![1., 0., 0.], vec![0., 1., 1.]]).unwrap();
    let slots = [ordinary(1.), ordinary(2_f64.sqrt())];
    let y = [1., 2., 3.];
    let team = ThreadTeam::new(1).unwrap();
    let original = solve(&problem(&x, &y, &slots), Policy::default(), &team).unwrap();
    let basis = FrozenBasis::from_fit(&original).unwrap();
    let weights = [1e-100, 1., 1.];
    // A large absorbed component keeps the pre-projection norm positive while
    // changing weights make the first within direction numerically negligible.
    let next = Problem {
        weights: Weights::Provided(&weights),
        ..problem(&x, &y, &slots)
    };
    let fresh = solve(&next, Policy::default(), &team).unwrap();
    assert_eq!(fresh.kept, vec![1]);
    for override_method in [
        SolveOverride::Automatic,
        SolveOverride::Qr,
        SolveOverride::Cholesky,
    ] {
        let error = solve_with_basis(
            &next,
            Policy {
                override_method,
                ..Policy::default()
            },
            Some(&basis),
            &team,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::NonConvergence);
    }
    assert!(basis.matches(&original));
}
#[test]
fn syntax_and_fe_omissions_remain_frozen_even_if_the_current_columns_grow() {
    let x = DenseMatrix::from_columns(&[vec![-1., 0., 1.], vec![0.; 3], vec![0.; 3], vec![0.; 3]])
        .unwrap();
    let slots = [
        ordinary(2.),
        ordinary(2.),
        Slot {
            role: Role::Base,
            reference_norm: 0.,
        },
        Slot {
            role: Role::AbsorbedIntercept,
            reference_norm: 2.,
        },
    ];
    let y = [-2., 0., 2.];
    let team = ThreadTeam::new(1).unwrap();
    let original = solve(&problem(&x, &y, &slots), Policy::default(), &team).unwrap();
    let basis = FrozenBasis::from_fit(&original).unwrap();
    assert_eq!(
        basis.statuses(),
        &[
            Status::Kept,
            Status::CollinearWithFixedEffects,
            Status::Base,
            Status::AbsorbedInterceptDeferred
        ]
    );
    let next_x = DenseMatrix::from_columns(&[
        vec![-1., 0., 1.],
        vec![1., -2., 1.],
        vec![0.; 3],
        vec![0.; 3],
    ])
    .unwrap();
    let next = problem(&next_x, &y, &slots);
    let frozen = solve_with_basis(&next, Policy::default(), Some(&basis), &team).unwrap();
    assert!(basis.matches(&frozen));
    assert!(!basis.matches(&solve(&next, Policy::default(), &team).unwrap()));
    let mut changed_roles = slots;
    changed_roles[2].role = Role::Ordinary;
    assert_eq!(
        solve_with_basis(
            &problem(&next_x, &y, &changed_roles),
            Policy::default(),
            Some(&basis),
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    let mut malformed = original;
    malformed.kept.push(0);
    assert!(FrozenBasis::from_fit(&malformed).is_err());
}
#[test]
fn zero_rank_and_new_positive_weight_frames_preserve_canonical_statuses() {
    let team = ThreadTeam::new(1).unwrap();
    let x = DenseMatrix::zeros(3, 0).unwrap();
    let original = solve(&problem(&x, &[1., 2., 3.], &[]), Policy::default(), &team).unwrap();
    let basis = FrozenBasis::from_fit(&original).unwrap();
    let weights = [3., 2., 7.];
    let next = Problem {
        weights: Weights::Provided(&weights),
        ..problem(&x, &[4., 5., 6.], &[])
    };
    let fit = solve_with_basis(&next, Policy::default(), Some(&basis), &team).unwrap();
    assert!(basis.matches(&fit));
    assert_eq!(fit.residuals, vec![4., 5., 6.]);
}
