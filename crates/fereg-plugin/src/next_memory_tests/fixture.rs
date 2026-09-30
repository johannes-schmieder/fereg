use fereg_engine::{
    data::{
        DomainDecision, Loaded, LoadedAdjustment, LoadedSource, NumericColumn, NumericDomainPolicy,
        WeightKind,
    },
    spec::{metadata::*, ClusterTerm, ModelSpec},
};

pub(super) fn model(clustered: bool) -> ModelSpec {
    ModelSpec {
        model: CompiledTermGraph {
            sources: ["g", "h", "x", "unit"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| SourceEntry {
                    id: SourceId::new(i as u32 + 1).unwrap(),
                    canonical_name: name.into(),
                })
                .collect(),
            terms: ["x", "duplicate", "omitted"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| ExpandedTerm {
                    id: TermId::new(i as u32 + 1).unwrap(),
                    order: TermOrder::new(i as u32 + 1).unwrap(),
                    coefficient_slot: CoefficientSlot::new(i as u32 + 1).unwrap(),
                    raw_kind: RawTermKind::Variable,
                    whole_column_omitted: i == 2,
                    canonical_token: name.into(),
                    components: vec![TermComponent {
                        order: ComponentOrder::new(1).unwrap(),
                        source: SourceId::new(if i == 2 { 4 } else { 3 }).unwrap(),
                        kind: ComponentKind::Continuous,
                        level_state: LevelState::Absent,
                        level_value: 0.,
                        base: MetadataFlag::False,
                        omitted: MetadataFlag::Unavailable,
                        raw_operator: "c".into(),
                        time_series_state: TimeSeriesState::None,
                        raw_time_series_operator: String::new(),
                    }],
                })
                .collect(),
        },
        absorbed_terms: (1..=2)
            .map(|i| AbsorbedTerm {
                id: AbsorbTermId::new(i).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(i).unwrap()],
                slope_sources: vec![],
                label: format!("g{i}"),
                saved_name: String::new(),
            })
            .collect(),
        clusters: if clustered {
            vec![
                ClusterTerm {
                    label: "g".into(),
                    sources: vec![SourceId::new(1).unwrap()],
                },
                ClusterTerm {
                    label: "unit".into(),
                    sources: vec![SourceId::new(4).unwrap()],
                },
            ]
        } else {
            vec![]
        },
        include_intercept: true,
        keep_singletons: true,
    }
}
pub(super) fn loaded(rows: usize, weighted: bool, ppml: bool) -> Loaded {
    let column = |values| NumericColumn {
        values,
        missing: None,
    };
    let numeric = |values| LoadedSource::Numeric(column(values));
    Loaded {
        candidate: (0..rows).map(|r| r % 5 != 0).collect(),
        outcome: column(
            (0..rows)
                .map(|r| {
                    3. + 0.2 * ((r / 8) % 4) as f64 + 0.1 * (r % 8) as f64 + 0.3 * (r as f64).sin()
                })
                .collect(),
        ),
        sources: vec![
            numeric((0..rows).map(|r| ((r / 8) % 4) as f64).collect()),
            numeric((0..rows).map(|r| (r % 8) as f64).collect()),
            numeric(
                (0..rows)
                    .map(|r| ((r * 17) % 97) as f64 / 97. - 0.5)
                    .collect(),
            ),
            numeric((0..rows).map(|r| (r / 32) as f64).collect()),
        ],
        weights: weighted.then(|| {
            (
                WeightKind::Frequency,
                column((0..rows).map(|r| (r % 3 + 1) as f64).collect()),
            )
        }),
        adjustment: if ppml {
            LoadedAdjustment::Offset(column((0..rows).map(|r| 0.05 * (r as f64).sin()).collect()))
        } else {
            LoadedAdjustment::None
        },
    }
}
pub(super) struct Positive;
impl NumericDomainPolicy for Positive {
    fn outcome(&self, _: usize, _: f64) -> fereg_engine::error::Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn weight(
        &self,
        _: usize,
        _: WeightKind,
        _: f64,
    ) -> fereg_engine::error::Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn offset(&self, _: usize, _: f64) -> fereg_engine::error::Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn exposure(&self, _: usize, _: f64) -> fereg_engine::error::Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
}
