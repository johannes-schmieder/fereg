use fereg_engine::{
    data::DenseMatrix,
    fe::{CmgSession, Design, StopReason, Weights},
    threads::ThreadTeam,
    ErrorCode,
};
mod support;

#[test]
fn nested_and_duplicate_nuisance_effects_keep_their_span_and_setup_work() {
    for (levels, expected_rank) in [(vec![30, 7, 2, 3, 5, 6], 0), (vec![30, 13, 3, 11, 11], 10)] {
        let sample = support::prepared(&levels, false, 22, 0);
        let n = sample.n_active();
        let weights: Vec<_> = (0..n).map(|i| 0.7 + (i % 13) as f64 / 7.).collect();
        let design = Design::new(&sample, Weights::Provided(&weights)).unwrap();
        let rhs = DenseMatrix::from_columns(&[
            (0..n).map(|i| (i as f64 * 0.123).sin()).collect(),
            (0..n).map(|i| (i % 11) as f64).collect(),
            vec![0.; n],
        ])
        .unwrap();
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let mut session = CmgSession::new(&design, [0, 1]).unwrap();
            assert_eq!(session.nuisance_rank(), None);
            assert!(session.solve(&rhs, 1000, 1e-12, &team).is_err());
            session.prepare(1000, 1e-12, &team).unwrap();
            assert_eq!(session.nuisance_rank(), Some(expected_rank));
            assert_eq!(
                session.setup_progress().len(),
                levels
                    .iter()
                    .skip(2)
                    .map(|levels| levels - 1)
                    .sum::<usize>()
            );
            assert!(session
                .setup_progress()
                .iter()
                .all(|p| p.stop == StopReason::Converged && p.iterations <= 1000));
            let mut candidate = session.solve(&rhs, 1000, 1e-12, &team).unwrap();
            assert!(candidate
                .progress
                .iter()
                .all(|p| p.stop == StopReason::Converged));
            assert_eq!(candidate.progress[2].iterations, 0);
            let unnormalized = session.solve(&rhs, 1000, 1e-12, &team).unwrap();
            assert_eq!(candidate, unnormalized);
            let raw_residuals = design
                .reconstruct(&rhs, &candidate.coefficients, &team)
                .unwrap();
            design
                .certify(&rhs, &candidate.coefficients, &raw_residuals, 1e-10, &team)
                .unwrap_or_else(|error| panic!("levels {levels:?}: {error:?}"));
            design
                .normalize_coefficients(&mut candidate.coefficients, 1e-10, &team)
                .unwrap_or_else(|error| panic!("levels {levels:?}: {error:?}"));
            let residuals = design
                .reconstruct(&rhs, &candidate.coefficients, &team)
                .unwrap();
            let certificate = design
                .certify(&rhs, &candidate.coefficients, &residuals, 1e-10, &team)
                .unwrap();
            let actual = (
                candidate,
                residuals,
                certificate,
                session.setup_progress().to_vec(),
            );
            if let Some(expected) = &reference {
                assert_eq!(expected, &actual);
            } else {
                reference = Some(actual);
            }
            assert_eq!(
                session.prepare(1000, 1e-12, &team).unwrap_err().code,
                ErrorCode::InvalidArgument
            );
        }
    }
}

#[test]
fn failed_setup_retains_work_and_cannot_reset_its_budget() {
    let sample = support::prepared(&[1103, 401, 7], false, 6, 0);
    let design = Design::new(&sample, Weights::Unit).unwrap();
    let team = ThreadTeam::new(2).unwrap();
    let mut session = CmgSession::new(&design, [0, 1]).unwrap();
    assert_eq!(
        session.prepare(1, 1e-12, &team).unwrap_err().code,
        ErrorCode::NonConvergence
    );
    assert_eq!(session.setup_progress().len(), 6);
    assert!(session
        .setup_progress()
        .iter()
        .all(|p| p.iterations == 1 && p.stop == StopReason::BudgetExhausted));
    let progress = session.setup_progress().to_vec();
    assert_eq!(
        session.prepare(1000, 1e-12, &team).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(session.setup_progress(), progress);
    assert_eq!(session.nuisance_rank(), None);
}

#[test]
fn nuisance_cap_and_slope_support_are_checked_before_solving() {
    for (levels, slopes, supported) in [
        (vec![70, 69, 65], false, true),
        (vec![70, 69, 66], false, false),
        (vec![7, 5, 3], true, false),
    ] {
        let sample = support::prepared(&levels, slopes, 6, 0);
        let design = Design::new(&sample, Weights::Unit).unwrap();
        let session = CmgSession::new(&design, [0, 1]);
        if supported {
            let mut session = session.unwrap();
            let team = ThreadTeam::new(2).unwrap();
            session.prepare(1000, 1e-12, &team).unwrap();
            assert_eq!(session.setup_progress().len(), 64);
            // The 70- and 65-level partitions share five residue classes,
            // so four of the 64 declared nuisance contrasts are redundant.
            assert_eq!(session.nuisance_rank(), Some(60));
            let rhs = DenseMatrix::from_columns(&[(0..sample.n_active())
                .map(|i| (i as f64 * 0.13).sin())
                .collect()])
            .unwrap();
            let candidate = session.solve(&rhs, 1000, 1e-12, &team).unwrap();
            let residuals = design
                .reconstruct(&rhs, &candidate.coefficients, &team)
                .unwrap();
            design
                .certify(&rhs, &candidate.coefficients, &residuals, 1e-10, &team)
                .unwrap();
        } else {
            assert_eq!(session.err().unwrap().code, ErrorCode::Unsupported);
        }
    }
}
