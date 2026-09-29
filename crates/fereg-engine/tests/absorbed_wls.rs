use fereg_engine::{
    data::*,
    fe::{Design, Engine, Weights},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::{self, Policy, Role},
    ErrorCode,
};
mod support;
#[test]
fn shared_absorbed_fit_recovers_finite_and_fe_coefficients_on_exact_models() {
    let team = ThreadTeam::new(1).unwrap();
    for levels in [vec![], vec![3], vec![3, 2]] {
        let sample = support::prepared(&levels, false, 20, 0);
        let n = sample.n_active();
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let x: Vec<_> = (0..n).map(|i| (i % 13) as f64 / 7. - 0.4).collect();
        let raw = DenseMatrix::from_columns(&[x.clone()]).unwrap();
        let options = Options {
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ols)
        };
        let mut engine = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
        for with_fe in [true, false] {
            let alpha = DenseMatrix::from_columns(&[(0..design.layout().total)
                .map(|i| if with_fe { i as f64 * 0.1 } else { 0. })
                .collect()])
            .unwrap();
            let absorbed = design.multiply(&alpha, &team).unwrap();
            let y: Vec<_> = (0..n).map(|i| 0.3 * x[i] + absorbed[(i, 0)]).collect();
            let fit = wls::solve_absorbed(
                &mut engine,
                &raw,
                &y,
                &[Role::Ordinary],
                Policy::default(),
                &team,
            )
            .unwrap_or_else(|e| panic!("levels={levels:?}, with_fe={with_fe}: {e}"));
            assert!((fit.finite.coefficients[0] - 0.3).abs() < 1e-12);
            assert!(fit.residuals.iter().all(|r| r.abs() < 1e-12));
            if levels == [3] && !with_fe {
                assert_eq!(fit.recovery_diagnostics.len(), 1);
                assert!(fit.model_certificate.weighted_rhs_norm[0] < 1e-14);
                assert!(fit.model_certificate.relative_score[0] <= 1e-12);
            }
        }
    }
}
#[test]
fn perfect_fit_coefficient_cancellation_is_not_accepted_at_pre_fit_scales() {
    let sample = support::prepared(&[3], false, 20, 0);
    let n = sample.n_active();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let x: Vec<_> = (0..n).map(|i| (i % 13) as f64 / 7. - 0.4).collect();
    let y: Vec<_> = x.iter().map(|x| 0.3 * x).collect();
    let rhs = DenseMatrix::from_columns(&[y.clone(), x.clone()]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let options = Options {
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ols)
    };
    let mut engine = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
    let projection = engine.project(&rhs, &team).unwrap();
    let within = DenseMatrix::from_columns(&[projection.residuals.column(1).unwrap()]).unwrap();
    let response = projection.residuals.column(0).unwrap();
    let slots = [wls::Slot {
        role: Role::Ordinary,
        reference_norm: x.iter().fold(0_f64, |r, x| r.hypot(*x)),
    }];
    let finite = wls::solve(
        &wls::Problem {
            design: &within,
            outcome: &response,
            weights: Weights::Unit,
            slots: &slots,
            has_absorbed_intercept: true,
        },
        Policy::default(),
        &team,
    )
    .unwrap();
    let beta = finite.coefficients[0];
    let alpha = DenseMatrix::from_columns(&[(0..3)
        .map(|i| projection.coefficients[(i, 0)] - beta * projection.coefficients[(i, 1)])
        .collect()])
    .unwrap();
    let target =
        DenseMatrix::from_columns(&[(0..n).map(|i| y[i] - beta * x[i]).collect()]).unwrap();
    let residual = design.reconstruct(&target, &alpha, &team).unwrap();
    assert!(target.as_slice().iter().all(|v| v.abs() < 1e-15));
    assert_eq!(
        design
            .certify(&target, &alpha, &residual, 1e-12, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    let recovered = engine.project(&target, &team).unwrap();
    assert!(recovered.certificate.relative_score[0] < 1e-12);
    assert_eq!(recovered.diagnostics.len(), 1);
}
#[test]
fn unsupported_cmg_slopes_are_never_silently_substituted() {
    let sample = support::prepared(&[3, 2], true, 5, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let options = Options {
        solver: Solver::Cmg,
        ..Options::for_estimator(Estimator::Ols)
    };
    assert_eq!(
        Engine::new(&design, Estimator::Ols, &options, &team)
            .err()
            .unwrap()
            .code,
        ErrorCode::Unsupported
    );
}

#[test]
fn changed_weights_rebuild_one_shared_frame_with_exact_worker_identity() {
    let sample = support::prepared(&[5, 3, 2], true, 18, 0);
    let n = sample.n_active();
    let x: Vec<_> = (0..n).map(|i| ((i * 7) % 31) as f64 / 11.).collect();
    let y: Vec<_> = (0..n)
        .map(|i| x[i] * 0.4 + (i % 7) as f64 * 0.2 + (i as f64).sin())
        .collect();
    let raw = DenseMatrix::from_columns(&[x, vec![1.; n]]).unwrap();
    let roles = [Role::Ordinary, Role::AbsorbedIntercept];
    let options = Options {
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let mut original = None;
    for changed in [false, true, false] {
        let weights: Vec<_> = (0..n)
            .map(|i| 1. + (i % 3) as f64 + if changed { (i % 11) as f64 * 0.7 } else { 0. })
            .collect();
        let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
        let mut baseline = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let mut engine = Engine::new(&design, Estimator::Ppml, &options, &team).unwrap();
            let actual =
                wls::solve_absorbed(&mut engine, &raw, &y, &roles, Policy::default(), &team)
                    .unwrap();
            assert_eq!(
                actual.finite.statuses[1],
                wls::Status::AbsorbedInterceptDeferred
            );
            if let Some(expected) = &baseline {
                assert_eq!(&actual, expected);
            } else {
                baseline = Some(actual);
            }
        }
        if let Some(expected) = &original {
            if changed {
                assert_ne!(&baseline, expected);
            } else {
                assert_eq!(&baseline, expected);
            }
        } else {
            original = Some(baseline);
        }
    }
}
#[test]
fn malformed_frames_and_exhausted_projection_budgets_fail_clearly() {
    let sample = support::prepared(&[7, 5, 3], false, 10, 0);
    let n = sample.n_active();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let options = Options {
        tolerance: 1e-12,
        iterations: 1,
        ..Options::for_estimator(Estimator::Ols)
    };
    let mut engine = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
    let raw = DenseMatrix::zeros(n, 0).unwrap();
    assert_eq!(
        wls::solve_absorbed(&mut engine, &raw, &[0.], &[], Policy::default(), &team)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let y: Vec<_> = (0..n).map(|i| (i as f64).sin()).collect();
    assert_eq!(
        wls::solve_absorbed(&mut engine, &raw, &y, &[], Policy::default(), &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}

#[test]
fn response_seeds_carry_only_coefficients_across_weight_frames() {
    for levels in [vec![5], vec![5, 3, 2]] {
        let sample = support::prepared(&levels, true, 25, 0);
        let n = sample.n_active();
        let raw =
            DenseMatrix::from_columns(&[(0..n).map(|i| ((i * 7) % 17) as f64 / 17.).collect()])
                .unwrap();
        let y: Vec<_> = (0..n).map(|i| (i % 7) as f64 + (i as f64).sin()).collect();
        let one = ThreadTeam::new(1).unwrap();
        let options = Options {
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ppml)
        };
        let old = Design::new(&sample, Weights::Unit).unwrap();
        let mut engine = Engine::new(&old, Estimator::Ppml, &options, &one).unwrap();
        let previous = wls::solve_absorbed(
            &mut engine,
            &raw,
            &y,
            &[Role::Ordinary],
            Policy::default(),
            &one,
        )
        .unwrap();
        let weights: Vec<_> = (0..n).map(|i| 1. + (i % 11) as f64 * 0.3).collect();
        let fresh = Design::new(&sample, Weights::Provided(&weights)).unwrap();
        let mut cold = Engine::new(&fresh, Estimator::Ppml, &options, &one).unwrap();
        let reference = wls::solve_absorbed(
            &mut cold,
            &raw,
            &y,
            &[Role::Ordinary],
            Policy::default(),
            &one,
        )
        .unwrap();
        let mut expected = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let mut warm = Engine::new(&fresh, Estimator::Ppml, &options, &team).unwrap();
            warm.seed_response(&previous.absorbed_coefficients, 2)
                .unwrap();
            assert!(warm
                .seed_response(&DenseMatrix::zeros(1, 1).unwrap(), 2)
                .is_err());
            assert!(warm
                .seed_response(&previous.absorbed_coefficients, 0)
                .is_err());
            let fit = wls::solve_absorbed(
                &mut warm,
                &raw,
                &y,
                &[Role::Ordinary],
                Policy::default(),
                &team,
            )
            .unwrap();
            assert!((fit.finite.coefficients[0] - reference.finite.coefficients[0]).abs() < 2e-9);
            for (&a, &b) in fit.residuals.iter().zip(&reference.residuals) {
                assert!((a - b).abs() < 2e-9);
            }
            assert!(fit
                .projection_certificate
                .relative_score
                .iter()
                .all(|v| *v <= 1e-12));
            if let Some(expected) = &expected {
                assert_eq!(&fit, expected);
            } else {
                expected = Some(fit);
            }
        }
    }
}
