#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
//! Harvested from legacy/fereg-core at b1700e6d; scientific validation is preserved.
use fereg_engine::{
    AbsorbTermId, AbsorbedTerm, CoefficientSlot, CompiledModelSpecification, CompiledTermGraph,
    ComponentKind, ComponentOrder, ErrorCode, ExpandedTerm, LevelState, MetadataFlag, ModelWireV2,
    RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};

#[test]
fn versions_and_numeric_headers_fail_closed() {
    let mut wire = minimal_wire();
    wire.major = 1.0;
    assert_code(&wire, ErrorCode::ProtocolMismatch);

    let mut wire = minimal_wire();
    wire.minor = 2.0;
    assert_code(&wire, ErrorCode::ProtocolMismatch);

    for value in [f64::NAN, f64::INFINITY, 2.5, 0.0, -1.0] {
        let mut wire = minimal_wire();
        wire.major = value;
        assert_rejected(&wire);
    }
    for value in [f64::NAN, f64::INFINITY, 1.5, -1.0] {
        let mut wire = minimal_wire();
        wire.terms.declared_count = value;
        assert_rejected(&wire);
    }
}

#[test]
fn absorbed_tables_are_versioned_and_structurally_strict() {
    let mut legacy_empty = minimal_wire();
    legacy_empty.minor = 0.0;
    assert!(legacy_empty
        .decode_specification()
        .unwrap()
        .absorbed_terms
        .is_empty());

    let base = minimal_wire().decode().unwrap();
    let specification = CompiledModelSpecification {
        model: base,
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).unwrap(),
            intercept: true,
            categorical_sources: vec![SourceId::new(1).unwrap()],
            slope_sources: Vec::new(),
            label: "x".into(),
            saved_name: String::new(),
        }],
    };
    let wire = ModelWireV2::from_specification(&specification).unwrap();
    assert_eq!(wire.decode_specification().unwrap(), specification);

    let mut old_minor = wire.clone();
    old_minor.minor = 0.0;
    assert_eq!(
        old_minor.decode_specification().unwrap_err().code,
        ErrorCode::ProtocolMismatch
    );

    let mut bad_id = wire.clone();
    bad_id.absorb_terms.rows[0][0] = 2.0;
    assert!(bad_id.decode_specification().is_err());

    let mut bad_role = wire.clone();
    bad_role.absorb_components.rows[0][1] = 9.0;
    assert!(bad_role.decode_specification().is_err());

    let mut bad_order = wire.clone();
    bad_order.absorb_components.rows[0][2] = 2.0;
    assert!(bad_order.decode_specification().is_err());

    let mut bad_source = wire.clone();
    bad_source.absorb_components.rows[0][3] = 2.0;
    assert!(bad_source.decode_specification().is_err());

    let mut bad_registry = wire.clone();
    bad_registry.absorb_saved_names.clear();
    assert!(bad_registry.decode_specification().is_err());

    let mut bad_sentinel = minimal_wire();
    bad_sentinel.absorb_terms.rows[0][0] = 1.0;
    assert!(bad_sentinel.decode_specification().is_err());
}

#[test]
fn widths_cardinalities_reserved_and_registries_are_exact() {
    let mut wire = minimal_wire();
    wire.terms.rows[0].pop();
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.terms.declared_count = 2.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.sources.rows[0][1] = 1.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.terms.rows[0][6] = 1.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][12] = 1.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.source_names.clear();
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.canonical_tokens.push("extra".into());
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.raw_component_operators.clear();
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.raw_time_series_operators.clear();
    assert_rejected(&wire);
}

#[test]
fn ids_orders_slots_and_component_parentage_are_strictly_one_based() {
    for (column, value) in [(0, 0.0), (0, 2.0), (1, 2.0), (2, 2.0)] {
        let mut wire = minimal_wire();
        wire.terms.rows[0][column] = value;
        assert_rejected(&wire);
    }

    let mut wire = minimal_wire();
    wire.sources.rows[0][0] = 0.0;
    assert_rejected(&wire);

    for value in [f64::NAN, f64::INFINITY, -1.0, 1.5] {
        let mut wire = minimal_wire();
        wire.components.rows[0][2] = value;
        assert_rejected(&wire);
    }

    let mut wire = minimal_wire();
    wire.components.rows[0][0] = 2.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][1] = 2.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][2] = 2.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][9] = 2.0;
    assert_rejected(&wire);

    let decoded = minimal_wire().decode().unwrap();
    assert_eq!(decoded.sources[0].id.get(), 1);
    assert_eq!(decoded.terms[0].components[0].source.get(), 1);

    let mut wire = two_term_wire();
    wire.terms.rows[1][1] = 1.0;
    assert_rejected(&wire);

    let mut wire = two_term_wire();
    wire.terms.rows[1][2] = 1.0;
    assert_rejected(&wire);
}

#[test]
fn unknown_enums_statuses_and_illegal_level_states_are_rejected() {
    for (column, value) in [(3, 99.0), (4, 99.0), (6, 99.0), (7, 99.0), (8, 99.0)] {
        let mut wire = minimal_wire();
        wire.components.rows[0][column] = value;
        assert_rejected(&wire);
    }

    let mut wire = minimal_wire();
    wire.terms.rows[0][3] = 99.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][4] = 2.0;
    wire.components.rows[0][5] = 7.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][5] = 7.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][6] = 1.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.raw_component_operators[0] = "invented".into();
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.components.rows[0][8] = 2.0;
    assert_rejected(&wire);
}

#[test]
fn term_component_counts_and_empty_sentinels_are_not_guessed() {
    let mut wire = minimal_wire();
    wire.terms.rows[0][4] = 2.0;
    assert_rejected(&wire);

    let mut wire = minimal_wire();
    wire.terms.rows[0][4] = 0.0;
    assert_rejected(&wire);

    let empty = CompiledTermGraph {
        sources: Vec::new(),
        terms: Vec::new(),
    };
    let mut wire = ModelWireV2::from_graph(&empty).unwrap();
    wire.terms.rows.clear();
    assert_rejected(&wire);

    let mut wire = ModelWireV2::from_graph(&empty).unwrap();
    wire.terms.rows[0][0] = 1.0;
    assert_rejected(&wire);
}

#[test]
fn generic_interaction_and_unavailable_metadata_remain_distinct() {
    let graph = CompiledTermGraph {
        sources: vec![
            SourceEntry {
                id: SourceId::new(1).unwrap(),
                canonical_name: "g".into(),
            },
            SourceEntry {
                id: SourceId::new(2).unwrap(),
                canonical_name: "x".into(),
            },
        ],
        terms: vec![ExpandedTerm {
            id: TermId::new(1).unwrap(),
            order: TermOrder::new(1).unwrap(),
            coefficient_slot: CoefficientSlot::new(1).unwrap(),
            raw_kind: RawTermKind::Interaction,
            whole_column_omitted: false,
            canonical_token: "2.g#c.x".into(),
            components: vec![
                TermComponent {
                    order: ComponentOrder::new(1).unwrap(),
                    source: SourceId::new(1).unwrap(),
                    kind: ComponentKind::Categorical,
                    level_state: LevelState::Present,
                    level_value: 2.0,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::Unavailable,
                    raw_operator: "2".into(),
                    time_series_state: TimeSeriesState::None,
                    raw_time_series_operator: String::new(),
                },
                TermComponent {
                    order: ComponentOrder::new(2).unwrap(),
                    source: SourceId::new(2).unwrap(),
                    kind: ComponentKind::Continuous,
                    level_state: LevelState::Absent,
                    level_value: 0.0,
                    base: MetadataFlag::False,
                    omitted: MetadataFlag::Unavailable,
                    raw_operator: "c".into(),
                    time_series_state: TimeSeriesState::Unavailable,
                    raw_time_series_operator: String::new(),
                },
            ],
        }],
    };
    let decoded = ModelWireV2::from_graph(&graph).unwrap().decode().unwrap();
    assert_eq!(decoded.terms[0].raw_kind, RawTermKind::Interaction);
    assert_eq!(decoded.terms[0].components[0].level_value, 2.0);
    assert_eq!(
        decoded.validate_bindable().unwrap_err().code,
        ErrorCode::Unsupported
    );
}

fn minimal_wire() -> ModelWireV2 {
    ModelWireV2::from_graph(&CompiledTermGraph {
        sources: vec![SourceEntry {
            id: SourceId::new(1).unwrap(),
            canonical_name: "x".into(),
        }],
        terms: vec![ExpandedTerm {
            id: TermId::new(1).unwrap(),
            order: TermOrder::new(1).unwrap(),
            coefficient_slot: CoefficientSlot::new(1).unwrap(),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "x".into(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).unwrap(),
                source: SourceId::new(1).unwrap(),
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: String::new(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }],
    })
    .unwrap()
}

fn two_term_wire() -> ModelWireV2 {
    let mut graph = minimal_wire().decode().unwrap();
    graph.terms.push(ExpandedTerm {
        id: TermId::new(2).unwrap(),
        order: TermOrder::new(2).unwrap(),
        coefficient_slot: CoefficientSlot::new(2).unwrap(),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: "x_again".into(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).unwrap(),
            source: SourceId::new(1).unwrap(),
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: String::new(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    });
    ModelWireV2::from_graph(&graph).unwrap()
}

fn assert_rejected(wire: &ModelWireV2) {
    assert!(wire.decode().is_err(), "mutated wire was accepted");
}

fn assert_code(wire: &ModelWireV2, expected: ErrorCode) {
    assert_eq!(wire.decode().unwrap_err().code, expected);
}
