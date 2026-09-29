use fereg_engine::{
    data::*,
    fe::{Backend, Design, Engine, FallbackReason, MonitorControl, PlanOverride, StopReason},
    ppml::irls::{fit_final_sample, Problem},
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::{self, AbsorbedControls, Role, Work},
    ErrorCode,
};
mod support;

#[test]
fn cmg_irls_accounts_for_fresh_setup_and_discarded_frames_at_all_worker_counts() {
    let mut sample = support::prepared(&[7, 5, 3], false, 30, 0);
    let n = sample.n_active();
    let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.31).sin()).collect();
    sample.outcome = (0..n)
        .map(|i| {
            (0.3 - 0.4 * x[i]
                + (i % 7) as f64 * 0.07
                + (i % 5) as f64 * 0.03
                + (i % 3) as f64 * 0.1)
                .exp()
        })
        .collect();
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: (0..n).map(|i| (1 + i % 4) as f64).collect(),
    };
    let raw = DenseMatrix::from_columns(&[x]).unwrap();
    let problem = Problem {
        sample: &sample,
        raw: &raw,
        roles: &[Role::Ordinary],
    };
    let options = Options {
        solver: Solver::Cmg,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let fit = fit_final_sample(&problem, options, &ThreadTeam::new(1).unwrap()).unwrap();
    let frames = fit.diagnostics.linear_frames;
    assert_eq!(
        fit.diagnostics.strict_checks,
        fit.diagnostics.fisher_checks + fit.diagnostics.hessian_checks
    );
    assert!(fit.diagnostics.fisher_checks >= 1);
    assert_eq!(fit.diagnostics.hessian_checks, 1);
    assert_eq!(
        frames,
        1 + fit.diagnostics.accepted_iterations + fit.diagnostics.strict_checks
    );
    let work = fit.diagnostics.work;
    assert_eq!(work.projection().calls(), frames);
    assert_eq!(work.projection().requested(Solver::Cmg), frames * 2);
    assert_eq!(work.projection().selected(Backend::Cmg), frames * 2);
    assert_eq!(work.setup().calls(), frames);
    assert_eq!(work.setup().backend_calls(Backend::Cmg), frames);
    assert_eq!(work.setup().counts().right_hand_sides, frames * 2);
    assert_eq!(work.setup().stopped(StopReason::Converged), frames);
    assert!(work.setup().counts().summed_iterations > 0);
    assert!(
        work.projection().counts().summed_iterations
            > fit
                .linear
                .projection_diagnostics
                .iter()
                .map(|d| d.iterations)
                .sum()
    );
    assert_eq!(
        work.counts().summed_iterations,
        work.projection().counts().summed_iterations
            + work.recovery().counts().summed_iterations
            + work.setup().counts().summed_iterations
    );
    assert!(work.counts().maximum_rhs_iterations <= 16_000);
    for workers in [2, 4, 8] {
        assert_eq!(
            fit,
            fit_final_sample(&problem, options, &ThreadTeam::new(workers).unwrap()).unwrap()
        );
    }
}

#[test]
fn reused_cmg_frame_counts_setup_once_and_recovery_separately() {
    let sample = support::prepared(&[7, 5, 3], false, 30, 0);
    let n = sample.n_active();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let options = Options {
        solver: Solver::Cmg,
        tolerance: 1e-10,
        ..Options::for_estimator(Estimator::Ols)
    };
    let mut engine = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
    let raw =
        DenseMatrix::from_columns(&[(0..n).map(|i| (i as f64 * 0.31).sin()).collect()]).unwrap();
    let y: Vec<_> = (0..n)
        .map(|i| 0.3 * raw[(i, 0)] + (i as f64 * 0.17).cos())
        .collect();
    let mut work = Work::default();
    let controls = AbsorbedControls::default();
    let first = wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &y,
        &[Role::Ordinary],
        controls,
        &mut work,
        &team,
    )
    .unwrap();
    let initial = work;
    let second = wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &y,
        &[Role::Ordinary],
        controls,
        &mut work,
        &team,
    )
    .unwrap();
    assert_eq!(first.finite, second.finite);
    assert_eq!(first.absorbed_coefficients, second.absorbed_coefficients);
    assert_eq!(first.residuals, second.residuals);
    assert_eq!(first.projection_certificate, second.projection_certificate);
    assert_eq!(first.model_certificate, second.model_certificate);
    assert_eq!(first.setup_diagnostics.len(), 1);
    assert!(second.setup_diagnostics.is_empty());
    assert_eq!(work.projection().calls(), 2);
    assert_eq!(work.projection().counts().right_hand_sides, 4);
    assert_eq!(work.setup(), initial.setup());
    assert_eq!(work.setup().calls(), 1);
    assert_eq!(work.setup().counts().right_hand_sides, 2);
    assert_eq!(
        work.recovery().calls(),
        2 * usize::from(!first.recovery_diagnostics.is_empty())
    );
}

#[test]
fn certificate_recovery_is_visible_as_a_derived_rhs_without_an_extra_frame() {
    let sample = support::prepared(&[3], false, 20, 0);
    let n = sample.n_active();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let options = Options {
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ols)
    };
    let mut engine = Engine::new(&design, Estimator::Ols, &options, &team).unwrap();
    let x: Vec<_> = (0..n).map(|i| (i % 13) as f64 / 7. - 0.4).collect();
    let y: Vec<_> = x.iter().map(|x| 0.3 * x).collect();
    let raw = DenseMatrix::from_columns(&[x]).unwrap();
    let mut work = Work::default();
    let fit = wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &y,
        &[Role::Ordinary],
        AbsorbedControls::default(),
        &mut work,
        &team,
    )
    .unwrap();
    assert_eq!(fit.recovery_diagnostics.len(), 1);
    assert_eq!(work.projection().calls(), 1);
    assert_eq!(work.projection().counts().right_hand_sides, 2);
    assert_eq!(work.recovery().calls(), 1);
    assert_eq!(work.recovery().counts().right_hand_sides, 1);
    assert_eq!(work.recovery().used(Backend::Direct), 1);
    assert_eq!(work.counts().summed_iterations, 0);
}

#[test]
fn numerical_failure_retains_primary_bank_and_bounded_fallback_work() {
    let sample = support::prepared(&[9, 7, 5], false, 17, 0);
    let n = sample.n_active();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let options = Options {
        iterations: 2,
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ols)
    };
    let mut engine = Engine::with_controls(
        &design,
        Estimator::Ols,
        &options,
        PlanOverride::Automatic,
        MonitorControl::StopAfter(1),
        &team,
    )
    .unwrap();
    let y: Vec<_> = (0..n).map(|i| (i as f64 * 0.17).cos()).collect();
    let raw =
        DenseMatrix::from_columns(&[(0..n).map(|i| (i as f64 * 0.31).sin()).collect()]).unwrap();
    let mut work = Work::default();
    let error = wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &y,
        &[Role::Ordinary],
        AbsorbedControls::default(),
        &mut work,
        &team,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(work.projection().calls(), 1);
    assert_eq!(work.projection().counts().right_hand_sides, 2);
    assert_eq!(
        work.projection()
            .backend(Backend::SchurPcg)
            .counts()
            .attempts,
        2
    );
    assert_eq!(
        work.projection().backend(Backend::Lsmr).counts().attempts,
        1
    );
    assert_eq!(
        work.projection()
            .fallback(FallbackReason::MeasuredStagnation),
        1
    );
    assert_eq!(work.counts().summed_iterations, 3);
    assert_eq!(work.counts().maximum_rhs_iterations, 2);
    assert_eq!(work.projection().stopped(StopReason::BudgetExhausted), 1);
    assert_eq!(work.projection().stopped(StopReason::Stagnation), 1);
}

#[test]
fn overflowing_work_merge_fails_atomically_instead_of_saturating() {
    let sample = support::prepared(&[], false, 3, 0);
    let n = sample.n_active();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let mut engine = Engine::new(
        &design,
        Estimator::Ols,
        &Options::for_estimator(Estimator::Ols),
        &team,
    )
    .unwrap();
    let raw = DenseMatrix::zeros(n, 0).unwrap();
    let mut work = Work::default();
    wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &sample.outcome,
        &[],
        AbsorbedControls::default(),
        &mut work,
        &team,
    )
    .unwrap();
    loop {
        let old = work;
        if let Err(error) = work.merge(old) {
            assert_eq!(error.code, ErrorCode::Internal);
            assert_eq!(work, old);
            break;
        }
        assert_eq!(
            work.counts().right_hand_sides,
            2 * old.counts().right_hand_sides
        );
    }
}

#[test]
fn finite_guard_rejection_keeps_completed_projection_work() {
    let sample = support::prepared(&[], false, 23, 0);
    let n = sample.n_active();
    let team = ThreadTeam::new(1).unwrap();
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let mut engine = Engine::new(
        &design,
        Estimator::Ols,
        &Options::for_estimator(Estimator::Ols),
        &team,
    )
    .unwrap();
    let raw = DenseMatrix::from_columns(&[
        vec![1.; n],
        (0..n).map(|i| (i as f64 * 0.31).sin()).collect(),
    ])
    .unwrap();
    let y: Vec<_> = (0..n).map(|i| (i as f64 * 0.17).cos()).collect();
    let mut work = Work::default();
    let controls = AbsorbedControls {
        finite: wls::Policy {
            score_tolerance: f64::MIN_POSITIVE,
            ..wls::Policy::default()
        },
        basis: None,
    };
    let error = wls::solve_absorbed_recorded(
        &mut engine,
        &raw,
        &y,
        &[Role::Ordinary; 2],
        controls,
        &mut work,
        &team,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert_eq!(work.projection().calls(), 1);
    assert_eq!(work.projection().counts().right_hand_sides, 3);
    assert_eq!(work.projection().stopped(StopReason::NoEffects), 3);
    assert_eq!(work.recovery().calls(), 0);
    assert_eq!(work.setup().calls(), 0);
}
