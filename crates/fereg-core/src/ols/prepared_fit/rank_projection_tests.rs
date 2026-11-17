use super::*;

#[test]
fn retained_rank_reprojects_exact_fe_dependencies_before_freezing() {
    // Every cell has all eight Walsh rows and a constant weight. Thus s,t,u,e
    // have exactly zero FE moments, are mutually orthogonal, and unit norm
    // after division by sqrt(sum(w)). Independent algebra gives rank 2 at
    // gap=0 and rank 3 otherwise, beta_t=.3 and residual=.2e in both cases.
    let cells = 137;
    let sign = |row: usize, mask: usize| {
        if (row & mask).count_ones() % 2 == 0 {
            1.
        } else {
            -1.
        }
    };
    for gap in [0., 1e-3] {
        let mut prior_kept = None;
        for scale in [1., 1e3] {
            for reverse in [false, true] {
                let order: Vec<_> = if reverse {
                    (0..cells * 8).rev().collect()
                } else {
                    (0..cells * 8).collect()
                };
                let mut x = vec![Vec::new(), Vec::new(), Vec::new()];
                let (mut y, mut weights, mut g, mut h, mut errors) =
                    (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for row in order {
                    let cell = row / 8;
                    let a = (cell % 17) as f64;
                    let b = ((cell * 7 + cell / 17) % 19) as f64;
                    let fa = (a * 0.37).sin() * 3.;
                    let fb = (b * 0.13).cos() * 5.;
                    let (s, t, u, e) = (sign(row, 1), sign(row, 2), sign(row, 3), sign(row, 4));
                    x[0].push(scale * (s + fb));
                    x[1].push((fa - s - fb + gap * u) / scale);
                    x[2].push(t);
                    y.push(0.7 * s + 0.3 * t + 0.2 * gap * u + fa + fb + 0.2 * e);
                    weights.push((1 + cell % 5) as f64);
                    g.push(a);
                    h.push(b);
                    errors.push(0.2 * e);
                }
                let data = prepared_multiway_categorical_weights(
                    &y,
                    &x,
                    &[g, h],
                    TestWeights::Analytic(&weights),
                );
                let frame = LinearSolveFrame::from_prepared_ols(&data).unwrap();
                for threads in [1, 3] {
                    let mut config = PreparedOlsFitConfig::default();
                    config.absorb.solver = FeSolverKind::Map;
                    config.absorb.map.tolerance = 1e-6;
                    config.absorb.lsmr.tolerance = 1e-6;
                    config.coefficient_tolerance = Some(1e-8);
                    config.execution.threads = ThreadConfig::from_requested(threads, true).unwrap();
                    let mut session = CertifiedLinearSession::new_with_fe_design(
                        &data,
                        frame,
                        config,
                        LinearSessionGenerations::new(0, 0, 0, 0, 0),
                        data.fit_owned_fe_design().unwrap(),
                    )
                    .unwrap();
                    let output = session.solve_fresh().unwrap_or_else(|e| {
                        panic!(
                            "gap={gap}, scale={scale}, reverse={reverse}, threads={threads}: {e:?}"
                        )
                    });
                    let fit = output.frame();
                    assert_eq!(fit.finite_rank(), if gap == 0. { 2 } else { 3 });
                    assert!(fit.certified());
                    if gap == 0. {
                        assert!(fit.absorption_report().checkpoint_retries > 0);
                    }
                    let kept = output.basis().kept_generated_columns().to_vec();
                    if let Some(prior) = &prior_kept {
                        assert_eq!(&kept, prior);
                    }
                    prior_kept = Some(kept);
                    let beta = output
                        .coefficients()
                        .find(|c| c.generated_column_index == Some(2))
                        .unwrap()
                        .value
                        .unwrap();
                    assert_close(beta, 0.3, 2e-10);
                    if gap > 0. {
                        for (generated, expected) in [(0, 0.9 / scale), (1, 0.2 * scale)] {
                            let actual = output
                                .coefficients()
                                .find(|c| c.generated_column_index == Some(generated))
                                .unwrap()
                                .value
                                .unwrap();
                            assert_close(actual, expected, 2e-8 * expected.abs().max(1e-3));
                        }
                        let total = frame.bread_weight_scale().reference_weight_sum;
                        let expected = [
                            (1.0 + gap.recip().powi(2)) / (scale * scale * total),
                            gap.recip().powi(2) / total,
                            0.0,
                            gap.recip().powi(2) / total,
                            scale * scale / (gap * gap * total),
                            0.0,
                            0.0,
                            0.0,
                            total.recip(),
                        ];
                        for row in 0..3 {
                            for column in 0..3 {
                                let norm = (expected[row * 3 + row]
                                    * expected[column * 3 + column])
                                    .sqrt();
                                assert_close(
                                    fit.bread()[row * 3 + column],
                                    expected[row * 3 + column],
                                    2e-8 * norm,
                                );
                            }
                        }
                    }
                    for (&actual, &expected) in fit.residuals().iter().zip(&errors) {
                        assert_close(actual, expected, 2e-8);
                    }
                }
            }
        }
    }
}

#[test]
fn frozen_rank_and_refinement_failures_have_narrow_canonical_fallback() {
    use crate::ols::linear_session::classify_frozen_linear_fallback;
    for message in [
        "frozen weighted-linear basis lost certified full rank",
        "retained coefficient refinement exhausted its accuracy budget",
        "retained coefficient refinement is nonfinite",
    ] {
        assert!(classify_frozen_linear_fallback(&FeregError::new(
            ErrorCode::NonConvergence,
            message
        ))
        .is_some());
        assert!(
            classify_frozen_linear_fallback(&FeregError::new(ErrorCode::Interrupted, message))
                .is_none()
        );
        assert!(
            classify_frozen_linear_fallback(&FeregError::new(ErrorCode::Allocation, message))
                .is_none()
        );
    }
    assert!(classify_frozen_linear_fallback(&FeregError::new(
        ErrorCode::InvalidData,
        "retained coefficient refinement inputs are invalid"
    ))
    .is_none());
}

#[test]
fn reported_inverse_congruence_rejects_scaled_bad_bread() {
    // Independent block identity: H = [[B,-Bm],[-m'B,1+m'Bm]],
    // with G_f=diag(4s^2,9/s^2), m=(2s,3/s), and total weight one.
    // Reciprocal units must change neither acceptance nor error detection.
    for scale in [1e-6_f64, 1., 1e6] {
        let square = scale * scale;
        let gram = [4. * square, 0., 0., 9. / square];
        let means = [2. * scale, 3. / scale];
        let mut inverse = [
            0.25 / square,
            0.,
            -0.5 / scale,
            0.,
            square / 9.,
            -scale / 3.,
            -0.5 / scale,
            -scale / 3.,
            3.,
        ];
        for reverse in [false, true] {
            let error =
                reported_inverse_relative_infinity_norm(&gram, &means, 1., &inverse, reverse)
                    .unwrap();
            assert!(
                error < 2e-15,
                "scale={scale} reverse={reverse} error={error}"
            );
        }
        inverse[0] *= 1.01;
        for reverse in [false, true] {
            let error =
                reported_inverse_relative_infinity_norm(&gram, &means, 1., &inverse, reverse)
                    .unwrap();
            assert!(
                error > 1e-4,
                "scale={scale} reverse={reverse} error={error}"
            );
        }
    }
}
