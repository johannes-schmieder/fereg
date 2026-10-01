use super::*;
use crate::{
    data::{DropReason, Prepared, PreparedAdjustment, PreparedWeights, WeightKind},
    execution::{self, interruption_tests::every_checkpoint, Poll},
    spec::metadata::{AbsorbTermId, AbsorbedTerm, CompiledTermGraph, SourceEntry, SourceId},
    threads::ThreadTeam,
    topology::prepare_fixed_effects,
    ModelSpec, Result,
};
use std::{cell::Cell, rc::Rc};

fn policy() -> Policy {
    Policy {
        drop_singletons: true,
        methods: Methods::only(Method::FixedEffects),
    }
}
fn prepared(columns: Vec<Vec<f64>>, outcome: Vec<f64>, wide: bool) -> Prepared {
    let n = outcome.len();
    let source = |id| SourceId::new(id).unwrap();
    let spec = ModelSpec {
        model: CompiledTermGraph {
            sources: ["g", "h", "x"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| SourceEntry {
                    id: source(index as u32 + 1),
                    canonical_name: name.into(),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: (1..=2)
            .map(|id| AbsorbedTerm {
                id: AbsorbTermId::new(id).unwrap(),
                categorical_sources: vec![source(id); if wide && id == 1 { 2053 } else { 1 }],
                slope_sources: if wide {
                    vec![source(3); if id == 1 { 1027 } else { 1 }]
                } else {
                    vec![]
                },
                intercept: true,
                label: format!("effect{id}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let mut reasons = vec![DropReason::MissingOutcome; 2 * n + 1];
    reasons[0] = DropReason::ExcludedByIfIn;
    for row in 0..n {
        reasons[2 * row + 1] = DropReason::Retained;
    }
    prepare_fixed_effects(
        &spec,
        Prepared {
            topology: None,
            n_original: reasons.len(),
            n_candidate: 2 * n,
            original_row_by_active: (0..n).map(|row| 2 * row + 1).collect(),
            outcome,
            regressors: columns,
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: reasons,
        },
    )
    .unwrap()
}
fn cascade(blocks: usize) -> Prepared {
    let g = [0., 0., 1., 1., 1., 2., 2., 2., 2., 2., 2.];
    let h = [0., 3., 0., 1., 2., 1., 1., 2., 2., 3., 3.];
    let y = [0., 0., 1., 0., 0., 1., 1., 1., 1., 1., 1.];
    let n = 11 * blocks;
    prepared(
        vec![
            (0..n)
                .map(|row| g[row % 11] + 3. * (row / 11) as f64)
                .collect(),
            (0..n)
                .map(|row| h[row % 11] + 4. * (row / 11) as f64)
                .collect(),
            (0..n).map(|row| row as f64 / 8.).collect(),
        ],
        (0..n).map(|row| y[row % 11]).collect(),
        false,
    )
}
type Snapshot = (Prepared, usize, u64, u64);
fn copy(sample: &Prepared) -> Prepared {
    Prepared {
        topology: sample.topology.clone(),
        n_original: sample.n_original,
        n_candidate: sample.n_candidate,
        original_row_by_active: sample.original_row_by_active.clone(),
        outcome: sample.outcome.clone(),
        regressors: sample.regressors.clone(),
        weights: match &sample.weights {
            PreparedWeights::None => PreparedWeights::None,
            PreparedWeights::Provided { kind, values } => PreparedWeights::Provided {
                kind: *kind,
                values: values.clone(),
            },
        },
        adjustment: match &sample.adjustment {
            PreparedAdjustment::None => PreparedAdjustment::None,
            PreparedAdjustment::Offset(values) => PreparedAdjustment::Offset(values.clone()),
            PreparedAdjustment::Exposure(values) => PreparedAdjustment::Exposure(values.clone()),
        },
        drop_reasons: sample.drop_reasons.clone(),
    }
}
fn snapshot(ledger: &Ledger) -> Snapshot {
    (
        copy(ledger.view().sample()),
        ledger.view().generation(),
        ledger.sample_digest(),
        ledger.trace_digest(),
    )
}
fn atomic_boundaries(
    original: &Prepared,
    operation: impl Fn(&mut Ledger, Poll<'_>) -> Result<Option<Commit>> + Sync,
) -> usize {
    let mut baseline = Ledger::new(copy(original), policy()).unwrap();
    let expected = operation(&mut baseline, None).unwrap();
    let final_state = snapshot(&baseline);
    let caller = std::thread::current().id();
    let count = Rc::new(Cell::new(0));
    let failure = FeregError::new(ErrorCode::NonConvergence, "generation host r(430)");
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let run = |ledger: &mut Ledger, stop| {
            count.set(0);
            execution::run(
                &team,
                |notify| {
                    let poll = || notify(());
                    operation(ledger, Some(&poll))
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
        let mut ledger = Ledger::new(copy(original), policy()).unwrap();
        assert_eq!(run(&mut ledger, usize::MAX).unwrap(), expected);
        assert_eq!(snapshot(&ledger), final_state);
        let total = count.get();
        assert!(total > 0);
        assert_eq!(*reference.get_or_insert(total), total);
        for stop in 1..=total {
            let mut ledger = Ledger::new(copy(original), policy()).unwrap();
            let before = snapshot(&ledger);
            assert_eq!(run(&mut ledger, stop).unwrap_err(), failure);
            assert_eq!(count.get(), stop);
            assert_eq!(snapshot(&ledger), before);
            assert_eq!(operation(&mut ledger, None).unwrap(), expected);
            assert_eq!(snapshot(&ledger), final_state);
        }
    }
    reference.unwrap()
}

#[test]
fn exact_generation_aborts_before_publishing_any_sample_or_digest_change() {
    let blocks = 373;
    for kind in [
        None,
        Some(WeightKind::Probability),
        Some(WeightKind::Frequency),
    ] {
        let mut input = cascade(blocks);
        input.weights = kind.map_or(PreparedWeights::None, |kind| PreparedWeights::Provided {
            kind,
            values: (0..input.n_active())
                .map(|row| 2. + (row % 3) as f64)
                .collect(),
        });
        input.adjustment = if kind == Some(WeightKind::Probability) {
            PreparedAdjustment::Exposure(
                (0..input.n_active())
                    .map(|row| 1. + row as f64 / 4.)
                    .collect(),
            )
        } else {
            PreparedAdjustment::Offset(
                (0..input.n_active())
                    .map(|row| -(row as f64) / 8.)
                    .collect(),
            )
        };
        let mut ledger = Ledger::new(copy(&input), policy()).unwrap();
        assert!(ledger.prune_initial_singletons().unwrap().is_none());
        let proposal = ledger.view().exact_proposal().unwrap();
        assert_eq!(proposal.rows.len(), 2 * blocks);
        let commit = ledger.accept_exact(&proposal).unwrap().unwrap();
        let frequency = kind == Some(WeightKind::Frequency);
        assert_eq!(commit.primary, 2 * blocks);
        assert_eq!(commit.singletons, usize::from(!frequency) * blocks);
        assert_eq!(commit.exact_followup, 2 * usize::from(!frequency) * blocks);
        let retained: Vec<_> = (0..11 * blocks)
            .filter(|row| row % 11 >= if frequency { 2 } else { 5 })
            .map(|row| 2 * row + 1)
            .collect();
        assert_eq!(ledger.view().sample().retained_rows(), retained);
        assert_eq!(
            ledger.view().sample().drop_reasons[0],
            DropReason::ExcludedByIfIn
        );
        assert!(
            every_checkpoint(|poll| {
                let ledger = Ledger::new_polled(copy(&input), policy(), poll)?;
                Ok(snapshot(&ledger))
            }) > 20
        );
        assert!(
            every_checkpoint(|poll| {
                Ledger::new(copy(&input), policy())?
                    .view()
                    .exact_proposal_polled(poll)
            }) > 20
        );
        assert!(
            atomic_boundaries(&input, |ledger, poll| ledger
                .accept_exact_polled(&proposal, poll))
                > 100
        );
    }
}

#[test]
fn initial_singleton_generations_cancel_long_cascades_and_respect_frequency_mass() {
    let chain = 4099;
    let n = chain + 4;
    let columns = vec![
        (0..n)
            .map(|row| {
                if row < chain {
                    (row / 2) as f64
                } else {
                    3000. + ((row - chain) / 2) as f64
                }
            })
            .collect(),
        (0..n)
            .map(|row| {
                if row < chain {
                    ((row + 1) / 2) as f64
                } else {
                    3000. + ((row - chain) % 2) as f64
                }
            })
            .collect(),
        (0..n).map(|row| row as f64).collect(),
    ];
    for frequency in [false, true] {
        let mut input = prepared(columns.clone(), vec![1.; n], false);
        if frequency {
            let mut values = vec![1.; n];
            values[0] = 2.;
            values[chain - 1] = 2.;
            input.weights = PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values,
            };
        }
        let mut ledger = Ledger::new(copy(&input), policy()).unwrap();
        let commit = ledger.prune_initial_singletons().unwrap();
        assert_eq!(commit.is_none(), frequency);
        assert_eq!(
            ledger.view().sample().n_active(),
            if frequency { n } else { 4 }
        );
        if let Some(commit) = commit {
            assert_eq!(commit.singletons, chain);
            assert_eq!(ledger.view().generation(), 1);
        }
        let checks = atomic_boundaries(&input, |ledger, poll| {
            ledger.prune_initial_singletons_polled(poll)
        });
        eprintln!("singleton frequency={frequency}: {checks} atomic checkpoints");
        assert!(checks > if frequency { 20 } else { 60 });
    }
}

#[test]
fn staged_wide_recipes_slopes_and_compaction_keep_original_axis_and_float_bits() {
    let n = 4099;
    let input = prepared(
        vec![
            (0..n).map(|row| row as f64).collect(),
            (0..n).map(|row| (row % 7) as f64).collect(),
            (0..n)
                .map(|row| if row % 2 == 0 { -0. } else { row as f64 / 8. })
                .collect(),
        ],
        vec![1.; n],
        true,
    );
    let reasons: Vec<_> = (0..n)
        .map(|row| {
            if row % 3 == 0 {
                DropReason::MuSeparation
            } else {
                DropReason::Retained
            }
        })
        .collect();
    let expected_rows: Vec<_> = (0..n).filter(|row| row % 3 != 0).collect();
    let expected = input.stage_deletions_polled(&reasons, None).unwrap();
    assert_eq!(
        expected.retained_rows(),
        expected_rows
            .iter()
            .map(|row| 2 * row + 1)
            .collect::<Vec<_>>()
    );
    for (source, column) in expected.regressors.iter().enumerate() {
        assert_eq!(
            column.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected_rows
                .iter()
                .map(|&row| input.regressors[source][row].to_bits())
                .collect::<Vec<_>>()
        );
    }
    let topology = expected.topology.as_ref().unwrap();
    assert_eq!(topology.effects()[0].categorical_sources.len(), 2053);
    assert_eq!(topology.effects()[0].slopes.len(), 1027);
    assert_eq!(
        topology.partitions()[0].original_levels(),
        expected_rows
            .iter()
            .map(|&row| row as u32)
            .collect::<Vec<_>>()
    );
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        assert!(
            team.install(|| every_checkpoint(|poll| input.stage_deletions_polled(&reasons, poll)))
                > 80
        );
    }
}

#[test]
fn last_prepublication_callback_panic_and_fatal_empty_generation_preserve_the_ledger() {
    let input = cascade(3);
    let mut ledger = Ledger::new(copy(&input), policy()).unwrap();
    let proposal = ledger.view().exact_proposal().unwrap();
    let count = Cell::new(0);
    let observe = || {
        count.set(count.get() + 1);
        true
    };
    // A Cell is not Sync; count the synchronous checkpoints through the scoped host relay.
    let team = ThreadTeam::new(1).unwrap();
    execution::run(
        &team,
        |notify| ledger.accept_exact_polled(&proposal, Some(&|| notify(()))),
        &mut |()| {
            observe();
            Ok(())
        },
    )
    .unwrap();
    let total = count.get();
    let final_state = snapshot(&ledger);
    let mut ledger = Ledger::new(input, policy()).unwrap();
    let before = snapshot(&ledger);
    count.set(0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execution::run(
            &team,
            |notify| ledger.accept_exact_polled(&proposal, Some(&|| notify(()))),
            &mut |()| {
                count.set(count.get() + 1);
                assert!(
                    count.get() < total,
                    "host panic at the last prepublication checkpoint"
                );
                Ok(())
            },
        )
    }));
    assert!(panic.is_err());
    assert_eq!(snapshot(&ledger), before);
    ledger.accept_exact(&proposal).unwrap();
    assert_eq!(snapshot(&ledger), final_state);
    let singleton = prepared(
        vec![vec![0., 1.], vec![0., 1.], vec![2., 3.]],
        vec![1., 0.],
        false,
    );
    assert!(
        every_checkpoint(|poll| {
            let mut ledger = Ledger::new(copy(&singleton), policy())?;
            let before = snapshot(&ledger);
            let error = ledger.prune_initial_singletons_polled(poll).unwrap_err();
            assert_eq!(snapshot(&ledger), before);
            if error.code == ErrorCode::Interrupted {
                Err(error)
            } else {
                assert_eq!(error.code, ErrorCode::InsufficientObservations);
                Ok(error)
            }
        }) > 5
    );
}
