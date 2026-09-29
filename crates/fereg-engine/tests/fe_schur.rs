use fereg_engine::{
    data::DenseMatrix,
    fe::{Design, Plan, SchurSession, StopReason, Weights},
    spec::{Estimator, Options},
    threads::ThreadTeam,
};
mod support;
fn options(iterations: usize) -> Options {
    Options {
        tolerance: 1e-12,
        iterations,
        ..Options::for_estimator(Estimator::Ols)
    }
}
#[test]
fn exact_intercept_projection_reconstructs_and_is_thread_identical() {
    let sample = support::prepared(&[7, 5, 3], false, 30, 0);
    let weights: Vec<_> = (0..sample.n_active())
        .map(|row| 1. + (row % 3) as f64)
        .collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..sample.n_active())
            .map(|row| 2. * (row % 7) as f64 - 3. * (row % 5) as f64 + (row % 3) as f64)
            .collect(),
        vec![0.; sample.n_active()],
        (0..sample.n_active())
            .map(|row| (row % 11) as f64)
            .collect(),
    ])
    .unwrap();
    let plan = Plan::new(&design, Estimator::Ols, &options(1000)).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let mut session = SchurSession::new(&design, &plan, &team).unwrap();
    let result = session.solve(&rhs, &team).unwrap();
    assert!(result
        .progress
        .iter()
        .all(|p| p.stop == StopReason::Converged));
    assert_eq!(result.progress[1].iterations, 0);
    assert!(result
        .residuals
        .column(0)
        .unwrap()
        .iter()
        .all(|v| v.abs() < 1e-10));
    assert_eq!(
        result.residuals,
        design
            .reconstruct(&rhs, &result.coefficients, &team)
            .unwrap()
    );
    for threads in [2, 4, 8] {
        let team = ThreadTeam::new(threads).unwrap();
        let mut session = SchurSession::new(&design, &plan, &team).unwrap();
        assert_eq!(result, session.solve(&rhs, &team).unwrap());
    }
    let warm = session.solve(&rhs, &team).unwrap();
    assert!(warm.progress.iter().all(|p| p.iterations == 0));
    session.clear_warm_start();
    assert_eq!(result, session.solve(&rhs, &team).unwrap());
}
#[test]
fn duplicate_terms_singular_slopes_and_disconnected_components_keep_the_span() {
    let sample = support::prepared(&[6, 4, 6], true, 30, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(4).unwrap();
    let rhs = DenseMatrix::from_columns(&[(0..sample.n_active())
        .map(|row| {
            let slope = (row % 7) as f64 - 2.;
            (row % 6) as f64 + (row % 4) as f64 * slope
        })
        .collect()])
    .unwrap();
    let plan = Plan::new(&design, Estimator::Ols, &options(1000)).unwrap();
    let result = SchurSession::new(&design, &plan, &team)
        .unwrap()
        .solve(&rhs, &team)
        .unwrap();
    assert_eq!(result.progress[0].stop, StopReason::Converged);
    assert!(result.residuals.as_slice().iter().all(|v| v.abs() < 1e-9));
}
#[test]
fn each_rhs_owns_its_budget_and_checkpoint() {
    let sample = support::prepared(&[17, 13, 5], true, 10, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..sample.n_active())
            .map(|row| (row as f64).sin())
            .collect(),
        vec![0.; sample.n_active()],
    ])
    .unwrap();
    let plan = Plan::new(&design, Estimator::Ols, &options(1)).unwrap();
    let result = SchurSession::new(&design, &plan, &team)
        .unwrap()
        .solve(&rhs, &team)
        .unwrap();
    assert_eq!(result.progress[0].iterations, 1);
    assert_eq!(result.progress[0].stop, StopReason::BudgetExhausted);
    assert_eq!(result.progress[1].iterations, 0);
    assert_eq!(result.progress[1].stop, StopReason::Converged);
    assert_eq!(
        result.residuals,
        design
            .reconstruct(&rhs, &result.coefficients, &team)
            .unwrap()
    );
}
