use fereg_core::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification, CompiledTermGraph,
    ComponentKind, ComponentOrder, ExpandedTerm, LevelState, MetadataFlag, ModelWireV2,
    RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};

#[test]
fn model_wire_21_round_trips_absorbed_terms_without_losing_empty_saved_names() {
    let specification = CompiledModelSpecification {
        model: CompiledTermGraph {
            sources: vec![
                SourceEntry {
                    id: SourceId::new(1).unwrap(),
                    canonical_name: "x".into(),
                },
                SourceEntry {
                    id: SourceId::new(2).unwrap(),
                    canonical_name: "g".into(),
                },
                SourceEntry {
                    id: SourceId::new(3).unwrap(),
                    canonical_name: "time".into(),
                },
            ],
            terms: vec![ExpandedTerm {
                id: TermId::new(1).unwrap(),
                order: TermOrder::new(1).unwrap(),
                coefficient_slot: CoefficientSlot::new(1).unwrap(),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "c.x".into(),
                components: vec![continuous(1, 1, "c", TimeSeriesState::None, "")],
            }],
        },
        absorbed_terms: vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![SourceId::new(2).unwrap()],
                slope_sources: Vec::new(),
                label: "g".into(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).unwrap(),
                intercept: false,
                categorical_sources: vec![SourceId::new(2).unwrap()],
                slope_sources: vec![SourceId::new(3).unwrap()],
                label: "g#c.time".into(),
                saved_name: "alpha".into(),
            },
        ],
    };
    let wire = ModelWireV2::from_specification(&specification).unwrap();
    assert_eq!(wire.minor, 1.0);
    assert_eq!(wire.absorb_saved_names, vec!["", "alpha"]);
    assert_eq!(wire.decode_specification().unwrap(), specification);
    assert!(
        wire.decode().is_err(),
        "finite-only decode must not drop FEs"
    );
}

#[test]
fn literal_expanded_graph_round_trips_without_reclassification() {
    let expected = CompiledTermGraph {
        sources: vec![
            SourceEntry {
                id: SourceId::new(1).unwrap(),
                canonical_name: "x".into(),
            },
            SourceEntry {
                id: SourceId::new(2).unwrap(),
                canonical_name: "g".into(),
            },
            SourceEntry {
                id: SourceId::new(3).unwrap(),
                canonical_name: "h".into(),
            },
            SourceEntry {
                id: SourceId::new(4).unwrap(),
                canonical_name: "z".into(),
            },
        ],
        terms: vec![
            ExpandedTerm {
                id: TermId::new(1).unwrap(),
                order: TermOrder::new(1).unwrap(),
                coefficient_slot: CoefficientSlot::new(1).unwrap(),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "x".into(),
                components: vec![continuous(1, 1, "", TimeSeriesState::None, "")],
            },
            ExpandedTerm {
                id: TermId::new(2).unwrap(),
                order: TermOrder::new(2).unwrap(),
                coefficient_slot: CoefficientSlot::new(2).unwrap(),
                raw_kind: RawTermKind::Factor,
                whole_column_omitted: false,
                canonical_token: "2.g".into(),
                components: vec![categorical(1, 2, 2.0, MetadataFlag::False, "2")],
            },
            ExpandedTerm {
                id: TermId::new(3).unwrap(),
                order: TermOrder::new(3).unwrap(),
                coefficient_slot: CoefficientSlot::new(3).unwrap(),
                raw_kind: RawTermKind::Factor,
                whole_column_omitted: true,
                canonical_token: "1b.g".into(),
                components: vec![categorical(1, 2, 1.0, MetadataFlag::True, "1b")],
            },
            ExpandedTerm {
                id: TermId::new(4).unwrap(),
                order: TermOrder::new(4).unwrap(),
                coefficient_slot: CoefficientSlot::new(4).unwrap(),
                raw_kind: RawTermKind::Factor,
                whole_column_omitted: false,
                canonical_token: "1bn.g".into(),
                components: vec![categorical(1, 2, 1.0, MetadataFlag::False, "1")],
            },
            ExpandedTerm {
                id: TermId::new(5).unwrap(),
                order: TermOrder::new(5).unwrap(),
                coefficient_slot: CoefficientSlot::new(5).unwrap(),
                raw_kind: RawTermKind::Interaction,
                whole_column_omitted: false,
                canonical_token: "2.g#2.h".into(),
                components: vec![
                    categorical(1, 2, 2.0, MetadataFlag::False, "2"),
                    categorical(2, 3, 2.0, MetadataFlag::False, "2"),
                ],
            },
            ExpandedTerm {
                id: TermId::new(6).unwrap(),
                order: TermOrder::new(6).unwrap(),
                coefficient_slot: CoefficientSlot::new(6).unwrap(),
                raw_kind: RawTermKind::Interaction,
                whole_column_omitted: false,
                canonical_token: "2.g#c.x".into(),
                components: vec![
                    categorical(1, 2, 2.0, MetadataFlag::False, "2"),
                    continuous(2, 1, "c", TimeSeriesState::None, ""),
                ],
            },
            ExpandedTerm {
                id: TermId::new(7).unwrap(),
                order: TermOrder::new(7).unwrap(),
                coefficient_slot: CoefficientSlot::new(7).unwrap(),
                raw_kind: RawTermKind::Interaction,
                whole_column_omitted: false,
                canonical_token: "2.g#2.h#c.z".into(),
                components: vec![
                    categorical(1, 2, 2.0, MetadataFlag::False, "2"),
                    categorical(2, 3, 2.0, MetadataFlag::False, "2"),
                    continuous(3, 4, "c", TimeSeriesState::None, ""),
                ],
            },
            ExpandedTerm {
                id: TermId::new(8).unwrap(),
                order: TermOrder::new(8).unwrap(),
                coefficient_slot: CoefficientSlot::new(8).unwrap(),
                raw_kind: RawTermKind::Interaction,
                whole_column_omitted: false,
                canonical_token: "c.x#c.z".into(),
                components: vec![
                    continuous(1, 1, "c", TimeSeriesState::None, ""),
                    continuous(2, 4, "c", TimeSeriesState::None, ""),
                ],
            },
            ExpandedTerm {
                id: TermId::new(9).unwrap(),
                order: TermOrder::new(9).unwrap(),
                coefficient_slot: CoefficientSlot::new(9).unwrap(),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "L.x".into(),
                components: vec![continuous(
                    1,
                    1,
                    "",
                    TimeSeriesState::PendingMaterialization,
                    "L.",
                )],
            },
            ExpandedTerm {
                id: TermId::new(10).unwrap(),
                order: TermOrder::new(10).unwrap(),
                coefficient_slot: CoefficientSlot::new(10).unwrap(),
                raw_kind: RawTermKind::Variable,
                whole_column_omitted: false,
                canonical_token: "__materialized_L_x".into(),
                components: vec![continuous(1, 1, "", TimeSeriesState::Materialized, "L.")],
            },
        ],
    };

    let decoded = ModelWireV2::from_graph(&expected)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(decoded.terms[4].raw_kind, RawTermKind::Interaction);
    assert_eq!(decoded.terms[4].components[0].level_value, 2.0);
    assert_eq!(decoded.terms[3].canonical_token, "1bn.g");
    assert_eq!(decoded.terms[3].components[0].raw_operator, "1");
}

fn continuous(
    order: u32,
    source: u32,
    raw_operator: &str,
    time_series_state: TimeSeriesState,
    raw_time_series_operator: &str,
) -> TermComponent {
    TermComponent {
        order: ComponentOrder::new(order).unwrap(),
        source: SourceId::new(source).unwrap(),
        kind: ComponentKind::Continuous,
        level_state: LevelState::Absent,
        level_value: 0.0,
        base: MetadataFlag::False,
        omitted: MetadataFlag::Unavailable,
        raw_operator: raw_operator.into(),
        time_series_state,
        raw_time_series_operator: raw_time_series_operator.into(),
    }
}

fn categorical(
    order: u32,
    source: u32,
    level: f64,
    base: MetadataFlag,
    raw_operator: &str,
) -> TermComponent {
    TermComponent {
        order: ComponentOrder::new(order).unwrap(),
        source: SourceId::new(source).unwrap(),
        kind: ComponentKind::Categorical,
        level_state: LevelState::Present,
        level_value: level,
        base,
        omitted: MetadataFlag::Unavailable,
        raw_operator: raw_operator.into(),
        time_series_state: TimeSeriesState::None,
        raw_time_series_operator: String::new(),
    }
}
