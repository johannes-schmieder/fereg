use fereg_engine::{
    data::*,
    fe::Design,
    outputs::{self, Kind, Outputs, PpmlRequest, Request},
    ppml::{
        inference, irls,
        residuals::{self, ResidualKind},
        separation,
    },
    spec::{Estimator, Options},
    statistics,
    threads::ThreadTeam,
    vce,
    wls::{Role, Status},
    ErrorCode,
};
mod support;
fn fixture(
    levels: &[usize],
    slopes: bool,
    weight: Option<WeightKind>,
    exposure: bool,
) -> (Prepared, CompiledColumns) {
    let mut sample = support::prepared(levels, slopes, 30, 1);
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
    sample.n_original = 2 * n + 3;
    sample.n_candidate = 2 * n + 2;
    sample.original_row_by_active = (0..n).map(|i| 2 * i + 1).collect();
    sample.drop_reasons = vec![DropReason::MissingOutcome; 2 * n + 3];
    sample.drop_reasons[0] = DropReason::ExcludedByIfIn;
    for &row in &sample.original_row_by_active {
        sample.drop_reasons[row] = DropReason::Retained;
    }
    let columns = CompiledColumns {
        slots: vec![
            ColumnSlot {
                name: "x".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(0),
            },
            ColumnSlot {
                name: "_cons".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(1),
            },
        ],
        recipes: vec![ColumnRecipe::Continuous { source }, ColumnRecipe::Intercept],
        roles: vec![
            Role::Ordinary,
            if levels.is_empty() {
                Role::Ordinary
            } else {
                Role::AbsorbedIntercept
            },
        ],
        constant_slot: Some(1),
        structural_intercept: true,
        implicit_constant: false,
    };
    (sample, columns)
}
fn run<'a>(
    sample: &'a Prepared,
    raw: &'a DenseMatrix,
    roles: &'a [Role],
    team: &ThreadTeam,
) -> irls::Fit<'a> {
    irls::fit_final_sample(
        &irls::Problem { sample, raw, roles },
        Options::for_estimator(Estimator::Ppml),
        team,
    )
    .unwrap()
}
fn full(sample: &Prepared) -> PpmlRequest {
    PpmlRequest {
        common: Request {
            residuals: true,
            xb: true,
            xbd: true,
            absorbed: true,
            drop_reason: true,
            saved: sample
                .topology
                .as_ref()
                .unwrap()
                .effects()
                .iter()
                .enumerate()
                .flat_map(|(i, e)| {
                    (0..e.slopes.len() + usize::from(e.intercept)).map(move |j| (i, j))
                })
                .collect(),
        },
        mean: true,
        residual_type: None,
    }
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
fn identical(a: &Outputs, b: &Outputs) {
    assert_eq!(a.certificate, b.certificate);
    assert_eq!(a.columns.len(), b.columns.len());
    for (a, b) in a.columns.iter().zip(&b.columns) {
        assert_eq!(a.kind, b.kind);
        assert_eq!(
            a.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
}
#[test]
fn disconnected_sloped_saved_effects_use_the_accepted_hessian_gauge_and_original_axis() {
    let (sample, columns) = fixture(&[4, 6, 4], true, Some(WeightKind::Probability), true);
    let raw = columns.generate(&sample).unwrap();
    let one = ThreadTeam::new(1).unwrap();
    let fit = run(&sample, &raw, &columns.roles, &one);
    let request = full(&sample);
    let output = outputs::ppml(&fit, &request, 1e-8, &one).unwrap();
    let design = Design::new(&sample, Weights::Provided(&fit.hessian_weights)).unwrap();
    let absorbed = design.multiply(&fit.point.absorbed, &one).unwrap();
    let mut prior_centering_differs = false;
    for (i, &row) in sample.retained_rows().iter().enumerate() {
        let PreparedAdjustment::Exposure(ref v) = sample.adjustment else {
            unreachable!()
        };
        close(
            output.get(Kind::Xb).unwrap()[row],
            raw[(i, 0)] * fit.point.finite[0] + v[i].ln(),
        );
        assert_eq!(output.get(Kind::Xbd).unwrap()[row], fit.point.eta[i]);
        assert_eq!(output.get(Kind::Mean).unwrap()[row], fit.evaluation.mean[i]);
        assert_eq!(output.get(Kind::Absorbed).unwrap()[row], absorbed[(i, 0)]);
        close(
            output.get(Kind::Xb).unwrap()[row] + output.get(Kind::Absorbed).unwrap()[row],
            fit.point.eta[i],
        );
        let sum: f64 = request
            .common
            .saved
            .iter()
            .map(|&(effect, channel)| output.get(Kind::Saved { effect, channel }).unwrap()[row])
            .sum();
        close(sum, absorbed[(i, 0)]);
        for &(effect, channel) in &request.common.saved {
            let level = design.topology().effect_partition(effect).unwrap().codes()[i] as usize;
            let coefficient =
                fit.point.absorbed[(design.coefficient_index(effect, channel, level).unwrap(), 0)];
            close(
                output.get(Kind::Saved { effect, channel }).unwrap()[row],
                coefficient * design.basis_value(effect, channel, i).unwrap(),
            );
        }
    }
    let q = ReferenceWeights::new(&sample.weights, sample.n_active(), Estimator::Ppml).unwrap();
    for effect in [1, 2] {
        for component in 0..2 {
            let saved = output.get(Kind::Saved { effect, channel: 0 }).unwrap();
            close(
                (0..sample.n_active())
                    .filter(|i| i % 2 == component)
                    .map(|i| fit.hessian_weights[i] * saved[sample.retained_rows()[i]])
                    .sum(),
                0.,
            );
            prior_centering_differs |= (0..sample.n_active())
                .filter(|i| i % 2 == component)
                .map(|i| q.value(i) * saved[sample.retained_rows()[i]])
                .sum::<f64>()
                .abs()
                > 1e-5;
        }
    }
    assert!(
        prior_centering_differs,
        "fixture must distinguish q from qmu normalization"
    );
    for row in 0..sample.n_original() {
        for column in &output.columns {
            if column.kind == Kind::DropReason {
                assert_eq!(
                    column.values[row],
                    f64::from(sample.drop_reason(row).unwrap().code())
                );
            } else if !sample.is_retained(row) {
                assert!(column.values[row].is_nan());
            }
        }
    }
    for workers in [2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let repeated = run(&sample, &raw, &columns.roles, &team);
        identical(
            &output,
            &outputs::ppml(&repeated, &request, 1e-8, &team).unwrap(),
        );
    }
}
#[test]
fn selection_creates_only_requested_banks_and_never_changes_fit_inference_or_statistics() {
    let (sample, columns) = fixture(&[3, 4], true, Some(WeightKind::Frequency), false);
    let raw = columns.generate(&sample).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = run(&sample, &raw, &columns.roles, &team);
    let stats = statistics::ppml(&fit).unwrap();
    let covariance = inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team)
        .unwrap()
        .covariance
        .values;
    let request = full(&sample);
    let output = outputs::ppml(&fit, &request, 1e-8, &team).unwrap();
    let none = outputs::ppml(&fit, &PpmlRequest::default(), 1e-8, &team).unwrap();
    assert!(none.columns.is_empty());
    assert!(none.certificate.is_none());
    let drops = outputs::ppml(
        &fit,
        &PpmlRequest {
            common: Request {
                drop_reason: true,
                ..Request::default()
            },
            ..PpmlRequest::default()
        },
        1e-8,
        &team,
    )
    .unwrap();
    assert_eq!(drops.columns.len(), 1);
    assert!(drops.certificate.is_none());
    for mask in 1..32 {
        let selected = PpmlRequest {
            common: Request {
                residuals: mask & 1 != 0,
                xb: mask & 2 != 0,
                xbd: mask & 4 != 0,
                absorbed: mask & 8 != 0,
                ..Request::default()
            },
            mean: mask & 16 != 0,
            residual_type: None,
        };
        let bank = outputs::ppml(&fit, &selected, 1e-8, &team).unwrap();
        assert_eq!(bank.columns.len(), (mask as u32).count_ones() as usize);
        for column in bank.columns {
            assert_eq!(
                column
                    .values
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                output
                    .get(column.kind)
                    .unwrap()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>()
            );
        }
    }
    let saved = outputs::ppml(
        &fit,
        &PpmlRequest {
            common: Request {
                saved: vec![(1, 1)],
                ..Request::default()
            },
            ..PpmlRequest::default()
        },
        1e-8,
        &team,
    )
    .unwrap();
    assert_eq!(saved.columns.len(), 1);
    assert_eq!(
        saved.columns[0].kind,
        Kind::Saved {
            effect: 1,
            channel: 1
        }
    );
    assert_eq!(stats, statistics::ppml(&fit).unwrap());
    assert_eq!(
        covariance,
        inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team)
            .unwrap()
            .covariance
            .values
    );
}
#[test]
fn no_fe_predictions_preserve_the_finite_constant_and_include_offset_or_logged_exposure() {
    for exposure in [false, true] {
        let (sample, columns) = fixture(&[], false, None, exposure);
        let raw = columns.generate(&sample).unwrap();
        let team = ThreadTeam::new(1).unwrap();
        let fit = run(&sample, &raw, &columns.roles, &team);
        let output = outputs::ppml(&fit, &full(&sample), 1e-8, &team).unwrap();
        assert_eq!(fit.linear.finite.statuses[1], Status::Kept);
        assert!(fit.point.finite[1].abs() > 0.01);
        for (i, &row) in sample.retained_rows().iter().enumerate() {
            let adjustment = match &sample.adjustment {
                PreparedAdjustment::Offset(v) => v[i],
                PreparedAdjustment::Exposure(v) => v[i].ln(),
                _ => 0.,
            };
            close(
                output.get(Kind::Xb).unwrap()[row],
                fit.point.finite[0] * raw[(i, 0)] + fit.point.finite[1] + adjustment,
            );
            assert_eq!(output.get(Kind::Absorbed).unwrap()[row], 0.);
            close(
                output.get(Kind::Xbd).unwrap()[row],
                output.get(Kind::Xb).unwrap()[row],
            );
            assert_eq!(
                output.get(Kind::Mean).unwrap()[row],
                output.get(Kind::Xbd).unwrap()[row].exp()
            );
        }
    }
}
#[test]
fn residual_definitions_cover_zero_fractional_and_near_saturated_outcomes_without_weights() {
    let y = [0_f64, 1., 4.];
    let mu = [0.5_f64, 1.5, 3.];
    for kind in [
        ResidualKind::Response,
        ResidualKind::Score,
        ResidualKind::Pearson,
        ResidualKind::Working,
        ResidualKind::Deviance,
        ResidualKind::Anscombe,
    ] {
        for i in 0..3 {
            let expected = match kind {
                ResidualKind::Response | ResidualKind::Score => y[i] - mu[i],
                ResidualKind::Pearson => (y[i] - mu[i]) / mu[i].sqrt(),
                ResidualKind::Working => (y[i] - mu[i]) / mu[i],
                ResidualKind::Deviance => {
                    if y[i] == 0. {
                        2. * mu[i]
                    } else {
                        2. * (mu[i] - y[i] + y[i] * (y[i] / mu[i]).ln())
                    }
                }
                ResidualKind::Anscombe => {
                    1.5 * (y[i].powf(2. / 3.) - mu[i].powf(2. / 3.)) / mu[i].powf(1. / 6.)
                }
            };
            close(residuals::value(y[i], mu[i], kind).unwrap(), expected);
        }
    }
    let y = f64::from_bits(1_f64.to_bits() + 1);
    assert!(residuals::value(y, 1., ResidualKind::Deviance).unwrap() > 0.);
    assert_eq!(
        residuals::value(-1., 1., ResidualKind::Response)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    assert_eq!(
        residuals::value(1., 0., ResidualKind::Response)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    assert_eq!(
        residuals::value(f64::MAX, f64::MIN_POSITIVE, ResidualKind::Working)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn invalid_requests_fail_before_any_output_bank_is_returned() {
    let (sample, columns) = fixture(&[3], false, None, false);
    let raw = columns.generate(&sample).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = run(&sample, &raw, &columns.roles, &team);
    for saved in [vec![(0, 0), (0, 0)], vec![(1, 0)], vec![(0, 1)]] {
        assert_eq!(
            outputs::ppml(
                &fit,
                &PpmlRequest {
                    common: Request {
                        saved,
                        ..Request::default()
                    },
                    ..PpmlRequest::default()
                },
                1e-8,
                &team
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );
    }
    assert_eq!(
        outputs::ppml(
            &fit,
            &PpmlRequest {
                residual_type: Some(ResidualKind::Pearson),
                ..PpmlRequest::default()
            },
            1e-8,
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );
    for tolerance in [0., 1., f64::NAN] {
        assert!(outputs::ppml(&fit, &PpmlRequest::default(), tolerance, &team).is_err());
    }
}
#[test]
fn separation_outputs_use_the_final_generation_and_leave_boundary_rows_missing() {
    let (mut sample, columns) = fixture(&[], false, None, false);
    sample.adjustment = PreparedAdjustment::None;
    sample.regressors[0].fill(0.);
    sample.regressors[0][0] = -1.;
    sample.outcome[0] = 0.;
    let removed = sample.retained_rows()[0];
    let team = ThreadTeam::new(1).unwrap();
    let final_sample = separation::run(
        sample,
        &columns,
        Options::for_estimator(Estimator::Ppml),
        separation::Policy {
            drop_singletons: false,
            methods: separation::Methods::default(),
        },
        separation::RunControls::default(),
        &team,
    )
    .unwrap();
    assert_eq!(final_sample.report().generation, 1);
    let fit = final_sample.fit(&team).unwrap();
    let output = outputs::ppml(&fit, &full(final_sample.sample()), 1e-8, &team).unwrap();
    assert_eq!(output.get(Kind::DropReason).unwrap()[removed], 10.);
    for column in &output.columns {
        if column.kind != Kind::DropReason {
            assert!(column.values[removed].is_nan());
        }
    }
    let expected =
        final_sample.sample().outcome.iter().sum::<f64>() / final_sample.sample().n_active() as f64;
    for &row in final_sample.sample().retained_rows() {
        close(output.get(Kind::Mean).unwrap()[row], expected);
    }
    let report = inference::estimate(&columns, &fit, &[], vce::Kind::Robust, 1e-8, &team).unwrap();
    assert_eq!(report.coordinates, vec![1]);
    assert_eq!(report.statuses[0], Status::NumericallyRankDeficient);
}
