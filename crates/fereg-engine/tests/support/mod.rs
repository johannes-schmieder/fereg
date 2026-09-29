#![allow(dead_code)]
use fereg_engine::{
    data::*, topology::prepare_fixed_effects, AbsorbTermId, AbsorbedTerm, CompiledTermGraph,
    ModelSpec, SourceEntry, SourceId,
};
pub fn prepared(
    levels: &[usize],
    slopes: bool,
    repetitions: usize,
    extra_columns: usize,
) -> Prepared {
    let n = levels.iter().copied().max().unwrap_or(4) * repetitions;
    let mut sources: Vec<Vec<f64>> = levels
        .iter()
        .map(|&levels| (0..n).map(|row| (row % levels) as f64).collect())
        .collect();
    let slope_source = sources.len();
    if slopes {
        sources.push((0..n).map(|row| (row % 7) as f64 - 2.).collect());
    }
    sources.extend((0..extra_columns).map(|_| vec![2.; n]));
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sources.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![],
        },
        absorbed_terms: levels
            .iter()
            .enumerate()
            .map(|(i, _)| AbsorbedTerm {
                id: AbsorbTermId::new(i as u32 + 1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(i as u32 + 1).unwrap()],
                slope_sources: if slopes {
                    vec![SourceId::new(slope_source as u32 + 1).unwrap()]
                } else {
                    vec![]
                },
                label: format!("effect{i}"),
                saved_name: String::new(),
            })
            .collect(),
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
        regressors: sources,
        weights: PreparedWeights::None,
        adjustment: PreparedAdjustment::None,
        drop_reasons: vec![DropReason::Retained; n],
    };
    prepare_fixed_effects(&model, sample).unwrap()
}
