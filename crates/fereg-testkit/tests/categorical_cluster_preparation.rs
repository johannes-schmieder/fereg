use std::collections::{BTreeMap, BTreeSet};

use fereg_core::{
    prepare_categorical_ols_clustered_data, AbsorbTermId, AbsorbedTerm, CoefficientSlot,
    CompiledTermGraph, ComponentKind, ComponentOrder, DropReason, ErrorCode, ExpandedTerm,
    LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsClusterTermV1,
    OlsClusteredRequestV1, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedCategoricalOlsClusteredData, RawTermKind, SourceEntry, SourceId, TermComponent, TermId,
    TermOrder, TimeSeriesState,
};

const X_SOURCE: usize = 0;
const FIRM_SOURCE: usize = 1;
const YEAR_SOURCE: usize = 2;
const CLUSTER_A_SOURCE: usize = 3;
const CLUSTER_B_SOURCE: usize = 4;

#[test]
fn simple_cluster_only_source_is_retained_and_borrowed_zero_copy() {
    let case = balanced_case(3);
    let prepared = prepare_case(
        &case,
        &[cluster_term("cluster_a", &[CLUSTER_A_SOURCE])],
        &[],
    )
    .expect("simple categorical cluster preparation");

    assert_eq!(prepared.ols().sample().n_active(), case.outcome.len());
    assert_eq!(prepared.ols().singletons().num_singletons, 0);
    assert_eq!(prepared.clusters().len(), 1);
    let dimension = &prepared.clusters().dimensions()[0];
    assert_eq!(dimension.label(), "cluster_a");
    assert_eq!(dimension.sources(), &[source_id(CLUSTER_A_SOURCE)]);
    assert!(!dimension.is_interaction());
    assert_eq!(
        dimension.groups(),
        case.cluster_a
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
    );

    let source = prepared
        .ols()
        .categorical_sources()
        .iter()
        .find(|source| source.source() == source_id(CLUSTER_A_SOURCE))
        .expect("retained cluster-only source");
    let source_codes = source
        .codes(prepared.ols().fe_topology())
        .expect("source codes");
    let cluster_codes = prepared.cluster_codes(0).expect("cluster codes");
    assert!(std::ptr::eq(source_codes.as_ptr(), cluster_codes.as_ptr()));
    assert_eq!(source_codes.len(), case.outcome.len());
}

#[test]
fn missing_cluster_is_reason_six_before_singleton_pruning() {
    let case = balanced_case(3);
    let prepared = prepare_case(
        &case,
        &[cluster_term("cluster_a", &[CLUSTER_A_SOURCE])],
        &[(CLUSTER_A_SOURCE, 0)],
    )
    .expect("missing cluster preparation");

    assert_eq!(
        prepared.ols().sample().drop_reason(0),
        Some(DropReason::MissingCluster)
    );
    assert_eq!(prepared.ols().sample().n_active(), case.outcome.len() - 1);
    assert_eq!(prepared.ols().singletons().num_singletons, 0);
    assert!(!prepared.ols().sample().retained_rows().contains(&0));
    assert_eq!(
        prepared
            .cluster_codes(0)
            .expect("final cluster codes")
            .len(),
        case.outcome.len() - 1
    );
}

#[test]
fn fixed_effect_missing_precedes_cluster_missing() {
    let case = balanced_case(3);
    let prepared = prepare_case(
        &case,
        &[cluster_term("cluster_a", &[CLUSTER_A_SOURCE])],
        &[(FIRM_SOURCE, 0), (CLUSTER_A_SOURCE, 0)],
    )
    .expect("joint missing preparation");

    assert_eq!(
        prepared.ols().sample().drop_reason(0),
        Some(DropReason::MissingFixedEffect)
    );
    assert_eq!(prepared.ols().singletons().num_singletons, 0);
}

#[test]
fn singleton_cluster_group_never_drives_fe_singleton_deletion() {
    let mut case = balanced_case(3);
    case.cluster_a.fill(0);
    *case.cluster_a.last_mut().expect("nonempty case") = 1;
    let prepared = prepare_case(
        &case,
        &[cluster_term("cluster_a", &[CLUSTER_A_SOURCE])],
        &[],
    )
    .expect("singleton cluster-group preparation");

    assert_eq!(prepared.ols().sample().n_active(), case.outcome.len());
    assert_eq!(prepared.ols().singletons().num_singletons, 0);
    assert_eq!(prepared.clusters().dimensions()[0].groups(), 2);
    assert_eq!(
        prepared
            .cluster_codes(0)
            .expect("singleton cluster-group codes")
            .iter()
            .filter(|code| **code == 1)
            .count(),
        1
    );
}

#[test]
fn categorical_cluster_interaction_matches_independent_tuple_partition() {
    let case = balanced_case(3);
    let prepared = prepare_case(
        &case,
        &[cluster_term(
            "cluster_a#cluster_b",
            &[CLUSTER_A_SOURCE, CLUSTER_B_SOURCE],
        )],
        &[],
    )
    .expect("categorical cluster interaction preparation");

    let dimension = &prepared.clusters().dimensions()[0];
    assert!(dimension.is_interaction());
    assert_eq!(
        dimension.sources(),
        &[source_id(CLUSTER_A_SOURCE), source_id(CLUSTER_B_SOURCE)]
    );
    let expected = independent_tuple_codes(
        &case.cluster_a,
        &case.cluster_b,
        prepared.ols().sample().retained_rows(),
    );
    assert_eq!(dimension.groups(), expected.1);
    assert_eq!(
        prepared.cluster_codes(0).expect("interaction codes"),
        expected.0
    );
}

#[test]
fn row_permutation_preserves_cluster_interaction_partition() {
    let case = balanced_case(3);
    let baseline = prepare_case(
        &case,
        &[cluster_term(
            "cluster_a#cluster_b",
            &[CLUSTER_A_SOURCE, CLUSTER_B_SOURCE],
        )],
        &[],
    )
    .expect("baseline interaction");
    let permutation = (0..case.outcome.len()).rev().collect::<Vec<_>>();
    let permuted_case = case.permute(&permutation);
    let permuted = prepare_case(
        &permuted_case,
        &[cluster_term(
            "cluster_a#cluster_b",
            &[CLUSTER_A_SOURCE, CLUSTER_B_SOURCE],
        )],
        &[],
    )
    .expect("permuted interaction");

    let baseline_codes = baseline.cluster_codes(0).expect("baseline codes");
    let permuted_codes = permuted.cluster_codes(0).expect("permuted codes");
    let mut restored = vec![0_u32; permuted_codes.len()];
    for (new_row, old_row) in permutation.iter().copied().enumerate() {
        restored[old_row] = permuted_codes[new_row];
    }
    assert_eq!(baseline_codes, restored);
}

#[test]
fn ten_cluster_dimensions_are_accepted_and_share_simple_source_codes() {
    let case = balanced_case(3);
    let clusters = (0..10)
        .map(|index| cluster_term(&format!("cluster_{index}"), &[CLUSTER_A_SOURCE]))
        .collect::<Vec<_>>();
    let prepared = prepare_case(&case, &clusters, &[]).expect("ten cluster dimensions");
    assert_eq!(prepared.clusters().len(), 10);
    let first = prepared.cluster_codes(0).expect("first codes");
    for dimension in 1..10 {
        let codes = prepared
            .cluster_codes(dimension)
            .expect("reused simple codes");
        assert!(std::ptr::eq(first.as_ptr(), codes.as_ptr()));
    }
}

#[test]
fn invalid_cluster_shapes_fail_closed() {
    let case = balanced_case(3);
    let no_clusters = prepare_case(&case, &[], &[]).expect_err("zero cluster dimensions");
    assert_eq!(no_clusters.code, ErrorCode::InvalidArgument);

    let eleven = (0..11)
        .map(|index| cluster_term(&format!("cluster_{index}"), &[CLUSTER_A_SOURCE]))
        .collect::<Vec<_>>();
    let too_many = prepare_case(&case, &eleven, &[]).expect_err("eleven cluster dimensions");
    assert_eq!(too_many.code, ErrorCode::InvalidArgument);

    let mut one_group = case.clone();
    one_group.cluster_a.fill(0);
    let insufficient = prepare_case(
        &one_group,
        &[cluster_term("one_group", &[CLUSTER_A_SOURCE])],
        &[],
    )
    .expect_err("one final cluster group");
    assert_eq!(insufficient.code, ErrorCode::InvalidData);
}

#[test]
fn cluster_code_accessor_rejects_out_of_range_dimension() {
    let case = balanced_case(3);
    let prepared = prepare_case(
        &case,
        &[cluster_term("cluster_a", &[CLUSTER_A_SOURCE])],
        &[],
    )
    .expect("simple cluster preparation");
    assert_eq!(
        prepared
            .cluster_codes(1)
            .expect_err("out-of-range dimension")
            .code,
        ErrorCode::InvalidArgument
    );
}

fn prepare_case(
    case: &ClusterCase,
    clusters: &[OlsClusterTermV1],
    missing: &[(usize, usize)],
) -> fereg_core::Result<PreparedCategoricalOlsClusteredData> {
    let nobs = case.outcome.len();
    assert_eq!(case.x.len(), nobs);
    assert_eq!(case.firm.len(), nobs);
    assert_eq!(case.year.len(), nobs);
    assert_eq!(case.cluster_a.len(), nobs);
    assert_eq!(case.cluster_b.len(), nobs);

    let uses_cluster_a = clusters
        .iter()
        .any(|term| term.sources.contains(&source_id(CLUSTER_A_SOURCE)));
    let uses_cluster_b = clusters
        .iter()
        .any(|term| term.sources.contains(&source_id(CLUSTER_B_SOURCE)));
    let mut included_sources = vec![X_SOURCE, FIRM_SOURCE, YEAR_SOURCE];
    if uses_cluster_a {
        included_sources.push(CLUSTER_A_SOURCE);
    }
    if uses_cluster_b {
        included_sources.push(CLUSTER_B_SOURCE);
    }

    let mut numeric_sources = included_sources
        .iter()
        .copied()
        .map(|index| {
            let values = match index {
                X_SOURCE => case.x.to_vec(),
                FIRM_SOURCE => case.firm.iter().map(|value| f64::from(*value)).collect(),
                YEAR_SOURCE => case.year.iter().map(|value| f64::from(*value)).collect(),
                CLUSTER_A_SOURCE => case
                    .cluster_a
                    .iter()
                    .map(|value| f64::from(*value))
                    .collect(),
                CLUSTER_B_SOURCE => case
                    .cluster_b
                    .iter()
                    .map(|value| f64::from(*value))
                    .collect(),
                _ => unreachable!("bounded test source index"),
            };
            (source_id(index), loaded_column(values))
        })
        .collect::<Vec<_>>();
    for (source_index, row) in missing.iter().copied() {
        assert!(row < nobs);
        let (_, column) = numeric_sources
            .iter_mut()
            .find(|(source, _)| *source == source_id(source_index))
            .expect("missing marker source is included");
        column.missing[row] = true;
    }

    let source_names = ["x", "firm", "year", "cluster_a", "cluster_b"];
    let sources = included_sources
        .iter()
        .copied()
        .map(|index| SourceEntry {
            id: source_id(index),
            canonical_name: source_names[index].to_owned(),
        })
        .collect::<Vec<_>>();
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
    bindings.extend(
        included_sources
            .iter()
            .copied()
            .map(|index| OlsInputBinding {
                stata_var_position: u32::try_from(index + 3).expect("small binding"),
                role: OlsInputRole::NumericSource(source_id(index)),
            }),
    );
    let request = OlsClusteredRequestV1 {
        ols: OlsRequestV1 {
            model: CompiledTermGraph {
                sources,
                terms: vec![continuous_term(source_id(X_SOURCE))],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed term id"),
                intercept: true,
                categorical_sources: vec![source_id(FIRM_SOURCE), source_id(YEAR_SOURCE)],
                slope_sources: Vec::new(),
                label: "firm#year".to_owned(),
                saved_name: String::new(),
            }],
            bindings,
            include_intercept: false,
            keep_singletons: false,
        },
        clusters: clusters.to_vec(),
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; nobs],
        outcome: loaded_column(case.outcome.clone()),
        numeric_sources,
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    let borrowed = prepare_categorical_ols_clustered_data(&request, &loaded);
    let owned = fereg_core::ols::prepare_categorical_ols_clustered_data_owned(&request, loaded);
    match (&borrowed, &owned) {
        (Ok(a), Ok(b)) => assert_eq!(a, b),
        (Err(a), Err(b)) => {
            assert_eq!(a.code, b.code);
            assert_eq!(a.message, b.message);
        }
        other => panic!("categorical ownership changed preparation: {other:?}"),
    }
    owned
}

fn cluster_term(label: &str, sources: &[usize]) -> OlsClusterTermV1 {
    OlsClusterTermV1 {
        label: label.to_owned(),
        sources: sources.iter().copied().map(source_id).collect(),
    }
}

fn continuous_term(source: SourceId) -> ExpandedTerm {
    ExpandedTerm {
        id: TermId::new(1).expect("term id"),
        order: TermOrder::new(1).expect("term order"),
        coefficient_slot: CoefficientSlot::new(1).expect("coefficient slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: "c.x".to_owned(),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component order"),
            source,
            kind: ComponentKind::Continuous,
            level_state: LevelState::Absent,
            level_value: 0.0,
            base: MetadataFlag::False,
            omitted: MetadataFlag::Unavailable,
            raw_operator: "c".to_owned(),
            time_series_state: TimeSeriesState::None,
            raw_time_series_operator: String::new(),
        }],
    }
}

fn independent_tuple_codes(
    first: &[u32],
    second: &[u32],
    retained_rows: &[usize],
) -> (Vec<u32>, usize) {
    let tuples = retained_rows
        .iter()
        .map(|row| (first[*row], second[*row]))
        .collect::<BTreeSet<_>>();
    let mapping = tuples
        .iter()
        .copied()
        .enumerate()
        .map(|(index, tuple)| (tuple, u32::try_from(index).expect("small interaction code")))
        .collect::<BTreeMap<_, _>>();
    let codes = retained_rows
        .iter()
        .map(|row| mapping[&(first[*row], second[*row])])
        .collect::<Vec<_>>();
    (codes, tuples.len())
}

fn source_id(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source id")).expect("source id")
}

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

#[derive(Clone)]
struct ClusterCase {
    outcome: Vec<f64>,
    x: Vec<f64>,
    firm: Vec<u32>,
    year: Vec<u32>,
    cluster_a: Vec<u32>,
    cluster_b: Vec<u32>,
}

impl ClusterCase {
    fn permute(&self, permutation: &[usize]) -> Self {
        Self {
            outcome: permute(&self.outcome, permutation),
            x: permute(&self.x, permutation),
            firm: permute(&self.firm, permutation),
            year: permute(&self.year, permutation),
            cluster_a: permute(&self.cluster_a, permutation),
            cluster_b: permute(&self.cluster_b, permutation),
        }
    }
}

fn balanced_case(repetitions: usize) -> ClusterCase {
    let mut firm = Vec::new();
    let mut year = Vec::new();
    let mut cluster_a = Vec::new();
    let mut cluster_b = Vec::new();
    for firm_value in 0_u32..4 {
        for year_value in 0_u32..3 {
            for repetition in 0..repetitions {
                firm.push(firm_value);
                year.push(year_value);
                cluster_a.push(
                    u32::try_from((firm_value as usize + 2 * repetition + year_value as usize) % 5)
                        .expect("bounded cluster a"),
                );
                cluster_b.push(
                    u32::try_from((2 * firm_value as usize + repetition + year_value as usize) % 4)
                        .expect("bounded cluster b"),
                );
            }
        }
    }
    let x = (0..firm.len())
        .map(|row| {
            (0.31 * row as f64).sin() + 0.17 * f64::from(firm[row]) - 0.09 * f64::from(year[row])
        })
        .collect::<Vec<_>>();
    let outcome = (0..firm.len())
        .map(|row| {
            1.4 * x[row] + 0.7 * f64::from(firm[row]) - 0.45 * f64::from(year[row])
                + 0.12 * f64::from(firm[row] * year[row])
                + 0.03 * (row as f64 * 0.8).cos()
        })
        .collect::<Vec<_>>();
    ClusterCase {
        outcome,
        x,
        firm,
        year,
        cluster_a,
        cluster_b,
    }
}

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|row| values[*row]).collect()
}
