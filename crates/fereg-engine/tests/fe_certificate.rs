use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, Plan, SchurSession, Weights},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    ErrorCode,
};
mod support;

#[test]
fn weighted_certificate_norms_are_range_safe_across_fixed_partition_boundaries() {
    let sample = support::prepared(&[1], false, 8193, 0);
    let weights: Vec<_> = (0..sample.n_active())
        .map(|row| if row % 4 < 2 { 1. } else { 4. })
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    for scale in [1e-200, 1e200] {
        let values: Vec<_> = (0..8193)
            .map(|row| {
                if row == 8192 {
                    0.
                } else {
                    [3., -3., 4., -4.][row % 4] * scale
                }
            })
            .collect();
        let rhs = DenseMatrix::from_columns(&[values, vec![0.; 8193]]).unwrap();
        let coefficients = DenseMatrix::zeros(design.layout().total, rhs.ncols()).unwrap();
        let expected_norm = (146_f64 * 2048.).sqrt() * scale;
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let certificate = design
                .certify(
                    &rhs,
                    &coefficients,
                    &rhs,
                    1e-10,
                    &ThreadTeam::new(workers).unwrap(),
                )
                .unwrap();
            assert!((certificate.weighted_rhs_norm[0] / expected_norm - 1.).abs() < 1e-13);
            assert_eq!(certificate.weighted_rhs_norm[1], 0.);
            assert_eq!(
                certificate.weighted_residual_norm,
                certificate.weighted_rhs_norm
            );
            if let Some(expected) = &reference {
                assert_eq!(expected, &certificate);
            } else {
                reference = Some(certificate);
            }
        }
    }
}
#[test]
fn certificate_accepts_direct_and_schur_with_worker_identity() {
    for levels in [vec![7], vec![7, 5, 3]] {
        let sample = support::prepared(&levels, true, 30, 0);
        let weights: Vec<_> = (0..sample.n_active())
            .map(|row| 1. + (row % 3) as f64)
            .collect();
        let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
        let team = ThreadTeam::new(1).unwrap();
        let rhs = DenseMatrix::from_columns(&[
            (0..sample.n_active())
                .map(|row| (row as f64).sin())
                .collect(),
            vec![0.; sample.n_active()],
        ])
        .unwrap();
        let (coefficients, residuals) = if levels.len() == 1 {
            let fit = design.direct_candidate(&rhs, &team).unwrap();
            (fit.coefficients, fit.residuals)
        } else {
            let options = Options {
                tolerance: 1e-12,
                ..Options::for_estimator(Estimator::Ols)
            };
            let plan = Plan::new(&design, Estimator::Ols, &options).unwrap();
            let fit = SchurSession::new(&design, &plan, &team)
                .unwrap()
                .solve(&rhs, &team)
                .unwrap();
            (fit.coefficients, fit.residuals)
        };
        let reference = design
            .certify(&rhs, &coefficients, &residuals, 1e-10, &team)
            .unwrap();
        assert!(reference.relative_score.iter().all(|v| *v < 1e-10));
        assert_eq!(reference.relative_score[1], 0.);
        for workers in [2, 4, 8] {
            assert_eq!(
                reference,
                design
                    .certify(
                        &rhs,
                        &coefficients,
                        &residuals,
                        1e-10,
                        &ThreadTeam::new(workers).unwrap()
                    )
                    .unwrap()
            );
        }
    }
}
#[test]
fn fresh_certificate_rejects_wrong_coefficients_residuals_and_weights() {
    let sample = support::prepared(&[3], false, 10, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let rhs = DenseMatrix::from_columns(&[(0..sample.n_active()).map(|row| row as f64).collect()])
        .unwrap();
    let fit = design.direct_candidate(&rhs, &team).unwrap();
    let mut bad = fit.residuals.clone();
    bad[(0, 0)] += 1.;
    assert_eq!(
        design
            .certify(&rhs, &fit.coefficients, &bad, 1e-10, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    let bad = DenseMatrix::zeros(design.layout().total, 1).unwrap();
    assert!(design.certify(&rhs, &bad, &rhs, 1e-10, &team).is_err());
    let weights: Vec<_> = (0..sample.n_active()).map(|row| 1. + row as f64).collect();
    assert!(Design::new(&sample, Weights::Provided(&weights))
        .unwrap()
        .certify(&rhs, &fit.coefficients, &fit.residuals, 1e-10, &team)
        .is_err());
    assert!(design
        .certify(&rhs, &fit.coefficients, &fit.residuals, f64::NAN, &team)
        .is_err());
}
#[test]
fn independent_span_witness_detects_small_raw_moments_in_a_weak_direction() {
    let first = vec![1., -1., 1., -1.];
    let contrast = vec![1., 1., -1., -1.];
    let second = first
        .iter()
        .zip(&contrast)
        .map(|(x, r)| x + 1e-8 * r)
        .collect();
    let sample = support::slope_sample(false, &[0; 4], &[first, second]);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let rhs = DenseMatrix::from_columns(&[contrast]).unwrap();
    let coefficients = DenseMatrix::zeros(design.layout().total, 1).unwrap();
    let raw = design.transpose_weighted(&rhs, &team).unwrap();
    assert!(raw.as_slice().iter().all(|v| v.abs() < 1e-6));
    assert_eq!(
        design
            .certify(&rhs, &coefficients, &rhs, 1e-6, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn overflowing_raw_units_do_not_override_a_valid_span_certificate() {
    let sample = support::slope_sample(false, &[0; 4], &[vec![1e150; 4]]);
    let weights = vec![1e150; 4];
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let rhs = DenseMatrix::from_columns(&[vec![1e150, -1e150, 1e150, -1e150 + 1e138]]).unwrap();
    let zero = DenseMatrix::zeros(design.layout().total, 1).unwrap();
    let certificate = design.certify(&rhs, &zero, &rhs, 1e-10, &team).unwrap();
    assert!(certificate.raw_score_max[0].is_infinite());
    assert!(certificate.relative_score[0] < 1e-10);
}
#[test]
fn disconnected_normalization_preserves_fits_slopes_and_weighted_centers() {
    let sample = support::prepared(&[6, 4, 6], true, 30, 0);
    let weights: Vec<_> = (0..sample.n_active())
        .map(|row| 1. + (row % 3) as f64)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let original =
        DenseMatrix::from_columns(&[(0..design.layout().total).map(|i| i as f64 - 7.).collect()])
            .unwrap();
    let before = design.multiply(&original, &team).unwrap();
    let mut normalized = original.clone();
    design
        .normalize_coefficients(&mut normalized, 1e-12, &team)
        .unwrap();
    let after = design.multiply(&normalized, &team).unwrap();
    for (&a, &b) in before.as_slice().iter().zip(after.as_slice()) {
        assert!((a - b).abs() < 1e-12);
    }
    for effect in 1..3 {
        let layout = &design.layout().effects[effect];
        let codes = design.topology().effect_partition(effect).unwrap().codes();
        for component in 0..2 {
            let mean: f64 = (0..sample.n_active())
                .filter(|row| row % 2 == component)
                .map(|row| weights[row] * normalized[(layout.offset + codes[row] as usize, 0)])
                .sum();
            assert!(mean.abs() < 1e-10);
        }
    }
    for layout in &design.layout().effects {
        for level in 0..layout.levels {
            assert_eq!(
                original[(layout.offset + layout.levels + level, 0)],
                normalized[(layout.offset + layout.levels + level, 0)]
            );
        }
    }
    let mut parallel = original;
    design
        .normalize_coefficients(&mut parallel, 1e-12, &ThreadTeam::new(8).unwrap())
        .unwrap();
    assert_eq!(normalized, parallel);
}
#[test]
fn normalization_checks_roundoff_in_both_gauges_on_zero_fitted_rows() {
    let sample = support::prepared(&[7, 5, 3], false, 10, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut coefficients = DenseMatrix::zeros(design.layout().total, 1).unwrap();
    for coefficient in 0..design.layout().effects[2].offset {
        coefficients[(coefficient, 0)] = 1e-20;
    }
    let third = design.layout().effects[2].offset;
    coefficients[(third + 1, 0)] = 1.;
    coefficients[(third + 2, 0)] = 2.;
    let rhs = design.multiply(&coefficients, &team).unwrap();
    assert_eq!(rhs[(0, 0)], 2e-20);
    design
        .normalize_coefficients(&mut coefficients, 1e-12, &team)
        .unwrap();
    let after = design.multiply(&coefficients, &team).unwrap();
    for (old, new) in rhs.as_slice().iter().zip(after.as_slice()) {
        assert!((old - new).abs() <= 4. * f64::EPSILON);
    }
    let residuals = design.reconstruct(&rhs, &coefficients, &team).unwrap();
    design
        .certify(&rhs, &coefficients, &residuals, 1e-12, &team)
        .unwrap();
}
#[test]
fn span_witness_is_invariant_to_channel_rotation_and_units() {
    let u = vec![1.; 4];
    let v = vec![-3., -1., 1., 3.];
    let rhs = DenseMatrix::from_columns(&[vec![0.97, -1.01, -0.99, 1.03]]).unwrap();
    let rotated = vec![
        u.iter().map(|x| x * 1e150).collect(),
        u.iter()
            .zip(&v)
            .map(|(a, b)| (a + 2. * b) * 1e-150)
            .collect(),
    ];
    let team = ThreadTeam::new(1).unwrap();
    let mut scores = Vec::new();
    for slopes in [vec![u, v], rotated] {
        let sample = support::slope_sample(false, &[0; 4], &slopes);
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let coefficients = DenseMatrix::zeros(design.layout().total, 1).unwrap();
        scores.push(
            design
                .certify(&rhs, &coefficients, &rhs, 0.1, &team)
                .unwrap()
                .relative_score[0],
        );
    }
    assert!((scores[0] - scores[1]).abs() < 1e-14);
}
