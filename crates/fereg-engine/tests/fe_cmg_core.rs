use fereg_engine::{
    data::DenseMatrix,
    fe::{CmgCore, Design, StopReason, Weights},
    threads::ThreadTeam,
    ErrorCode,
};
mod support;

#[test]
fn multigrid_core_is_certified_and_identical_across_workers_and_reused_banks() {
    let sample = support::prepared(&[1103, 401], false, 6, 0);
    let n = sample.n_active();
    let weights: Vec<_> = (0..n).map(|i| 0.7 + (i % 13) as f64 / 7.).collect();
    let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..n).map(|i| (i as f64 * 0.123).sin()).collect(),
        (0..n)
            .map(|i| (i % 1103) as f64 / 1103. - (i % 401) as f64 / 401.)
            .collect(),
        vec![0.; n],
    ])
    .unwrap();
    let mut reference = None;
    for workers in [1, 2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let mut core = CmgCore::new(&design, [0, 1]).unwrap();
        assert!(core.hierarchy_levels() > 1);
        let candidate = core.solve(&rhs, 1000, 1e-10, &team).unwrap();
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
        let single = DenseMatrix::from_columns(&[rhs.column(0).unwrap()]).unwrap();
        let smaller = core.solve(&single, 1000, 1e-10, &team).unwrap();
        assert_eq!(
            smaller.coefficients.column(0).unwrap(),
            candidate.coefficients.column(0).unwrap()
        );
        assert_eq!(candidate, core.solve(&rhs, 1000, 1e-10, &team).unwrap());
        let actual = (candidate, residuals, certificate);
        if let Some(expected) = &reference {
            assert_eq!(expected, &actual);
        } else {
            reference = Some(actual);
        }
    }
}

#[test]
fn budget_and_support_failures_are_visible_without_solver_substitution() {
    let sample = support::prepared(&[1103, 401], false, 6, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let rhs = DenseMatrix::from_columns(&[
        (0..sample.n_active())
            .map(|i| (i as f64 * 0.123).sin())
            .collect(),
        vec![0.; sample.n_active()],
    ])
    .unwrap();
    let team = ThreadTeam::new(2).unwrap();
    let mut core = CmgCore::new(&design, [1, 0]).unwrap();
    let partial = core.solve(&rhs, 1, 1e-12, &team).unwrap();
    assert_eq!(partial.progress[0].iterations, 1);
    assert_eq!(partial.progress[0].stop, StopReason::BudgetExhausted);
    assert_eq!(partial.progress[1].iterations, 0);
    assert_eq!(partial.progress[1].stop, StopReason::Converged);
    assert_eq!(
        core.solve(&rhs, 0, 1e-12, &team).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert!(core
        .solve(&DenseMatrix::zeros(1, 1).unwrap(), 1000, 1e-12, &team)
        .is_err());
    for pair in [[0, 0], [0, 2]] {
        assert_eq!(
            CmgCore::new(&design, pair).err().unwrap().code,
            ErrorCode::Unsupported
        );
    }
    let slopes = support::prepared(&[5, 3], true, 10, 0);
    let design = Design::new(&slopes, Weights::Unit).unwrap();
    assert_eq!(
        CmgCore::new(&design, [0, 1]).err().unwrap().code,
        ErrorCode::Unsupported
    );
}
