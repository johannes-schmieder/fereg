#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
use fereg_engine::{
    data::*, dof::*, topology::*, AbsorbTermId, AbsorbedTerm, CompiledTermGraph, ErrorCode,
    ModelSpec, SourceEntry, SourceId,
};
fn model(sources: usize, recipes: &[&[usize]], keep: bool) -> ModelSpec {
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
                intercept: true,
                categorical_sources: recipe
                    .iter()
                    .map(|&source| SourceId::new(source as u32 + 1).unwrap())
                    .collect(),
                slope_sources: vec![],
                label: format!("effect{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: vec![],
        include_intercept: true,
        keep_singletons: keep,
    }
}
fn sample(columns: &[&[f64]]) -> Prepared {
    let n = columns.first().map_or(0, |column| column.len());
    Prepared {
        topology: None,
        n_original: n,
        n_candidate: n,
        original_row_by_active: (0..n).collect(),
        outcome: vec![1.; n],
        regressors: columns.iter().map(|column| column.to_vec()).collect(),
        weights: PreparedWeights::None,
        adjustment: PreparedAdjustment::None,
        drop_reasons: vec![DropReason::Retained; n],
    }
}
#[test]
fn recursive_singletons_compact_the_authoritative_sample_and_all_banks() {
    let input = sample(&[&[0., 0., 1., 2.], &[0., 1., 1., 2.]]);
    let prepared = prepare_fixed_effects(&model(2, &[&[0], &[1]], false), input).unwrap();
    assert!(prepared.retained_rows().is_empty());
    assert_eq!(prepared.n_original, 4);
    assert_eq!(prepared.drop_reasons, vec![DropReason::Singleton; 4]);
    let topology = prepared.topology.as_ref().unwrap();
    assert_eq!(topology.components().n_components, 0);
    assert!(topology
        .partitions()
        .iter()
        .all(|partition| partition.n_levels() == 0));
    topology.validate(&prepared).unwrap();
}
#[test]
fn frequency_mass_and_physical_membership_have_distinct_singleton_rules() {
    let mut input = sample(&[&[0., 1.]]);
    input.weights = PreparedWeights::Provided {
        kind: WeightKind::Frequency,
        values: vec![2., 1.],
    };
    let prepared = prepare_fixed_effects(&model(1, &[&[0]], false), input).unwrap();
    assert_eq!(prepared.retained_rows(), &[0]);
    assert_eq!(
        prepared.weights,
        PreparedWeights::Provided {
            kind: WeightKind::Frequency,
            values: vec![2.]
        }
    );
    let topology = prepared.topology.as_ref().unwrap();
    assert_eq!(
        topology.partitions()[0].membership().members(0).unwrap(),
        &[0]
    );
}
#[test]
fn repeated_and_reordered_interactions_share_one_membership_index() {
    let input = sample(&[&[2., 0., 2., 1., 0., 2., 1.], &[1., 2., 1., 0., 2., 0., 0.]]);
    let prepared =
        prepare_fixed_effects(&model(2, &[&[0, 1], &[1, 0], &[0, 1, 0]], true), input).unwrap();
    let topology = prepared.topology.as_ref().unwrap();
    assert_eq!(topology.partitions().len(), 1);
    assert_eq!(topology.partitions()[0].codes(), &[3, 0, 3, 1, 0, 2, 1]);
    assert!(topology
        .effects()
        .iter()
        .all(|effect| effect.partition == 0));
    let report = compute_absorbed_dof(&prepared, true, DofPolicy::Pairwise, &[]).unwrap();
    assert_eq!(report.absorbed_rank, 4);
    assert_eq!(
        report
            .effects
            .iter()
            .map(|row| row.redundant)
            .collect::<Vec<_>>(),
        vec![0, 4, 4]
    );
    assert_eq!(report.effects[1].label, "g2#g1");
    assert!(report.is_approximate);
}
#[test]
fn continuous_dof_preserves_the_pinned_local_slope_convention() {
    for intercept in [false, true] {
        let mut specification = model(2, &[&[0]], true);
        specification.absorbed_terms[0].intercept = intercept;
        specification.absorbed_terms[0].slope_sources = vec![SourceId::new(2).unwrap()];
        let prepared = prepare_fixed_effects(
            &specification,
            sample(&[&[0., 0., 1., 1., 2., 2.], &[0., 1., 2., 2., 0., 0.]]),
        )
        .unwrap();
        let report = compute_absorbed_dof(&prepared, true, DofPolicy::Continuous, &[]).unwrap();
        let slope = report.effects.last().unwrap();
        assert_eq!(slope.label, "g1#c.g2");
        assert_eq!(slope.categories, 3);
        assert_eq!(slope.redundant, if intercept { 2 } else { 1 });
        assert_eq!(report.absorbed_rank, if intercept { 4 } else { 2 });
        assert!(report.is_approximate);
        let without_adjustment =
            compute_absorbed_dof(&prepared, true, DofPolicy::None, &[]).unwrap();
        assert_eq!(
            without_adjustment.absorbed_rank,
            if intercept { 6 } else { 3 }
        );
    }
}
#[test]
fn compacting_a_later_generation_invalidates_topology_and_preserves_original_rows() {
    let specification = model(1, &[&[0]], true);
    let prepared = prepare_fixed_effects(&specification, sample(&[&[0., 0., 1., 1.]])).unwrap();
    let next = prepared
        .compact_active_rows(&[false, true, true, false], DropReason::MuSeparation)
        .unwrap();
    assert!(next.topology.is_none());
    let next = prepare_fixed_effects(&specification, next).unwrap();
    assert_eq!(next.retained_rows(), &[1, 2]);
    assert_eq!(
        next.topology.as_ref().unwrap().partitions()[0].codes(),
        &[0, 1]
    );
    assert_eq!(next.n_candidate, 4);
}
#[test]
fn no_absorb_and_cluster_nesting_follow_the_reference_dof_policy() {
    let prepared =
        prepare_fixed_effects(&model(1, &[], false), sample(&[&[0., 0., 1., 1.]])).unwrap();
    assert_eq!(
        compute_absorbed_dof(&prepared, true, DofPolicy::None, &[])
            .unwrap()
            .absorbed_rank,
        1
    );
    assert_eq!(
        compute_absorbed_dof(&prepared, false, DofPolicy::None, &[])
            .unwrap()
            .absorbed_rank,
        0
    );
    let prepared = prepare_fixed_effects(
        &model(2, &[&[0], &[1]], true),
        sample(&[&[0., 0., 1., 1.], &[0., 1., 0., 1.]]),
    )
    .unwrap();
    let report =
        compute_absorbed_dof(&prepared, true, DofPolicy::Pairwise, &[&[0, 0, 1, 1]]).unwrap();
    assert_eq!(report.nested_categories, 2);
    assert!(report.effects[0].nested);
    assert_eq!(report.effects[1].redundant, 1);
    assert_eq!(report.absorbed_rank, 1);
    assert_eq!(
        compute_absorbed_dof(&prepared, true, DofPolicy::Pairwise, &[&[0]])
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}
#[test]
fn component_labels_and_csr_match_literal_independent_membership() {
    let first = [4, 0, 2, 4, 1, 6, 2, 0];
    let second = [3, 1, 5, 3, 0, 4, 5, 1];
    let third = [2, 0, 4, 2, 1, 3, 4, 0];
    let info = connected_components(&[&first, &second, &third], &[7, 6, 5], &[true; 8]).unwrap();
    assert_eq!(info.n_components, 5);
    assert_eq!(
        info.component_by_effect_level,
        vec![
            vec![0, 1, 2, usize::MAX, 3, usize::MAX, 4],
            vec![1, 0, usize::MAX, 3, 4, 2],
            vec![0, 1, 3, 4, 2]
        ]
    );
    let index = LevelIndex::from_codes(&first, 7).unwrap();
    for level in 0..7 {
        let expected: Vec<_> = first
            .iter()
            .enumerate()
            .filter_map(|(row, &code)| (code as usize == level).then_some(row))
            .collect();
        assert_eq!(index.members(level).unwrap(), expected);
    }
    assert!(index.members(7).is_err());
    assert_eq!(
        LevelIndex::from_codes(&[], usize::MAX).unwrap_err().code,
        ErrorCode::Allocation
    );
}
