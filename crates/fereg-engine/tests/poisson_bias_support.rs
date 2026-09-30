use fereg_engine::{
    data::*,
    ppml::{bias, irls},
    spec::{Estimator, Options},
    statistics,
    threads::ThreadTeam,
    wls::Role,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode, FeregError, ModelSpec, SourceEntry,
    SourceId,
};
mod support;
fn frame<'a>(
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
fn fixture() -> (Prepared, Vec<f64>) {
    let mut sample = support::prepared(&[2, 2], false, 4, 0);
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![9., 1., 9., 1., 1., 9., 1., 9.],
    };
    let compact = [1., 2., 1., 2., 2., 1., 2., 1.];
    let n = sample.n_active();
    sample.n_original = 2 * n + 3;
    sample.n_candidate = 2 * n + 2;
    sample.original_row_by_active = (0..n).map(|i| 2 * i + 1).collect();
    sample.drop_reasons = vec![DropReason::MissingOutcome; sample.n_original];
    sample.drop_reasons[0] = DropReason::ExcludedByIfIn;
    let mut units = vec![f64::NAN; sample.n_original];
    for (i, &row) in sample.original_row_by_active.iter().enumerate() {
        sample.drop_reasons[row] = DropReason::Retained;
        units[row] = compact[i];
    }
    (sample, units)
}
#[test]
fn accepted_information_support_and_original_axis_contracts() {
    let (sample, units) = fixture();
    let raw = DenseMatrix::from_columns(&[vec![1.; sample.n_active()]]).unwrap();
    let roles = [Role::AbsorbedIntercept];
    let mut baseline = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let fit = frame(&sample, &raw, &roles, &team);
        let before = statistics::ppml(&fit).unwrap();
        let mut missing = vec![true; sample.n_original];
        for &row in sample.retained_rows() {
            missing[row] = false;
        }
        let got = bias::screen(
            &fit,
            NumericColumnView::with_missing(&units, &missing),
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(got.identities(), &[1., 2.]);
        assert_eq!(got.minimum_unit_rows(), 4);
        assert_eq!(got.maximum_unit_rows(), 4);
        for summary in got.effects {
            assert_eq!(&summary.values()[..8], &[2., 2., 2., 2., 2., 2., 0., 0.]);
            assert!((summary.minimum_information_units - 100. / 82.).abs() < 1e-14);
            assert!((summary.median_information_units - 100. / 82.).abs() < 1e-14);
            assert!((summary.maximum_unit_information_share - 0.9).abs() < 1e-14);
        }
        assert_eq!(before, statistics::ppml(&fit).unwrap());
        if let Some(ref previous) = baseline {
            assert_eq!(&got, previous);
        } else {
            baseline = Some(got);
        }
        missing[sample.retained_rows()[0]] = true;
        assert_eq!(
            bias::screen(
                &fit,
                NumericColumnView::with_missing(&units, &missing),
                &mut || Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            bias::screen(
                &fit,
                NumericColumnView::new(&units[..units.len() - 1]),
                &mut || Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );
    }
}
#[test]
fn frequency_copies_and_signed_zero_do_not_manufacture_independent_units() {
    let (sample, units) = fixture();
    let raw = DenseMatrix::from_columns(&[vec![1.; sample.n_active()]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = frame(&sample, &raw, &[Role::AbsorbedIntercept], &team);
    let mut aligned = units.clone();
    for (i, &row) in sample.retained_rows().iter().enumerate() {
        aligned[row] = (i % 2) as f64;
    }
    let error = bias::screen(&fit, NumericColumnView::new(&aligned), &mut || Ok(())).unwrap_err();
    assert!(error.message.contains("FE1: 2/2 and FE2: 2/2"));
    let mut zeros = units.clone();
    for (i, &row) in sample.retained_rows().iter().enumerate() {
        zeros[row] = if units[row] == 1. {
            if i % 2 == 0 {
                -0.
            } else {
                0.
            }
        } else {
            2.
        };
    }
    let got = bias::screen(&fit, NumericColumnView::new(&zeros), &mut || Ok(())).unwrap();
    assert_eq!(
        got.identities()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        vec![0, 2_f64.to_bits()]
    );
    assert_eq!(got.effects[0].minimum_units, 2);
    let all_zero = vec![-0.; sample.n_original];
    assert_eq!(
        bias::screen(&fit, NumericColumnView::new(&all_zero), &mut || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InsufficientObservations
    );
    let mut invalid = units;
    invalid[sample.retained_rows()[1]] = f64::INFINITY;
    assert_eq!(
        bias::screen(&fit, NumericColumnView::new(&invalid), &mut || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
}
#[test]
fn unsupported_topologies_and_every_separation_reason_refuse_support() {
    let team = ThreadTeam::new(1).unwrap();
    for (levels, slopes) in [
        (&[][..], false),
        (&[2][..], false),
        (&[2, 3, 2][..], false),
        (&[2, 3][..], true),
    ] {
        let sample = support::prepared(levels, slopes, 20, 0);
        let raw = DenseMatrix::from_columns(&[vec![1.; sample.n_active()]]).unwrap();
        let role = if levels.is_empty() {
            Role::Ordinary
        } else {
            Role::AbsorbedIntercept
        };
        let roles = [role];
        let fit = frame(&sample, &raw, &roles, &team);
        let units: Vec<_> = (0..sample.n_original).map(|i| i as f64).collect();
        assert_eq!(
            bias::screen(&fit, NumericColumnView::new(&units), &mut || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::Unsupported
        );
    }
    for reason in [
        DropReason::FixedEffectSeparation,
        DropReason::SimplexSeparation,
        DropReason::IteratedRectifierSeparation,
        DropReason::MuSeparation,
    ] {
        let (mut sample, units) = fixture();
        sample.drop_reasons[0] = reason;
        sample.n_candidate += 1;
        let raw = DenseMatrix::from_columns(&[vec![1.; sample.n_active()]]).unwrap();
        let fit = frame(&sample, &raw, &[Role::AbsorbedIntercept], &team);
        let error = bias::screen(&fit, NumericColumnView::new(&units), &mut || Ok(())).unwrap_err();
        assert!(error.message.contains("outcome-dependent"));
    }
}
#[test]
fn interruption_at_each_support_boundary_returns_no_partial_result() {
    let (sample, units) = fixture();
    let raw = DenseMatrix::from_columns(&[vec![1.; sample.n_active()]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let fit = frame(&sample, &raw, &[Role::AbsorbedIntercept], &team);
    let mut boundaries = 0;
    bias::screen(&fit, NumericColumnView::new(&units), &mut || {
        boundaries += 1;
        Ok(())
    })
    .unwrap();
    let before = statistics::ppml(&fit).unwrap();
    for stop in 1..=boundaries {
        let mut visits = 0;
        let error = bias::screen(&fit, NumericColumnView::new(&units), &mut || {
            visits += 1;
            if visits == stop {
                Err(FeregError::new(ErrorCode::Interrupted, "break"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Interrupted);
        assert_eq!(before, statistics::ppml(&fit).unwrap());
    }
}
#[test]
fn literal_frequency_replication_preserves_support_and_information_concentration() {
    let (sample, units) = fixture();
    let PreparedWeights::Provided {
        values: frequency, ..
    } = &sample.weights
    else {
        unreachable!()
    };
    let mut expanded = support::prepared(&[2, 2], false, 20, 0);
    expanded.regressors = sample
        .regressors
        .iter()
        .map(|column| {
            column
                .iter()
                .zip(frequency)
                .flat_map(|(&value, &copies)| std::iter::repeat_n(value, copies as usize))
                .collect()
        })
        .collect();
    let mut expanded_units: Vec<_> = sample
        .retained_rows()
        .iter()
        .zip(frequency)
        .flat_map(|(&row, &copies)| std::iter::repeat_n(units[row], copies as usize))
        .collect();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=2)
                .map(|i| SourceEntry {
                    id: SourceId::new(i).unwrap(),
                    canonical_name: format!("g{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=2)
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i).unwrap(),
                categorical_sources: vec![SourceId::new(i).unwrap()],
                intercept: true,
                slope_sources: vec![],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    expanded = fereg_engine::topology::prepare_fixed_effects(&model, expanded).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; sample.n_active()]]).unwrap();
    let expanded_raw = DenseMatrix::from_columns(&[vec![1.; expanded.n_active()]]).unwrap();
    let a = frame(&sample, &raw, &[Role::AbsorbedIntercept], &team);
    let b = frame(&expanded, &expanded_raw, &[Role::AbsorbedIntercept], &team);
    let left = bias::screen(&a, NumericColumnView::new(&units), &mut || Ok(())).unwrap();
    let right = bias::screen(&b, NumericColumnView::new(&expanded_units), &mut || Ok(())).unwrap();
    assert_eq!(left.identities(), right.identities());
    for (a, b) in left.effects.iter().zip(right.effects) {
        for (a, b) in a.values().into_iter().zip(b.values()) {
            assert!((a - b).abs() < 1e-14);
        }
    }
    assert_eq!(right.minimum_unit_rows(), 4);
    assert_eq!(right.maximum_unit_rows(), 36);
    drop(b);
    for source in &mut expanded.regressors {
        source.reverse();
    }
    expanded_units.reverse();
    expanded = fereg_engine::topology::prepare_fixed_effects(&model, expanded).unwrap();
    let reversed = frame(&expanded, &expanded_raw, &[Role::AbsorbedIntercept], &team);
    let reversed_support = bias::screen(
        &reversed,
        NumericColumnView::new(&expanded_units),
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(right.effects, reversed_support.effects);
    assert_eq!(right.identities(), reversed_support.identities());
    assert_eq!(
        right.minimum_unit_rows(),
        reversed_support.minimum_unit_rows()
    );
    assert_eq!(
        right.maximum_unit_rows(),
        reversed_support.maximum_unit_rows()
    );
}
