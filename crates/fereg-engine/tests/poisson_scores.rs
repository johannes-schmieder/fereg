use fereg_engine::{
    data::*,
    fe::{Design, Weights},
    ppml::family::Family,
    spec::Estimator,
    threads::ThreadTeam,
    ErrorCode,
};
mod support;
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() < 3e-14 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
#[test]
fn finite_and_fe_scores_are_separate_and_zero_directions_remain_in_rms() {
    let sample = support::slope_sample(true, &[0, 0, 1, 1], &[]);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&PreparedWeights::None, 4, Estimator::Ppml).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4], vec![0.; 4]]).unwrap();
    let family = Family::new(&[1., 1., 3., 3.], q, None).unwrap();
    let team = ThreadTeam::new(2).unwrap();
    let cert = family
        .score_certificate(&[2_f64.ln(); 4], &raw, &fe, 1e-8, &team)
        .unwrap();
    assert!(!cert.passed);
    assert_eq!(cert.finite.relative_max, 0.);
    close(cert.scale, 10_f64.sqrt());
    close(cert.fixed_effects.absolute_max, 2.);
    close(cert.fixed_effects.relative_max, 1. / 10_f64.sqrt());
    close(cert.relative_rms, 1. / 20_f64.sqrt());
    assert_eq!(cert.finite.directions, 2);
    assert_eq!(cert.fixed_effects.directions, 2);
    assert_eq!(
        cert.convergence_scores().fixed_effects,
        cert.fixed_effects.relative_max
    );
    let family = Family::new(&[1., 3., 1., 3.], q, None).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4], vec![-1., 1., -1., 1.]]).unwrap();
    let cert = family
        .score_certificate(&[2_f64.ln(); 4], &raw, &fe, 1e-8, &team)
        .unwrap();
    assert!(!cert.passed);
    assert_eq!(cert.fixed_effects.relative_max, 0.);
    close(cert.finite.absolute_max, 4.);
    close(cert.finite.relative_max, 4. / 80_f64.sqrt());
}
#[test]
fn exact_fit_keeps_the_outcome_reference_scale_without_a_floor() {
    let sample = support::prepared(&[], false, 1, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&PreparedWeights::None, 4, Estimator::Ppml).unwrap();
    let family = Family::new(&[1.; 4], q, None).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let cert = family
        .score_certificate(&[0.; 4], &raw, &fe, 1e-12, &ThreadTeam::new(1).unwrap())
        .unwrap();
    assert!(cert.passed);
    assert_eq!(cert.scale, 2.);
    assert_eq!(cert.relative_rms, 0.);
    assert_eq!(cert.fixed_effects.directions, 0);
}
#[test]
fn fresh_predictors_and_original_columns_detect_false_cached_convergence() {
    let sample = support::prepared(&[2], true, 2, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&PreparedWeights::None, 4, Estimator::Ppml).unwrap();
    let family = Family::new(&[1.; 4], q, None).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4], vec![1., 0., 0., 0.]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let good = family
        .score_certificate(&[0.; 4], &raw, &fe, 1e-12, &team)
        .unwrap();
    let bad = family
        .score_certificate(&[0.1, 0., 0., 0.], &raw, &fe, 1e-12, &team)
        .unwrap();
    assert!(good.passed && !bad.passed);
    assert!(bad.finite.relative_max > 1e-3 && bad.fixed_effects.relative_max > 1e-3);
    // A stale numerical FE weight frame cannot supply the Poisson information weights.
    let other = Design::new(&sample, Weights::Provided(&[7., 2., 1., 9.])).unwrap();
    assert_eq!(
        bad,
        family
            .score_certificate(&[0.1, 0., 0., 0.], &raw, &other, 1e-12, &team)
            .unwrap()
    );
    assert_eq!(
        good,
        family
            .score_certificate(&[0.; 4], &raw, &fe, 1e-12, &team)
            .unwrap()
    );
}
#[test]
fn scores_are_identical_across_workers_with_many_levels_and_generated_columns() {
    let n = 4103;
    let codes: Vec<_> = (0..n).map(|i| (i % 11) as u32).collect();
    let slope: Vec<_> = (0..n).map(|i| (i % 13) as f64 - 6.).collect();
    let sample = support::slope_sample(true, &codes, &[slope.clone(), vec![0.; n]]);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::from_columns(&[
        vec![1.; n],
        slope,
        (0..n).map(|i| f64::from(i % 3 == 0)).collect(),
    ])
    .unwrap();
    let y: Vec<_> = (0..n).map(|i| (i % 7) as f64 / 2.).collect();
    let eta: Vec<_> = (0..n).map(|i| ((i % 5) as f64 - 2.) * 0.2).collect();
    for kind in [
        None,
        Some(WeightKind::Frequency),
        Some(WeightKind::Probability),
    ] {
        let raw_weights = kind.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
            kind,
            values: (0..n).map(|i| (1 + i % 4) as f64).collect(),
        });
        let q = ReferenceWeights::new(&raw_weights, n, Estimator::Ppml).unwrap();
        let family = Family::new(&y, q, None).unwrap();
        let mut baseline = None;
        for workers in [1, 2, 4, 8] {
            let actual = family
                .score_certificate(&eta, &raw, &fe, 1e-8, &ThreadTeam::new(workers).unwrap())
                .unwrap();
            assert_eq!(*baseline.get_or_insert(actual), actual);
        }
    }
}
#[test]
fn scaled_norms_handle_tiny_and_huge_outcomes_without_false_zero_scores() {
    let sample = support::prepared(&[], false, 1, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&PreparedWeights::None, 4, Estimator::Ppml).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    for mu in [1e-300_f64, 1., 1e300] {
        let y = [2. * mu; 4];
        let family = Family::new(&y, q, None).unwrap();
        let cert = family
            .score_certificate(&[mu.ln(); 4], &raw, &fe, 1e-8, &team)
            .unwrap();
        assert!(!cert.passed);
        assert!((cert.finite.relative_max - 0.5).abs() < 1e-13);
    }
    let family = Family::new(&[2e-300; 4], q, None).unwrap();
    let tiny = DenseMatrix::from_columns(&[vec![1e-100; 4]]).unwrap();
    assert_eq!(
        family
            .score_certificate(&[1e-300_f64.ln(); 4], &tiny, &fe, 1e-8, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn malformed_and_nonrepresentable_frames_fail_closed() {
    let sample = support::prepared(&[], false, 1, 0);
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let q = ReferenceWeights::new(&PreparedWeights::None, 4, Estimator::Ppml).unwrap();
    let family = Family::new(&[1.; 4], q, None).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    for tolerance in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(family
            .score_certificate(&[0.; 4], &raw, &fe, tolerance, &team)
            .is_err());
    }
    for eta in [f64::NAN, 1000., -1000.] {
        assert!(family
            .score_certificate(&[eta; 4], &raw, &fe, 1e-8, &team)
            .is_err());
    }
    let mut bad = raw.clone();
    bad[(0, 0)] = f64::NAN;
    assert!(family
        .score_certificate(&[0.; 4], &bad, &fe, 1e-8, &team)
        .is_err());
    assert!(family
        .score_certificate(&[0.; 3], &raw, &fe, 1e-8, &team)
        .is_err());
    assert!(family
        .score_certificate(
            &[0.; 4],
            &DenseMatrix::zeros(3, 0).unwrap(),
            &fe,
            1e-8,
            &team
        )
        .is_err());
}
