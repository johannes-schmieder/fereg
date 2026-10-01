use super::*;
use crate::{
    data::{
        DenseMatrix, DropReason, Prepared, PreparedAdjustment, PreparedWeights, ReferenceWeights,
        WeightKind, Weights,
    },
    execution::{self, interruption_tests::every_checkpoint, Probe},
    fe::Design,
    spec::{Estimator, Options},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};
use std::{cell::Cell, rc::Rc};

fn sample(n: usize, effects: usize, wide: bool) -> Prepared {
    let source = |id| SourceId::new(id).unwrap();
    let spec = ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=3)
                .map(|id| SourceEntry {
                    id: source(id),
                    canonical_name: format!("s{id}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=effects)
            .map(|id| AbsorbedTerm {
                id: AbsorbTermId::new(id as u32).unwrap(),
                categorical_sources: vec![source(id as u32); if wide { 2053 } else { 1 }],
                slope_sources: if wide { vec![source(3); 1027] } else { vec![] },
                intercept: true,
                label: format!("g{id}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let mut drop_reasons = vec![DropReason::MissingOutcome; 2 * n + 1];
    drop_reasons[0] = DropReason::ExcludedByIfIn;
    for row in 0..n {
        drop_reasons[2 * row + 1] = DropReason::Retained;
    }
    prepare_fixed_effects(
        &spec,
        Prepared {
            topology: None,
            n_original: drop_reasons.len(),
            n_candidate: 2 * n,
            original_row_by_active: (0..n).map(|row| 2 * row + 1).collect(),
            outcome: (0..n)
                .map(|row| if row % 2 == 0 { 1. } else { 0. })
                .collect(),
            regressors: vec![
                (0..n).map(|row| ((row / 2) % 5) as f64).collect(),
                (0..n).map(|row| ((row / 10) % 7) as f64).collect(),
                (0..n)
                    .map(|row| if row % 2 == 0 { 0. } else { -1. })
                    .collect(),
            ],
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons,
        },
    )
    .unwrap()
}
fn ledger(n: usize, effects: usize) -> Ledger {
    Ledger::new(
        sample(n, effects, false),
        Policy {
            drop_singletons: false,
            methods: Methods::only(Method::IteratedRectifier),
        },
    )
    .unwrap()
}
fn model<'a>(raw: &'a DenseMatrix, roles: &'a [crate::wls::Role], probe: Probe<'a>) -> Model<'a> {
    Model {
        probe,
        raw,
        roles,
        options: Options {
            tolerance: 1e-12,
            ..Options::for_estimator(Estimator::Ppml)
        },
        controls: Controls::default(),
    }
}

fn span_checkpoints(
    view: View<'_>,
    proposal: &DirectionProposal,
    raw: &DenseMatrix,
    roles: &[crate::wls::Role],
    team: &ThreadTeam,
) -> usize {
    let expected = view
        .certify_direction(proposal, &model(raw, roles, Probe::default()), team)
        .unwrap();
    let count = Cell::new(0);
    let caller = std::thread::current().id();
    let failure = FeregError::new(ErrorCode::Interrupted, "span caller cancellation");
    let run = |stop| {
        count.set(0);
        execution::run(
            team,
            |notify| {
                let poll = || notify(());
                view.certify_direction(
                    proposal,
                    &model(raw, roles, Probe::with_dense(&poll, &poll)),
                    team,
                )
            },
            &mut |()| {
                assert_eq!(std::thread::current().id(), caller);
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(failure.clone())
                } else {
                    Ok(())
                }
            },
        )
    };
    assert_eq!(run(usize::MAX).unwrap(), expected);
    let total = count.get();
    for stop in 1..=total {
        assert_eq!(run(stop).unwrap_err(), failure);
        assert_eq!(count.get(), stop);
        assert_eq!(
            view.certify_direction(proposal, &model(raw, roles, Probe::default()), team)
                .unwrap(),
            expected
        );
    }
    total
}

#[test]
fn cone_span_and_identity_abort_every_checkpoint_with_exact_original_row_claims() {
    let ledger = ledger(4099, 0);
    let view = ledger.view();
    let direction = view.sample().regressors[2].clone();
    let raw = DenseMatrix::from_columns(&[direction.clone(), vec![1.; direction.len()]]).unwrap();
    let roles = [crate::wls::Role::Ordinary; 2];
    let proposal = view
        .direction_proposal(
            Method::IteratedRectifier,
            1,
            direction.clone(),
            Controls::default(),
        )
        .unwrap();
    assert_eq!(
        proposal.rows,
        (0..4099)
            .filter(|row| row % 2 == 1)
            .map(|row| 2 * row + 1)
            .collect::<Vec<_>>()
    );
    assert_eq!(proposal.certificate.interior_zero_relative, 0.);
    assert_eq!(proposal.certificate.boundary_violation_relative, 0.);
    assert_eq!(proposal.certificate.negative_margin_relative, 1.);
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let claim = every_checkpoint(|poll| {
            view.direction_proposal_polled(
                Method::IteratedRectifier,
                1,
                direction.clone(),
                Controls::default(),
                poll,
            )
        });
        let span = span_checkpoints(view, &proposal, &raw, &roles, &team);
        let identity = every_checkpoint(|poll| view.numerical_identity_polled(poll));
        assert!(claim > 10 && span > 30 && identity > 4);
        assert_eq!(
            *reference.get_or_insert((claim, span, identity)),
            (claim, span, identity)
        );
    }
    let invalid = DirectionProposal {
        sample_identity: proposal.sample_identity ^ 1,
        ..proposal.clone()
    };
    let team = ThreadTeam::new(1).unwrap();
    let observe = || true;
    assert_eq!(
        view.certify_direction(&invalid, &model(&raw, &roles, Probe::default()), &team)
            .unwrap_err(),
        view.certify_direction(
            &invalid,
            &model(&raw, &roles, Probe::with_dense(&observe, &observe)),
            &team
        )
        .unwrap_err(),
    );
}

#[test]
fn borrowed_design_validation_polls_rows_recipes_slopes_and_objective_weights() {
    for (n, wide) in [(7, true), (4099, false)] {
        design_validation_checkpoints(n, wide);
    }
}

fn design_validation_checkpoints(n: usize, wide: bool) {
    let mut sample = sample(n, 2, wide);
    sample.weights = PreparedWeights::Provided {
        kind: WeightKind::Probability,
        values: (0..n).map(|row| 1. + (row % 3) as f64).collect(),
    };
    let reference_weights = ReferenceWeights::new(&sample.weights, n, Estimator::Ppml).unwrap();
    let provided: Vec<_> = (0..n).map(|row| 0.5 + (row % 7) as f64).collect();
    for weights in [
        Weights::Unit,
        Weights::Provided(&provided),
        Weights::Reference(reference_weights),
    ] {
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let count = team.install(|| {
                every_checkpoint(|poll| {
                    let design = Design::new_polled(&sample, weights, poll)?;
                    assert_eq!(
                        design.layout().effects[0].channels,
                        if wide { 1028 } else { 1 }
                    );
                    Ok((
                        design.layout().clone(),
                        [weights.at(0), weights.at(n / 2), weights.at(n - 1)],
                    ))
                })
            });
            assert!(count > 10);
            assert_eq!(*reference.get_or_insert(count), count);
        }
    }
    let observe = || true;
    let mut malformed = provided;
    malformed[n / 2] = f64::NAN;
    assert_eq!(
        Design::new_polled(&sample, Weights::Provided(&malformed), None).unwrap_err(),
        Design::new_polled(&sample, Weights::Provided(&malformed), Some(&observe)).unwrap_err(),
    );
}

#[test]
fn numerical_span_errors_keep_non_send_caller_identity_and_leave_the_sample_unchanged() {
    let ledger = ledger(2053, 0);
    let view = ledger.view();
    let raw =
        DenseMatrix::from_columns(&[view.sample().regressors[2].clone(), vec![1.; 2053]]).unwrap();
    let roles = [crate::wls::Role::Ordinary; 2];
    let proposal = view
        .direction_proposal(
            Method::IteratedRectifier,
            1,
            view.sample().regressors[2].clone(),
            Controls::default(),
        )
        .unwrap();
    let accepted = (
        ledger.sample_digest(),
        ledger.trace_digest(),
        view.generation(),
        view.sample().retained_rows().to_vec(),
    );
    let caller = std::thread::current().id();
    let count = Rc::new(Cell::new(0));
    let failure = FeregError::new(ErrorCode::NonConvergence, "numerical span host r(430)");
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let run = |stop| {
            count.set(0);
            execution::run(
                &team,
                |notify| {
                    let poll = || notify(());
                    view.certify_direction(
                        &proposal,
                        &model(&raw, &roles, Probe::with_dense(&poll, &poll)),
                        &team,
                    )
                },
                &mut |()| {
                    assert_eq!(std::thread::current().id(), caller);
                    count.set(count.get() + 1);
                    if count.get() == stop {
                        Err(failure.clone())
                    } else {
                        Ok(())
                    }
                },
            )
        };
        let expected = run(usize::MAX).unwrap();
        let total = count.get();
        assert!(total > 30);
        assert_eq!(*reference.get_or_insert(total), total);
        for stop in [1, total / 2, total] {
            assert_eq!(run(stop).unwrap_err(), failure);
            assert_eq!(count.get(), stop);
            assert_eq!(
                (
                    ledger.sample_digest(),
                    ledger.trace_digest(),
                    view.generation(),
                    view.sample().retained_rows().to_vec()
                ),
                accepted
            );
            assert_eq!(run(usize::MAX).unwrap(), expected);
        }
    }
}
