use fereg_engine::{
    data::*,
    ppml::separation::*,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    wls::Role,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode, ModelSpec, SourceEntry, SourceId,
};
mod support;
fn make_model<'a>(raw: &'a DenseMatrix, roles: &'a [Role], solver: Solver) -> Model<'a> {
    Model {
        probe: Default::default(),
        raw,
        roles,
        options: Options {
            solver,
            ..Options::for_estimator(Estimator::Ppml)
        },
        controls: Controls::default(),
    }
}
fn ledger(sample: Prepared) -> Ledger {
    Ledger::new(
        sample,
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::IteratedRectifier),
        },
    )
    .unwrap()
}
fn categorical(codes: &[Vec<u32>], outcome: &[f64]) -> Prepared {
    let spec = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..codes.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("g{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (0..codes.len())
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                categorical_sources: vec![SourceId::new(i as u32 + 1).unwrap()],
                intercept: true,
                slope_sources: vec![],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    prepare_fixed_effects(
        &spec,
        Prepared {
            topology: None,
            n_original: outcome.len(),
            n_candidate: outcome.len(),
            original_row_by_active: (0..outcome.len()).collect(),
            outcome: outcome.to_vec(),
            regressors: codes
                .iter()
                .map(|v| v.iter().map(|&v| f64::from(v)).collect())
                .collect(),
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; outcome.len()],
        },
    )
    .unwrap()
}
#[test]
fn combined_two_fe_ray_matches_the_retained_legacy_regression_and_commits_atomically() {
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let mut ledger = ledger(categorical(
            &[vec![0, 0, 1, 1, 1], vec![0, 0, 0, 1, 1]],
            &[0., 1., 0., 0., 1.],
        ));
        let raw = DenseMatrix::zeros(5, 0).unwrap();
        let model = make_model(&raw, &[], solver);
        assert!(ledger.view().exact_proposal().unwrap().rows.is_empty());
        let one = ThreadTeam::new(1).unwrap();
        let report = ledger.view().iterated_rectifier(&model, 100, &one).unwrap();
        assert_eq!(report.status, Status::Certified);
        let proposal = report.direction.as_ref().unwrap();
        assert_eq!(proposal.rows, [2]);
        assert_eq!(
            report.work.projection().calls(),
            report.iterations + report.span_frames
        );
        for workers in [2, 4, 8] {
            assert_eq!(
                report,
                ledger
                    .view()
                    .iterated_rectifier(&model, 100, &ThreadTeam::new(workers).unwrap())
                    .unwrap()
            );
        }
        assert_eq!(
            ledger
                .view()
                .replay_ir(&report, &model, 100, &one)
                .unwrap_err()
                .code,
            ErrorCode::NonConvergence
        );
        let accepted = ledger.accept_direction(proposal, &model, &one).unwrap();
        assert_eq!(accepted.commit.primary, 1);
        assert_eq!(accepted.validation.work.projection().calls(), 1);
        assert_eq!(ledger.view().sample().retained_rows(), [0, 1, 3, 4]);
        let raw = DenseMatrix::zeros(4, 0).unwrap();
        let model = make_model(&raw, &[], solver);
        let terminal = ledger.view().iterated_rectifier(&model, 100, &one).unwrap();
        assert_eq!(terminal.status, Status::NoSeparation);
        let replay = ledger
            .view()
            .replay_ir(&terminal, &model, 100, &one)
            .unwrap();
        assert_eq!(replay.generation(), 1);
        assert_eq!(replay.iterations(), terminal.iterations);
        assert_eq!(replay.work(), terminal.work);
    }
}
#[test]
fn paired_positive_rows_prove_no_separation_and_require_fresh_terminal_replay() {
    let mut sample = support::prepared(&[], false, 6, 0);
    let n = sample.n_active();
    sample.outcome = (0..n).map(|i| if i % 2 == 0 { 0. } else { 1. }).collect();
    let ledger = ledger(sample);
    let raw = DenseMatrix::from_columns(&[vec![1.; n], (0..n).map(|i| (i / 2) as f64).collect()])
        .unwrap();
    let model = make_model(&raw, &[Role::Ordinary; 2], Solver::Auto);
    let one = ThreadTeam::new(1).unwrap();
    let report = ledger.view().iterated_rectifier(&model, 100, &one).unwrap();
    assert_eq!(report.status, Status::NoSeparation);
    assert!(report.direction.is_none());
    assert!(report.fixed_point.unwrap().boundary_violation_relative >= 0.);
    let replay = ledger.view().replay_ir(&report, &model, 100, &one).unwrap();
    assert_eq!(replay.status(), Status::NoSeparation);
    assert_eq!(replay.sample_identity(), report.sample_identity);
    assert_eq!(replay.work(), report.work);
    for workers in [2, 4, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        assert_eq!(
            report,
            ledger
                .view()
                .iterated_rectifier(&model, 100, &team)
                .unwrap()
        );
        assert_eq!(
            replay,
            ledger
                .view()
                .replay_ir(&report, &model, 100, &team)
                .unwrap()
        );
    }
    for field in 0..6 {
        let mut changed = report.clone();
        match field {
            0 => changed.iterations += 1,
            1 => {
                changed
                    .fixed_point
                    .as_mut()
                    .unwrap()
                    .boundary_violation_relative = -1.
            }
            2 => changed.fixed_point.as_mut().unwrap().direction_scale *= 2.,
            3 => changed.controls.tolerance *= 2.,
            4 => changed.status = Status::NoCandidates,
            _ => changed.work.merge(report.work).unwrap(),
        }
        assert_eq!(
            ledger
                .view()
                .replay_ir(&changed, &model, 100, &one)
                .unwrap_err()
                .code,
            ErrorCode::NonConvergence
        );
    }
    let mut changed = report.clone();
    changed.generation += 1;
    assert_eq!(
        ledger
            .view()
            .replay_ir(&changed, &model, 100, &one)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        ledger
            .view()
            .replay_ir(&report, &model, 99, &one)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    // A new zero-only regressor creates a cone ray despite the old fixed-point
    // fields. Replaying over the actual model must find it and reject the claim.
    let raw = DenseMatrix::from_columns(&[
        vec![1.; n],
        (0..n).map(|i| (i / 2) as f64).collect(),
        (0..n).map(|i| if i % 2 == 0 { 1. } else { 0. }).collect(),
    ])
    .unwrap();
    let changed_model = make_model(&raw, &[Role::Ordinary; 3], Solver::Auto);
    assert_eq!(
        ledger
            .view()
            .replay_ir(&report, &changed_model, 100, &one)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}
#[test]
fn slope_only_rays_ignore_prior_weights_and_adjustments_but_need_a_span_certificate() {
    let n = 90;
    let codes: Vec<_> = (0..n).map(|i| (i % 3) as u32).collect();
    let slope: Vec<_> = (0..n).map(|i| if i % 5 == 0 { 1. } else { 0. }).collect();
    let mut sample = support::slope_sample(false, &codes, &[slope.clone()]);
    sample.outcome = slope.iter().map(|&v| 1. - v).collect();
    let mut weighted = support::slope_sample(false, &codes, &[slope.clone()]);
    weighted.outcome = sample.outcome.clone();
    weighted.weights = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: (0..n).map(|i| 1. + (i % 7) as f64).collect(),
    };
    weighted.adjustment = PreparedAdjustment::Offset(vec![1e300; n]);
    let plain = ledger(sample);
    let weighted = ledger(weighted);
    assert_eq!(
        plain.view().exact_proposal().unwrap().status,
        Status::NoCandidates
    );
    let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let model = make_model(&raw, &[Role::Ordinary], Solver::Auto);
    let team = ThreadTeam::new(1).unwrap();
    let report = plain.view().iterated_rectifier(&model, 100, &team).unwrap();
    assert_eq!(
        report,
        weighted
            .view()
            .iterated_rectifier(&model, 100, &team)
            .unwrap()
    );
    let p = report.direction.as_ref().unwrap();
    assert_eq!(p.rows, (0..n).step_by(5).collect::<Vec<_>>());
    plain.view().certify_direction(p, &model, &team).unwrap();
}
#[test]
fn unresolved_rectifier_is_fatal_and_never_claims_no_separation() {
    let mut sample = support::prepared(&[], false, 1, 0);
    sample.outcome = vec![1., 1., 0., 0.];
    let ledger = ledger(sample);
    let raw = DenseMatrix::from_columns(&[vec![1.; 4], vec![0., 0., 1., -2.]]).unwrap();
    let model = make_model(&raw, &[Role::Ordinary; 2], Solver::Auto);
    let digest = ledger.trace_digest();
    for budget in [1, 100] {
        let error = ledger
            .view()
            .iterated_rectifier(&model, budget, &ThreadTeam::new(1).unwrap())
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::NonConvergence);
        assert!(error.message.contains("exhausted"));
        assert_eq!(ledger.trace_digest(), digest);
    }
}
#[test]
fn no_candidate_path_still_checks_budget_alignment_and_explicit_solver_support() {
    let sample = support::prepared(&[], false, 1, 0);
    let n = sample.n_active();
    let ledger = ledger(sample);
    let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let model = make_model(&raw, &[Role::Ordinary], Solver::Auto);
    let team = ThreadTeam::new(1).unwrap();
    let report = ledger
        .view()
        .iterated_rectifier(&model, 100, &team)
        .unwrap();
    assert_eq!(report.status, Status::NoCandidates);
    assert_eq!(report.iterations, 0);
    assert_eq!(report.span_frames, 0);
    assert_eq!(report.work.counts().right_hand_sides, 0);
    ledger
        .view()
        .replay_ir(&report, &model, 100, &team)
        .unwrap();
    assert_eq!(
        ledger
            .view()
            .iterated_rectifier(&model, 0, &team)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
    let unsupported = make_model(&raw, &[Role::Ordinary], Solver::Cmg);
    assert_eq!(
        ledger
            .view()
            .iterated_rectifier(&unsupported, 100, &team)
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    let malformed = make_model(&raw, &[], Solver::Auto);
    assert_eq!(
        ledger
            .view()
            .iterated_rectifier(&malformed, 100, &team)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn detector_span_cannot_inherit_the_estimators_fe_collinearity_threshold() {
    use fereg_engine::{
        fe::{Design, Engine},
        wls,
    };
    let n = 128;
    let y: Vec<_> = (0..n)
        .map(|row| if row % 4 == 0 { 0. } else { 1. })
        .collect();
    let ledger = ledger(categorical(&[vec![0; n]], &y));
    let contrast = 2_f64.powi(-16);
    let raw = DenseMatrix::from_columns(&[(0..n)
        .map(|row| 1. - if row % 4 == 0 { contrast } else { 0. })
        .collect()])
    .unwrap();
    let one = ThreadTeam::new(1).unwrap();
    let design = Design::new(ledger.view().sample(), Weights::Unit).unwrap();
    let mut engine = Engine::new(
        &design,
        Estimator::Ppml,
        &Options::for_estimator(Estimator::Ppml),
        &one,
    )
    .unwrap();
    let ordinary = wls::solve_absorbed(
        &mut engine,
        &raw,
        &vec![0.; n],
        &[Role::Ordinary],
        wls::Policy::default(),
        &one,
    )
    .unwrap();
    assert_eq!(
        ordinary.finite.statuses,
        [wls::Status::CollinearWithFixedEffects]
    );
    // x-1 is an exact negative cone ray despite the estimator's omission
    // threshold. Auxiliary span rank has no public-omission authority.
    let model = make_model(&raw, &[Role::Ordinary], Solver::Auto);
    let report = ledger.view().iterated_rectifier(&model, 100, &one).unwrap();
    assert_eq!(report.status, Status::Certified);
    let p = report.direction.as_ref().unwrap();
    assert_eq!(p.rows, (0..n).step_by(4).collect::<Vec<_>>());
    ledger.view().certify_direction(p, &model, &one).unwrap();
}

#[test]
fn full_model_ir_and_fresh_span_replay_are_identical_across_large_fixed_partitions() {
    let mut sample = support::prepared(&[37, 31, 7], false, 120, 0);
    let n = sample.n_active();
    sample.outcome = (0..n)
        .map(|row| if row % 13 == 0 { 0. } else { 1. })
        .collect();
    let ledger = ledger(sample);
    let raw = DenseMatrix::from_columns(&[(0..n)
        .map(|row| if row % 13 == 0 { -1. } else { 0. })
        .collect()])
    .unwrap();
    let expected: Vec<_> = (0..n).step_by(13).collect();
    for solver in [Solver::Auto, Solver::Lsmr, Solver::Cmg] {
        let model = make_model(&raw, &[Role::Ordinary], solver);
        let mut reference = None;
        for workers in [1, 2, 4, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let report = ledger
                .view()
                .iterated_rectifier(&model, 100, &team)
                .unwrap();
            assert_eq!(report.status, Status::Certified);
            assert_eq!(report.linear_tolerance, 1e-10);
            assert_eq!(report.span_tolerance, 1e-12);
            let p = report.direction.as_ref().unwrap();
            assert_eq!(p.rows, expected);
            let span = ledger.view().certify_direction(p, &model, &team).unwrap();
            assert_eq!(
                report.work.projection().calls(),
                report.iterations + report.span_frames
            );
            assert!(report.work.counts().maximum_rhs_iterations <= 16_000);
            if let Some((prior, prior_span)) = &reference {
                assert_eq!(prior, &report);
                assert_eq!(prior_span, &span);
            } else {
                reference = Some((report, span));
            }
        }
    }
}
