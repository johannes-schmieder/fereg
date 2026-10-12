//! Metadata adapters for complete-estimator comparisons; no numerical kernels.
use fereg_core as old;
use fereg_oracle::designs::{generate, DesignSpec, Graph, WeightKind as OracleWeight};

pub struct Case {
    pub request: old::OlsRequestV1,
    pub loaded: old::LoadedOlsInput,
    pub clusters: Vec<old::OlsClusterTermV1>,
    pub vce: usize,
    pub dof: usize,
}

pub fn case(seed: usize) -> Case {
    let effects = (seed / 16) % 5;
    let n = 193 + seed % 23;
    let design = generate(
        seed as u64,
        &DesignSpec {
            rows: n,
            levels: match effects {
                0 => vec![],
                1 | 4 => vec![5],
                _ => vec![5, 4],
            },
            regressors: 2,
            graph: if seed % 11 == 0 {
                Graph::Disconnected
            } else {
                Graph::DensePanel
            },
            slopes: effects == 4 || seed % 7 == 0,
            weights: [
                OracleWeight::Unit,
                OracleWeight::Analytic,
                OracleWeight::Frequency,
                OracleWeight::Probability,
            ][seed % 4],
            singleton: effects > 0,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let (mut request, mut loaded) = super::support::inputs(&design, seed % 9 == 0);
    request.include_intercept = seed % 13 != 0;
    if effects == 3 {
        let second = request.absorbed_terms.remove(1);
        request.absorbed_terms[0]
            .categorical_sources
            .extend(second.categorical_sources);
        request.absorbed_terms[0]
            .slope_sources
            .extend(second.slope_sources);
        request.absorbed_terms[0].label = "g0#g1".into();
    }
    if effects == 4 {
        request.absorbed_terms[0].intercept = false;
    }
    let (f, z, ts_source) = add_regressors(&mut request, &mut loaded, &design.regressors[0], seed);
    let vce = (seed / 4) % 4;
    let mut clusters = vec![];
    if vce >= 2 {
        let c = if seed % 5 == 0 && !request.absorbed_terms.is_empty() {
            request.absorbed_terms[0].categorical_sources[0]
        } else {
            add_source(
                &mut request,
                &mut loaded,
                "cluster",
                (0..n)
                    .map(|row| ((row / 3 + row % 5) % 11) as f64)
                    .collect(),
            )
        };
        clusters.push(old::OlsClusterTermV1 {
            label: "cluster".into(),
            sources: vec![c],
        });
        if vce == 3 {
            let sources = if seed % 2 == 0 { vec![f, z] } else { vec![f] };
            clusters.push(old::OlsClusterTermV1 {
                label: "f#z".into(),
                sources,
            });
            if seed % 5 == 0 {
                clusters.push(clusters[0].clone());
            }
        }
        loaded.numeric_sources[c.get() as usize - 1].1.missing[6] = true;
    }
    loaded.candidate.explicit_mut().unwrap()[0] = false;
    loaded.candidate.explicit_mut().unwrap()[8] = false;
    loaded.outcome.missing[1] = true;
    loaded.numeric_sources[0].1.missing[2] = true;
    loaded.numeric_sources[f.get() as usize - 1].1.missing[4] = true;
    loaded.numeric_sources[f.get() as usize - 1].1.values[8] = 9.;
    if effects > 0 {
        // The generated last FE level is a singleton under unit multiplicity;
        // factor 9 therefore also tests disappearance after FE pruning.
        loaded.numeric_sources[f.get() as usize - 1].1.values[n - 1] = 9.;
    }
    loaded.numeric_sources[ts_source.get() as usize - 1]
        .1
        .missing[7] = true;
    let mask = &mut loaded.numeric_sources[ts_source.get() as usize - 1]
        .1
        .missing;
    match seed % 3 {
        0 => {
            mask[0] = true;
            mask[3] = true;
        }
        1 => {
            mask[n - 1] = true;
            mask[1] = true;
        }
        _ => {
            mask[0] = true;
            mask[2] = true;
            mask[3] = true;
        }
    }
    if let Some(effect) = request.absorbed_terms.first() {
        loaded.numeric_sources[effect.categorical_sources[0].get() as usize - 1]
            .1
            .missing[5] = true;
    }
    for column in [
        &mut loaded.analytic_weight,
        &mut loaded.frequency_weight,
        &mut loaded.probability_weight,
    ]
    .into_iter()
    .flatten()
    {
        column.missing[3] = true;
    }
    if let Some(column) = &mut loaded.analytic_weight {
        column.values[10] = 0.;
    }
    Case {
        request,
        loaded,
        clusters,
        vce,
        dof: (seed / 80) % 4,
    }
}

fn add_regressors(
    request: &mut old::OlsRequestV1,
    loaded: &mut old::LoadedOlsInput,
    x: &[f64],
    seed: usize,
) -> (old::SourceId, old::SourceId, old::SourceId) {
    let n = x.len();
    let f = add_source(
        request,
        loaded,
        "f",
        (0..n)
            .map(|row| ((row * 17 + seed + row / 5) % 3) as f64)
            .collect(),
    );
    let z = add_source(
        request,
        loaded,
        "z",
        (0..n)
            .map(|row| ((row * 13 + row / 7) % 2) as f64)
            .collect(),
    );
    for (level, base) in [(0., true), (1., false), (2., false), (9., false)] {
        let mut term = variable(
            request,
            f,
            format!("{level}{}.f", if base { "b" } else { "" }),
        );
        term.raw_kind = old::RawTermKind::Factor;
        term.whole_column_omitted = base;
        term.components[0].kind = old::ComponentKind::Categorical;
        term.components[0].level_state = old::LevelState::Present;
        term.components[0].level_value = level;
        term.components[0].base = if base {
            old::MetadataFlag::True
        } else {
            old::MetadataFlag::False
        };
        term.components[0].raw_operator = format!("{level}{}", if base { "b" } else { "" });
        request.model.terms.push(term);
    }
    for (level, base) in [(0., true), (1., false)] {
        let mut term = request.model.terms[2].clone();
        set_identity(&mut term, request.model.terms.len());
        term.canonical_token = format!("{level}{}.z", if base { "b" } else { "" });
        term.whole_column_omitted = base;
        term.components[0].source = z;
        term.components[0].level_value = level;
        term.components[0].base = if base {
            old::MetadataFlag::True
        } else {
            old::MetadataFlag::False
        };
        term.components[0].raw_operator = format!("{level}{}", if base { "b" } else { "" });
        request.model.terms.push(term);
    }
    let mut interaction = request.model.terms[3].clone();
    set_identity(&mut interaction, request.model.terms.len());
    interaction.canonical_token = "1.f#1.z".into();
    interaction.raw_kind = old::RawTermKind::Interaction;
    let mut component = interaction.components[0].clone();
    component.order = old::ComponentOrder::new(2).unwrap();
    component.source = z;
    component.level_value = 1.;
    interaction.components.push(component);
    request.model.terms.push(interaction);
    let ts = match seed % 3 {
        0 => "L",
        1 => "F",
        _ => "D",
    };
    let values = (0..n)
        .map(|row| match ts {
            "L" => x[row.saturating_sub(1)],
            "F" => x[(row + 1).min(n - 1)],
            _ => x[row] - x[row.saturating_sub(1)],
        })
        .collect();
    let ts_source = add_source(request, loaded, "materialized", values);
    let mut term = variable(request, ts_source, format!("{ts}.x0"));
    term.components[0].time_series_state = old::TimeSeriesState::Materialized;
    term.components[0].raw_time_series_operator = ts.into();
    request.model.terms.push(term);
    let duplicate = variable(
        request,
        old::SourceId::new(1).unwrap(),
        "duplicate.x0".into(),
    );
    request.model.terms.push(duplicate);
    let mut omitted = variable(request, old::SourceId::new(2).unwrap(), "o.x1".into());
    omitted.whole_column_omitted = true;
    omitted.components[0].omitted = old::MetadataFlag::True;
    request.model.terms.push(omitted);
    (f, z, ts_source)
}

fn add_source(
    request: &mut old::OlsRequestV1,
    loaded: &mut old::LoadedOlsInput,
    name: &str,
    values: Vec<f64>,
) -> old::SourceId {
    let id = old::SourceId::new(request.model.sources.len() as u32 + 1).unwrap();
    request.model.sources.push(old::SourceEntry {
        id,
        canonical_name: name.into(),
    });
    request.bindings.push(old::OlsInputBinding {
        stata_var_position: request.bindings.len() as u32 + 1,
        role: old::OlsInputRole::NumericSource(id),
    });
    loaded.numeric_sources.push((
        id,
        old::LoadedNumericColumn {
            missing: vec![false; values.len()],
            values,
        },
    ));
    id
}

fn set_identity(term: &mut old::ExpandedTerm, index: usize) {
    term.id = old::TermId::new(index as u32 + 1).unwrap();
    term.order = old::TermOrder::new(index as u32 + 1).unwrap();
    term.coefficient_slot = old::CoefficientSlot::new(index as u32 + 1).unwrap();
}
fn variable(
    request: &old::OlsRequestV1,
    source: old::SourceId,
    token: String,
) -> old::ExpandedTerm {
    let mut term = request.model.terms[0].clone();
    set_identity(&mut term, request.model.terms.len());
    term.canonical_token = token;
    term.components[0].source = source;
    term
}
