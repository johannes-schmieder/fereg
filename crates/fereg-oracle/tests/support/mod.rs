//! Production-runtime adapter for differential tests: builds native requests
//! from oracle designs; metadata only, never used by an oracle computation.
pub mod ppml;
use fereg_core::*;
use fereg_oracle::designs::{Design, WeightKind};

pub fn native_fit(
    design: &Design,
    config: PreparedOlsFitConfig,
    keep_singletons: bool,
) -> Result<(CertifiedPreparedOlsFit, Vec<usize>)> {
    let (request, loaded) = inputs(design, keep_singletons);
    if design.effects.is_empty() {
        let prepared = prepare_model(&request, &[], &loaded, false)?;
        let fit = fit_prepared_model_coefficients(&prepared, config)?;
        Ok((fit, prepared.sample().original_row_by_active.clone()))
    } else {
        let prepared = prepare_model(&request, &[], &loaded, true)?;
        let fit = fit_prepared_model_coefficients(&prepared, config)?;
        Ok((fit, prepared.sample().original_row_by_active.clone()))
    }
}

pub fn inputs(design: &Design, keep_singletons: bool) -> (OlsRequestV1, LoadedOlsInput) {
    let mut values = design.regressors.clone();
    let mut absorbed_terms = Vec::new();
    for (j, effect) in design.effects.iter().enumerate() {
        let categorical_source = source(values.len());
        values.push(effect.codes().iter().copied().map(f64::from).collect());
        let mut slope_sources = Vec::new();
        for slope in effect.slopes() {
            slope_sources.push(source(values.len()));
            values.push(slope.clone());
        }
        absorbed_terms.push(AbsorbedTerm {
            id: AbsorbTermId::new((j + 1) as u32).unwrap(),
            intercept: effect.has_intercept(),
            categorical_sources: vec![categorical_source],
            slope_sources,
            label: format!("g{j}"),
            saved_name: String::new(),
        });
    }
    let model = CompiledTermGraph {
        sources: (0..values.len())
            .map(|i| SourceEntry {
                id: source(i),
                canonical_name: format!("s{i}"),
            })
            .collect(),
        terms: (0..design.regressors.len()).map(term).collect(),
    };
    let mut bindings = vec![
        OlsInputBinding {
            stata_var_position: 1,
            role: OlsInputRole::Candidate,
        },
        OlsInputBinding {
            stata_var_position: 2,
            role: OlsInputRole::Outcome,
        },
    ];
    bindings.extend((0..values.len()).map(|i| OlsInputBinding {
        stata_var_position: (i + 3) as u32,
        role: OlsInputRole::NumericSource(source(i)),
    }));
    let weight_role = match design.weight_kind {
        WeightKind::Unit => None,
        WeightKind::Analytic => Some(OlsInputRole::AnalyticWeight),
        WeightKind::Frequency => Some(OlsInputRole::FrequencyWeight),
        WeightKind::Probability => Some(OlsInputRole::ProbabilityWeight),
    };
    if let Some(role) = weight_role {
        bindings.push(OlsInputBinding {
            stata_var_position: (values.len() + 3) as u32,
            role,
        });
    }
    let request = OlsRequestV1 {
        model,
        absorbed_terms,
        bindings,
        include_intercept: true,
        keep_singletons,
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; design.outcome.len()],
        outcome: LoadedNumericColumn {
            values: design.outcome.clone(),
            missing: design.missing.clone(),
        },
        numeric_sources: values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (source(i), column(v)))
            .collect(),
        analytic_weight: (design.weight_kind == WeightKind::Analytic)
            .then(|| column(design.weights.clone())),
        frequency_weight: (design.weight_kind == WeightKind::Frequency)
            .then(|| column(design.weights.clone())),
        probability_weight: (design.weight_kind == WeightKind::Probability)
            .then(|| column(design.weights.clone())),
    };
    (request, loaded)
}

fn source(i: usize) -> SourceId {
    SourceId::new((i + 1) as u32).unwrap()
}
fn column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}
fn term(i: usize) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new((i + 1) as u32).unwrap(),
        order: TermOrder::new((i + 1) as u32).unwrap(),
        coefficient_slot: CoefficientSlot::new((i + 1) as u32).unwrap(),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.s{i}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: source(i),
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".into(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}
