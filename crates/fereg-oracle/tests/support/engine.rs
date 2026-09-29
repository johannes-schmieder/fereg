#![allow(dead_code)]
use fereg_engine::{
    data::*, topology::prepare_fixed_effects, AbsorbTermId, AbsorbedTerm, CompiledTermGraph,
    ModelSpec, SourceEntry, SourceId,
};
use fereg_oracle::explicit_ols::DummyEffect;
pub fn prepare(effects: &[DummyEffect], outcome: &[f64]) -> Prepared {
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    for (index, effect) in effects.iter().enumerate() {
        sources.push(effect.codes().iter().map(|&code| f64::from(code)).collect());
        let category = SourceId::new(sources.len() as u32).unwrap();
        let mut slopes = Vec::new();
        for slope in effect.slopes() {
            sources.push(slope.clone());
            slopes.push(SourceId::new(sources.len() as u32).unwrap());
        }
        terms.push(AbsorbedTerm {
            id: AbsorbTermId::new(index as u32 + 1).unwrap(),
            categorical_sources: vec![category],
            intercept: effect.has_intercept(),
            slope_sources: slopes,
            label: format!("g{index}"),
            saved_name: String::new(),
        });
    }
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
        absorbed_terms: terms,
        clusters: vec![],
        include_intercept: true,
        keep_singletons: true,
    };
    prepare_fixed_effects(
        &model,
        Prepared {
            topology: None,
            n_original: outcome.len(),
            n_candidate: outcome.len(),
            original_row_by_active: (0..outcome.len()).collect(),
            outcome: outcome.to_vec(),
            regressors: sources,
            weights: PreparedWeights::None,
            adjustment: PreparedAdjustment::None,
            drop_reasons: vec![DropReason::Retained; outcome.len()],
        },
    )
    .unwrap()
}
