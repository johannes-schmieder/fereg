#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
use fereg_core as old;
use fereg_engine::{
    data::*, dof::*, topology::*, AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ModelSpec,
    SourceEntry, SourceId,
};
use fereg_oracle::designs::{generate, DesignSpec, Graph};

struct Case {
    columns: Vec<Vec<u32>>,
    levels: Vec<usize>,
    recipes: Vec<Vec<usize>>,
    active: Vec<bool>,
    weights: Vec<f64>,
    frequency: bool,
    keep: bool,
}
fn case(seed: usize) -> Case {
    let n = [7, 31, 127, 257][seed % 4];
    let design = generate(
        seed as u64,
        &DesignSpec {
            rows: n,
            levels: vec![5, 4, 3],
            graph: [Graph::DensePanel, Graph::AkmWeak, Graph::Disconnected][seed % 3],
            singleton: true,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let columns: Vec<Vec<_>> = design
        .effects
        .iter()
        .map(|effect| effect.codes().to_vec())
        .collect();
    let levels = columns
        .iter()
        .map(|column| column.iter().max().copied().unwrap_or(0) as usize + 1)
        .collect();
    let recipes = match seed % 5 {
        0 => vec![],
        1 => vec![vec![0]],
        2 => vec![vec![0], vec![1]],
        3 => vec![vec![0, 1], vec![1, 0], vec![2]],
        _ => vec![vec![0], vec![1], vec![2], vec![0]],
    };
    Case {
        columns,
        levels,
        recipes,
        active: (0..n).map(|row| (row * 17 + seed) % 19 != 0).collect(),
        weights: (0..n)
            .map(|row| if (row + seed) % 3 == 0 { 2. } else { 1. })
            .collect(),
        frequency: seed % 2 == 0,
        keep: seed % 7 == 0,
    }
}
fn new_model(case: &Case) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (0..case.columns.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("g{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: case
            .recipes
            .iter()
            .enumerate()
            .map(|(i, recipe)| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                intercept: true,
                categorical_sources: recipe
                    .iter()
                    .map(|&i| SourceId::new(i as u32 + 1).unwrap())
                    .collect(),
                slope_sources: vec![],
                label: format!("effect{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: case.keep,
    }
}
fn new_sample(case: &Case) -> Prepared {
    let rows: Vec<_> = case
        .active
        .iter()
        .enumerate()
        .filter_map(|(i, &active)| active.then_some(i))
        .collect();
    Prepared {
        topology: None,
        n_original: case.active.len(),
        n_candidate: case.active.len(),
        outcome: vec![1.; rows.len()],
        regressors: case
            .columns
            .iter()
            .map(|column| rows.iter().map(|&row| column[row] as f64).collect())
            .collect(),
        weights: if case.frequency {
            PreparedWeights::Provided {
                kind: WeightKind::Frequency,
                values: rows.iter().map(|&row| case.weights[row]).collect(),
            }
        } else {
            PreparedWeights::None
        },
        adjustment: PreparedAdjustment::None,
        drop_reasons: case
            .active
            .iter()
            .map(|&active| {
                if active {
                    DropReason::Retained
                } else {
                    DropReason::OtherModelExclusion
                }
            })
            .collect(),
        original_row_by_active: rows,
    }
}
fn compare(seed: usize) {
    let case = case(seed);
    let source_views: Vec<_> = case
        .columns
        .iter()
        .zip(&case.levels)
        .map(|(codes, &levels)| old::CategoricalCodeView::new(codes, levels))
        .collect();
    let recipes: Vec<_> = case.recipes.iter().map(Vec::as_slice).collect();
    let previous = old::prune_categorical_partition_singletons(
        &source_views,
        &recipes,
        &case.active,
        if case.frequency {
            old::SingletonMultiplicity::Frequency(&case.weights)
        } else {
            old::SingletonMultiplicity::Unit
        },
        case.keep,
    )
    .unwrap();
    let previous = previous.singleton();
    let previous_topology =
        old::finalize_categorical_topology(&source_views, &recipes, &previous.active).unwrap();
    let current = prepare_fixed_effects(&new_model(&case), new_sample(&case)).unwrap();
    let topology = current.topology.as_ref().unwrap();
    assert_eq!(
        current.retained_rows(),
        previous_topology.retained_rows(),
        "seed={seed}"
    );
    assert_eq!(
        current
            .drop_reasons
            .iter()
            .filter(|&&reason| reason == DropReason::Singleton)
            .count(),
        previous.dropped_observations.len(),
        "seed={seed}"
    );
    let previous_views: Vec<_> = previous_topology
        .compacted_sources()
        .iter()
        .map(|source| source.view())
        .collect();
    let mut old_codes = vec![];
    let mut old_levels = vec![];
    for (effect, &index) in previous_topology.effect_to_topology().iter().enumerate() {
        let source = previous_topology.registry().source(index).unwrap();
        let codes = source.codes(&previous_views).unwrap();
        let current = topology.effect_partition(effect).unwrap();
        assert_eq!(codes, current.codes(), "seed={seed}, effect={effect}");
        assert_eq!(source.n_levels(), current.n_levels());
        old_codes.push(codes);
        old_levels.push(source.n_levels());
        for level in 0..current.n_levels() {
            assert_eq!(
                source.members(level).unwrap(),
                current.membership().members(level).unwrap()
            );
        }
    }
    let components =
        old::connected_components(&old_codes, &old_levels, &vec![true; current.n_active()])
            .unwrap();
    assert_eq!(
        components.n_components,
        topology.components().n_components,
        "seed={seed}"
    );
    for (index, effect) in topology.effects().iter().enumerate() {
        assert_eq!(
            components.component_by_effect_level[index],
            topology.components().component_by_effect_level[effect.partition]
        );
    }
    compare_dof(&case, &current, &previous_topology, seed);
}
fn compare_dof(
    case: &Case,
    current: &Prepared,
    previous: &old::FinalizedCategoricalTopology,
    seed: usize,
) {
    let specs: Vec<_> = case
        .recipes
        .iter()
        .enumerate()
        .map(|(i, recipe)| old::CategoricalTopologyDofEffectSpec {
            term_id: old::AbsorbTermId::new(i as u32 + 1).unwrap(),
            sources: recipe
                .iter()
                .map(|&i| old::SourceId::new(i as u32 + 1).unwrap())
                .collect(),
            intercept: true,
            label: recipe
                .iter()
                .map(|i| format!("g{i}"))
                .collect::<Vec<_>>()
                .join("#"),
            slopes: vec![],
        })
        .collect();
    let cluster: Vec<_> = current
        .retained_rows()
        .iter()
        .map(|&row| case.columns[0][row])
        .collect();
    let crossing: Vec<_> = current
        .retained_rows()
        .iter()
        .map(|&row| (row % 3) as u32)
        .collect();
    for clusters in [
        vec![],
        vec![cluster.as_slice()],
        vec![crossing.as_slice()],
        vec![cluster.as_slice(), crossing.as_slice()],
    ] {
        for (old_policy, new_policy) in [
            (old::CategoricalTopologyDofPolicy::None, DofPolicy::None),
            (
                old::CategoricalTopologyDofPolicy::FirstPair,
                DofPolicy::FirstPair,
            ),
            (
                old::CategoricalTopologyDofPolicy::Pairwise,
                DofPolicy::Pairwise,
            ),
            (
                old::CategoricalTopologyDofPolicy::Continuous,
                DofPolicy::Continuous,
            ),
        ] {
            let a = old::compute_categorical_topology_absorbed_dof(
                previous, &specs, true, old_policy, &clusters,
            )
            .unwrap();
            let b = compute_absorbed_dof(current, true, new_policy, &clusters).unwrap();
            assert_eq!(
                (
                    a.initial_categories,
                    a.redundant_categories,
                    a.absorbed_rank,
                    a.nested_categories,
                    a.is_approximate
                ),
                (
                    b.initial_categories,
                    b.redundant_categories,
                    b.absorbed_rank,
                    b.nested_categories,
                    b.is_approximate
                ),
                "seed={seed},policy={new_policy:?}"
            );
            assert_eq!(a.effects.len(), b.effects.len());
            for (a, b) in a.effects.iter().zip(&b.effects) {
                assert_eq!(
                    (
                        &a.label,
                        a.categories,
                        a.redundant,
                        a.coefficients,
                        a.redundancy_exact,
                        a.nested
                    ),
                    (
                        &b.label,
                        b.categories,
                        b.redundant,
                        b.coefficients,
                        b.redundancy_exact,
                        b.nested
                    ),
                    "seed={seed}"
                );
            }
        }
    }
}
#[test]
fn topology_singletons_components_and_dof_are_exact_against_legacy() {
    for seed in 0..200 {
        compare(seed);
    }
}
#[test]
#[ignore = "long topology/DoF differential"]
fn long_topology_and_dof_differential() {
    for seed in 200..2200 {
        compare(seed);
    }
}
