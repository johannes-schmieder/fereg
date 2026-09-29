//! Literal grouping oracle: no production CSR, queue, or exact-FE detector reuse.
use fereg_engine::{
    data::*, ppml::separation::*, topology::prepare_fixed_effects, AbsorbTermId, AbsorbedTerm,
    CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};
use std::collections::BTreeMap;

struct Case {
    columns: Vec<Vec<u32>>,
    intercept: Vec<bool>,
    y: Vec<f64>,
    frequency: Vec<usize>,
    policy: Policy,
}
fn case(seed: usize) -> Case {
    let n = 17 + seed % 41;
    let mut state = seed as u64 + 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state as usize
    };
    let columns = (0..seed % 4)
        .map(|_| {
            (0..n)
                .map(|row| {
                    if row >= n - 4 {
                        n as u32
                    } else {
                        (next() % (n / 3)) as u32
                    }
                })
                .collect()
        })
        .collect::<Vec<Vec<u32>>>();
    let intercept = (0..columns.len()).map(|i| (seed + i) % 5 != 0).collect();
    let y = (0..n)
        .map(|row| {
            if row >= n - 4 || next() % 5 == 0 {
                1. + (row % 3) as f64 * 0.5
            } else {
                0.
            }
        })
        .collect();
    let frequency = (0..n)
        .map(|_| {
            if seed % 3 == 0 && next() % 3 == 0 {
                2
            } else {
                1
            }
        })
        .collect();
    Case {
        columns,
        intercept,
        y,
        frequency,
        policy: Policy {
            drop_singletons: seed % 7 != 0,
            methods: if seed % 11 != 0 {
                Methods::only(Method::FixedEffects)
            } else {
                Methods::none()
            },
        },
    }
}
fn prepared(c: &Case) -> Prepared {
    let n = c.y.len();
    let slope = c.columns.len();
    let spec = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..=slope)
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: c
            .columns
            .iter()
            .enumerate()
            .map(|(i, _)| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                categorical_sources: vec![SourceId::new(i as u32 + 1).unwrap()],
                intercept: c.intercept[i],
                slope_sources: vec![SourceId::new(slope as u32 + 1).unwrap()],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let mut regressors: Vec<_> = c
        .columns
        .iter()
        .map(|column| column.iter().map(|&v| f64::from(v)).collect())
        .collect();
    regressors.push((0..n).map(|row| (row % 7) as f64 * 0.25).collect());
    let mut reasons = vec![DropReason::MissingOutcome; 2 * n];
    for row in 0..n {
        reasons[2 * row] = DropReason::Retained;
    }
    prepare_fixed_effects(
        &spec,
        Prepared {
            topology: None,
            n_original: 2 * n,
            n_candidate: 2 * n,
            original_row_by_active: (0..n).map(|row| 2 * row).collect(),
            outcome: c.y.clone(),
            regressors,
            weights: PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values: c.frequency.iter().map(|&v| v as f64).collect(),
            },
            adjustment: PreparedAdjustment::Offset((0..n).map(|row| row as f64 * 0.01).collect()),
            drop_reasons: reasons,
        },
    )
    .unwrap()
}
fn groups(column: &[u32], reasons: &[DropReason]) -> BTreeMap<u32, Vec<usize>> {
    let mut groups = BTreeMap::<u32, Vec<usize>>::new();
    for (row, &code) in column.iter().enumerate() {
        if reasons[row] == DropReason::Retained {
            groups.entry(code).or_default().push(row);
        }
    }
    groups
}
fn singleton_oracle(c: &Case, reasons: &mut [DropReason]) {
    if !c.policy.drop_singletons {
        return;
    }
    loop {
        let mut deleted = false;
        for column in &c.columns {
            for rows in groups(column, reasons).values() {
                if rows.iter().map(|&row| c.frequency[row]).sum::<usize>() == 1 {
                    reasons[rows[0]] = DropReason::Singleton;
                    deleted = true;
                }
            }
        }
        if !deleted {
            break;
        }
    }
}
fn exact_oracle(c: &Case, reasons: &mut [DropReason]) -> Vec<usize> {
    let mut rows = vec![];
    if c.policy.methods.contains(Method::FixedEffects) {
        // Compute the union on the unchanged generation, independent of traversal order.
        for (column, &intercept) in c.columns.iter().zip(&c.intercept) {
            if intercept {
                for members in groups(column, reasons).values() {
                    if members.iter().all(|&row| c.y[row] == 0.) {
                        rows.extend_from_slice(members);
                    }
                }
            }
        }
    }
    rows.sort_unstable();
    rows.dedup();
    for &row in &rows {
        reasons[row] = DropReason::FixedEffectSeparation;
    }
    rows
}
fn compare(seed: usize) {
    let c = case(seed);
    let mut expected = vec![DropReason::Retained; c.y.len()];
    singleton_oracle(&c, &mut expected);
    // Pinned legacy singleton parity in addition to the literal grouping oracle.
    let views = c
        .columns
        .iter()
        .map(|column| fereg_core::CategoricalCodeView::new(column, c.y.len() + 1))
        .collect::<Vec<_>>();
    let recipes = (0..c.columns.len()).map(|i| vec![i]).collect::<Vec<_>>();
    let recipe_views = recipes.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let frequency = c.frequency.iter().map(|&f| f as f64).collect::<Vec<_>>();
    let legacy = fereg_core::prune_categorical_partition_singletons(
        &views,
        &recipe_views,
        &vec![true; c.y.len()],
        fereg_core::SingletonMultiplicity::Frequency(&frequency),
        !c.policy.drop_singletons,
    )
    .unwrap();
    assert_eq!(
        legacy.singleton().active,
        expected
            .iter()
            .map(|&r| r == DropReason::Retained)
            .collect::<Vec<_>>(),
        "seed={seed}"
    );
    let mut ledger = Ledger::new(prepared(&c), c.policy).unwrap();
    let mut generation = usize::from(expected.iter().any(|&r| r != DropReason::Retained));
    let initial = ledger.prune_initial_singletons().unwrap();
    assert_eq!(initial.is_some(), generation > 0);
    if c.policy.methods.contains(Method::FixedEffects) {
        loop {
            let proposal = ledger.view().exact_proposal().unwrap();
            let primary = exact_oracle(&c, &mut expected);
            assert_eq!(
                proposal.rows,
                primary.iter().map(|row| 2 * row).collect::<Vec<_>>(),
                "seed={seed}"
            );
            if primary.is_empty() {
                assert!(ledger.accept_exact(&proposal).unwrap().is_none());
                break;
            }
            let old_singletons = expected
                .iter()
                .filter(|&&r| r == DropReason::Singleton)
                .count();
            let mut followup = 0;
            loop {
                singleton_oracle(&c, &mut expected);
                let drops = exact_oracle(&c, &mut expected);
                followup += drops.len();
                if drops.is_empty() {
                    break;
                }
            }
            generation += 1;
            let commit = ledger.accept_exact(&proposal).unwrap().unwrap();
            assert_eq!(
                (commit.generation, commit.primary, commit.exact_followup),
                (generation, primary.len(), followup),
                "seed={seed}"
            );
            assert_eq!(
                commit.singletons,
                expected
                    .iter()
                    .filter(|&&r| r == DropReason::Singleton)
                    .count()
                    - old_singletons
            );
        }
    }
    let out = ledger.view().sample();
    let retained = expected
        .iter()
        .enumerate()
        .filter_map(|(row, &r)| (r == DropReason::Retained).then_some(row))
        .collect::<Vec<_>>();
    assert_eq!(
        out.retained_rows(),
        retained.iter().map(|row| 2 * row).collect::<Vec<_>>(),
        "seed={seed}"
    );
    for (row, &reason) in expected.iter().enumerate() {
        assert_eq!(out.drop_reasons[2 * row], reason);
        assert_eq!(out.drop_reasons[2 * row + 1], DropReason::MissingOutcome);
    }
    let topology = out.topology.as_ref().unwrap();
    topology.validate(out).unwrap();
    let codes = topology
        .partitions()
        .iter()
        .map(|partition| partition.codes())
        .collect::<Vec<_>>();
    let levels = topology
        .partitions()
        .iter()
        .map(|partition| partition.n_levels())
        .collect::<Vec<_>>();
    let old_components =
        fereg_core::connected_components(&codes, &levels, &vec![true; out.n_active()]).unwrap();
    assert_eq!(
        topology.components().n_components,
        old_components.n_components
    );
    assert_eq!(
        topology.components().component_by_effect_level,
        old_components.component_by_effect_level
    );
    for (i, column) in c.columns.iter().enumerate() {
        let levels = groups(column, &expected).into_keys().collect::<Vec<_>>();
        let partition = topology.effect_partition(i).unwrap();
        let expected_codes = retained
            .iter()
            .map(|&row| levels.binary_search(&column[row]).unwrap() as u32)
            .collect::<Vec<_>>();
        assert_eq!(partition.codes(), expected_codes, "seed={seed}");
    }
    assert_eq!(
        out.outcome,
        retained.iter().map(|&row| c.y[row]).collect::<Vec<_>>()
    );
}
#[test]
fn exact_generations_match_literal_groups_and_legacy_singletons() {
    for seed in 0..200 {
        compare(seed);
    }
}
#[test]
#[ignore = "long separation generation differential"]
fn long_exact_generation_differential() {
    for seed in 200..2200 {
        compare(seed);
    }
}
