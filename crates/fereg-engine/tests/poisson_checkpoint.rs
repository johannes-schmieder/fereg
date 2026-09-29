use fereg_engine::{
    data::*,
    fe::{Design, Engine, Weights},
    ppml::{
        checkpoint::{Checkpoint, Decision, Stage},
        family::Family,
        step::Point,
    },
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{self, FrozenBasis, Policy, Role},
    ErrorCode,
};
mod support;
fn basis(fe: &Design<'_>, raw: &DenseMatrix, roles: &[Role], team: &ThreadTeam) -> FrozenBasis {
    let options = Options {
        tolerance: 1e-12,
        ..Options::for_estimator(Estimator::Ppml)
    };
    let mut engine = Engine::new(fe, Estimator::Ppml, &options, team).unwrap();
    let fit = wls::solve_absorbed(
        &mut engine,
        raw,
        &vec![1.; raw.nrows()],
        roles,
        Policy::default(),
        team,
    )
    .unwrap();
    FrozenBasis::from_fit(&fit.finite).unwrap()
}
fn point(
    fe: &Design<'_>,
    raw: &DenseMatrix,
    finite: Vec<f64>,
    absorbed: DenseMatrix,
    offset: &[f64],
    team: &ThreadTeam,
) -> Point {
    let d = fe.multiply(&absorbed, team).unwrap();
    let eta = (0..raw.nrows())
        .map(|row| {
            offset[row]
                + (0..raw.ncols())
                    .map(|col| raw[(row, col)] * finite[col])
                    .sum::<f64>()
                + d[(row, 0)]
        })
        .collect();
    Point {
        finite,
        absorbed,
        eta,
    }
}
#[test]
fn strict_and_hessian_checks_rebuild_multieffect_sloped_states_identically_across_workers() {
    for levels in [vec![], vec![5], vec![5, 3], vec![5, 3, 2]] {
        let sample = support::prepared(&levels, true, 211, 0);
        let n = sample.n_active();
        let fe = Design::new(&sample, Weights::Unit).unwrap();
        let raw = DenseMatrix::from_columns(&[
            vec![1.; n],
            (0..n).map(|r| ((r * 11) % 19) as f64 / 19.).collect(),
        ])
        .unwrap();
        let roles = [
            if levels.is_empty() {
                Role::Ordinary
            } else {
                Role::AbsorbedIntercept
            },
            Role::Ordinary,
        ];
        let team = ThreadTeam::new(1).unwrap();
        let frozen = basis(&fe, &raw, &roles, &team);
        let offset: Vec<_> = (0..n).map(|i| (i % 7) as f64 * 0.01).collect();
        let alpha = DenseMatrix::from_row_major(
            fe.layout().total,
            1,
            (0..fe.layout().total)
                .map(|i| (i % 5) as f64 * 0.01)
                .collect(),
        )
        .unwrap();
        let accepted = point(
            &fe,
            &raw,
            vec![if levels.is_empty() { 0.2 } else { 0. }, -0.1],
            alpha,
            &offset,
            &team,
        );
        let y: Vec<_> = accepted.eta.iter().map(|v| v.exp()).collect();
        let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
        let family = Family::new(&y, q, Some(&offset)).unwrap();
        let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
        let checkpoint = Checkpoint {
            family: &family,
            design: &fe,
            raw: &raw,
            roles: &roles,
            basis: &frozen,
            options: Options::for_estimator(Estimator::Ppml),
        };
        for stage in [Stage::Fisher, Stage::FinalHessian] {
            let result = checkpoint
                .run(&accepted, &evaluation, stage, &team)
                .unwrap_or_else(|e| panic!("levels={levels:?}, stage={stage:?}: {e}"));
            assert_eq!(result.decision, Decision::Stable);
            let candidate = result.candidate.as_ref().unwrap();
            assert_eq!(candidate.hessian_weights, evaluation.mean);
            assert!(candidate.basis_unchanged);
            assert!(candidate.scores.passed);
            for workers in [2, 4, 8] {
                assert_eq!(
                    result,
                    checkpoint
                        .run(
                            &accepted,
                            &evaluation,
                            stage,
                            &ThreadTeam::new(workers).unwrap()
                        )
                        .unwrap()
                );
            }
        }
        assert_eq!(evaluation, family.evaluate(&accepted.eta, &team).unwrap());
    }
}
#[test]
fn near_zero_coefficient_movement_resumes_despite_small_scores_and_objective_change() {
    let sample = support::prepared(&[], false, 5, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let roles = [Role::Ordinary];
    let frozen = basis(&fe, &raw, &roles, &team);
    let y = vec![1.; n];
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let accepted = Point {
        finite: vec![1e-10],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![1e-10; n],
    };
    let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &roles,
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    let result = checkpoint
        .run(&accepted, &evaluation, Stage::Fisher, &team)
        .unwrap();
    assert!(result.accepted_scores.passed);
    assert_eq!(result.decision, Decision::ResumeCandidate);
    let candidate = result.candidate.unwrap();
    assert!(candidate.movement.coefficients > 1e-8);
    assert!(candidate.movement.predictor < 1e-8);
    assert!(candidate.movement.deviance < 1e-8);
    assert_eq!(accepted.finite, vec![1e-10]);
    // With no Fisher correction the linear point barely moves, but a large
    // score still prevents the final-Hessian checkpoint from claiming stability.
    let displaced = Point {
        finite: vec![0.1],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![0.1; n],
    };
    let displaced_eval = family.evaluate(&displaced.eta, &team).unwrap();
    let result = checkpoint
        .run(&displaced, &displaced_eval, Stage::FinalHessian, &team)
        .unwrap();
    assert!(!result.accepted_scores.passed);
    assert_eq!(result.decision, Decision::ResumeCandidate);
}
#[test]
fn final_hessian_rejects_a_same_rank_pivot_change_without_mutating_accepted_state() {
    let sample = support::prepared(&[1], false, 35, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let x: Vec<_> = (0..n).map(|r| (r % 5) as f64 - 2.).collect();
    let raw = DenseMatrix::from_columns(&[x.clone(), x.iter().map(|v| v + 2.).collect()]).unwrap();
    let roles = [Role::Ordinary; 2];
    let team = ThreadTeam::new(1).unwrap();
    let frozen = basis(&fe, &raw, &roles, &team);
    assert_eq!(frozen.kept(), &[0]);
    let accepted = point(
        &fe,
        &raw,
        vec![-2., 0.],
        DenseMatrix::zeros(1, 1).unwrap(),
        &vec![0.; n],
        &team,
    );
    let y: Vec<_> = accepted.eta.iter().map(|v| v.exp()).collect();
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &roles,
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    let result = checkpoint
        .run(&accepted, &evaluation, Stage::Fisher, &team)
        .unwrap();
    assert_eq!(result.decision, Decision::ResumeCandidate);
    let candidate = result.candidate.unwrap();
    assert_eq!(candidate.linear.finite.kept, vec![1]);
    assert!(!candidate.basis_unchanged);
    let error = checkpoint
        .run(&accepted, &evaluation, Stage::FinalHessian, &team)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert!(error.message.contains("canonical rank basis"));
    assert_eq!(accepted.finite, vec![-2., 0.]);
    assert_eq!(evaluation, family.evaluate(&accepted.eta, &team).unwrap());
}
#[test]
fn stale_accepted_model_or_objective_is_rejected_before_a_checkpoint_can_publish() {
    let sample = support::prepared(&[], false, 3, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let roles = [Role::Ordinary];
    let team = ThreadTeam::new(1).unwrap();
    let frozen = basis(&fe, &raw, &roles, &team);
    let y = vec![1.; n];
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let mut accepted = Point {
        finite: vec![0.],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![0.; n],
    };
    let mut evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &roles,
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    accepted.finite[0] = 1.;
    assert_eq!(
        checkpoint
            .run(&accepted, &evaluation, Stage::FinalHessian, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    accepted.finite[0] = 0.;
    evaluation.objective.log_likelihood += 0.1;
    assert_eq!(
        checkpoint
            .run(&accepted, &evaluation, Stage::Fisher, &team)
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
}

#[test]
fn overflowing_poisson_trial_returns_the_accepted_state_for_bounded_resumption() {
    let sample = support::prepared(&[], false, 3, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::from_columns(&[vec![1.; n]]).unwrap();
    let roles = [Role::Ordinary];
    let team = ThreadTeam::new(1).unwrap();
    let frozen = basis(&fe, &raw, &roles, &team);
    let y = vec![1.; n];
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let accepted = Point {
        finite: vec![-10.],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![-10.; n],
    };
    let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &roles,
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    let report = checkpoint
        .run(&accepted, &evaluation, Stage::Fisher, &team)
        .unwrap();
    assert_eq!(report.decision, Decision::ResumeAccepted);
    assert!(report.candidate.is_none());
    assert_eq!(evaluation, family.evaluate(&accepted.eta, &team).unwrap());
}

#[test]
fn final_hessian_omission_change_is_fatal_even_at_zero_poisson_score() {
    let sample = support::prepared(&[1], false, 25, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let mut x = vec![1.; n];
    x[0] = 2.;
    let raw = DenseMatrix::from_columns(&[x]).unwrap();
    let roles = [Role::Ordinary];
    let team = ThreadTeam::new(1).unwrap();
    let frozen = basis(&fe, &raw, &roles, &team);
    assert_eq!(frozen.kept(), &[0]);
    let accepted = point(
        &fe,
        &raw,
        vec![-230.],
        DenseMatrix::from_row_major(1, 1, vec![230.]).unwrap(),
        &vec![0.; n],
        &team,
    );
    let y: Vec<_> = accepted.eta.iter().map(|v| v.exp()).collect();
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    assert!(
        family
            .score_certificate(&accepted.eta, &raw, &fe, 1e-8, &team)
            .unwrap()
            .passed
    );
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &roles,
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    // Strict Fisher checks may propose a new basis and resume; only the final
    // Hessian owns the fatal unchanged-omission publication requirement.
    let fisher = checkpoint
        .run(&accepted, &evaluation, Stage::Fisher, &team)
        .unwrap();
    assert_eq!(fisher.decision, Decision::ResumeCandidate);
    assert!(fisher.candidate.unwrap().linear.finite.kept.is_empty());
    let error = checkpoint
        .run(&accepted, &evaluation, Stage::FinalHessian, &team)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::NonConvergence);
    assert!(error.message.contains("canonical rank basis"), "{error}");
}

#[test]
fn explicit_omissions_are_excluded_from_scores_and_must_have_zero_coefficients() {
    let sample = support::prepared(&[], false, 3, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let y: Vec<_> = (0..n).map(|i| if i % 2 == 0 { 1. } else { 3. }).collect();
    let raw = DenseMatrix::from_columns(&[vec![1.; n], y.clone()]).unwrap();
    let roles = [Role::Ordinary, Role::ExplicitlyOmitted];
    let team = ThreadTeam::new(1).unwrap();
    let frozen = basis(&fe, &raw, &roles, &team);
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let mut accepted = Point {
        finite: vec![2_f64.ln(), 0.],
        absorbed: DenseMatrix::zeros(0, 1).unwrap(),
        eta: vec![2_f64.ln(); n],
    };
    let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    assert!(
        !family
            .score_certificate(&accepted.eta, &raw, &fe, 1e-8, &team)
            .unwrap()
            .passed
    );
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &roles,
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    for stage in [Stage::Fisher, Stage::FinalHessian] {
        let report = checkpoint
            .run(&accepted, &evaluation, stage, &team)
            .unwrap();
        assert_eq!(report.decision, Decision::Stable);
        assert_eq!(report.accepted_scores.finite.directions, 1);
    }
    accepted.finite[1] = 1e-20;
    assert_eq!(
        checkpoint
            .run(&accepted, &evaluation, Stage::FinalHessian, &team)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn pure_fixed_effect_checkpoints_need_no_artificial_finite_regressor() {
    let sample = support::prepared(&[4], false, 8, 0);
    let n = sample.n_active();
    let fe = Design::new(&sample, Weights::Unit).unwrap();
    let raw = DenseMatrix::zeros(n, 0).unwrap();
    let team = ThreadTeam::new(1).unwrap();
    let frozen = basis(&fe, &raw, &[], &team);
    let alpha =
        DenseMatrix::from_row_major(4, 1, (1..=4).map(|i| (i as f64).ln()).collect()).unwrap();
    let accepted = point(&fe, &raw, vec![], alpha, &vec![0.; n], &team);
    let y: Vec<_> = accepted.eta.iter().map(|v| v.exp()).collect();
    let q = ReferenceWeights::new(&PreparedWeights::None, n, Estimator::Ppml).unwrap();
    let family = Family::new(&y, q, None).unwrap();
    let evaluation = family.evaluate(&accepted.eta, &team).unwrap();
    let checkpoint = Checkpoint {
        family: &family,
        design: &fe,
        raw: &raw,
        roles: &[],
        basis: &frozen,
        options: Options::for_estimator(Estimator::Ppml),
    };
    for stage in [Stage::Fisher, Stage::FinalHessian] {
        let report = checkpoint
            .run(&accepted, &evaluation, stage, &team)
            .unwrap();
        assert_eq!(report.decision, Decision::Stable);
        assert_eq!(report.accepted_scores.finite.directions, 0);
        assert_eq!(report.accepted_scores.fixed_effects.directions, 4);
        assert!(report.candidate.unwrap().linear.finite.kept.is_empty());
    }
}
