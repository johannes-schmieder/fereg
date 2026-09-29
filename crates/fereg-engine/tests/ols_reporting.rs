use fereg_engine::{
    data::*,
    dof::ResidualDof,
    ols,
    spec::Estimator,
    threads::ThreadTeam,
    vce::{self, Kind},
    wls::{self, Policy, Problem, Role, Slot, Status},
};
fn compiled(constant: bool) -> CompiledColumns {
    let mut result = CompiledColumns {
        slots: vec![
            ColumnSlot {
                name: "base".into(),
                syntax: SyntaxStatus::Base,
                generated: None,
            },
            ColumnSlot {
                name: "x".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(0),
            },
            ColumnSlot {
                name: "duplicate".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(1),
            },
            ColumnSlot {
                name: "omitted".into(),
                syntax: SyntaxStatus::ExplicitlyOmitted,
                generated: None,
            },
        ],
        recipes: vec![
            ColumnRecipe::Continuous { source: 0 },
            ColumnRecipe::Continuous { source: 1 },
        ],
        roles: vec![Role::Ordinary; 2],
        constant_slot: None,
        structural_intercept: true,
        implicit_constant: true,
    };
    if constant {
        result.constant_slot = Some(result.slots.len());
        result.slots.push(ColumnSlot {
            name: "_cons".into(),
            syntax: SyntaxStatus::Active,
            generated: Some(2),
        });
        result.roles.push(Role::AbsorbedIntercept);
        result.recipes.push(ColumnRecipe::Intercept);
    }
    result
}
fn slots(raw: &DenseMatrix, weights: ReferenceWeights<'_>, roles: &[Role]) -> Vec<Slot> {
    roles
        .iter()
        .enumerate()
        .map(|(j, &role)| Slot {
            role,
            reference_norm: (0..raw.nrows()).fold(0_f64, |norm, i| {
                norm.hypot(raw[(i, j)] * weights.value(i).sqrt())
            }),
        })
        .collect()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * b.abs().max(1.), "{a} != {b}");
}
#[test]
fn reported_constant_and_covariance_match_independent_full_design_fit() {
    let x = vec![2., 3., 4., 5., 6., 7., 8., 9.];
    let y = vec![3., 8., 5., 12., 11., 10., 9., 14.];
    for kind in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Analytic),
        Some(WeightKind::Probability),
    ] {
        let raw_weights = kind.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
            kind,
            values: vec![1., 2., 1., 3., 2., 1., 2., 1.],
        });
        let weights = ReferenceWeights::new(&raw_weights, x.len(), Estimator::Ols).unwrap();
        let xmean = weights.mean(&x).unwrap();
        let ymean = weights.mean(&y).unwrap();
        let wx: Vec<_> = x.iter().map(|v| v - xmean).collect();
        let wy: Vec<_> = y.iter().map(|v| v - ymean).collect();
        let mut baseline = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let full = DenseMatrix::from_columns(&[x.clone(), vec![1.; x.len()]]).unwrap();
            let direct = wls::solve(
                &Problem {
                    design: &full,
                    outcome: &y,
                    weights: Weights::Reference(weights),
                    slots: &slots(&full, weights, &[Role::Ordinary; 2]),
                    has_absorbed_intercept: false,
                },
                Policy::default(),
                &team,
            )
            .unwrap();
            for constant in [false, true] {
                let columns = compiled(constant);
                let mut raw_columns = vec![x.clone(), x.clone()];
                let mut within_columns = vec![wx.clone(), wx.clone()];
                if constant {
                    raw_columns.push(vec![1.; x.len()]);
                    within_columns.push(vec![0.; x.len()]);
                }
                let raw = DenseMatrix::from_columns(&raw_columns).unwrap();
                let within = DenseMatrix::from_columns(&within_columns).unwrap();
                let fit = wls::solve(
                    &Problem {
                        design: &within,
                        outcome: &wy,
                        weights: Weights::Reference(weights),
                        slots: &slots(&raw, weights, &columns.roles),
                        has_absorbed_intercept: true,
                    },
                    Policy::default(),
                    &team,
                )
                .unwrap();
                let report = ols::report(&columns, &raw, &within, &y, weights, &fit).unwrap();
                assert_eq!(report.finite_rank, 1);
                assert_eq!(
                    report.statuses[..4],
                    [
                        Status::Base,
                        Status::Kept,
                        Status::NumericallyRankDeficient,
                        Status::ExplicitlyOmitted
                    ]
                );
                assert_eq!(
                    report.coordinates,
                    if constant { vec![1, 4] } else { vec![1] }
                );
                assert_eq!(report.coefficients[0], 0.);
                assert_eq!(report.coefficients[2], 0.);
                assert_eq!(report.coefficients[3], 0.);
                close(report.coefficients[1], direct.coefficients[0]);
                if !constant {
                    assert_eq!(report.reported_constant, 0.);
                    continue;
                }
                close(report.reported_constant, direct.coefficients[1]);
                assert_eq!(report.statuses[4], Status::NormalizedAbsorbedIntercept);
                for (a, b) in report.bread.as_slice().iter().zip(direct.bread.as_slice()) {
                    close(*a, *b);
                }
                for (a, b) in report.score_design.as_slice().iter().zip(full.as_slice()) {
                    close(*a, *b);
                }
                for vce in [Kind::Iid, Kind::Robust] {
                    if kind == Some(WeightKind::Probability) && vce == Kind::Iid {
                        continue;
                    }
                    let cov = vce::estimate(
                        &vce::Problem {
                            design: &report.score_design,
                            bread: &report.bread,
                            residual: &fit.residuals,
                            weights,
                            clusters: &[],
                            dof: ResidualDof::new(
                                Estimator::Ols,
                                weights.effective(),
                                1,
                                1,
                                0,
                                &[],
                            )
                            .unwrap(),
                        },
                        vce,
                        5e-10,
                        &team,
                    )
                    .unwrap();
                    let full_cov = vce::estimate(
                        &vce::Problem {
                            design: &full,
                            bread: &direct.bread,
                            residual: &direct.residuals,
                            weights,
                            clusters: &[],
                            dof: ResidualDof::new(
                                Estimator::Ols,
                                weights.effective(),
                                2,
                                0,
                                0,
                                &[],
                            )
                            .unwrap(),
                        },
                        vce,
                        5e-10,
                        &team,
                    )
                    .unwrap();
                    for (a, b) in cov.values.as_slice().iter().zip(full_cov.values.as_slice()) {
                        close(*a, *b);
                    }
                }
                if let Some(expected) = &baseline {
                    assert_eq!(&report, expected);
                } else {
                    baseline = Some(report);
                }
            }
        }
    }
}
#[test]
fn intercept_only_and_missing_report_slots_keep_their_rank_semantics() {
    let mut columns = compiled(true);
    columns.slots.drain(0..4);
    columns.slots[0].generated = Some(0);
    columns.constant_slot = Some(0);
    columns.recipes = vec![ColumnRecipe::Intercept];
    columns.roles = vec![Role::AbsorbedIntercept];
    let raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let within = DenseMatrix::zeros(4, 1).unwrap();
    let weights = ReferenceWeights::new(&PreparedWeights::None, 4, Estimator::Ols).unwrap();
    let fit = wls::solve(
        &Problem {
            design: &within,
            outcome: &[-2., 0., 0., 2.],
            weights: Weights::Unit,
            slots: &slots(&raw, weights, &columns.roles),
            has_absorbed_intercept: true,
        },
        Policy::default(),
        &ThreadTeam::new(1).unwrap(),
    )
    .unwrap();
    let report = ols::report(&columns, &raw, &within, &[0., 2., 2., 4.], weights, &fit).unwrap();
    assert_eq!(report.coefficients, vec![2.]);
    assert_eq!(report.finite_rank, 0);
    assert_eq!(report.bread[(0, 0)], 0.25);
    columns.constant_slot = None;
    assert!(ols::report(&columns, &raw, &within, &[0., 2., 2., 4.], weights, &fit).is_err());
    assert!(weights.mean(&[1., 2.]).is_err());
    assert!(weights.mean(&[1., f64::NAN, 2., 3.]).is_err());
    columns.constant_slot = Some(usize::MAX);
    assert!(ols::report(&columns, &raw, &within, &[0., 2., 2., 4.], weights, &fit).is_err());
}
