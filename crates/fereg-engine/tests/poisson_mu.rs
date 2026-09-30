use fereg_engine::{
    data::*,
    ppml::{
        irls::{fit_final_sample, MuStop, Problem},
        separation::*,
    },
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
    ErrorCode,
};
mod support;
fn ledger(y: &[f64]) -> Ledger {
    let mut sample = support::slope_sample(true, &vec![0; y.len()], &[]);
    sample.outcome = y.to_vec();
    Ledger::new(
        sample,
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::Mu),
        },
    )
    .unwrap()
}
fn model<'a>(raw: &'a DenseMatrix, roles: &'a [Role]) -> Model<'a> {
    Model {
        probe: Default::default(),
        raw,
        roles,
        controls: Controls::default(),
        options: Options::for_estimator(Estimator::Ppml),
    }
}
#[test]
fn exploratory_budget_can_return_fitted_means_but_cannot_publish_a_fit() {
    let l = ledger(&[1., 0., 1., 0.]);
    let raw = DenseMatrix::zeros(4, 0).unwrap();
    let mut m = model(&raw, &[]);
    m.options.iterations = 1;
    let one = ThreadTeam::new(1).unwrap();
    let r = l.view().fitted_mu(&m, DEFAULT_MU_TOLERANCE, &one).unwrap();
    assert_eq!(r.status, Status::NoCandidates);
    assert_eq!(r.trajectory.terminal, MuStop::MaximumIterations);
    assert_eq!(r.trajectory.accepted_iterations, 1);
    assert_eq!(r.trajectory.certified_linear_frames, 2);
    assert_eq!(r.reference_mean, 0.5);
    assert_eq!(
        r.work.projection().calls(),
        r.trajectory_diagnostics.linear_frames
    );
    let terminal = l
        .view()
        .replay_mu(&r, &m, DEFAULT_MU_TOLERANCE, &one)
        .unwrap();
    assert_eq!(terminal.iterations(), 1);
    assert_eq!(terminal.work(), r.work);
    assert_eq!(
        fit_final_sample(
            &Problem {
                sample: l.view().sample(),
                raw: &raw,
                roles: &[]
            },
            m.options,
            &one
        )
        .unwrap_err()
        .code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn converged_paired_rows_have_fresh_no_candidate_replay_and_tampering_rejection() {
    let l = ledger(&[1., 0., 1., 0.]);
    let raw = DenseMatrix::zeros(4, 0).unwrap();
    let m = model(&raw, &[]);
    let one = ThreadTeam::new(1).unwrap();
    let r = l.view().fitted_mu(&m, DEFAULT_MU_TOLERANCE, &one).unwrap();
    assert_eq!(r.status, Status::NoCandidates);
    assert_eq!(r.trajectory.terminal, MuStop::Converged);
    assert_eq!(r.trajectory.accepted_iterations, 2);
    assert_eq!(r.candidates, 0);
    assert_eq!(r.projection_frames, 0);
    assert!(r.direction.is_none());
    assert!(r.trajectory.state.predictor_relative_max <= 1e-8);
    let terminal = l
        .view()
        .replay_mu(&r, &m, DEFAULT_MU_TOLERANCE, &one)
        .unwrap();
    assert_eq!(terminal.generation(), 0);
    assert_eq!(terminal.sample_identity(), r.sample_identity);
    for workers in [2, 4, 8] {
        assert_eq!(
            r,
            l.view()
                .fitted_mu(&m, DEFAULT_MU_TOLERANCE, &ThreadTeam::new(workers).unwrap())
                .unwrap()
        );
    }
    let mut forged = r.clone();
    forged.trajectory.certified_linear_frames += 1;
    assert_eq!(
        l.view()
            .replay_mu(&forged, &m, DEFAULT_MU_TOLERANCE, &one)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    forged = r.clone();
    forged.generation += 1;
    assert_eq!(
        l.view()
            .replay_mu(&forged, &m, DEFAULT_MU_TOLERANCE, &one)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let changed = DenseMatrix::from_columns(&[vec![0., -1., 0., -1.]]).unwrap();
    let changed_model = model(&changed, &[Role::Ordinary]);
    assert!(l
        .view()
        .replay_mu(&r, &changed_model, DEFAULT_MU_TOLERANCE, &one)
        .is_err());
}
#[test]
fn fitted_mean_ray_is_independent_of_the_discarded_session_after_atomic_drop() {
    let one = ThreadTeam::new(1).unwrap();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let mut sample = support::prepared(&[7, 5], false, 20, 0);
        let n = sample.n_active();
        sample.outcome[0] = 0.;
        sample.weights = PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values: (0..n).map(|r| (1 + r % 3) as f64).collect(),
        };
        let mut l = Ledger::new(
            sample,
            Policy {
                drop_singletons: false,
                methods: Methods::only(Method::Mu),
            },
        )
        .unwrap();
        let mut x = vec![0.; n];
        x[0] = -1.;
        let raw = DenseMatrix::from_columns(&[x]).unwrap();
        let mut m = model(&raw, &[Role::Ordinary]);
        m.options.solver = solver;
        let r = l.view().fitted_mu(&m, DEFAULT_MU_TOLERANCE, &one).unwrap();
        assert_eq!(r.status, Status::Certified);
        assert_eq!(r.trajectory.terminal, MuStop::CandidateThresholdReached);
        assert_eq!(r.candidates, 1);
        assert!(r.trajectory.accepted_iterations > 1);
        assert_eq!(
            r.work.projection().calls(),
            r.trajectory_diagnostics.linear_frames + 1
        );
        let p = r.direction.as_ref().unwrap();
        assert_eq!(p.rows, [0]);
        for workers in [2, 4, 8] {
            assert_eq!(
                r,
                l.view()
                    .fitted_mu(&m, DEFAULT_MU_TOLERANCE, &ThreadTeam::new(workers).unwrap())
                    .unwrap()
            );
        }
        let before = l.trace_digest();
        let accepted = l.accept_direction(p, &m, &one).unwrap();
        assert_eq!(accepted.commit.primary, 1);
        assert_ne!(l.trace_digest(), before);
        assert_eq!(l.view().generation(), 1);
        assert_eq!(l.view().sample().drop_reasons[0], DropReason::MuSeparation);
        assert!(l
            .view()
            .replay_mu(&r, &m, DEFAULT_MU_TOLERANCE, &one)
            .is_err());
        let final_raw = DenseMatrix::zeros(n - 1, 1).unwrap();
        let fit = fit_final_sample(
            &Problem {
                sample: l.view().sample(),
                raw: &final_raw,
                roles: &[Role::Ordinary],
            },
            m.options,
            &one,
        )
        .unwrap();
        assert_eq!(fit.point.finite, [0.]);
        assert!(fit.evaluation.mean.iter().all(|v| (v - 1.).abs() < 1e-10));
        assert!(fit.diagnostics.strict_checks >= 2);
    }
}
#[test]
fn tiny_means_from_offsets_are_not_a_separation_certificate() {
    let mut sample = support::slope_sample(true, &[0, 0, 0], &[]);
    sample.outcome = vec![1., 0., 0.];
    sample.adjustment = PreparedAdjustment::Offset(vec![0., -50., -60.]);
    let l = Ledger::new(
        sample,
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::Mu),
        },
    )
    .unwrap();
    let raw = DenseMatrix::zeros(3, 0).unwrap();
    let m = model(&raw, &[]);
    let before = l.trace_digest();
    let err = l
        .view()
        .fitted_mu(&m, DEFAULT_MU_TOLERANCE, &ThreadTeam::new(1).unwrap())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NonConvergence);
    assert!(err.message.contains("certificate"));
    assert_eq!(l.trace_digest(), before);
    assert!(l
        .view()
        .sample()
        .drop_reasons
        .iter()
        .all(|r| *r == DropReason::Retained));
}
#[test]
fn thresholds_alignment_and_explicit_solver_support_fail_before_any_report() {
    let l = ledger(&[1., 0.]);
    let raw = DenseMatrix::zeros(2, 0).unwrap();
    let mut m = model(&raw, &[]);
    let one = ThreadTeam::new(1).unwrap();
    for threshold in [0., 1., f64::NAN, f64::INFINITY] {
        assert_eq!(
            l.view().fitted_mu(&m, threshold, &one).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }
    m.roles = &[Role::Ordinary];
    assert_eq!(
        l.view()
            .fitted_mu(&m, DEFAULT_MU_TOLERANCE, &one)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let mut sample = support::slope_sample(false, &[0, 0, 1, 1], &[vec![1., 1., 1., 1.]]);
    sample.outcome = vec![1., 0., 1., 0.];
    let l = Ledger::new(
        sample,
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::Mu),
        },
    )
    .unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; 4]]).unwrap();
    let mut m = model(&raw, &[Role::Ordinary]);
    m.options.solver = Solver::Cmg;
    assert_eq!(
        l.view()
            .fitted_mu(&m, DEFAULT_MU_TOLERANCE, &one)
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
}
