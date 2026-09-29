use fereg_engine::{
    data::*,
    dof::{compute_absorbed_dof, DofPolicy},
    factor::factorize_code_interaction,
    spec::ClusterTerm,
    *,
};
mod support;
fn model(sources: usize, recipes: &[&[u32]]) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: (1..=sources)
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: vec![],
        clusters: recipes
            .iter()
            .enumerate()
            .map(|(i, recipe)| ClusterTerm {
                label: format!("cluster{i}"),
                sources: recipe
                    .iter()
                    .map(|&id| SourceId::new(id).unwrap())
                    .collect(),
            })
            .collect(),
        include_intercept: true,
        keep_singletons: true,
    }
}
#[test]
fn cluster_dimensions_reuse_fe_and_duplicate_partitions_without_relabeling() {
    let sample = support::prepared(&[7, 5], false, 10, 0);
    let spec = model(2, &[&[1], &[1, 2], &[2, 1], &[2], &[1]]);
    let frame = ClusterFrame::new(&spec, &sample).unwrap();
    let views = frame.views().unwrap();
    let topology = sample.topology.as_ref().unwrap();
    assert!(std::ptr::eq(
        views[0].codes,
        topology.effect_partition(0).unwrap().codes()
    ));
    assert!(std::ptr::eq(
        views[3].codes,
        topology.effect_partition(1).unwrap().codes()
    ));
    assert!(std::ptr::eq(views[0].codes, views[4].codes));
    assert!(std::ptr::eq(views[1].codes, views[2].codes));
    assert_eq!(
        views[1].codes,
        factorize_code_interaction(&[views[0], views[3]])
            .unwrap()
            .codes
    );
    let codes: Vec<_> = views.iter().map(|v| v.codes).collect();
    let dof = compute_absorbed_dof(&sample, true, DofPolicy::Pairwise, &codes).unwrap();
    assert_eq!(dof.nested_categories, 12);
    assert_eq!(dof.absorbed_rank, 0);
}
#[test]
fn cluster_only_sources_use_collision_safe_observed_tuples_and_share_storage() {
    let mut sample = support::prepared(&[], false, 1, 2);
    sample.regressors = vec![vec![-0.5, -0.5, 2.25, 2.25], vec![2., 3., 2., 3.]];
    for recipes in [
        vec![vec![1], vec![2], vec![1, 2], vec![2, 1], vec![1]],
        vec![vec![1, 2], vec![1], vec![2], vec![2, 1], vec![1]],
    ] {
        let borrowed: Vec<_> = recipes.iter().map(Vec::as_slice).collect();
        let frame = ClusterFrame::new(&model(2, &borrowed), &sample).unwrap();
        let views = frame.views().unwrap();
        for (dimension, recipe) in recipes.iter().enumerate() {
            assert_eq!(
                views[dimension].n_levels,
                if recipe.len() == 1 { 2 } else { 4 }
            );
            for a in 0..4 {
                for b in 0..4 {
                    assert_eq!(
                        views[dimension].codes[a] == views[dimension].codes[b],
                        recipe
                            .iter()
                            .all(|&id| sample.regressors[id as usize - 1][a]
                                == sample.regressors[id as usize - 1][b])
                    );
                }
            }
        }
        let first = recipes.iter().position(|r| *r == vec![1]).unwrap();
        assert!(std::ptr::eq(views[first].codes, views[4].codes));
    }
}
#[test]
fn cluster_frame_requires_final_alignment_and_rejects_bad_metadata() {
    let mut sample = support::prepared(&[3], false, 3, 0);
    assert!(ClusterFrame::new(&model(1, &[]), &sample)
        .unwrap()
        .views()
        .unwrap()
        .is_empty());
    assert!(ClusterFrame::new(&model(1, &[&[1, 1]]), &sample).is_err());
    assert!(ClusterFrame::new(&model(2, &[&[2]]), &sample).is_err());
    sample.regressors[0].pop();
    assert!(ClusterFrame::new(&model(1, &[&[1]]), &sample).is_err());
    sample.regressors[0].push(2.);
    sample.topology = None;
    assert!(ClusterFrame::new(&model(1, &[&[1]]), &sample).is_err());
}

#[test]
fn interacted_fe_partition_is_borrowed_after_singleton_compaction() {
    let mut sample = support::prepared(&[], false, 1, 2);
    sample.regressors = vec![vec![0., 0., 1., 2.], vec![1., 1., 1., 2.]];
    let mut spec = model(2, &[&[2, 1], &[1], &[2]]);
    spec.keep_singletons = false;
    spec.absorbed_terms = vec![AbsorbedTerm {
        id: AbsorbTermId::new(1).unwrap(),
        intercept: true,
        categorical_sources: vec![SourceId::new(2).unwrap(), SourceId::new(1).unwrap()],
        slope_sources: vec![],
        label: "s2#s1".into(),
        saved_name: String::new(),
    }];
    let sample = fereg_engine::topology::prepare_fixed_effects(&spec, sample).unwrap();
    assert_eq!(sample.original_row_by_active, vec![0, 1]);
    let frame = ClusterFrame::new(&spec, &sample).unwrap();
    let views = frame.views().unwrap();
    assert!(std::ptr::eq(
        views[0].codes,
        sample
            .topology
            .as_ref()
            .unwrap()
            .effect_partition(0)
            .unwrap()
            .codes()
    ));
    assert!(views
        .iter()
        .all(|view| view.n_levels == 1 && view.codes == [0, 0]));
}
