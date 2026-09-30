use fereg_engine::{
    data::*,
    factor::CategoricalCodeView,
    ppml::{
        inference::{self, Inference},
        irls,
    },
    spec::{Estimator, Options},
    threads::ThreadTeam,
    vce::Kind,
    wls::{Role, Status},
    ErrorCode,
};
mod support;
fn columns(source: usize, absorbed: bool) -> CompiledColumns {
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
        roles: vec![
            Role::Ordinary,
            Role::Ordinary,
            if absorbed {
                Role::AbsorbedIntercept
            } else {
                Role::Ordinary
            },
        ],
        constant_slot: Some(4),
        implicit_constant: false,
        structural_intercept: true,
    }
}
fn fixture(effects: bool, slope_only: bool) -> (Prepared, CompiledColumns) {
    let mut sample = if slope_only {
        let codes = (0..120).map(|i| (i % 2) as u32).collect::<Vec<_>>();
        let slope = (0..120).map(|i| (i % 7) as f64 * 0.2 - 0.4).collect();
        support::slope_sample(false, &codes, &[slope])
    } else {
        support::prepared(if effects { &[2] } else { &[] }, false, 30, 0)
    };
    let n = sample.n_active();
    let source = sample.regressors.len();
    sample
        .regressors
        .push((0..n).map(|i| (i as f64 * 1.7).sin()).collect());
    sample.outcome = (0..n).map(|i| 0.4 + (i % 7) as f64 * 0.3).collect();
    sample.adjustment = PreparedAdjustment::Offset((0..n).map(|i| (i % 5) as f64 * 0.02).collect());
    (sample, columns(source, effects && !slope_only))
}
fn infer(
    sample: &Prepared,
    columns: &CompiledColumns,
    clusters: &[CategoricalCodeView<'_>],
    kind: Kind,
    workers: usize,
) -> Inference {
    let raw = columns.generate(sample).unwrap();
    let team = ThreadTeam::new(workers).unwrap();
    let fit = irls::fit_final_sample(
        &irls::Problem {
            sample,
            raw: &raw,
            roles: &columns.roles,
        },
        Options::for_estimator(Estimator::Ppml),
        &team,
    )
    .unwrap();
    inference::estimate(columns, &fit, clusters, kind, 1e-8, &team).unwrap()
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
fn identical(a: &Inference, b: &Inference) {
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a.coefficients), bits(&b.coefficients));
    assert_eq!(
        bits(a.covariance.values.as_slice()),
        bits(b.covariance.values.as_slice())
    );
    assert_eq!(
        bits(&a.covariance.standard_errors),
        bits(&b.covariance.standard_errors)
    );
    assert_eq!(bits(&a.z_statistics), bits(&b.z_statistics));
    assert_eq!(a.statuses, b.statuses);
    assert_eq!(a.coordinates, b.coordinates);
    assert_eq!(a.dof, b.dof);
    assert_eq!(a.absorbed_dof, b.absorbed_dof);
    assert_eq!(a.wald, b.wald);
    assert_eq!(a.covariance.psd, b.covariance.psd);
    assert_eq!(
        a.covariance.correction.to_bits(),
        b.covariance.correction.to_bits()
    );
    assert_eq!(
        a.covariance.reconstruction_error.to_bits(),
        b.covariance.reconstruction_error.to_bits()
    );
    assert_eq!(a.covariance.cluster_counts, b.covariance.cluster_counts);
    assert_eq!(a.covariance.subset_counts, b.covariance.subset_counts);
}
#[test]
fn constants_omissions_and_workers_preserve_canonical_covariance_axes() {
    for (effects, slope_only) in [(false, false), (true, false), (true, true)] {
        let (sample, columns) = fixture(effects, slope_only);
        let baseline = infer(&sample, &columns, &[], Kind::Robust, 1);
        assert_eq!(
            baseline.statuses[..4],
            [
                Status::Base,
                Status::Kept,
                Status::NumericallyRankDeficient,
                Status::ExplicitlyOmitted
            ]
        );
        assert_eq!(baseline.wald.tested, vec![1]);
        assert_eq!(baseline.wald.degrees_of_freedom, 1);
        close(
            baseline.wald.statistic.unwrap(),
            baseline.z_statistics[1].powi(2),
        );
        let normalized = effects && !slope_only;
        assert_eq!(
            baseline.coordinates,
            if normalized { vec![1] } else { vec![1, 4] }
        );
        if normalized {
            assert_eq!(baseline.coefficients[4], 0.);
            assert_eq!(baseline.statuses[4], Status::NormalizedAbsorbedIntercept);
        } else {
            assert_eq!(baseline.statuses[4], Status::Kept);
            assert!(baseline.coefficients[4].abs() > 0.01);
            assert!(baseline.covariance.standard_errors[4] > 0.);
            assert!(baseline.z_statistics[4].is_finite());
        }
        for slot in [0, 2, 3].into_iter().chain(normalized.then_some(4)) {
            assert_eq!(baseline.coefficients[slot], 0.);
            assert!(baseline.covariance.standard_errors[slot].is_nan());
            assert!(baseline.z_statistics[slot].is_nan());
            for j in 0..5 {
                assert_eq!(baseline.covariance.values[(slot, j)], 0.);
                assert_eq!(baseline.covariance.values[(j, slot)], 0.);
            }
        }
        assert_eq!(baseline.dof.finite, 2);
        for workers in [2, 4, 8] {
            identical(
                &baseline,
                &infer(&sample, &columns, &[], Kind::Robust, workers),
            );
        }
    }
}
#[test]
fn normalization_direction_and_nested_intercepts_are_charged_once() {
    let (sample, columns) = fixture(true, false);
    let robust = infer(&sample, &columns, &[], Kind::Robust, 1);
    assert_eq!(
        (robust.dof.finite, robust.dof.absorbed, robust.dof.nested),
        (2, 1, 0)
    );
    assert_eq!(robust.dof.structural, sample.n_active() - 3);
    close(
        robust.covariance.correction,
        sample.n_active() as f64 / (sample.n_active() - 1) as f64,
    );
    let codes = (0..sample.n_active())
        .map(|i| (i % 2) as u32)
        .collect::<Vec<_>>();
    let nested = infer(
        &sample,
        &columns,
        &[CategoricalCodeView::new(&codes, 2)],
        Kind::Cluster,
        1,
    );
    assert_eq!(
        (nested.dof.finite, nested.dof.absorbed, nested.dof.nested),
        (2, 0, 2)
    );
    assert_eq!(nested.dof.structural, sample.n_active() - 2);
    assert_eq!(nested.dof.reported, 1);
    assert_eq!(nested.covariance.correction, 2.);
    assert!(nested
        .absorbed_dof
        .effects
        .iter()
        .all(|e| e.nested && e.coefficients == 0));
}
#[test]
fn frequency_covariance_matches_literal_replication_for_robust_and_cluster() {
    let (mut sample, columns) = fixture(false, false);
    let n = sample.n_active();
    let frequencies = (0..n).map(|i| (1 + i % 3) as f64).collect::<Vec<_>>();
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: frequencies.clone(),
    };
    let indices = (0..n)
        .flat_map(|i| std::iter::repeat_n(i, frequencies[i] as usize))
        .collect::<Vec<_>>();
    let selected = |v: &[f64]| indices.iter().map(|&i| v[i]).collect::<Vec<_>>();
    let mut expanded = support::prepared(&[], false, indices.len() / 4, 0);
    expanded.n_original = indices.len();
    expanded.n_candidate = indices.len();
    expanded.original_row_by_active = (0..indices.len()).collect();
    expanded.outcome = selected(&sample.outcome);
    expanded.regressors = sample.regressors.iter().map(|v| selected(v)).collect();
    expanded.weights = PreparedWeights::None;
    let PreparedAdjustment::Offset(ref v) = sample.adjustment else {
        unreachable!()
    };
    expanded.adjustment = PreparedAdjustment::Offset(selected(v));
    expanded.drop_reasons = vec![DropReason::Retained; indices.len()];
    let codes = (0..n).map(|i| (i % 5) as u32).collect::<Vec<_>>();
    let repeated = indices.iter().map(|&i| codes[i]).collect::<Vec<_>>();
    for kind in [Kind::Robust, Kind::Cluster] {
        let a_views = vec![CategoricalCodeView::new(&codes, 5)];
        let b_views = vec![CategoricalCodeView::new(&repeated, 5)];
        let a = infer(
            &sample,
            &columns,
            if kind == Kind::Cluster { &a_views } else { &[] },
            kind,
            1,
        );
        let b = infer(
            &expanded,
            &columns,
            if kind == Kind::Cluster { &b_views } else { &[] },
            kind,
            1,
        );
        assert_eq!(a.dof, b.dof);
        for (a, b) in a.coefficients.iter().zip(&b.coefficients) {
            close(*a, *b);
        }
        for (a, b) in a
            .covariance
            .values
            .as_slice()
            .iter()
            .zip(b.covariance.values.as_slice())
        {
            close(*a, *b);
        }
    }
}
#[test]
fn probability_scaling_and_five_way_cluster_replay_preserve_inference() {
    let (mut sample, columns) = fixture(true, false);
    let n = sample.n_active();
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: (0..n).map(|i| (1 + i % 7) as f64).collect(),
    };
    let codes = (0..5)
        .map(|j| {
            (0..n)
                .map(|i| ((i / (j + 2) + j) % (j + 3)) as u32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for dimensions in 1..=5 {
        let views = codes[..dimensions]
            .iter()
            .enumerate()
            .map(|(j, c)| CategoricalCodeView::new(c, j + 3))
            .collect::<Vec<_>>();
        let a = infer(&sample, &columns, &views, Kind::Cluster, 1);
        assert_eq!(a.covariance.subset_counts.len(), (1 << dimensions) - 1);
        identical(&a, &infer(&sample, &columns, &views, Kind::Cluster, 8));
        let (mut scaled, _) = fixture(true, false);
        scaled.weights = PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: (0..n).map(|i| (1 + i % 7) as f64).collect(),
        };
        let PreparedWeights::Provided { ref mut values, .. } = scaled.weights else {
            unreachable!()
        };
        for value in values {
            *value *= 7.7;
        }
        let b = infer(&scaled, &columns, &views, Kind::Cluster, 1);
        for (a, b) in a
            .covariance
            .values
            .as_slice()
            .iter()
            .zip(b.covariance.values.as_slice())
        {
            close(*a, *b);
        }
    }
}
#[test]
fn malformed_report_metadata_and_cluster_frames_fail_closed() {
    let (sample, columns) = fixture(false, false);
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
    assert_eq!(
        inference::estimate(&columns, &fit, &[], Kind::Iid, 1e-8, &team)
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    for change in 0..7 {
        let mut invalid = columns.clone();
        match change {
            0 => invalid.constant_slot = None,
            1 => invalid.constant_slot = Some(usize::MAX),
            2 => invalid.roles[2] = Role::AbsorbedIntercept,
            3 => invalid.implicit_constant = true,
            4 => invalid.slots[1].generated = Some(1),
            5 => invalid.recipes[0] = ColumnRecipe::Intercept,
            _ => invalid.recipes[2] = ColumnRecipe::Continuous { source: 0 },
        }
        assert_eq!(
            inference::estimate(&invalid, &fit, &[], Kind::Robust, 1e-8, &team)
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
    }
    let n = sample.n_active();
    let singleton = vec![0; n];
    assert_eq!(
        inference::estimate(
            &columns,
            &fit,
            &[CategoricalCodeView::new(&singleton, 1)],
            Kind::Cluster,
            1e-8,
            &team
        )
        .unwrap_err()
        .code,
        ErrorCode::InsufficientObservations
    );
    let short = vec![0; n - 1];
    assert!(inference::estimate(
        &columns,
        &fit,
        &[CategoricalCodeView::new(&short, 1)],
        Kind::Cluster,
        1e-8,
        &team
    )
    .is_err());
    let bad = vec![3; n];
    assert!(inference::estimate(
        &columns,
        &fit,
        &[CategoricalCodeView::new(&bad, 2)],
        Kind::Cluster,
        1e-8,
        &team
    )
    .is_err());
    assert!(inference::estimate(&columns, &fit, &[], Kind::Cluster, 1e-8, &team).is_err());
    let a = infer(&sample, &columns, &[], Kind::Robust, 1);
    assert!(a.covariance.clone().scatter(5, &[4, 1, 3, 2, 0]).is_err());
}
#[test]
fn absorbed_intercept_only_has_no_score_coordinate_or_wald_test() {
    let (sample, mut columns) = fixture(true, false);
    columns.slots.drain(..4);
    columns.slots[0].generated = Some(0);
    columns.roles = vec![Role::AbsorbedIntercept];
    columns.recipes = vec![ColumnRecipe::Intercept];
    columns.constant_slot = Some(0);
    let report = infer(&sample, &columns, &[], Kind::Robust, 1);
    assert!(report.coordinates.is_empty());
    assert_eq!(report.coefficients, vec![0.]);
    assert_eq!(report.covariance.values.as_slice(), &[0.]);
    assert!(report.covariance.standard_errors[0].is_nan());
    assert!(report.z_statistics[0].is_nan());
    assert_eq!(report.wald.statistic, None);
    assert_eq!(report.wald.degrees_of_freedom, 0);
    assert_eq!((report.dof.finite, report.dof.absorbed), (1, 1));
}
