use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, LsmrSession, StopReason, Weights},
    threads::ThreadTeam,
};
mod support;

#[test]
fn weighted_multiway_candidates_are_thread_identical_and_certified() {
    let sample = support::prepared(&[31, 17, 7], false, 650, 0);
    let weights: Vec<_> = (0..sample.n_active())
        .map(|i| 0.7 + (i % 13) as f64 / 7.)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..sample.n_active())
            .map(|i| (i as f64 * 0.123).sin())
            .collect(),
        (0..sample.n_active())
            .map(|i| (i % 31 + i % 17) as f64)
            .collect(),
        vec![0.; sample.n_active()],
    ])
    .unwrap();
    let mut expected = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let session = LsmrSession::new(&design, &team).unwrap();
        let candidate = session.solve(&rhs, 1000, 1e-12, &team).unwrap();
        assert!(candidate
            .progress
            .iter()
            .all(|p| p.stop == StopReason::Converged));
        assert_eq!(candidate.progress[2].iterations, 0);
        let residuals = design
            .reconstruct(&rhs, &candidate.coefficients, &team)
            .unwrap();
        let certificate = design
            .certify(&rhs, &candidate.coefficients, &residuals, 1e-10, &team)
            .unwrap();
        let observed = (candidate, residuals, certificate);
        if let Some(reference) = &expected {
            assert_eq!(reference, &observed);
        } else {
            expected = Some(observed);
        }
    }
}

#[test]
fn local_nearly_collinear_slopes_preserve_the_declared_span() {
    let n = 80;
    let z: Vec<f64> = (0..n).map(|i| (i as f64 * 0.37).sin()).collect();
    let v: Vec<f64> = (0..n).map(|i| (i as f64 * 0.23).cos()).collect();
    let second: Vec<f64> = z.iter().zip(&v).map(|(a, b)| a + 1e-6 * b).collect();
    let sample = support::slope_sample(false, &vec![0; n], &[z, second]);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::from_columns(&[v]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let candidate = LsmrSession::new(&design, &team)
        .unwrap()
        .solve(&rhs, 1000, 1e-12, &team)
        .unwrap();
    let residual = design
        .reconstruct(&rhs, &candidate.coefficients, &team)
        .unwrap();
    let direct = design.direct_candidate(&rhs, &team).unwrap();
    assert!(direct.residuals.as_slice().iter().all(|r| r.abs() < 2e-8));
    design
        .certify(&rhs, &candidate.coefficients, &residual, 1e-10, &team)
        .unwrap();
    assert!(residual.as_slice().iter().all(|r| r.abs() < 2e-8));
    // At a tighter target the raw-coordinate representation reaches its
    // cancellation floor. Refinement must charge work and fail closed.
    let session = LsmrSession::new(&design, &team).unwrap();
    assert!(design
        .certify(&rhs, &candidate.coefficients, &residual, 1e-12, &team)
        .is_err());
    let first_iterations = candidate.progress[0].iterations;
    let bounded = session
        .solve_refined(&rhs, first_iterations + 1, 1e-12, &team)
        .unwrap();
    assert_eq!(bounded.progress[0].iterations, first_iterations + 1);
    assert_eq!(bounded.progress[0].refinements, 1);
    assert_eq!(bounded.progress[0].stop, StopReason::BudgetExhausted);
    let stalled = session.solve_refined(&rhs, 1000, 1e-12, &team).unwrap();
    assert_eq!(stalled.progress[0].stop, StopReason::Stagnation);
    assert!(stalled.progress[0].iterations > first_iterations);
    assert!(stalled.progress[0].refinements > 0);
    let eight = ThreadTeam::new(8).unwrap();
    assert_eq!(
        stalled,
        LsmrSession::new(&design, &eight)
            .unwrap()
            .solve_refined(&rhs, 1000, 1e-12, &eight)
            .unwrap()
    );
}

#[test]
fn explicit_engine_projects_empty_and_single_effects_with_original_equations() {
    use fereg_engine::{
        fe::{Backend, Engine},
        spec::{Estimator, Options, Solver},
    };
    for sample in [
        support::prepared(&[], false, 17, 0),
        support::prepared(&[5], true, 17, 0),
    ] {
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let rhs = DenseMatrix::from_columns(&[(0..sample.n_active())
            .map(|i| (i as f64 * 0.17).sin())
            .collect()])
        .unwrap();
        let options = Options {
            solver: Solver::Lsmr,
            tolerance: 1e-10,
            ..Options::for_estimator(Estimator::Ols)
        };
        let team = ThreadTeam::new(2).unwrap();
        let actual = Engine::new(&design, Estimator::Ols, &options, &team)
            .unwrap()
            .project(&rhs, &team)
            .unwrap();
        let expected = design.direct_candidate(&rhs, &team).unwrap();
        for (a, b) in actual
            .residuals
            .as_slice()
            .iter()
            .zip(expected.residuals.as_slice())
        {
            assert!((a - b).abs() < 1e-9);
        }
        assert_eq!(actual.diagnostics[0].used, Backend::Lsmr);
        assert_eq!(actual.diagnostics[0].attempts.len(), 1);
        assert_eq!(
            actual.residuals,
            design
                .reconstruct(&rhs, &actual.coefficients, &team)
                .unwrap()
        );
    }
}

#[test]
fn exhausted_and_malformed_lsmr_requests_cannot_publish_a_projection() {
    use fereg_engine::{
        fe::Engine,
        spec::{Estimator, Options, Solver},
        ErrorCode,
    };
    let sample = support::prepared(&[17, 13, 5], true, 10, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..sample.n_active()).map(|i| (i as f64).sin()).collect(),
        vec![0.; sample.n_active()],
    ])
    .unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let session = LsmrSession::new(&design, &team).unwrap();
    let candidate = session.solve(&rhs, 1, 1e-12, &team).unwrap();
    assert_eq!(candidate.progress[0].iterations, 1);
    assert_eq!(candidate.progress[0].stop, StopReason::BudgetExhausted);
    assert_eq!(candidate.progress[1].iterations, 0);
    let options = Options {
        solver: Solver::Lsmr,
        iterations: 1,
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ols)
    };
    assert_eq!(
        Engine::new(&design, Estimator::Ols, &options, &team)
            .unwrap()
            .project(&rhs, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    assert_eq!(
        session.solve(&rhs, 0, 1e-12, &team).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert!(session
        .solve(&DenseMatrix::zeros(1, 1).unwrap(), 100, 1e-12, &team)
        .is_err());
    let mut invalid = rhs;
    invalid[(0, 0)] = f64::NAN;
    assert!(session.solve(&invalid, 100, 1e-12, &team).is_err());
}

#[test]
fn reference_weight_frames_match_their_materialized_objective_weights() {
    use fereg_engine::{
        data::{PreparedWeights, ReferenceWeights, WeightKind},
        fe::Engine,
        spec::{Estimator, Options, Solver},
    };
    let mut sample = support::prepared(&[5, 3, 2], false, 30, 0);
    let n = sample.n_active();
    let rhs =
        DenseMatrix::from_columns(&[(0..n).map(|i| (i as f64 * 0.31).sin()).collect()]).unwrap();
    let options = Options {
        solver: Solver::Lsmr,
        ..Options::for_estimator(Estimator::Ols)
    };
    let team = ThreadTeam::new(2).unwrap();
    for kind in [
        WeightKind::Analytic,
        WeightKind::Frequency,
        WeightKind::Probability,
    ] {
        sample.weights = PreparedWeights::Provided {
            kind,
            values: (0..n).map(|i| (1 + i % 13) as f64).collect(),
        };
        let q = ReferenceWeights::new(&sample.weights, n, Estimator::Ols).unwrap();
        let values: Vec<_> = (0..n).map(|i| q.value(i)).collect();
        let reference = Design::new(&sample, Weights::Reference(q)).unwrap();
        let materialized = Design::new(&sample, Weights::Provided(&values)).unwrap();
        let mut actual = Engine::new(&reference, Estimator::Ols, &options, &team).unwrap();
        let mut expected = Engine::new(&materialized, Estimator::Ols, &options, &team).unwrap();
        assert_eq!(
            actual.project(&rhs, &team).unwrap(),
            expected.project(&rhs, &team).unwrap()
        );
        let empty = actual
            .project(&DenseMatrix::zeros(n, 0).unwrap(), &team)
            .unwrap();
        assert!(empty.diagnostics.is_empty());
        assert_eq!(empty.residuals.nrows(), n);
        assert_eq!(empty.residuals.ncols(), 0);
    }
}
