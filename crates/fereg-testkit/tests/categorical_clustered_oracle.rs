use std::collections::{BTreeMap, BTreeSet};

use fereg_core::{
    fit_prepared_model_clustered_with_loaded_moments, prepare_model, AbsorbTermId, AbsorbedTerm,
    CertifiedPreparedOlsClustered, CoefficientSlot, CompiledTermGraph, ComponentKind,
    ComponentOrder, ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag,
    OlsClusterTermV1, OlsClusteredRequestV1, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsClusteredConfig, PreparedOlsFitStatus, RawTermKind, SourceEntry, SourceId,
    TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const COVARIANCE_TOLERANCE: f64 = 1e-7;
const COEFFICIENT_TOLERANCE: f64 = 4e-8;
const RESIDUAL_TOLERANCE: f64 = 4e-8;

const X1_SOURCE: usize = 0;
const X2_SOURCE: usize = 1;
const FIRM_SOURCE: usize = 2;
const YEAR_SOURCE: usize = 3;
const CLUSTER_A_SOURCE: usize = 4;
const CLUSTER_B_SOURCE: usize = 5;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
}

#[test]
fn one_and_two_way_clustered_covariances_match_explicit_interaction_dummies() {
    let case = balanced_case(4);
    for cluster_specs in [
        vec![vec![CLUSTER_A_SOURCE]],
        vec![vec![CLUSTER_A_SOURCE], vec![CLUSTER_B_SOURCE]],
    ] {
        for include_intercept in [false, true] {
            assert_case_matches_oracle(&case, include_intercept, CaseWeights::Unit, &cluster_specs);
        }
    }
}

#[test]
fn declared_cluster_interaction_matches_independent_tuple_oracle() {
    let case = balanced_case(4);
    for include_intercept in [false, true] {
        assert_case_matches_oracle(
            &case,
            include_intercept,
            CaseWeights::Unit,
            &[vec![CLUSTER_A_SOURCE, CLUSTER_B_SOURCE]],
        );
    }
}

#[test]
fn analytic_and_probability_weight_scaling_match_explicit_cluster_scores() {
    let case = balanced_case(4);
    let weights = case
        .x1
        .iter()
        .enumerate()
        .map(|(row, value)| 0.4 + 0.17 * value.abs() + 0.23 * (row % 7) as f64)
        .collect::<Vec<_>>();
    let scaled = weights.iter().map(|value| value * 1e8).collect::<Vec<_>>();
    let cluster_specs = vec![vec![CLUSTER_A_SOURCE], vec![CLUSTER_B_SOURCE]];

    let analytic =
        assert_case_matches_oracle(&case, true, CaseWeights::Analytic(&weights), &cluster_specs);
    let probability = assert_case_matches_oracle(
        &case,
        true,
        CaseWeights::Probability(&weights),
        &cluster_specs,
    );
    let probability_scaled = assert_case_matches_oracle(
        &case,
        true,
        CaseWeights::Probability(&scaled),
        &cluster_specs,
    );

    assert_slice_close(
        analytic.covariance().raw_values(),
        probability.covariance().raw_values(),
        COVARIANCE_TOLERANCE,
    );
    assert_slice_close(
        probability.covariance().raw_values(),
        probability_scaled.covariance().raw_values(),
        COVARIANCE_TOLERANCE,
    );
}

#[test]
fn row_permutation_preserves_clustered_interaction_result() {
    let case = balanced_case(4);
    let cluster_specs = vec![vec![CLUSTER_A_SOURCE], vec![CLUSTER_B_SOURCE]];
    let baseline = fit_case(&case, true, CaseWeights::Unit, &cluster_specs);
    let permutation = (0..case.outcome.len()).rev().collect::<Vec<_>>();
    let permuted_case = case.permute(&permutation);
    let permuted = fit_case(&permuted_case, true, CaseWeights::Unit, &cluster_specs);

    assert_eq!(baseline.fit().finite_rank(), permuted.fit().finite_rank());
    assert_eq!(baseline.cluster_counts(), permuted.cluster_counts());
    assert_eq!(baseline.subsets().len(), permuted.subsets().len());
    for (left, right) in baseline
        .fit()
        .coefficients()
        .iter()
        .zip(permuted.fit().coefficients())
    {
        assert_eq!(left.status, right.status);
        match (left.value, right.value) {
            (Some(left), Some(right)) => {
                assert_close(left, right, COEFFICIENT_TOLERANCE);
            }
            (None, None) => {}
            values => panic!("coefficient availability differs: {values:?}"),
        }
    }
    assert_slice_close(
        baseline.covariance().raw_values(),
        permuted.covariance().raw_values(),
        COVARIANCE_TOLERANCE,
    );
    assert_slice_close(
        baseline.covariance().values(),
        permuted.covariance().values(),
        COVARIANCE_TOLERANCE,
    );
    let mut restored_residuals = vec![0.0_f64; permuted.fit().residuals().len()];
    for (new_row, old_row) in permutation.iter().copied().enumerate() {
        restored_residuals[old_row] = permuted.fit().residuals()[new_row];
    }
    assert_slice_close(
        baseline.fit().residuals(),
        &restored_residuals,
        RESIDUAL_TOLERANCE,
    );
    assert!(baseline.certificate().passed);
    assert!(permuted.certificate().passed);
}

fn assert_case_matches_oracle(
    case: &ClusteredInteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    cluster_specs: &[Vec<usize>],
) -> CertifiedPreparedOlsClustered {
    let production = fit_case(case, include_intercept, weights, cluster_specs);
    let (effect_codes, effect_levels) = independent_interaction_codes(&case.firm, &case.year);
    let oracle_clusters = cluster_specs
        .iter()
        .map(|sources| oracle_cluster_codes(case, sources))
        .collect::<Vec<_>>();
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: case.outcome.clone(),
        regressors: vec![case.x1.clone(), case.x2.clone()],
        effects: vec![
            DummyEffect::new(effect_codes, effect_levels, true, Vec::new())
                .expect("explicit firm-year interaction effect"),
        ],
        weights: oracle_weights(weights),
        clusters: oracle_clusters.clone(),
    })
    .expect("explicit-dummy clustered oracle");

    assert_eq!(production.fit().finite_rank(), oracle.finite_regressor_rank);
    for (index, expected) in oracle.coefficients.iter().enumerate() {
        let expected = expected.expect("identified explicit-dummy coefficient");
        let actual = &production.fit().coefficients()[index];
        assert_eq!(actual.status, PreparedOlsFitStatus::Kept, "slot {index}");
        assert_close(
            actual
                .value
                .expect("identified categorical clustered coefficient"),
            expected,
            COEFFICIENT_TOLERANCE,
        );
    }
    assert_slice_close(
        production.fit().residuals(),
        &oracle.residuals,
        RESIDUAL_TOLERANCE,
    );

    let expected_covariance = if include_intercept {
        oracle
            .normalized_reported_intercept_clustered_vce
            .as_ref()
            .expect("reported-intercept clustered oracle")
    } else {
        oracle
            .clustered_vce
            .as_ref()
            .expect("clustered covariance oracle")
    };
    assert_slice_close(
        production.covariance().raw_values(),
        expected_covariance.as_slice(),
        COVARIANCE_TOLERANCE,
    );

    let expected_counts = oracle_clusters
        .iter()
        .map(|codes| codes.iter().copied().collect::<BTreeSet<_>>().len())
        .collect::<Vec<_>>();
    assert_eq!(production.cluster_counts(), expected_counts.as_slice());
    assert_eq!(
        production.subsets().len(),
        (1_usize << cluster_specs.len()) - 1
    );
    assert_eq!(
        production.dof().reported_residual_dof,
        oracle
            .residual_degrees_of_freedom
            .min(production.dof().minimum_cluster_count - 1)
    );
    assert!(production.fit().absorption_report().certified());
    assert!(production.fit().residual_absorption_report().certified());
    assert!(production.fit().rank_certificate().passed);
    assert!(production.fit().score_certificate().passed);
    assert!(production.certificate().passed);
    production
}

fn fit_case(
    case: &ClusteredInteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    cluster_specs: &[Vec<usize>],
) -> CertifiedPreparedOlsClustered {
    let prepared = prepare_case(case, include_intercept, weights, cluster_specs);
    fit_prepared_model_clustered_with_loaded_moments(
        &prepared,
        PreparedOlsClusteredConfig::default(),
        None,
    )
    .expect("certified categorical clustered fit")
}

fn prepare_case(
    case: &ClusteredInteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    cluster_specs: &[Vec<usize>],
) -> fereg_core::PreparedOlsData {
    let nobs = case.outcome.len();
    assert_eq!(case.x1.len(), nobs);
    assert_eq!(case.x2.len(), nobs);
    assert_eq!(case.firm.len(), nobs);
    assert_eq!(case.year.len(), nobs);
    assert_eq!(case.cluster_a.len(), nobs);
    assert_eq!(case.cluster_b.len(), nobs);

    let mut included_sources = vec![X1_SOURCE, X2_SOURCE, FIRM_SOURCE, YEAR_SOURCE];
    for source in cluster_specs.iter().flatten().copied() {
        if !included_sources.contains(&source) {
            included_sources.push(source);
        }
    }
    let source_names = ["x1", "x2", "firm", "year", "cluster_a", "cluster_b"];
    let sources = included_sources
        .iter()
        .copied()
        .map(|index| SourceEntry {
            id: source_id(index),
            canonical_name: source_names[index].to_owned(),
        })
        .collect::<Vec<_>>();
    let terms = vec![
        continuous_term(0, source_id(X1_SOURCE)),
        continuous_term(1, source_id(X2_SOURCE)),
    ];
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
            .enumerate()
            .map(|(position, index)| OlsInputBinding {
                stata_var_position: u32::try_from(position + 3).expect("small source binding"),
                role: OlsInputRole::NumericSource(source_id(index)),
            }),
    );
    if let Some(role) = weight_role(weights) {
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(included_sources.len() + 3)
                .expect("small weight binding"),
            role,
        });
    }

    let numeric_sources = included_sources
        .iter()
        .copied()
        .map(|index| {
            let values = match index {
                X1_SOURCE => case.x1.clone(),
                X2_SOURCE => case.x2.clone(),
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
                _ => unreachable!("bounded categorical clustered source"),
            };
            (source_id(index), loaded_column(values))
        })
        .collect::<Vec<_>>();
    let raw_weights = weight_values(weights);
    let loaded = LoadedOlsInput {
        candidate: vec![true; nobs],
        outcome: loaded_column(case.outcome.clone()),
        numeric_sources,
        analytic_weight: matches!(weights, CaseWeights::Analytic(_))
            .then(|| loaded_column(raw_weights.clone())),
        frequency_weight: None,
        probability_weight: matches!(weights, CaseWeights::Probability(_))
            .then(|| loaded_column(raw_weights)),
    };
    let request = OlsClusteredRequestV1 {
        ols: OlsRequestV1 {
            model: CompiledTermGraph { sources, terms },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("absorbed interaction id"),
                intercept: true,
                categorical_sources: vec![source_id(FIRM_SOURCE), source_id(YEAR_SOURCE)],
                slope_sources: Vec::new(),
                label: "firm#year".to_owned(),
                saved_name: String::new(),
            }],
            bindings,
            include_intercept,
            keep_singletons: false,
        },
        clusters: cluster_specs
            .iter()
            .enumerate()
            .map(|(index, sources)| OlsClusterTermV1 {
                label: format!("cluster_{}", index + 1),
                sources: sources.iter().copied().map(source_id).collect(),
            })
            .collect(),
    };
    prepare_model(&request.ols, &request.clusters, &loaded, true)
        .expect("prepared categorical clustered oracle case")
}

fn oracle_cluster_codes(case: &ClusteredInteractionCase, sources: &[usize]) -> Vec<u64> {
    match sources {
        [CLUSTER_A_SOURCE] => case
            .cluster_a
            .iter()
            .map(|value| u64::from(*value))
            .collect(),
        [CLUSTER_B_SOURCE] => case
            .cluster_b
            .iter()
            .map(|value| u64::from(*value))
            .collect(),
        [CLUSTER_A_SOURCE, CLUSTER_B_SOURCE] | [CLUSTER_B_SOURCE, CLUSTER_A_SOURCE] => {
            let (codes, _) = independent_interaction_codes(&case.cluster_a, &case.cluster_b);
            codes.into_iter().map(u64::from).collect()
        }
        _ => panic!("unsupported bounded cluster source recipe: {sources:?}"),
    }
}

fn continuous_term(index: usize, source: SourceId) -> ExpandedTerm {
    let one_based = u32::try_from(index + 1).expect("small term index");
    ExpandedTerm {
        id: TermId::new(one_based).expect("term id"),
        order: TermOrder::new(one_based).expect("term order"),
        coefficient_slot: CoefficientSlot::new(one_based).expect("coefficient slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.x{one_based}"),
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

fn independent_interaction_codes(first: &[u32], second: &[u32]) -> (Vec<u32>, usize) {
    assert_eq!(first.len(), second.len());
    let tuples = first
        .iter()
        .copied()
        .zip(second.iter().copied())
        .collect::<BTreeSet<_>>();
    let mapping = tuples
        .iter()
        .copied()
        .enumerate()
        .map(|(index, tuple)| (tuple, u32::try_from(index).expect("small interaction code")))
        .collect::<BTreeMap<_, _>>();
    let codes = first
        .iter()
        .copied()
        .zip(second.iter().copied())
        .map(|tuple| mapping[&tuple])
        .collect::<Vec<_>>();
    (codes, tuples.len())
}

fn source_id(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source index")).expect("source id")
}

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

fn weight_role(weights: CaseWeights<'_>) -> Option<OlsInputRole> {
    match weights {
        CaseWeights::Unit => None,
        CaseWeights::Analytic(_) => Some(OlsInputRole::AnalyticWeight),
        CaseWeights::Probability(_) => Some(OlsInputRole::ProbabilityWeight),
    }
}

fn weight_values(weights: CaseWeights<'_>) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => Vec::new(),
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => values.to_vec(),
    }
}

fn oracle_weights(weights: CaseWeights<'_>) -> OracleWeights {
    match weights {
        CaseWeights::Unit => OracleWeights::Unit,
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => {
            OracleWeights::Wls(values.to_vec())
        }
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "actual={actual:.17e}, expected={expected:.17e}, tolerance={tolerance:.3e}"
    );
}

fn assert_slice_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().copied().zip(expected.iter().copied()) {
        assert_close(actual, expected, tolerance);
    }
}

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|row| values[*row]).collect()
}

#[derive(Clone)]
struct ClusteredInteractionCase {
    outcome: Vec<f64>,
    x1: Vec<f64>,
    x2: Vec<f64>,
    firm: Vec<u32>,
    year: Vec<u32>,
    cluster_a: Vec<u32>,
    cluster_b: Vec<u32>,
}

impl ClusteredInteractionCase {
    fn permute(&self, permutation: &[usize]) -> Self {
        Self {
            outcome: permute(&self.outcome, permutation),
            x1: permute(&self.x1, permutation),
            x2: permute(&self.x2, permutation),
            firm: permute(&self.firm, permutation),
            year: permute(&self.year, permutation),
            cluster_a: permute(&self.cluster_a, permutation),
            cluster_b: permute(&self.cluster_b, permutation),
        }
    }
}

fn balanced_case(repetitions: usize) -> ClusteredInteractionCase {
    let mut firm = Vec::new();
    let mut year = Vec::new();
    let mut repetition_index = Vec::new();
    for firm_value in 0_u32..4 {
        for year_value in 0_u32..3 {
            for repetition in 0..repetitions {
                firm.push(firm_value);
                year.push(year_value);
                repetition_index.push(repetition);
            }
        }
    }
    let x1 = (0..firm.len())
        .map(|row| {
            let repetition = repetition_index[row] as f64;
            (0.29 * row as f64).sin() + 0.41 * repetition + 0.13 * f64::from(year[row])
        })
        .collect::<Vec<_>>();
    let x2 = (0..firm.len())
        .map(|row| {
            let repetition = repetition_index[row] as f64;
            (0.17 * row as f64).cos() - 0.33 * repetition + 0.19 * f64::from(firm[row])
        })
        .collect::<Vec<_>>();
    let cluster_a = (0..firm.len())
        .map(|row| {
            u32::try_from((firm[row] as usize + 2 * year[row] as usize + repetition_index[row]) % 7)
                .expect("bounded cluster A")
        })
        .collect::<Vec<_>>();
    let cluster_b = (0..firm.len())
        .map(|row| {
            u32::try_from(
                (2 * firm[row] as usize + year[row] as usize + 3 * repetition_index[row]) % 5,
            )
            .expect("bounded cluster B")
        })
        .collect::<Vec<_>>();
    let outcome = (0..firm.len())
        .map(|row| {
            let cell = 0.75 * f64::from(firm[row]) - 0.44 * f64::from(year[row])
                + 0.16 * f64::from(firm[row] * year[row]);
            1.55 * x1[row] - 0.68 * x2[row] + cell + 0.05 * (0.83 * row as f64).sin()
        })
        .collect::<Vec<_>>();
    ClusteredInteractionCase {
        outcome,
        x1,
        x2,
        firm,
        year,
        cluster_a,
        cluster_b,
    }
}
