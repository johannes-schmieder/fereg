#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
use fereg_core as old;
use fereg_engine::{
    data::*, dof::*, topology::prepare_fixed_effects, AbsorbTermId, AbsorbedTerm,
    CompiledTermGraph, ModelSpec, SourceEntry, SourceId,
};

fn fixture(
    seed: usize,
    intercept: bool,
) -> (old::OlsRequestV1, old::LoadedOlsInput, ModelSpec, Prepared) {
    let n = 48;
    let groups: Vec<f64> = (0..n).map(|row| (row % 4) as f64).collect();
    let slopes: Vec<f64> = (0..n)
        .map(|row| match row % 4 {
            0 => 0.,
            1 => 2.,
            2 => 2. + (row / 4) as f64 * 1e-8,
            _ => ((row * 17 + seed * 7) % 19) as f64 / 7.,
        })
        .collect();
    let values = [groups, slopes];
    let names = ["group", "slope"];
    let old_sources: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| old::SourceEntry {
            id: old::SourceId::new(i as u32 + 1).unwrap(),
            canonical_name: (*name).into(),
        })
        .collect();
    let old_terms = vec![old::AbsorbedTerm {
        id: old::AbsorbTermId::new(1).unwrap(),
        intercept,
        categorical_sources: vec![old::SourceId::new(1).unwrap()],
        slope_sources: vec![old::SourceId::new(2).unwrap()],
        label: "group##c.slope".into(),
        saved_name: String::new(),
    }];
    let mut bindings = vec![
        old::OlsInputBinding {
            stata_var_position: 1,
            role: old::OlsInputRole::Candidate,
        },
        old::OlsInputBinding {
            stata_var_position: 2,
            role: old::OlsInputRole::Outcome,
        },
    ];
    bindings.extend((0..2).map(|i| old::OlsInputBinding {
        stata_var_position: i + 3,
        role: old::OlsInputRole::NumericSource(old::SourceId::new(i + 1).unwrap()),
    }));
    let request = old::OlsRequestV1 {
        model: old::CompiledTermGraph {
            sources: old_sources,
            terms: vec![],
        },
        absorbed_terms: old_terms,
        bindings,
        include_intercept: true,
        keep_singletons: true,
    };
    let column = |values: Vec<f64>| old::LoadedNumericColumn {
        missing: vec![false; n],
        values,
    };
    let loaded = old::LoadedOlsInput {
        candidate: vec![true; n],
        outcome: column(vec![1.; n]),
        numeric_sources: values
            .iter()
            .enumerate()
            .map(|(i, values)| {
                (
                    old::SourceId::new(i as u32 + 1).unwrap(),
                    column(values.clone()),
                )
            })
            .collect(),
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    let specification = ModelSpec {
        model: CompiledTermGraph {
            sources: names
                .iter()
                .enumerate()
                .map(|(i, name)| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: (*name).into(),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept,
            categorical_sources: vec![SourceId::new(1).unwrap()],
            slope_sources: vec![SourceId::new(2).unwrap()],
            label: "group##c.slope".into(),
            saved_name: String::new(),
        }],
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    let sample = Prepared {
        topology: None,
        n_original: n,
        n_candidate: n,
        original_row_by_active: (0..n).collect(),
        outcome: vec![1.; n],
        regressors: values.to_vec(),
        weights: PreparedWeights::None,
        adjustment: PreparedAdjustment::None,
        drop_reasons: vec![DropReason::Retained; n],
    };
    (request, loaded, specification, sample)
}
#[test]
fn slope_channels_and_continuous_redundancy_match_the_legacy_prepared_path() {
    for seed in 0..32 {
        for intercept in [false, true] {
            let (request, loaded, specification, sample) = fixture(seed, intercept);
            let previous = old::prepare_categorical_ols_data(&request, &loaded).unwrap();
            let current = prepare_fixed_effects(&specification, sample).unwrap();
            let cluster: Vec<_> = (0..48).map(|row| (row % 4) as u32).collect();
            for clusters in [vec![], vec![cluster.as_slice()]] {
                for (a, b) in [
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
                    let a = previous.absorbed_dof(a, &clusters).unwrap();
                    let b = compute_absorbed_dof(&current, true, b, &clusters).unwrap();
                    assert_eq!(
                        (
                            a.absorbed_rank,
                            a.redundant_categories,
                            a.nested_categories,
                            a.is_approximate
                        ),
                        (
                            b.absorbed_rank,
                            b.redundant_categories,
                            b.nested_categories,
                            b.is_approximate
                        )
                    );
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
                            )
                        );
                    }
                }
            }
        }
    }
}
