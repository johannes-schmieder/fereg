//! Lift an independently tested final design back to the public loaded axis.
use fereg_engine::{
    data::*,
    memory::Inputs,
    outputs::{self, Kind},
    ppml::{self, inference::Inference, irls},
    spec::{ClusterTerm, Options},
    statistics,
    threads::ThreadTeam,
    *,
};
fn scatter(sample: &Prepared, values: &[f64]) -> NumericColumn {
    let mut result = vec![f64::NAN; sample.n_original];
    for (&original, &value) in sample.retained_rows().iter().zip(values) {
        result[original] = value;
    }
    NumericColumn {
        values: result,
        missing: None,
    }
}
fn term(slot: usize, source: usize, base: bool, omitted: bool, name: &str) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(slot as u32 + 1).unwrap(),
        order: TermOrder::new(slot as u32 + 1).unwrap(),
        coefficient_slot: CoefficientSlot::new(slot as u32 + 1).unwrap(),
        raw_kind: if base {
            RawTermKind::Factor
        } else {
            RawTermKind::Variable
        },
        whole_column_omitted: omitted,
        canonical_token: name.into(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: SourceId::new(source as u32 + 1).unwrap(),
            kind: if base {
                ComponentKind::Categorical
            } else {
                ComponentKind::Continuous
            },
            level_state: if base {
                LevelState::Present
            } else {
                LevelState::Absent
            },
            level_value: 0.,
            base: if base {
                MetadataFlag::True
            } else {
                MetadataFlag::False
            },
            omitted: MetadataFlag::False,
            raw_operator: if base { "0b" } else { "c" }.into(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}
fn inputs(sample: &Prepared, clusters: &[Vec<u32>]) -> (ModelSpec, Loaded) {
    let ordinary = sample.regressors.len() - 1;
    let mut sources: Vec<_> = sample
        .regressors
        .iter()
        .map(|values| LoadedSource::Numeric(scatter(sample, values)))
        .collect();
    let base = sources.len();
    sources.push(LoadedSource::Numeric(scatter(
        sample,
        &vec![0.; sample.n_active()],
    )));
    let cluster_terms = clusters
        .iter()
        .enumerate()
        .map(|(index, codes)| {
            let source = sources.len();
            sources.push(LoadedSource::Numeric(scatter(
                sample,
                &codes.iter().map(|&v| f64::from(v)).collect::<Vec<_>>(),
            )));
            ClusterTerm {
                label: format!("cluster{index}"),
                sources: vec![SourceId::new(source as u32 + 1).unwrap()],
            }
        })
        .collect();
    let model = ModelSpec {
        model: CompiledTermGraph {
            sources: (0..sources.len())
                .map(|i| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: format!("s{i}"),
                })
                .collect(),
            terms: vec![
                term(0, base, true, true, "base"),
                term(1, ordinary, false, false, "x"),
                term(2, ordinary, false, false, "duplicate"),
                term(3, ordinary, false, true, "omitted"),
            ],
        },
        absorbed_terms: sample
            .topology
            .as_ref()
            .unwrap()
            .effects()
            .iter()
            .map(|effect| AbsorbedTerm {
                id: effect.term_id.unwrap(),
                categorical_sources: effect.categorical_sources.clone(),
                intercept: effect.intercept,
                slope_sources: effect.slopes.iter().map(|s| s.source).collect(),
                label: effect.label.clone(),
                saved_name: String::new(),
            })
            .collect(),
        clusters: cluster_terms,
        include_intercept: true,
        keep_singletons: true,
    };
    let loaded = Loaded {
        candidate: sample
            .drop_reasons
            .iter()
            .map(|reason| *reason != DropReason::ExcludedByIfIn)
            .collect(),
        outcome: scatter(sample, &sample.outcome),
        sources,
        weights: match &sample.weights {
            PreparedWeights::None => None,
            PreparedWeights::Provided { kind, values } => Some((*kind, scatter(sample, values))),
        },
        adjustment: match &sample.adjustment {
            PreparedAdjustment::None => LoadedAdjustment::None,
            PreparedAdjustment::Offset(values) => LoadedAdjustment::Offset(scatter(sample, values)),
            PreparedAdjustment::Exposure(values) => {
                LoadedAdjustment::Exposure(scatter(sample, values))
            }
        },
    };
    (model, loaded)
}
fn same_inference(actual: &ppml::PpmlFit, expected: &Inference) {
    let bits = |values: &[f64]| values.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&actual.coefficients), bits(&expected.coefficients));
    assert_eq!(actual.statuses, expected.statuses);
    assert_eq!(actual.coordinates, expected.coordinates);
    assert_eq!(actual.covariance.values, expected.covariance.values);
    assert_eq!(
        bits(&actual.covariance.standard_errors),
        bits(&expected.covariance.standard_errors)
    );
    assert_eq!(bits(&actual.z_statistics), bits(&expected.z_statistics));
    assert_eq!(actual.covariance.psd, expected.covariance.psd);
    assert_eq!(actual.covariance.correction, expected.covariance.correction);
    assert_eq!(
        actual.covariance.cluster_counts,
        expected.covariance.cluster_counts
    );
    assert_eq!(
        actual.covariance.subset_counts,
        expected.covariance.subset_counts
    );
    assert_eq!(
        actual.covariance.reconstruction_error,
        expected.covariance.reconstruction_error
    );
    assert_eq!(actual.wald, expected.wald);
    assert_eq!(actual.dof, expected.dof);
    assert_eq!(actual.absorbed_dof, expected.absorbed_dof);
}
pub fn compare(
    fit: &irls::Fit<'_>,
    expected: &Inference,
    clusters: &[Vec<u32>],
    mut options: Options,
    team: &ThreadTeam,
    seed: usize,
) {
    let sample = fit.problem().sample;
    let (model, loaded) = inputs(sample, clusters);
    options.threads = team.threads();
    let request = ppml::Request {
        outputs: outputs::PpmlRequest {
            mean: true,
            common: outputs::Request {
                residuals: true,
                xb: true,
                xbd: true,
                absorbed: true,
                drop_reason: true,
                saved: model
                    .absorbed_terms
                    .iter()
                    .enumerate()
                    .flat_map(|(i, term)| {
                        (0..term.slope_sources.len() + usize::from(term.intercept))
                            .map(move |channel| (i, channel))
                    })
                    .collect(),
            },
            residual_type: None,
        },
        ..Default::default()
    };
    let inputs = Inputs::from_loaded(&loaded, 0, 0).unwrap();
    let make = |options: &Options, request: &ppml::Request| {
        ppml::Call::new(
            &model,
            options,
            request,
            inputs,
            loaded.weights.as_ref().map(|(kind, _)| *kind),
            ppml::Resources::default(),
        )
        .unwrap()
        .run_with_team(&loaded, None, team, &mut |_| Ok(()))
        .unwrap()
    };
    let actual = make(&options, &request);
    same_inference(&actual, expected);
    assert_eq!(actual.sample.retained_rows, sample.retained_rows());
    assert_eq!(actual.sample.drop_reasons, sample.drop_reasons);
    assert_eq!(actual.sample.original_rows, sample.n_original);
    assert_eq!(actual.sample.candidate_rows, sample.n_candidate);
    assert_eq!(actual.statistics, statistics::ppml(fit).unwrap());
    assert_eq!(actual.diagnostics.irls, fit.diagnostics);
    assert_eq!(actual.diagnostics.scores, fit.scores);
    assert_eq!(actual.diagnostics.state, fit.state);
    assert!(actual.bias.is_none());
    for (row, &original) in sample.retained_rows().iter().enumerate() {
        assert_eq!(
            actual.outputs.get(Kind::Mean).unwrap()[original],
            fit.evaluation.mean[row]
        );
        assert_eq!(
            actual.outputs.get(Kind::Xbd).unwrap()[original],
            fit.point.eta[row]
        );
    }
    for column in &actual.outputs.columns {
        for (&value, &reason) in column.values.iter().zip(&sample.drop_reasons) {
            if column.kind == Kind::DropReason {
                assert_eq!(value, f64::from(reason.code()));
            } else {
                assert_eq!(value.is_finite(), reason == DropReason::Retained);
            }
        }
    }
    if seed % 19 == 0 {
        let off = make(&options, &ppml::Request::default());
        same_inference(&off, expected);
        assert_eq!(off.statistics, actual.statistics);
        assert_eq!(off.diagnostics, actual.diagnostics);
        assert_eq!(off.separation, actual.separation);
        assert!(off.outputs.columns.is_empty());
    }
}
