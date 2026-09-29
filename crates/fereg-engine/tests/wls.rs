use fereg_engine::{data::DenseMatrix, threads::ThreadTeam, wls::*};
fn slots(matrix: &DenseMatrix, weights: &[f64]) -> Vec<Slot> {
    (0..matrix.ncols())
        .map(|column| Slot {
            role: Role::Ordinary,
            reference_norm: (0..matrix.nrows()).fold(0_f64, |norm, row| {
                norm.hypot(weights[row].sqrt() * matrix[(row, column)])
            }),
        })
        .collect()
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1.),
        "{actual} != {expected}"
    );
}
#[test]
fn hand_computed_fit_and_bread_match_both_factorizations_and_workers() {
    let matrix = DenseMatrix::from_columns(&[vec![1.; 3], vec![-1., 0., 1.]]).unwrap();
    let weights = vec![1.; 3];
    let slots = slots(&matrix, &weights);
    let problem = Problem {
        design: &matrix,
        outcome: &[2., 0., 4.],
        weights: fereg_engine::data::Weights::Provided(&weights),
        slots: &slots,
        has_absorbed_intercept: false,
    };
    for override_method in [
        SolveOverride::Automatic,
        SolveOverride::Cholesky,
        SolveOverride::Qr,
    ] {
        let policy = Policy {
            override_method,
            ..Policy::default()
        };
        let fit = solve(&problem, policy, &ThreadTeam::new(1).unwrap()).unwrap();
        close(fit.coefficients[0], 2., 1e-14);
        close(fit.coefficients[1], 1., 1e-14);
        close(fit.bread[(0, 0)], 1. / 3., 1e-14);
        close(fit.bread[(1, 1)], 0.5, 1e-14);
        close(fit.bread[(0, 1)], 0., 1e-14);
        assert_eq!(fit.kept, vec![0, 1]);
        for workers in [2, 4, 8] {
            assert_eq!(
                fit,
                solve(&problem, policy, &ThreadTeam::new(workers).unwrap()).unwrap()
            );
        }
    }
}
#[test]
fn rank_deficiency_uses_qr_and_preserves_slot_positions() {
    let x = vec![-3., -1., 1., 3.];
    let matrix = DenseMatrix::from_columns(&[
        vec![1.; 4],
        x.clone(),
        x.iter().map(|x| 2. * x).collect(),
        vec![0.; 4],
        vec![8.; 4],
    ])
    .unwrap();
    let weights = vec![1.; 4];
    let mut slots = slots(&matrix, &weights);
    slots[3].role = Role::Base;
    slots[4].role = Role::ExplicitlyOmitted;
    let problem = Problem {
        design: &matrix,
        outcome: &[-4., 0., 4., 8.],
        weights: fereg_engine::data::Weights::Provided(&weights),
        slots: &slots,
        has_absorbed_intercept: false,
    };
    let fit = solve(&problem, Policy::default(), &ThreadTeam::new(1).unwrap()).unwrap();
    assert_eq!(fit.method, Method::ColumnPivotedQr);
    assert_eq!(
        fit.statuses,
        vec![
            Status::Kept,
            Status::Kept,
            Status::NumericallyRankDeficient,
            Status::Base,
            Status::ExplicitlyOmitted
        ]
    );
    close(fit.coefficients[0], 2., 1e-14);
    close(fit.coefficients[1], 2., 1e-14);
    assert_eq!(&fit.coefficients[2..], &[0.; 3]);
    assert!(fit.residuals.iter().all(|v| v.abs() < 1e-13));
    assert!(solve(
        &problem,
        Policy {
            override_method: SolveOverride::Cholesky,
            ..Policy::default()
        },
        &ThreadTeam::new(1).unwrap()
    )
    .is_err());
}
#[test]
fn fe_collinearity_uses_squared_norm_ratio_and_defers_the_absorbed_constant() {
    let matrix = DenseMatrix::from_columns(&[
        vec![0.; 4],
        vec![1e-6, -1e-6, 1e-6, -1e-6],
        vec![-3., -1., 1., 3.],
    ])
    .unwrap();
    let weights = vec![1.; 4];
    let mut slots = slots(&matrix, &weights);
    slots[0] = Slot {
        role: Role::AbsorbedIntercept,
        reference_norm: 2.,
    };
    slots[1].reference_norm = 2.;
    let problem = Problem {
        design: &matrix,
        outcome: &[-6., -2., 2., 6.],
        weights: fereg_engine::data::Weights::Provided(&weights),
        slots: &slots,
        has_absorbed_intercept: true,
    };
    let fit = solve(&problem, Policy::default(), &ThreadTeam::new(1).unwrap()).unwrap();
    assert_eq!(
        fit.statuses,
        vec![
            Status::AbsorbedInterceptDeferred,
            Status::CollinearWithFixedEffects,
            Status::Kept
        ]
    );
    close(fit.coefficients[2], 2., 1e-14);
    assert_eq!(fit.coefficients[0], 0.);
    assert_eq!(fit.bread[(1, 1)], 0.);
}
#[test]
fn weak_identified_direction_falls_back_without_losing_rank() {
    let n = 40;
    let x: Vec<_> = (0..n).map(|r| r as f64 / 20. - 1.).collect();
    let z: Vec<_> = (0..n).map(|r| if r % 2 == 0 { -1. } else { 1. }).collect();
    let perturbed: Vec<_> = x.iter().zip(&z).map(|(x, z)| x + 1e-7 * z).collect();
    let matrix = DenseMatrix::from_columns(&[vec![1.; n], x, perturbed]).unwrap();
    let weights = vec![1.; n];
    let slots = slots(&matrix, &weights);
    let problem = Problem {
        design: &matrix,
        outcome: &z,
        weights: fereg_engine::data::Weights::Provided(&weights),
        slots: &slots,
        has_absorbed_intercept: false,
    };
    let fit = solve(&problem, Policy::default(), &ThreadTeam::new(1).unwrap()).unwrap();
    assert_eq!(fit.method, Method::ColumnPivotedQr);
    assert_eq!(fit.kept.len(), 3);
    assert!(fit.refinements > 0 && fit.refinements <= 2);
    assert!(fit.residuals.iter().all(|v| v.abs() < 2e-8));
    close(fit.coefficients[1], -1e7, 2e-8);
    close(fit.coefficients[2], 1e7, 2e-8);
}
#[test]
fn common_weight_scaling_changes_bread_only_and_empty_design_is_valid() {
    let matrix = DenseMatrix::from_columns(&[vec![1.; 5], vec![-2., -1., 0., 1., 2.]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut reference: Option<Fit> = None;
    for scale in [1., 1e-100, 1e100] {
        let weights: Vec<_> = [1., 2., 3., 2., 1.].iter().map(|w| w * scale).collect();
        let slots = slots(&matrix, &weights);
        let fit = solve(
            &Problem {
                design: &matrix,
                outcome: &[2., 0., 3., 2., 7.],
                weights: fereg_engine::data::Weights::Provided(&weights),
                slots: &slots,
                has_absorbed_intercept: false,
            },
            Policy::default(),
            &team,
        )
        .unwrap();
        if let Some(reference) = &reference {
            for i in 0..2 {
                close(fit.coefficients[i], reference.coefficients[i], 1e-13);
                for j in 0..2 {
                    close(fit.bread[(i, j)] * scale, reference.bread[(i, j)], 1e-13);
                }
            }
        } else {
            reference = Some(fit);
        }
    }
    let matrix = DenseMatrix::zeros(3, 0).unwrap();
    let fit = solve(
        &Problem {
            design: &matrix,
            outcome: &[1., 2., 3.],
            weights: fereg_engine::data::Weights::Provided(&[1.; 3]),
            slots: &[],
            has_absorbed_intercept: false,
        },
        Policy::default(),
        &team,
    )
    .unwrap();
    assert_eq!(fit.method, Method::Empty);
    assert_eq!(fit.residuals, vec![1., 2., 3.]);
    assert!(fit.kept.is_empty());
}
#[test]
fn fwl_rank_pivots_use_pre_projection_reference_norms() {
    let matrix =
        DenseMatrix::from_columns(&[vec![-1., 1., -1., 1.], vec![-1., 1., -1., 1.]]).unwrap();
    let slots = [
        Slot {
            role: Role::Ordinary,
            reference_norm: 20.,
        },
        Slot {
            role: Role::Ordinary,
            reference_norm: 2.,
        },
    ];
    let fit = solve(
        &Problem {
            design: &matrix,
            outcome: &[-2., 2., -2., 2.],
            weights: fereg_engine::data::Weights::Provided(&[1.; 4]),
            slots: &slots,
            has_absorbed_intercept: true,
        },
        Policy::default(),
        &ThreadTeam::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fit.statuses,
        vec![Status::NumericallyRankDeficient, Status::Kept]
    );
    assert_eq!(fit.kept, vec![1]);
    close(fit.coefficients[1], 2., 1e-14);
}
