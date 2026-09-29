use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use fereg_core::{
    fit_prepared_categorical_ols_unadjusted_map, prepare_categorical_ols_data, AbsorbTermId,
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ErrorCode,
    ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedCategoricalOlsData, PreparedOlsUnadjustedConfig,
    RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
    WeightKind,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, ExplicitOlsResult, OracleWeights,
};

const IID_TOLERANCE: f64 = 2e-7;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn firm_year_iid_inference_matches_explicit_dummy_oracle() {
    let case = balanced_case();
    let analytic = case.regressors[0]
        .iter()
        .enumerate()
        .map(|(row, value)| 0.35 + value.abs() * 0.2 + (row % 5) as f64 * 0.31)
        .collect::<Vec<_>>();
    let frequencies = (0..case.outcome.len())
        .map(|row| u32::try_from(1 + (row * 7) % 5).expect("bounded frequency"))
        .collect::<Vec<_>>();

    for include_intercept in [false, true] {
        for reverse_absorb_sources in [false, true] {
            for weights in [
                CaseWeights::Unit,
                CaseWeights::Analytic(&analytic),
                CaseWeights::Frequency(&frequencies),
            ] {
                assert_iid_case_matches_oracle(
                    &case,
                    include_intercept,
                    weights,
                    reverse_absorb_sources,
                );
            }
        }
    }
}

#[test]
fn firm_year_probability_weight_iid_fails_closed() {
    let case = balanced_case();
    let probability = case.regressors[1]
        .iter()
        .enumerate()
        .map(|(row, value)| 0.4 + value.abs() * 0.15 + (row % 4) as f64 * 0.27)
        .collect::<Vec<_>>();
    let prepared = prepare_case(&case, true, CaseWeights::Probability(&probability), false);
    let error = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect_err("probability-weight iid covariance must fail closed");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(error.message.contains("probability weights"));
}

fn assert_iid_case_matches_oracle(
    case: &InteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    reverse_absorb_sources: bool,
) {
    let prepared = prepare_case(case, include_intercept, weights, reverse_absorb_sources);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified categorical iid inference");
    assert!(result.fit().absorption_report().certified());
    assert!(result.fit().residual_absorption_report().certified());
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.fit().bread().certificate().passed);
    assert!(result.certificate().passed);

    let (interaction, n_levels) = independent_interaction_codes(&case.firm, &case.year);
    let effect = DummyEffect::new(interaction, n_levels, true, Vec::new())
        .expect("explicit interaction effect");
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: case.outcome.clone(),
        regressors: case.regressors.clone(),
        effects: vec![effect.clone()],
        weights: oracle_weights(weights),
        clusters: Vec::new(),
    })
    .expect("explicit-dummy iid oracle");
    let nuisance_oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: case.outcome.clone(),
        regressors: Vec::new(),
        effects: vec![effect],
        weights: oracle_weights(weights),
        clusters: Vec::new(),
    })
    .expect("explicit-dummy nuisance-only oracle");

    let weight_report = result.weights();
    let reference_weights = iid_reference_weights(weights, case.outcome.len());
    let effective_observations = iid_effective_observations(weights, case.outcome.len());
    assert_eq!(weight_report.effective_observations, effective_observations);
    assert_eq!(weight_report.physical_observations, case.outcome.len());
    assert_weight_report(weight_report, weights, case.outcome.len());

    let dof = result.dof();
    assert_eq!(dof.finite_rank, oracle.finite_regressor_rank);
    assert_eq!(dof.finite_model_dof, oracle.finite_regressor_rank);
    assert_eq!(dof.absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(dof.model_rank, oracle.model_rank);
    assert_eq!(dof.residual_dof, oracle.residual_degrees_of_freedom);
    assert!(dof.has_structural_intercept);
    assert_eq!(dof.has_normalized_absorbed_intercept, include_intercept);
    assert!(!dof.absorbed_rank_is_approximate);

    let absorbed = result.absorbed_dof();
    assert_eq!(absorbed.initial_categories, n_levels);
    assert_eq!(absorbed.absorbed_rank, oracle.explicit_dummy_rank);
    assert_eq!(
        absorbed.redundant_categories,
        n_levels - oracle.explicit_dummy_rank
    );
    assert_eq!(absorbed.nested_categories, 0);
    assert!(!absorbed.is_approximate);
    assert_eq!(absorbed.effects.len(), 1);
    assert_eq!(absorbed.effects[0].categories, n_levels);
    assert_eq!(absorbed.effects[0].coefficients, oracle.explicit_dummy_rank);
    assert!(!absorbed.effects[0].nested);

    let expected_rss = weighted_sum_of_squares(&oracle.residuals, &reference_weights, 0.0);
    let outcome_mean = weighted_mean(&case.outcome, &reference_weights);
    let expected_tss = weighted_sum_of_squares(&case.outcome, &reference_weights, outcome_mean);
    let expected_within_tss =
        weighted_sum_of_squares(&nuisance_oracle.residuals, &reference_weights, 0.0);
    let expected_variance_scale = expected_rss / dof.residual_dof as f64;

    let statistics = result.statistics();
    assert_close(statistics.rss, expected_rss, IID_TOLERANCE);
    assert_close(statistics.tss, expected_tss, IID_TOLERANCE);
    assert_close(statistics.within_tss, expected_within_tss, IID_TOLERANCE);
    assert_optional_close(
        statistics.r_squared,
        Some(1.0 - expected_rss / expected_tss),
    );
    assert_optional_close(
        statistics.adjusted_r_squared,
        Some(
            1.0 - (expected_rss / dof.residual_dof as f64)
                / (expected_tss / (effective_observations - 1) as f64),
        ),
    );
    assert_optional_close(
        statistics.within_r_squared,
        Some(1.0 - expected_rss / expected_within_tss),
    );
    assert_optional_close(
        statistics.adjusted_within_r_squared,
        Some(
            1.0 - (expected_rss / dof.residual_dof as f64)
                / (expected_within_tss / (dof.residual_dof + dof.finite_model_dof) as f64),
        ),
    );
    assert_close(
        statistics.root_mean_squared_error,
        expected_variance_scale.sqrt(),
        IID_TOLERANCE,
    );
    assert_optional_close(
        statistics.gaussian_log_likelihood,
        Some(gaussian_log_likelihood(
            expected_rss,
            effective_observations,
        )),
    );
    assert_optional_close(
        statistics.gaussian_null_log_likelihood,
        Some(gaussian_log_likelihood(
            expected_within_tss,
            effective_observations,
        )),
    );
    assert_optional_close(statistics.f_statistic, Some(expected_wald_f(&oracle)));

    let (dimension, expected_bread) =
        expected_reported_bread(case, &oracle, weights, include_intercept);
    let expected_covariance = expected_bread
        .iter()
        .map(|bread| expected_variance_scale * bread)
        .collect::<Vec<_>>();
    let covariance = result.covariance();
    assert_eq!(covariance.dimension(), dimension);
    assert_close(
        covariance.variance_scale(),
        expected_variance_scale,
        IID_TOLERANCE,
    );
    assert_slice_close(covariance.values(), &expected_covariance, IID_TOLERANCE);
    for (index, standard_error) in covariance.standard_errors().iter().copied().enumerate() {
        assert_close(
            standard_error,
            expected_covariance[index * dimension + index].sqrt(),
            IID_TOLERANCE,
        );
    }

    let finite_dimension = oracle.kept_regressors.len();
    for row in 0..finite_dimension {
        for column in 0..finite_dimension {
            assert_close(
                covariance.values()[row * dimension + column],
                oracle.iid_vce[(row, column)],
                IID_TOLERANCE,
            );
        }
    }
}

fn assert_weight_report(
    report: fereg_core::PreparedOlsReferenceWeightReport,
    weights: CaseWeights<'_>,
    nobs: usize,
) {
    match weights {
        CaseWeights::Unit => {
            assert_eq!(report.kind, None);
            assert_eq!(report.raw_weight_sum, None);
            assert_close(report.solver_weight_divisor, 1.0, IID_TOLERANCE);
            assert_close(report.reference_weight_multiplier, 1.0, IID_TOLERANCE);
        }
        CaseWeights::Analytic(values) => {
            let sum = values.iter().sum::<f64>();
            let maximum = values.iter().copied().fold(0.0_f64, f64::max);
            assert_eq!(report.kind, Some(WeightKind::Analytic));
            assert_optional_close(report.raw_weight_sum, Some(sum));
            assert_close(report.solver_weight_divisor, maximum, IID_TOLERANCE);
            assert_close(
                report.reference_weight_multiplier,
                nobs as f64 * maximum / sum,
                IID_TOLERANCE,
            );
        }
        CaseWeights::Frequency(values) => {
            let sum = values.iter().map(|value| f64::from(*value)).sum::<f64>();
            let maximum = values
                .iter()
                .copied()
                .map(f64::from)
                .fold(0.0_f64, f64::max);
            assert_eq!(report.kind, Some(WeightKind::Frequency));
            assert_optional_close(report.raw_weight_sum, Some(sum));
            assert_close(report.solver_weight_divisor, maximum, IID_TOLERANCE);
            assert_close(report.reference_weight_multiplier, maximum, IID_TOLERANCE);
        }
        CaseWeights::Probability(_) => panic!("probability iid must fail before weight reporting"),
    }
}

fn expected_wald_f(oracle: &ExplicitOlsResult) -> f64 {
    assert_eq!(oracle.kept_regressors.len(), 2);
    let first =
        oracle.coefficients[oracle.kept_regressors[0]].expect("first identified coefficient");
    let second =
        oracle.coefficients[oracle.kept_regressors[1]].expect("second identified coefficient");
    let v00 = oracle.iid_vce[(0, 0)];
    let v01 = oracle.iid_vce[(0, 1)];
    let v11 = oracle.iid_vce[(1, 1)];
    let determinant = v00 * v11 - v01 * v01;
    assert!(determinant > 0.0);
    let quadratic =
        (first * first * v11 - 2.0 * first * second * v01 + second * second * v00) / determinant;
    quadratic / 2.0
}

fn gaussian_log_likelihood(sum_of_squares: f64, observations: usize) -> f64 {
    let observations = observations as f64;
    -0.5 * observations * (TAU.ln() + 1.0 + (sum_of_squares / observations).ln())
}

fn expected_reported_bread(
    case: &InteractionCase,
    oracle: &ExplicitOlsResult,
    weights: CaseWeights<'_>,
    include_intercept: bool,
) -> (usize, Vec<f64>) {
    let finite_dimension = oracle.kept_regressors.len();
    let dimension = finite_dimension + usize::from(include_intercept);
    let reference_multiplier = match weights {
        CaseWeights::Analytic(raw) | CaseWeights::Probability(raw) => {
            raw.iter().sum::<f64>() / case.outcome.len() as f64
        }
        CaseWeights::Unit | CaseWeights::Frequency(_) => 1.0,
    };
    let mut expected = vec![0.0_f64; dimension * dimension];
    for row in 0..finite_dimension {
        for column in 0..finite_dimension {
            expected[row * dimension + column] = oracle.bread[(row, column)] * reference_multiplier;
        }
    }
    if !include_intercept {
        return (dimension, expected);
    }

    let reference_weights = iid_reference_weights(weights, case.outcome.len());
    let means = oracle
        .kept_regressors
        .iter()
        .map(|column| weighted_mean(&case.regressors[*column], &reference_weights))
        .collect::<Vec<_>>();
    let intercept_position = finite_dimension;
    let mut side = vec![0.0_f64; finite_dimension];
    for row in 0..finite_dimension {
        side[row] = -(0..finite_dimension)
            .map(|column| expected[row * dimension + column] * means[column])
            .sum::<f64>();
        expected[row * dimension + intercept_position] = side[row];
        expected[intercept_position * dimension + row] = side[row];
    }
    let reference_weight_sum = iid_effective_observations(weights, case.outcome.len()) as f64;
    expected[intercept_position * dimension + intercept_position] = reference_weight_sum.recip()
        - means
            .iter()
            .zip(&side)
            .map(|(mean, side)| mean * side)
            .sum::<f64>();
    (dimension, expected)
}

fn prepare_case(
    case: &InteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    reverse_absorb_sources: bool,
) -> PreparedCategoricalOlsData {
    let nobs = case.outcome.len();
    assert!(case.regressors.iter().all(|values| values.len() == nobs));
    assert_eq!(case.firm.len(), nobs);
    assert_eq!(case.year.len(), nobs);
    let regressor_count = case.regressors.len();
    let firm_source = source_id(regressor_count);
    let year_source = source_id(regressor_count + 1);
    let source_count = regressor_count + 2;
    let sources = (0..source_count)
        .map(|index| SourceEntry {
            id: source_id(index),
            canonical_name: match index {
                index if index < regressor_count => format!("x{}", index + 1),
                index if index == regressor_count => "firm".to_owned(),
                _ => "year".to_owned(),
            },
        })
        .collect::<Vec<_>>();
    let terms = (0..regressor_count)
        .map(|index| continuous_term(index, source_id(index)))
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
    bindings.extend((0..source_count).map(|index| OlsInputBinding {
        stata_var_position: u32::try_from(index + 3).expect("small binding"),
        role: OlsInputRole::NumericSource(source_id(index)),
    }));
    if let Some(role) = weight_role(weights) {
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(source_count + 3).expect("small weight binding"),
            role,
        });
    }
    let raw_weight = weight_values(weights);
    let loaded = LoadedOlsInput {
        candidate: vec![true; nobs],
        outcome: loaded_column(case.outcome.clone()),
        numeric_sources: case
            .regressors
            .iter()
            .cloned()
            .chain([
                case.firm.iter().map(|value| f64::from(*value)).collect(),
                case.year.iter().map(|value| f64::from(*value)).collect(),
            ])
            .enumerate()
            .map(|(index, values)| (source_id(index), loaded_column(values)))
            .collect(),
        analytic_weight: matches!(weights, CaseWeights::Analytic(_))
            .then(|| loaded_column(raw_weight.clone())),
        frequency_weight: matches!(weights, CaseWeights::Frequency(_))
            .then(|| loaded_column(raw_weight.clone())),
        probability_weight: matches!(weights, CaseWeights::Probability(_))
            .then(|| loaded_column(raw_weight)),
    };
    let categorical_sources = if reverse_absorb_sources {
        vec![year_source, firm_source]
    } else {
        vec![firm_source, year_source]
    };
    let request = OlsRequestV1 {
        model: CompiledTermGraph { sources, terms },
        absorbed_terms: vec![AbsorbedTerm {
            id: AbsorbTermId::new(1).expect("interaction effect id"),
            intercept: true,
            categorical_sources,
            slope_sources: Vec::new(),
            label: "firm#year".to_owned(),
            saved_name: String::new(),
        }],
        bindings,
        include_intercept,
        keep_singletons: false,
    };
    prepare_categorical_ols_data(&request, &loaded).expect("prepared categorical iid case")
}

fn continuous_term(index: usize, source: SourceId) -> ExpandedTerm {
    let one_based = u32::try_from(index + 1).expect("small term");
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
    let map = tuples
        .iter()
        .copied()
        .enumerate()
        .map(|(index, tuple)| {
            (
                tuple,
                u32::try_from(index).expect("small interaction level"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let codes = first
        .iter()
        .copied()
        .zip(second.iter().copied())
        .map(|tuple| map[&tuple])
        .collect::<Vec<_>>();
    (codes, tuples.len())
}

fn source_id(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source id")
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
        CaseWeights::Frequency(_) => Some(OlsInputRole::FrequencyWeight),
    }
}

fn weight_values(weights: CaseWeights<'_>) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => Vec::new(),
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => values.to_vec(),
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).collect(),
    }
}

fn oracle_weights(weights: CaseWeights<'_>) -> OracleWeights {
    match weights {
        CaseWeights::Unit => OracleWeights::Unit,
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => {
            OracleWeights::Wls(values.to_vec())
        }
        CaseWeights::Frequency(values) => OracleWeights::Frequency(values.to_vec()),
    }
}

fn iid_reference_weights(weights: CaseWeights<'_>, nobs: usize) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => vec![1.0; nobs],
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => {
            let multiplier = nobs as f64 / values.iter().sum::<f64>();
            values.iter().map(|value| value * multiplier).collect()
        }
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).collect(),
    }
}

fn iid_effective_observations(weights: CaseWeights<'_>, nobs: usize) -> usize {
    match weights {
        CaseWeights::Frequency(values) => values
            .iter()
            .map(|value| usize::try_from(*value).expect("frequency fits usize"))
            .sum(),
        CaseWeights::Unit | CaseWeights::Analytic(_) | CaseWeights::Probability(_) => nobs,
    }
}

fn weighted_mean(values: &[f64], weights: &[f64]) -> f64 {
    values
        .iter()
        .zip(weights)
        .map(|(value, weight)| value * weight)
        .sum::<f64>()
        / weights.iter().sum::<f64>()
}

fn weighted_sum_of_squares(values: &[f64], weights: &[f64], center: f64) -> f64 {
    values
        .iter()
        .zip(weights)
        .map(|(value, weight)| weight * (value - center) * (value - center))
        .sum()
}

fn assert_optional_close(actual: Option<f64>, expected: Option<f64>) {
    match (actual, expected) {
        (Some(actual), Some(expected)) => assert_close(actual, expected, IID_TOLERANCE),
        (None, None) => {}
        values => panic!("optional statistic differs: {values:?}"),
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

struct InteractionCase {
    outcome: Vec<f64>,
    regressors: Vec<Vec<f64>>,
    firm: Vec<u32>,
    year: Vec<u32>,
}

fn balanced_case() -> InteractionCase {
    let mut firm = Vec::new();
    let mut year = Vec::new();
    for firm_value in 0_u32..4 {
        for year_value in 0_u32..3 {
            firm.push(firm_value);
            year.push(year_value);
            firm.push(firm_value);
            year.push(year_value);
        }
    }
    let x1 = (0..firm.len())
        .map(|row| {
            let within = if row % 2 == 0 { -0.8 } else { 1.3 };
            within + 0.17 * row as f64 + 0.11 * f64::from(year[row])
        })
        .collect::<Vec<_>>();
    let x2 = (0..firm.len())
        .map(|row| {
            let within = if row % 2 == 0 {
                1.1 + 0.07 * f64::from(firm[row])
            } else {
                -0.6 + 0.13 * f64::from(year[row])
            };
            within - 0.09 * row as f64 + 0.23 * f64::from(firm[row])
        })
        .collect::<Vec<_>>();
    let outcome = (0..firm.len())
        .map(|row| {
            let cell = 0.7 * f64::from(firm[row]) - 0.45 * f64::from(year[row])
                + 0.18 * f64::from(firm[row] * year[row]);
            1.65 * x1[row] - 0.72 * x2[row] + cell + 0.04 * (row as f64 * 0.7).sin()
        })
        .collect::<Vec<_>>();
    InteractionCase {
        outcome,
        regressors: vec![x1, x2],
        firm,
        year,
    }
}
