use fereg_engine::{
    data::*,
    ppml::irls::{fit_final_sample, Fit, Problem},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    topology::prepare_implicit_constant,
    wls::{Role, Status},
    ErrorCode,
};
mod support;
fn run<'a>(
    sample: &'a Prepared,
    raw: &'a DenseMatrix,
    roles: &'a [Role],
    options: Options,
    workers: usize,
) -> fereg_engine::Result<Fit<'a>> {
    fit_final_sample(
        &Problem { sample, raw, roles },
        options,
        &ThreadTeam::new(workers).unwrap(),
    )
}
fn options() -> Options {
    Options::for_estimator(Estimator::Ppml)
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() < 2e-9 * a.abs().max(b.abs()).max(1.),
        "{a:.17e} != {b:.17e}"
    );
}
#[test]
fn analytic_one_fe_start_and_exact_accepted_hessian_match_group_optima() {
    let mut sample = support::prepared(&[4], false, 12, 0);
    let n = sample.n_active();
    sample.outcome = (0..n).map(|i| (1 + i % 11) as f64).collect();
    let q: Vec<_> = (0..n).map(|i| (1 + i % 3) as f64).collect();
    let exposure: Vec<_> = (0..n).map(|i| 1. + (i % 5) as f64 * 0.2).collect();
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: q.clone(),
    };
    sample.adjustment = PreparedAdjustment::Exposure(exposure.clone());
    let raw = DenseMatrix::zeros(n, 0).unwrap();
    let fit = run(
        &sample,
        &raw,
        &[],
        Options {
            iterations: 2,
            ..options()
        },
        1,
    )
    .unwrap();
    assert!(fit.diagnostics.analytic_start);
    assert_eq!(fit.diagnostics.accepted_iterations, 2);
    assert_eq!(
        fit.diagnostics.work.projection().calls(),
        fit.diagnostics.linear_frames
    );
    assert_eq!(fit.diagnostics.work.counts().summed_iterations, 0);
    for group in 0..4 {
        let sum_y: f64 = (group..n)
            .step_by(4)
            .map(|i| q[i] * sample.outcome[i])
            .sum();
        let sum_e: f64 = (group..n).step_by(4).map(|i| q[i] * exposure[i]).sum();
        for row in (group..n).step_by(4) {
            close(fit.evaluation.mean[row], exposure[row] * sum_y / sum_e);
            assert_eq!(fit.hessian_weights[row], q[row] * fit.evaluation.mean[row]);
        }
    }
}
#[test]
fn full_steps_are_tried_then_halved_jointly_with_worker_identical_results() {
    let mut sample = support::prepared(&[], false, 25, 0);
    let n = sample.n_active();
    sample.outcome = vec![1.; n];
    sample.outcome[0] = 100_000.;
    let mut rare = vec![0.; n];
    rare[0] = 1.;
    let raw = DenseMatrix::from_columns(&[vec![1.; n], rare]).unwrap();
    let roles = [Role::Ordinary; 2];
    let fit = run(&sample, &raw, &roles, options(), 1).unwrap();
    assert!(fit.diagnostics.step_halvings > 0);
    close(fit.point.finite[0], 0.);
    close(fit.point.finite[1], 100_000_f64.ln());
    for workers in [2, 4, 8] {
        assert_eq!(fit, run(&sample, &raw, &roles, options(), workers).unwrap());
    }
}
#[test]
fn exposure_and_logged_offset_share_one_input_state_without_double_logging() {
    let mut sample = support::prepared(&[], false, 14, 0);
    let mut logged = support::prepared(&[], false, 14, 0);
    let n = sample.n_active();
    let exposure: Vec<_> = (0..n).map(|i| 0.4 + (i % 5) as f64 * 0.3).collect();
    sample.outcome = (0..n).map(|i| ((i * 7) % 13) as f64 / 2.).collect();
    logged.outcome = sample.outcome.clone();
    sample.adjustment = PreparedAdjustment::Exposure(exposure.clone());
    logged.adjustment = PreparedAdjustment::Offset(exposure.iter().map(|v| v.ln()).collect());
    let raw =
        DenseMatrix::from_columns(&[vec![1.; n], (0..n).map(|i| (i % 11) as f64 / 11.).collect()])
            .unwrap();
    let roles = [Role::Ordinary; 2];
    let exposure_fit = run(&sample, &raw, &roles, options(), 1).unwrap();
    let logged_fit = run(&logged, &raw, &roles, options(), 1).unwrap();
    assert_eq!(exposure_fit.problem().sample.adjustment, sample.adjustment);
    assert_eq!(logged_fit.problem().sample.adjustment, logged.adjustment);
    assert_ne!(exposure_fit.problem(), logged_fit.problem());
    assert_eq!(exposure_fit.into_state(), logged_fit.into_state());
}
#[test]
fn strict_resumption_preserves_the_outer_budget() {
    let mut sample = support::prepared(&[], false, 25, 0);
    let n = sample.n_active();
    let x: Vec<_> = (0..n).map(|i| (i % 31) as f64 / 15. - 1.).collect();
    sample.outcome = x
        .iter()
        .map(|v| (0.2 + 1.1 * v + 0.3 * v * v).exp())
        .collect();
    let raw = DenseMatrix::from_columns(&[vec![1.; n], x]).unwrap();
    let loose = Options {
        tolerance: 0.3,
        ..options()
    };
    let fit = run(&sample, &raw, &[Role::Ordinary; 2], loose, 1).unwrap();
    assert!(fit.diagnostics.strict_resumes > 0, "{:?}", fit.diagnostics);
    let tight = run(&sample, &raw, &[Role::Ordinary; 2], options(), 1).unwrap();
    for (&a, &b) in fit.point.finite.iter().zip(&tight.point.finite) {
        close(a, b);
    }
    assert_eq!(
        run(
            &sample,
            &raw,
            &[Role::Ordinary; 2],
            Options {
                iterations: fit.diagnostics.accepted_iterations - 1,
                ..loose
            },
            1
        )
        .unwrap_err()
        .code,
        ErrorCode::NonConvergence
    );
    assert_eq!(
        run(
            &sample,
            &raw,
            &[Role::Ordinary; 2],
            Options {
                iterations: 1,
                ..options()
            },
            1
        )
        .unwrap_err()
        .code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn implicit_constant_and_aliases_keep_canonical_zero_slots() {
    let mut sample = support::prepared(&[], false, 12, 0);
    let n = sample.n_active();
    sample.outcome = (0..n).map(|i| (1 + i % 7) as f64).collect();
    let raw = DenseMatrix::from_columns(&[vec![1.; n], vec![1.; n]]).unwrap();
    let ordinary = run(&sample, &raw, &[Role::Ordinary; 2], options(), 1)
        .unwrap()
        .into_state();
    assert_eq!(
        ordinary.linear.finite.statuses,
        vec![Status::Kept, Status::NumericallyRankDeficient]
    );
    assert_eq!(ordinary.point.finite[1], 0.);
    let sample = prepare_implicit_constant(sample).unwrap();
    let implicit = run(
        &sample,
        &raw,
        &[Role::Ordinary, Role::AbsorbedIntercept],
        options(),
        1,
    )
    .unwrap();
    assert_eq!(implicit.point.finite, vec![0., 0.]);
    for (&a, &b) in ordinary
        .evaluation
        .mean
        .iter()
        .zip(&implicit.evaluation.mean)
    {
        close(a, b);
    }
}
#[test]
fn slope_only_effects_retain_the_finite_structural_constant() {
    let n = 80;
    let codes: Vec<_> = (0..n).map(|i| (i % 4) as u32).collect();
    let slope: Vec<_> = (0..n).map(|i| (i % 7) as f64 - 2.).collect();
    let x: Vec<_> = (0..n).map(|i| ((i * 11) % 19) as f64 / 19.).collect();
    let mut sample = support::slope_sample(false, &codes, &[slope.clone()]);
    sample.outcome = (0..n)
        .map(|i| (0.3 - 0.2 * x[i] + 0.02 * codes[i] as f64 * slope[i]).exp())
        .collect();
    let raw = DenseMatrix::from_columns(&[vec![1.; n], x]).unwrap();
    let fit = run(&sample, &raw, &[Role::Ordinary; 2], options(), 1).unwrap();
    assert!(!fit.diagnostics.analytic_start);
    close(fit.point.finite[0], 0.3);
    close(fit.point.finite[1], -0.2);
    for row in 0..n {
        close(fit.evaluation.mean[row], sample.outcome[row]);
    }
}
#[test]
fn unsupported_requests_and_invalid_final_samples_never_return_a_partial_fit() {
    let mut sample = support::prepared(&[3, 2, 3, 2], false, 6, 0);
    let n = sample.n_active();
    let raw = DenseMatrix::zeros(n, 0).unwrap();
    assert_eq!(
        run(
            &sample,
            &raw,
            &[],
            Options {
                solver: Solver::Cmg,
                ..options()
            },
            1
        )
        .unwrap_err()
        .code,
        ErrorCode::Unsupported
    );
    sample.adjustment = PreparedAdjustment::Exposure(vec![0.; n]);
    assert_eq!(
        run(&sample, &raw, &[], options(), 1).unwrap_err().code,
        ErrorCode::InvalidData
    );
    sample.adjustment = PreparedAdjustment::None;
    sample.outcome.fill(0.);
    assert_eq!(
        run(&sample, &raw, &[], options(), 1).unwrap_err().code,
        ErrorCode::InsufficientObservations
    );
    sample.outcome[0] = -1.;
    assert_eq!(
        run(&sample, &raw, &[], options(), 1).unwrap_err().code,
        ErrorCode::InvalidData
    );
}

#[test]
fn strict_basis_change_is_represented_and_committed_before_bounded_resumption() {
    let mut sample = support::prepared(&[1], false, 35, 0);
    let n = sample.n_active();
    let x: Vec<_> = (0..n).map(|i| (i % 5) as f64 - 2.).collect();
    sample.outcome = x.iter().map(|v| (-2. * v).exp()).collect();
    let raw = DenseMatrix::from_columns(&[x.clone(), x.iter().map(|v| v + 2.).collect()]).unwrap();
    let fit = run(&sample, &raw, &[Role::Ordinary; 2], options(), 1).unwrap();
    assert!(fit.diagnostics.strict_resumes > 0);
    assert_eq!(fit.linear.finite.kept, vec![1]);
    assert_eq!(fit.point.finite[0], 0.);
    close(fit.point.finite[1], -2.);
    assert!(fit.diagnostics.basis_rebases > 0);
    assert!(
        fit.diagnostics.linear_frames
            > 1 + fit.diagnostics.accepted_iterations + fit.diagnostics.strict_checks
    );
    assert_eq!(
        fit.diagnostics.work.projection().calls(),
        fit.diagnostics.linear_frames
    );
    assert_eq!(
        fit.diagnostics.work.projection().counts().right_hand_sides,
        fit.diagnostics.linear_frames * 3
    );
    for (&eta, &x) in fit.point.eta.iter().zip(&x) {
        close(eta, -2. * x);
    }
    assert_eq!(
        fit,
        run(&sample, &raw, &[Role::Ordinary; 2], options(), 8).unwrap()
    );
}

#[test]
fn explicit_lsmr_keeps_ppml_means_and_fresh_hessians_identical_across_workers() {
    for levels in [&[5, 3][..], &[5, 3, 2][..], &[7, 5, 3, 2][..]] {
        let mut sample = support::prepared(levels, false, 40, 0);
        let n = sample.n_active();
        let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.31).sin()).collect();
        let offset: Vec<_> = (0..n).map(|i| (i as f64 * 0.17).cos() * 0.2).collect();
        sample.outcome = (0..n)
            .map(|i| {
                let fe: f64 = levels.iter().map(|l| (i % l) as f64 * 0.07).sum();
                (0.3 - 0.4 * x[i] + fe + offset[i]).exp()
            })
            .collect();
        sample.weights = PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: (0..n).map(|i| 0.4 + (i % 13) as f64 / 3.).collect(),
        };
        sample.adjustment = PreparedAdjustment::Offset(offset);
        let raw = DenseMatrix::from_columns(&[x]).unwrap();
        let options = Options {
            solver: Solver::Lsmr,
            ..options()
        };
        let fit = run(&sample, &raw, &[Role::Ordinary], options, 1).unwrap();
        assert_eq!(
            fit.diagnostics.work.projection().calls(),
            fit.diagnostics.linear_frames
        );
        let work = fit.diagnostics.work.projection();
        assert_eq!(
            work.requested(Solver::Lsmr),
            fit.diagnostics.linear_frames * 2
        );
        assert_eq!(
            work.used(fereg_engine::fe::Backend::Lsmr),
            work.counts().right_hand_sides
        );
        assert!(
            work.counts().summed_iterations
                > fit
                    .linear
                    .projection_diagnostics
                    .iter()
                    .map(|d| d.iterations)
                    .sum()
        );
        close(fit.point.finite[0], -0.4);
        for (mean, truth) in fit.evaluation.mean.iter().zip(&sample.outcome) {
            close(*mean, *truth);
        }
        assert!(fit
            .linear
            .projection_diagnostics
            .iter()
            .all(|d| d.requested == Solver::Lsmr
                && d.used == fereg_engine::fe::Backend::Lsmr
                && d.attempts.len() == 1
                && d.fallback.is_none()));
        for workers in [2, 4, 8] {
            assert_eq!(
                fit,
                run(&sample, &raw, &[Role::Ordinary], options, workers).unwrap()
            );
        }
    }
}
