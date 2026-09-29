use fereg_engine::{
    data::DenseMatrix,
    fe::{
        Backend, Design, Engine, FallbackReason, MonitorControl, Plan, PlanOverride, SchurSession,
        StopReason, Weights,
    },
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    ErrorCode,
};
mod support;

#[test]
fn natural_weak_graph_monitor_preserves_budget_accounting() {
    let sample = support::prepared(&[101, 100], false, 2, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = bank(sample.n_active());
    let team = ThreadTeam::new(1).unwrap();
    let mut observed = false;
    for budget in [32, 64, 128] {
        let mut engine = Engine::new(
            &design,
            Estimator::Ols,
            &options(Solver::Auto, budget),
            &team,
        )
        .unwrap();
        let result = engine.project(&rhs, &team).unwrap();
        for report in engine.last_diagnostics() {
            assert!(report.iterations <= budget);
            assert_eq!(
                report.iterations,
                report.attempts.iter().map(|a| a.iterations).sum::<usize>()
            );
            observed |= matches!(
                report.fallback,
                Some(FallbackReason::MeasuredStagnation | FallbackReason::ProjectedExhaustion)
            );
        }
        assert_eq!(result.diagnostics, engine.last_diagnostics());
    }
    assert!(observed, "weak graph should exercise the measured monitor");
}

fn options(solver: Solver, iterations: usize) -> Options {
    Options {
        solver,
        iterations,
        tolerance: 1e-10,
        ..Options::for_estimator(Estimator::Ols)
    }
}
fn bank(n: usize) -> DenseMatrix {
    DenseMatrix::from_columns(&[
        (0..n).map(|i| (i as f64 * 0.13).sin()).collect(),
        vec![0.; n],
        (0..n).map(|i| (i as f64 * 0.23).cos()).collect(),
    ])
    .unwrap()
}

#[test]
fn monitored_columns_transfer_once_and_completed_columns_are_retained() {
    let sample = support::prepared(&[17, 13, 5], true, 10, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = bank(sample.n_active());
    let mut reference = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut engine = Engine::with_controls(
            &design,
            Estimator::Ols,
            &options(Solver::Auto, 1000),
            PlanOverride::Automatic,
            MonitorControl::StopAfter(1),
            &team,
        )
        .unwrap();
        let actual = engine.project(&rhs, &team).unwrap();
        assert_eq!(engine.last_diagnostics(), actual.diagnostics);
        for column in [0, 2] {
            let report = &actual.diagnostics[column];
            assert_eq!(report.requested, Solver::Auto);
            assert_eq!(report.selected, Backend::SchurPcg);
            assert_eq!(report.used, Backend::Lsmr);
            assert_eq!(report.checkpoint_backend, Backend::Lsmr);
            assert_eq!(report.attempts.len(), 2);
            assert_eq!(report.attempts[0].iterations, 1);
            assert_eq!(report.attempts[0].stop, StopReason::Stagnation);
            assert_eq!(report.fallback, Some(FallbackReason::MeasuredStagnation));
            assert_eq!(
                report.iterations,
                report.attempts.iter().map(|a| a.iterations).sum::<usize>()
            );
            assert!(report.iterations <= 1000);
        }
        let zero = &actual.diagnostics[1];
        assert_eq!(zero.used, Backend::SchurPcg);
        assert_eq!(zero.attempts.len(), 1);
        assert_eq!(zero.iterations, 0);
        assert_eq!(zero.fallback, None);
        let expected = Engine::new(&design, Estimator::Ols, &options(Solver::Lsmr, 1000), &team)
            .unwrap()
            .project(&rhs, &team)
            .unwrap();
        for (a, b) in actual
            .residuals
            .as_slice()
            .iter()
            .zip(expected.residuals.as_slice())
        {
            assert!((a - b).abs() < 1e-10);
        }
        if let Some(expected) = &reference {
            assert_eq!(expected, &actual);
        } else {
            reference = Some(actual);
        }
    }
}

#[test]
fn exhaustion_never_resets_work_and_preserves_the_whole_primary_bank_report() {
    let sample = support::prepared(&[17, 13, 5], true, 10, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = bank(sample.n_active());
    let team = ThreadTeam::new(1).unwrap();
    let mut engine = Engine::with_controls(
        &design,
        Estimator::Ols,
        &options(Solver::Auto, 2),
        PlanOverride::Automatic,
        MonitorControl::StopAfter(1),
        &team,
    )
    .unwrap();
    assert_eq!(
        engine.project(&rhs, &team).unwrap_err().code,
        ErrorCode::NonConvergence
    );
    let reports = engine.last_diagnostics();
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].attempts.len(), 2);
    assert_eq!(reports[0].iterations, 2);
    assert_eq!(reports[0].attempts[1].iterations, 1);
    assert_eq!(reports[0].stop, StopReason::BudgetExhausted);
    assert_eq!(reports[1].iterations, 0);
    assert_eq!(reports[2].iterations, 1);
    for solver in [Solver::Auto, Solver::Lsmr] {
        let mut engine = Engine::new(&design, Estimator::Ols, &options(solver, 1), &team).unwrap();
        assert_eq!(
            engine.project(&rhs, &team).unwrap_err().code,
            ErrorCode::NonConvergence
        );
        assert_eq!(engine.last_diagnostics().len(), 3);
        assert_eq!(engine.last_diagnostics()[0].attempts.len(), 1);
        assert_eq!(engine.last_diagnostics()[0].iterations, 1);
        assert_eq!(engine.last_diagnostics()[2].iterations, 1);
    }
}

#[test]
fn explicit_cmg_dispatches_supported_structures_and_reports_setup_once() {
    for levels in [vec![], vec![5], vec![7, 5], vec![7, 5, 3], vec![7, 5, 3, 2]] {
        let sample = support::prepared(&levels, false, 30, 0);
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let rhs = bank(sample.n_active());
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let mut engine =
                Engine::new(&design, Estimator::Ols, &options(Solver::Cmg, 1000), &team).unwrap();
            let actual = engine.project(&rhs, &team).unwrap();
            let used = match levels.len() {
                0 => Backend::NoEffects,
                1 => Backend::Direct,
                _ => Backend::Cmg,
            };
            assert!(actual.diagnostics.iter().all(|d| d.requested == Solver::Cmg
                && d.selected == Backend::Cmg
                && d.used == used
                && d.attempts.len() == 1));
            if levels.len() >= 2 {
                assert_eq!(actual.setup_diagnostics.len(), 1);
                assert_eq!(
                    actual.setup_diagnostics[0].right_hand_sides.len(),
                    levels.iter().skip(2).map(|l| l - 1).sum::<usize>()
                );
                assert_eq!(engine.last_setup_diagnostics(), actual.setup_diagnostics);
            } else {
                assert!(actual.setup_diagnostics.is_empty());
            }
            let repeated = engine.project(&rhs, &team).unwrap();
            assert!(repeated.setup_diagnostics.is_empty());
            assert_eq!(actual.coefficients, repeated.coefficients);
            assert_eq!(actual.diagnostics, repeated.diagnostics);
            if let Some(expected) = &reference {
                assert_eq!(expected, &actual);
            } else {
                reference = Some(actual);
            }
            let ppml = Engine::new(&design, Estimator::Ppml, &options(Solver::Cmg, 1000), &team);
            if (2..=3).contains(&levels.len()) {
                assert!(ppml.is_ok());
            } else {
                assert_eq!(ppml.err().unwrap().code, ErrorCode::Unsupported);
            }
        }
    }
}

#[test]
fn failed_cmg_setup_is_visible_and_reused_without_resetting_auxiliary_budgets() {
    let sample = support::prepared(&[1103, 401, 7], false, 6, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::zeros(sample.n_active(), 1).unwrap();
    let team = ThreadTeam::new(2).unwrap();
    let mut engine = Engine::new(&design, Estimator::Ols, &options(Solver::Cmg, 1), &team).unwrap();
    let actual = engine.project(&rhs, &team).unwrap();
    let setup = &actual.setup_diagnostics[0];
    assert_eq!(setup.stop, StopReason::BudgetExhausted);
    assert_eq!(setup.right_hand_sides.len(), 6);
    assert!(setup
        .right_hand_sides
        .iter()
        .all(|p| p.iterations == 1 && p.stop == StopReason::BudgetExhausted));
    assert_eq!(actual.diagnostics[0].attempts.len(), 2);
    assert_eq!(actual.diagnostics[0].iterations, 0);
    assert_eq!(actual.diagnostics[0].used, Backend::Lsmr);
    assert_eq!(
        actual.diagnostics[0].fallback,
        Some(FallbackReason::NumericalNonConvergence)
    );
    let repeated = engine.project(&rhs, &team).unwrap();
    assert!(repeated.setup_diagnostics.is_empty());
    assert_eq!(actual.diagnostics, repeated.diagnostics);
}

#[test]
fn certificate_failure_transfers_once_then_fails_closed_at_the_representation_floor() {
    let n = 80;
    let z: Vec<_> = (0..n).map(|i| (i as f64 * 0.37).sin()).collect();
    let v: Vec<_> = (0..n).map(|i| (i as f64 * 0.23).cos()).collect();
    let second: Vec<_> = z.iter().zip(&v).map(|(a, b)| a + 1e-6 * b).collect();
    let sample = support::slope_sample(false, &vec![0; n], &[z, second]);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::from_columns(&[vec![0.; n], v.clone(), v]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut controls = options(Solver::Auto, 1000);
    controls.tolerance = 1e-12;
    let mut engine = Engine::new(&design, Estimator::Ols, &controls, &team).unwrap();
    assert_eq!(
        engine.project(&rhs, &team).unwrap_err().code,
        ErrorCode::NonConvergence
    );
    let reports = engine.last_diagnostics();
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].used, Backend::Direct);
    assert_eq!(reports[0].attempts.len(), 1);
    assert_eq!(reports[0].fallback, None);
    assert_eq!(reports[2].attempts.len(), 1);
    assert_eq!(reports[2].iterations, 0);
    let report = &reports[1];
    assert_eq!(report.attempts.len(), 2);
    assert_eq!(report.attempts[0].backend, Backend::Direct);
    assert_eq!(report.attempts[0].iterations, 0);
    assert_eq!(report.attempts[0].stop, StopReason::CertificateFailure);
    assert_eq!(report.fallback, Some(FallbackReason::CertificateFailure));
    assert_eq!(report.attempts[1].backend, Backend::Lsmr);
    assert_eq!(report.stop, StopReason::Stagnation);
    assert!(report.iterations > 0 && report.iterations <= controls.iterations);
}

#[test]
fn bank_acceptance_matches_individual_original_input_certificates_exactly() {
    for levels in [vec![7], vec![7, 5, 3]] {
        let sample = support::prepared(&levels, true, 30, 0);
        let weights: Vec<_> = (0..sample.n_active())
            .map(|row| 1. + (row % 3) as f64)
            .collect();
        let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
        let rhs = bank(sample.n_active());
        let controls = options(Solver::Auto, 1000);
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let mut engine = Engine::with_controls(
                &design,
                Estimator::Ols,
                &controls,
                PlanOverride::Automatic,
                MonitorControl::Disabled,
                &team,
            )
            .unwrap();
            let actual = engine.project(&rhs, &team).unwrap();
            assert!(actual.diagnostics.iter().all(|d| d.attempts.len() == 1));
            let raw = if levels.len() == 1 {
                design.direct_candidate(&rhs, &team).unwrap().coefficients
            } else {
                let plan = Plan::new(&design, Estimator::Ols, &controls).unwrap();
                SchurSession::new(&design, &plan, &team)
                    .unwrap()
                    .solve(&rhs, &team)
                    .unwrap()
                    .coefficients
            };
            for column in 0..rhs.ncols() {
                let independent = design
                    .finish_candidate(
                        &DenseMatrix::from_columns(&[rhs.column(column).unwrap()]).unwrap(),
                        DenseMatrix::from_columns(&[raw.column(column).unwrap()]).unwrap(),
                        vec![actual.diagnostics[column].clone()],
                        controls.tolerance,
                        &team,
                    )
                    .unwrap();
                assert_eq!(
                    actual.coefficients.column(column).unwrap(),
                    independent.coefficients.column(0).unwrap()
                );
                assert_eq!(
                    actual.residuals.column(column).unwrap(),
                    independent.residuals.column(0).unwrap()
                );
                let a = &actual.certificate;
                let b = &independent.certificate;
                for (full, single) in [
                    (&a.relative_score, &b.relative_score),
                    (&a.relative_score_rms, &b.relative_score_rms),
                    (&a.raw_score_max, &b.raw_score_max),
                    (&a.weighted_rhs_norm, &b.weighted_rhs_norm),
                    (&a.weighted_residual_norm, &b.weighted_residual_norm),
                    (&a.relative_reconstruction, &b.relative_reconstruction),
                ] {
                    assert_eq!(full[column], single[0]);
                }
            }
            if let Some(expected) = &reference {
                assert_eq!(expected, &actual);
            } else {
                reference = Some(actual);
            }
        }
    }
}
