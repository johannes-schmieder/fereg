use fereg_engine::{
    data::*,
    ppml::separation::*,
    spec::{Estimator, Options, Solver},
    threads::ThreadTeam,
    wls::Role,
    ErrorCode,
};
mod support;
fn model<'a>(raw: &'a DenseMatrix, roles: &'a [Role]) -> Model<'a> {
    Model {
        probe: Default::default(),
        raw,
        roles,
        options: Options::for_estimator(Estimator::Ppml),
        controls: Controls::default(),
    }
}
fn ledger(levels: &[usize], slopes: bool) -> Ledger {
    let mut sample = support::prepared(levels, slopes, 24, 0);
    for (row, y) in sample.outcome.iter_mut().enumerate() {
        if row % 13 == 0 {
            *y = 0.;
        }
    }
    Ledger::new(
        sample,
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::IteratedRectifier),
        },
    )
    .unwrap()
}
fn direction(ledger: &Ledger) -> Vec<f64> {
    ledger
        .view()
        .sample()
        .outcome
        .iter()
        .map(|&y| if y == 0. { -1. } else { 0. })
        .collect()
}
fn proposal(ledger: &Ledger) -> DirectionProposal {
    ledger
        .view()
        .direction_proposal(
            Method::IteratedRectifier,
            2,
            direction(ledger),
            Controls::default(),
        )
        .unwrap()
}

#[test]
fn methods_use_canonical_order_reject_duplicates_and_keep_mu_opt_in() {
    assert_eq!(
        Methods::default().iter().collect::<Vec<_>>(),
        [
            Method::FixedEffects,
            Method::Simplex,
            Method::IteratedRectifier
        ]
    );
    assert_eq!(
        Methods::parse("mu ir fe simplex")
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [
            Method::FixedEffects,
            Method::Simplex,
            Method::IteratedRectifier,
            Method::Mu
        ]
    );
    assert_eq!(Methods::parse("none").unwrap(), Methods::none());
    assert_eq!(Methods::parse("").unwrap(), Methods::default());
    for text in [
        "fe fe",
        "none ir",
        "ir none",
        "none none",
        "IR ir",
        "irls",
        "auto",
    ] {
        assert_eq!(Methods::parse(text).unwrap_err().code, ErrorCode::Syntax);
    }
}

#[test]
fn valid_signs_do_not_establish_membership_in_the_model_span() {
    let mut ledger = ledger(&[3, 2], false);
    let proposal = proposal(&ledger);
    let raw = DenseMatrix::zeros(ledger.view().sample().n_active(), 0).unwrap();
    let digest = ledger.trace_digest();
    assert_eq!(
        ledger
            .accept_direction(&proposal, &model(&raw, &[]), &ThreadTeam::new(1).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    assert_eq!(ledger.view().generation(), 0);
    assert_eq!(ledger.trace_digest(), digest);
    // The same direction is valid after adding its actual finite regressor.
    let raw = DenseMatrix::from_columns(&[proposal.direction.clone()]).unwrap();
    let commit = ledger
        .accept_direction(
            &proposal,
            &model(&raw, &[Role::Ordinary]),
            &ThreadTeam::new(1).unwrap(),
        )
        .unwrap();
    assert_eq!(commit.commit.primary, proposal.rows.len());
    assert!(commit.validation.span_residual_relative < 1e-10);
    for row in proposal.rows {
        assert_eq!(
            ledger.view().sample().drop_reasons[row],
            DropReason::IteratedRectifierSeparation
        );
    }
}

#[test]
fn fresh_certificates_are_identical_across_workers_and_direction_scales() {
    for levels in [vec![], vec![3], vec![3, 2], vec![3, 2, 5]] {
        let ledger = ledger(&levels, true);
        let base = proposal(&ledger);
        let raw = DenseMatrix::from_columns(&[base.direction.clone()]).unwrap();
        let model = model(&raw, &[Role::Ordinary]);
        for scale in [1e-200, 1., 1e200] {
            let p = ledger
                .view()
                .direction_proposal(
                    Method::IteratedRectifier,
                    2,
                    base.direction.iter().map(|v| v * scale).collect(),
                    Controls::default(),
                )
                .unwrap();
            let a = ledger
                .view()
                .certify_direction(&p, &model, &ThreadTeam::new(1).unwrap())
                .unwrap();
            for workers in [2, 4, 8] {
                assert_eq!(
                    a,
                    ledger
                        .view()
                        .certify_direction(&p, &model, &ThreadTeam::new(workers).unwrap())
                        .unwrap()
                );
            }
        }
    }
}

#[test]
fn cone_equality_sign_margin_and_nonzero_scale_are_all_required() {
    let ledger = ledger(&[], false);
    let valid = direction(&ledger);
    let controls = Controls::default();
    let mut invalid = vec![vec![0.; valid.len()], vec![f64::MIN_POSITIVE; valid.len()]];
    for (row, value) in [(1, 1e-4), (13, 1e-4), (0, f64::NAN), (0, f64::INFINITY)] {
        let mut d = valid.clone();
        d[row] = value;
        invalid.push(d);
    }
    invalid.push(vec![0.; valid.len() - 1]);
    for d in invalid {
        assert_eq!(
            ledger
                .view()
                .direction_proposal(Method::Simplex, 1, d, controls)
                .unwrap_err()
                .code,
            ErrorCode::NonConvergence
        );
    }
    for bad in [0., 1., f64::NAN] {
        assert_eq!(
            ledger
                .view()
                .direction_proposal(
                    Method::Simplex,
                    1,
                    valid.clone(),
                    Controls {
                        tolerance: bad,
                        ..controls
                    }
                )
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
    }
    assert!(ledger
        .view()
        .direction_proposal(Method::FixedEffects, 1, valid.clone(), controls)
        .is_err());
    assert!(ledger
        .view()
        .direction_proposal(Method::Simplex, 0, valid, controls)
        .is_err());
}

#[test]
fn tampering_any_claim_field_fails_before_sample_publication() {
    let mut ledger = ledger(&[3], false);
    let valid = proposal(&ledger);
    let raw = DenseMatrix::from_columns(&[valid.direction.clone()]).unwrap();
    let model = model(&raw, &[Role::Ordinary]);
    let mut altered = vec![];
    let mut p = valid.clone();
    p.rows.reverse();
    altered.push(p);
    let mut p = valid.clone();
    p.rows.push(usize::MAX);
    altered.push(p);
    let mut p = valid.clone();
    p.rows.pop();
    altered.push(p);
    let mut p = valid.clone();
    p.rows[0] = 1;
    altered.push(p);
    let mut p = valid.clone();
    p.rows.insert(0, 0);
    altered.push(p);
    let mut p = valid.clone();
    p.generation += 1;
    altered.push(p);
    let mut p = valid.clone();
    p.sample_identity ^= 1;
    altered.push(p);
    let mut p = valid.clone();
    p.iterations = 0;
    altered.push(p);
    let mut p = valid.clone();
    p.status = Status::NoSeparation;
    altered.push(p);
    let mut p = valid.clone();
    p.method = Method::Mu;
    altered.push(p);
    let mut p = valid.clone();
    p.certificate.direction_scale *= 2.;
    altered.push(p);
    let mut p = valid.clone();
    p.certificate.interior_zero_relative = 1.;
    altered.push(p);
    let mut p = valid.clone();
    p.certificate.boundary_violation_relative = 1.;
    altered.push(p);
    let mut p = valid.clone();
    p.certificate.negative_margin_relative = 0.;
    altered.push(p);
    let mut p = valid.clone();
    p.certificate.positive_maximum = 1.;
    altered.push(p);
    let digest = ledger.trace_digest();
    for p in altered {
        assert!(ledger
            .accept_direction(&p, &model, &ThreadTeam::new(1).unwrap())
            .is_err());
        assert_eq!(ledger.view().generation(), 0);
        assert_eq!(ledger.trace_digest(), digest);
    }
    ledger
        .accept_direction(&valid, &model, &ThreadTeam::new(1).unwrap())
        .unwrap();
    assert!(ledger
        .accept_direction(&valid, &model, &ThreadTeam::new(1).unwrap())
        .is_err());
}

#[test]
fn source_identity_catches_changed_outcomes_even_when_signs_still_match() {
    let original = ledger(&[3], false);
    let p = proposal(&original);
    let mut changed = original.into_sample();
    changed.outcome[1] = 2.;
    let changed = Ledger::new(
        changed,
        Policy {
            drop_singletons: false,
            methods: Methods::default(),
        },
    )
    .unwrap();
    let raw = DenseMatrix::from_columns(&[p.direction.clone()]).unwrap();
    assert_eq!(
        changed
            .view()
            .certify_direction(
                &p,
                &model(&raw, &[Role::Ordinary]),
                &ThreadTeam::new(1).unwrap()
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn slope_channels_are_in_the_span_but_omitted_finite_columns_are_not() {
    let mut input = support::slope_sample(false, &[0, 0, 1, 1], &[vec![-1., 0., -2., 0.]]);
    input.outcome = vec![0., 1., 0., 1.];
    let ledger = Ledger::new(
        input,
        Policy {
            drop_singletons: false,
            methods: Methods::default(),
        },
    )
    .unwrap();
    let p = ledger
        .view()
        .direction_proposal(
            Method::Simplex,
            1,
            vec![-1., 0., -2., 0.],
            Controls::default(),
        )
        .unwrap();
    let raw = DenseMatrix::zeros(4, 0).unwrap();
    ledger
        .view()
        .certify_direction(&p, &model(&raw, &[]), &ThreadTeam::new(1).unwrap())
        .unwrap();
    let finite = self::ledger(&[], false);
    let p = proposal(&finite);
    let raw = DenseMatrix::from_columns(&[p.direction.clone()]).unwrap();
    assert_eq!(
        finite
            .view()
            .certify_direction(
                &p,
                &model(&raw, &[Role::ExplicitlyOmitted]),
                &ThreadTeam::new(1).unwrap()
            )
            .unwrap_err()
            .code,
        ErrorCode::NonConvergence
    );
    for solver in [Solver::Lsmr, Solver::Cmg] {
        let mut model = model(&raw, &[Role::Ordinary]);
        model.options.solver = solver;
        assert_eq!(
            finite
                .view()
                .certify_direction(&p, &model, &ThreadTeam::new(1).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::Unsupported
        );
    }
}
