use super::*;
use crate::{
    data::PreparedAdjustment,
    execution::interruption_tests::every_checkpoint,
    spec::metadata::{AbsorbTermId, AbsorbedTerm, CompiledTermGraph, SourceEntry},
    threads::ThreadTeam,
    ErrorCode, FeregError,
};

fn model(sources: usize, recipes: &[Vec<usize>], slopes: bool) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sources)
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("g{}", i + 1),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: recipes
            .iter()
            .enumerate()
            .map(|(i, recipe)| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                categorical_sources: recipe
                    .iter()
                    .map(|&source| SourceId::new(source as u32 + 1).unwrap())
                    .collect(),
                slope_sources: if slopes {
                    vec![SourceId::new(sources as u32).unwrap()]
                } else {
                    vec![]
                },
                intercept: true,
                label: format!("effect{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: false,
    }
}

fn sample(n: usize, columns: &[Vec<f64>], frequency: Option<&[f64]>) -> Prepared {
    let rows: Vec<_> = (0..n).map(|i| 2 * i + 1).collect();
    let mut reasons = vec![DropReason::ExcludedByIfIn; 2 * n + 3];
    for &row in &rows {
        reasons[row] = DropReason::Retained;
    }
    Prepared {
        topology: None,
        n_original: reasons.len(),
        n_candidate: n,
        original_row_by_active: rows,
        outcome: (0..n).map(|i| i as f64 + 1.).collect(),
        regressors: columns.to_vec(),
        weights: frequency.map_or(PreparedWeights::None, |values| PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values: values.to_vec(),
        }),
        adjustment: PreparedAdjustment::Offset((0..n).map(|i| -(i as f64)).collect()),
        drop_reasons: reasons,
    }
}

#[test]
fn complete_topology_cancels_canonical_coding_pruning_compaction_membership_and_slopes() {
    let n = 4099;
    let mut columns: Vec<Vec<f64>> = vec![
        (0..n).map(|i| (i % 37) as f64).collect(),
        (0..n).map(|i| (i % 13) as f64).collect(),
        (0..n).map(|i| i as f64 / 8.).collect(),
    ];
    columns[0][n - 1] = -1234.;
    columns[1][n - 1] = -9876.;
    let mut frequency = vec![1.; n];
    frequency[n - 1] = 2.;
    let mut spec = model(3, &[vec![1, 0], vec![0, 1, 0], vec![0], vec![1]], true);
    for keep in [false, true] {
        spec.keep_singletons = keep;
        for weights in [None, Some(frequency.as_slice())] {
            let expected = prepare_fixed_effects(&spec, sample(n, &columns, weights)).unwrap();
            let retained = n - usize::from(!keep && weights.is_none());
            assert_eq!(expected.n_active(), retained);
            assert_eq!(
                expected.outcome,
                (0..retained).map(|i| i as f64 + 1.).collect::<Vec<_>>()
            );
            assert_eq!(
                expected.retained_rows(),
                (0..retained).map(|i| 2 * i + 1).collect::<Vec<_>>()
            );
            for (source, column) in columns.iter().enumerate() {
                assert_eq!(expected.regressors[source], column[..retained]);
            }
            assert_eq!(
                expected.adjustment,
                PreparedAdjustment::Offset((0..retained).map(|i| -(i as f64)).collect())
            );
            let topology = expected.topology.as_ref().unwrap();
            assert_eq!(topology.partitions().len(), 3);
            assert_eq!(
                topology.effects()[0].partition,
                topology.effects()[1].partition
            );
            assert_eq!(topology.effects()[0].label, "g2#g1");
            assert_eq!(topology.effects()[1].label, "g1#g2#g1");
            if retained < n {
                assert_eq!(expected.drop_reasons[2 * n - 1], DropReason::Singleton);
                assert_eq!(
                    topology.partitions()[1].original_levels(),
                    (1..=37).collect::<Vec<_>>()
                );
            }
            let mut reference = None;
            for workers in [1, 8] {
                let team = ThreadTeam::new(workers).unwrap();
                let count = team.install(|| {
                    every_checkpoint(|poll| {
                        prepare_fixed_effects_polled(&spec, sample(n, &columns, weights), poll)
                    })
                });
                assert!(count > 80);
                assert_eq!(*reference.get_or_insert(count), count);
            }
        }
    }
}

#[test]
fn singleton_queue_cancels_a_long_cascade_with_exact_frequency_multiplicity() {
    let n = 4099;
    let left: Vec<_> = (0..n).map(|i| (i / 2) as u32).collect();
    let right: Vec<_> = (0..n).map(|i| ((i + 1) / 2) as u32).collect();
    let codes = [left.as_slice(), right.as_slice()];
    let levels = [2050, 2050];
    let active = vec![true; n];
    let mut frequency = vec![1.; n];
    frequency[n - 1] = 2.;
    for multiplicity in [
        SingletonMultiplicity::Unit,
        SingletonMultiplicity::Frequency(&frequency),
    ] {
        let expected = super::super::prune_singletons_with_multiplicity(
            &codes,
            &levels,
            &active,
            multiplicity,
            false,
        )
        .unwrap();
        let survives = usize::from(matches!(multiplicity, SingletonMultiplicity::Frequency(_)));
        assert_eq!(expected.cascade_events, n - survives);
        assert_eq!(
            expected.active.iter().filter(|&&keep| keep).count(),
            survives
        );
        assert_eq!(expected.active[n - 1], survives == 1);
        let mut reference = None;
        for workers in [1, 8] {
            let team = ThreadTeam::new(workers).unwrap();
            let count = team.install(|| {
                every_checkpoint(|poll| {
                    prune_singletons_polled(&codes, &levels, &active, multiplicity, false, poll)
                })
            });
            assert!(count > 20);
            assert_eq!(*reference.get_or_insert(count), count);
        }
    }
}

#[test]
fn wide_recipes_poll_source_copy_sort_deduplication_labels_and_shared_partition_lookup() {
    let n = 3;
    let width = 2053;
    let columns = vec![vec![0., 1., 0.]; 2];
    let recipe: Vec<_> = (0..width).map(|i| i % 2).collect();
    let mut spec = model(
        2,
        &[recipe.clone(), recipe.into_iter().rev().collect()],
        false,
    );
    spec.keep_singletons = true;
    let expected = prepare_fixed_effects(&spec, sample(n, &columns, None)).unwrap();
    let topology = expected.topology.as_ref().unwrap();
    assert_eq!(topology.partitions().len(), 1);
    assert_eq!(topology.partitions()[0].codes(), &[0, 1, 0]);
    assert_eq!(topology.partitions()[0].recipe().len(), 2);
    let canonical: Vec<_> = (1..=width)
        .map(|i| SourceId::new(i as u32).unwrap())
        .collect();
    let mut input: Vec<_> = canonical.iter().rev().copied().collect();
    input.extend([canonical[0], canonical[width - 1]]);
    assert_eq!(canonical_recipe(&input, None).unwrap(), canonical);
    let mut wrong = canonical.clone();
    wrong[width - 1] = SourceId::new(width as u32 + 1).unwrap();
    let partitions: Vec<_> = [wrong, canonical.clone()]
        .into_iter()
        .map(|recipe| PendingPartition {
            recipe,
            codes: vec![],
            n_levels: 0,
        })
        .collect();
    assert_eq!(
        find_partition(&partitions, &canonical, None).unwrap(),
        Some(1)
    );
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let counts = team.install(|| {
            (
                every_checkpoint(|poll| {
                    prepare_fixed_effects_polled(&spec, sample(n, &columns, None), poll)
                }),
                every_checkpoint(|poll| canonical_recipe(&input, poll)),
                every_checkpoint(|poll| find_partition(&partitions, &canonical, poll)),
            )
        });
        assert!(counts.0 > 20 && counts.1 > 5 && counts.2 > 1);
        assert_eq!(*reference.get_or_insert(counts), counts);
    }
}

#[test]
fn implicit_constant_and_empty_topology_cancel_without_a_second_representation() {
    let spec = model(0, &[], false);
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        team.install(|| {
            every_checkpoint(|poll| {
                prepare_fixed_effects_polled(&spec, sample(0, &[], None), poll)
            });
            let total = every_checkpoint(|poll| {
                super::super::prepare_implicit_constant_polled(
                    prepare_fixed_effects(&spec, sample(4099, &[], None))?,
                    poll,
                )
            });
            assert!(total > 10);
        });
    }
}

#[test]
fn malformed_inputs_and_caller_errors_keep_their_identity_and_discard_owned_work() {
    use std::{cell::Cell, rc::Rc};
    let spec = model(1, &[vec![0]], false);
    let columns = vec![(0..4099).map(|i| (i % 31) as f64).collect()];
    let observe = || true;
    let mut malformed = sample(4099, &columns, None);
    malformed.regressors[0].pop();
    let original = prepare_fixed_effects(&spec, malformed).unwrap_err();
    let mut malformed = sample(4099, &columns, None);
    malformed.regressors[0].pop();
    assert_eq!(
        prepare_fixed_effects_polled(&spec, malformed, Some(&observe)).unwrap_err(),
        original
    );
    for (codes, frequency) in [
        (vec![2; 2053], vec![1.; 2053]),
        (vec![0; 2053], vec![1.5; 2053]),
    ] {
        let plain = super::super::prune_singletons_with_multiplicity(
            &[&codes],
            &[1],
            &vec![true; 2053],
            SingletonMultiplicity::Frequency(&frequency),
            false,
        )
        .unwrap_err();
        let polled = prune_singletons_polled(
            &[&codes],
            &[1],
            &vec![true; 2053],
            SingletonMultiplicity::Frequency(&frequency),
            false,
            Some(&observe),
        )
        .unwrap_err();
        assert_eq!(plain, polled);
    }
    let caller = std::thread::current().id();
    let calls = Rc::new(Cell::new(0));
    let sentinel = FeregError::new(ErrorCode::NonConvergence, "topology host r(430)");
    let expected = prepare_fixed_effects(&spec, sample(4099, &columns, None)).unwrap();
    for workers in [1, 8] {
        calls.set(0);
        let team = ThreadTeam::new(workers).unwrap();
        let error = execution::run(
            &team,
            |relay| {
                prepare_fixed_effects_polled(
                    &spec,
                    sample(4099, &columns, None),
                    Some(&|| relay(())),
                )
            },
            &mut |_| {
                assert_eq!(std::thread::current().id(), caller);
                calls.set(calls.get() + 1);
                if calls.get() == 4 {
                    Err(sentinel.clone())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error, sentinel);
        assert_eq!(calls.get(), 4);
        assert_eq!(
            prepare_fixed_effects(&spec, sample(4099, &columns, None)).unwrap(),
            expected
        );
    }
}
