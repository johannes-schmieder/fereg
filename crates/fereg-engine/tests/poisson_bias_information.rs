use fereg_engine::{
    data::*,
    ppml::{bias, inference, irls},
    spec::{Estimator, Options, Solver},
    statistics,
    threads::ThreadTeam,
    vce,
    wls::Role,
    ErrorCode, FeregError,
};
mod support;
fn columns(source: usize) -> CompiledColumns {
    CompiledColumns {
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
            ColumnSlot {
                name: "_cons".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(2),
            },
        ],
        recipes: vec![
            ColumnRecipe::Continuous { source },
            ColumnRecipe::Continuous { source },
            ColumnRecipe::Intercept,
        ],
        roles: vec![Role::Ordinary, Role::Ordinary, Role::AbsorbedIntercept],
        constant_slot: Some(4),
        structural_intercept: true,
        implicit_constant: false,
    }
}
fn fixture(weight: Option<WeightKind>, exposure: bool) -> (Prepared, CompiledColumns) {
    let mut sample = support::prepared(&[6, 4], false, 20, 1);
    let n = sample.n_active();
    let source = sample.regressors.len() - 1;
    sample.regressors[source] = (0..n).map(|i| (i as f64 * 1.7).sin()).collect();
    sample.outcome = (0..n).map(|i| 0.5 + ((i * 7) % 11) as f64 * 0.5).collect();
    sample.weights = weight.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
        kind,
        values: (0..n).map(|i| (1 + i % 5) as f64).collect(),
    });
    let offset: Vec<_> = (0..n).map(|i| (i % 7) as f64 * 0.08 - 0.24).collect();
    sample.adjustment = if exposure {
        PreparedAdjustment::Exposure(offset.iter().map(|v| v.exp()).collect())
    } else {
        PreparedAdjustment::Offset(offset)
    };
    (sample, columns(source))
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "{a:.17e} != {b:.17e}"
    );
}
fn response(sample: &Prepared, raw: &DenseMatrix, finite: f64) -> Vec<f64> {
    let topology = sample.topology.as_ref().unwrap();
    let left = topology.effect_partition(0).unwrap().codes();
    let right = topology.effect_partition(1).unwrap().codes();
    (0..sample.n_active())
        .map(|i| {
            0.3 + 0.01 * left[i] as f64
                + if right[i] < 2 {
                    0.
                } else {
                    0.25 * (right[i] - 1) as f64
                }
                + finite * raw[(i, 0)]
        })
        .collect()
}
fn check_raw(
    sample: &Prepared,
    raw: &DenseMatrix,
    fit: &irls::Fit<'_>,
    info: &bias::Information<'_, '_>,
) {
    let topology = sample.topology.as_ref().unwrap();
    let left = topology.effect_partition(0).unwrap().codes();
    let right = topology.effect_partition(1).unwrap().codes();
    assert_eq!(info.finite_slots(), &[1]);
    assert_eq!(info.first_levels(), 6);
    assert_eq!(info.second_levels(), 4);
    assert_eq!(info.dimension(), 11);
    assert_eq!(&info.raw()[6..8], &[0., 0.]);
    assert_eq!(info.raw()[10], fit.point.finite[0]);
    for i in 0..sample.n_active() {
        let adjustment = match &sample.adjustment {
            PreparedAdjustment::None => 0.,
            PreparedAdjustment::Offset(v) => v[i],
            PreparedAdjustment::Exposure(v) => v[i].ln(),
        };
        close(
            info.raw()[left[i] as usize]
                + info.raw()[6 + right[i] as usize]
                + raw[(i, 0)] * info.raw()[10]
                + adjustment,
            fit.point.eta[i],
        );
    }
}
fn actions(
    sample: &Prepared,
    raw: &DenseMatrix,
    fit: &irls::Fit<'_>,
    columns: &CompiledColumns,
    team: &ThreadTeam,
) -> (Vec<Vec<f64>>, bias::Diagnostics) {
    bias::with_information(fit, columns, team, &mut || Ok(()), |info, interrupt| {
        check_raw(sample, raw, fit, info);
        assert_eq!(info.requested(), fit.options().solver);
        let mut answers = vec![info.raw().to_vec()];
        for finite in [-0.17, 0., 0.19] {
            let target = response(sample, raw, finite);
            let mut prediction = vec![f64::NAN; sample.n_active()];
            let mut coefficients = vec![f64::NAN; info.dimension()];
            info.solve(&target, &mut prediction, Some(&mut coefficients), interrupt)?;
            for (&actual, &expected) in prediction.iter().zip(&target) {
                close(actual, expected);
            }
            for (level, &alpha) in coefficients[..6].iter().enumerate() {
                close(alpha, 0.3 + 0.01 * level as f64);
            }
            assert_eq!(&coefficients[6..8], &[0., 0.]);
            close(coefficients[8], 0.25);
            close(coefficients[9], 0.5);
            close(coefficients[10], finite);
            answers.push(coefficients);
            answers.push(prediction);
        }
        let diagnostics = info.diagnostics();
        assert_eq!(diagnostics.solves, 3);
        assert_eq!(diagnostics.raw_recoveries, 1);
        assert!(diagnostics.work.projection().counts().right_hand_sides >= 12);
        assert!(diagnostics.work.recovery().counts().right_hand_sides >= 1);
        assert!(diagnostics.maximum_score <= fit.options().tolerance);
        assert!(diagnostics.maximum_reconstruction <= fit.options().tolerance);
        Ok((answers, diagnostics))
    })
    .unwrap()
}
#[test]
fn accepted_full_system_actions_preserve_rank_anchor_and_all_ordinary_results() {
    for weight in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Probability),
    ] {
        let (sample, columns) = fixture(weight, true);
        let raw = columns.generate(&sample).unwrap();
        for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
            let mut baseline = None;
            for workers in [1, 2, 4, 8] {
                let team = ThreadTeam::new(workers).unwrap();
                let options = Options {
                    solver,
                    ..Options::for_estimator(Estimator::Ppml)
                };
                let fit = irls::fit_final_sample(
                    &irls::Problem {
                        sample: &sample,
                        raw: &raw,
                        roles: &columns.roles,
                    },
                    options,
                    &team,
                )
                .unwrap();
                assert_eq!(fit.options(), options);
                let stats = statistics::ppml(&fit).unwrap();
                let v = inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team)
                    .unwrap()
                    .covariance
                    .values;
                let result = actions(&sample, &raw, &fit, &columns, &team);
                assert_eq!(stats, statistics::ppml(&fit).unwrap());
                assert_eq!(
                    v,
                    inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team)
                        .unwrap()
                        .covariance
                        .values
                );
                if let Some(ref previous) = baseline {
                    assert_eq!(&result, previous);
                } else {
                    baseline = Some(result);
                }
            }
        }
    }
}
#[test]
fn failed_or_interrupted_actions_publish_neither_destination() {
    let (sample, columns) = fixture(None, false);
    let raw = columns.generate(&sample).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = irls::fit_final_sample(
        &irls::Problem {
            sample: &sample,
            raw: &raw,
            roles: &columns.roles,
        },
        Options::for_estimator(Estimator::Ppml),
        &team,
    )
    .unwrap();
    bias::with_information(&fit, &columns, &team, &mut || Ok(()), |info, _| {
        let mut prediction = vec![42.; sample.n_active()];
        let mut coefficients = vec![43.; info.dimension()];
        let mut target = response(&sample, &raw, 0.17);
        target[0] = f64::NAN;
        let error = info
            .solve(
                &target,
                &mut prediction,
                Some(&mut coefficients),
                &mut || Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(info.diagnostics().solves, 0);
        assert_eq!(prediction, vec![42.; sample.n_active()]);
        assert_eq!(coefficients, vec![43.; info.dimension()]);
        target[0] = response(&sample, &raw, 0.17)[0];
        for stop in [1, 2] {
            let mut visits = 0;
            let error = info
                .solve(
                    &target,
                    &mut prediction,
                    Some(&mut coefficients),
                    &mut || {
                        visits += 1;
                        if visits == stop {
                            Err(FeregError::new(ErrorCode::Interrupted, "break"))
                        } else {
                            Ok(())
                        }
                    },
                )
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::Interrupted);
            assert_eq!(prediction, vec![42.; sample.n_active()]);
            assert_eq!(coefficients, vec![43.; info.dimension()]);
        }
        assert_eq!(info.diagnostics().solves, 1);
        assert!(
            info.diagnostics()
                .work
                .projection()
                .counts()
                .right_hand_sides
                >= 4
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn stale_or_malformed_finite_metadata_has_no_information_authority() {
    let (sample, columns) = fixture(None, false);
    let raw = columns.generate(&sample).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = irls::fit_final_sample(
        &irls::Problem {
            sample: &sample,
            raw: &raw,
            roles: &columns.roles,
        },
        Options::for_estimator(Estimator::Ppml),
        &team,
    )
    .unwrap();
    let mut stale = columns.clone();
    stale.recipes[0] = ColumnRecipe::Continuous { source: 0 };
    let error =
        bias::with_information(&fit, &stale, &team, &mut || Ok(()), |_, _| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported);
    stale = columns.clone();
    stale.constant_slot = Some(999);
    let error =
        bias::with_information(&fit, &stale, &team, &mut || Ok(()), |_, _| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
}
