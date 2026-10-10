use std::collections::{BTreeMap, BTreeSet};

use fereg_core::{
    fit_prepared_model_robust_with_loaded_moments, prepare_model, AbsorbTermId, AbsorbedTerm,
    CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm, LevelState,
    LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsData, PreparedOlsRobustConfig, RawTermKind, SourceEntry, SourceId, TermComponent,
    TermId, TermOrder, TimeSeriesState, WeightKind,
};
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, ExplicitOlsResult, OracleWeights,
};

const ROBUST_TOLERANCE: f64 = 8e-7;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn firm_year_hc1_matches_explicit_dummy_oracle_for_all_weight_rules() {
    let case = balanced_case();
    let analytic = case.regressors[0]
        .iter()
        .enumerate()
        .map(|(row, value)| 0.35 + value.abs() * 0.2 + (row % 5) as f64 * 0.31)
        .collect::<Vec<_>>();
    let probability = case.regressors[1]
        .iter()
        .enumerate()
        .map(|(row, value)| 0.4 + value.abs() * 0.15 + (row % 4) as f64 * 0.27)
        .collect::<Vec<_>>();
    let frequencies = (0..case.outcome.len())
        .map(|row| u32::try_from(1 + (row * 7) % 5).expect("bounded frequency"))
        .collect::<Vec<_>>();

    for include_intercept in [false, true] {
        for reverse_absorb_sources in [false, true] {
            for weights in [
                CaseWeights::Unit,
                CaseWeights::Analytic(&analytic),
                CaseWeights::Probability(&probability),
                CaseWeights::Frequency(&frequencies),
            ] {
                assert_robust_case_matches_oracle(
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
fn row_permuted_firm_year_hc1_matches_explicit_dummy_oracle() {
    let case = balanced_case();
    let permutation = (0..case.outcome.len())
        .map(|row| (row * 17 + 5) % case.outcome.len())
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    assert!(permutation.iter().all(|row| seen.insert(*row)));
    let permuted = InteractionCase {
        outcome: permute(&case.outcome, &permutation),
        regressors: case
            .regressors
            .iter()
            .map(|values| permute(values, &permutation))
            .collect(),
        firm: permute(&case.firm, &permutation),
        year: permute(&case.year, &permutation),
    };
    let probability = permuted.regressors[0]
        .iter()
        .enumerate()
        .map(|(row, value)| 0.6 + value.abs() * 0.17 + (row % 6) as f64 * 0.19)
        .collect::<Vec<_>>();
    assert_robust_case_matches_oracle(
        &permuted,
        true,
        CaseWeights::Probability(&probability),
        true,
    );
}

fn assert_robust_case_matches_oracle(
    case: &InteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    reverse_absorb_sources: bool,
) {
    let prepared = prepare_case(case, include_intercept, weights, reverse_absorb_sources);
    let result = fit_prepared_model_robust_with_loaded_moments(
        &prepared,
        PreparedOlsRobustConfig::default(),
        None,
    )
    .expect("certified categorical HC1 inference");
    assert!(result.fit().absorption_report().certified());
    assert!(result.fit().residual_absorption_report().certified());
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.fit().bread().certificate().passed);
    assert!(result.certificate().passed);

    let (interaction, n_levels) = independent_interaction_codes(&case.firm, &case.year);
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: case.outcome.clone(),
        regressors: case.regressors.clone(),
        effects: vec![DummyEffect::new(interaction, n_levels, true, Vec::new())
            .expect("explicit interaction effect")],
        weights: oracle_weights(weights),
        clusters: Vec::new(),
    })
    .expect("explicit-dummy HC1 oracle");

    let weight_report = result.weights();
    let effective_observations = effective_observations(weights, case.outcome.len());
    assert_eq!(weight_report.physical_observations, case.outcome.len());
    assert_eq!(weight_report.effective_observations, effective_observations);
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

    let expected = if include_intercept {
        oracle
            .normalized_reported_intercept_hc1_vce
            .as_ref()
            .expect("normalized reported intercept HC1 oracle")
    } else {
        &oracle.hc1_vce
    };
    let covariance = result.covariance();
    assert_eq!(covariance.dimension(), expected.nrows());
    assert_eq!(expected.nrows(), expected.ncols());
    assert_slice_close(covariance.values(), expected.as_slice(), ROBUST_TOLERANCE);
    assert_close(
        covariance.hc1_correction(),
        effective_observations as f64 / dof.residual_dof as f64,
        ROBUST_TOLERANCE,
    );
    for row in 0..covariance.dimension() {
        for column in 0..covariance.dimension() {
            assert_close(
                covariance.values()[row * covariance.dimension() + column],
                covariance.values()[column * covariance.dimension() + row],
                ROBUST_TOLERANCE,
            );
        }
        let diagonal = covariance.values()[row * covariance.dimension() + row];
        assert!(diagonal >= -ROBUST_TOLERANCE);
        assert_close(
            covariance.standard_errors()[row],
            diagonal.max(0.0).sqrt(),
            ROBUST_TOLERANCE,
        );
    }

    assert_optional_close(
        result.statistics().f_statistic,
        Some(expected_robust_wald_f(&oracle)),
    );
    let certificate = result.certificate();
    assert!(certificate.reference_weight_sum_relative_error <= 5e-9);
    assert!(certificate.score_design_left_inverse_relative_infinity_norm <= 5e-9);
    assert!(certificate.score_design_right_inverse_relative_infinity_norm <= 5e-9);
    assert!(certificate.intercept_coordinate_relative_infinity_norm <= 5e-9);
    assert!(certificate.hc1_correction_relative_error <= 5e-9);
    assert!(certificate.sandwich_formula_relative_infinity_norm <= 5e-9);
    assert!(certificate.covariance_symmetry_relative_infinity_norm <= 5e-9);
    assert!(certificate.standard_error_relative_infinity_norm <= 5e-9);
    assert!(certificate.wald_solve_relative_infinity_norm <= 5e-9);
    assert!(certificate.statistic_formula_relative_infinity_norm <= 5e-9);
}

fn expected_robust_wald_f(oracle: &ExplicitOlsResult) -> f64 {
    assert_eq!(oracle.kept_regressors.len(), 2);
    let first =
        oracle.coefficients[oracle.kept_regressors[0]].expect("first identified coefficient");
    let second =
        oracle.coefficients[oracle.kept_regressors[1]].expect("second identified coefficient");
    let v00 = oracle.hc1_vce[(0, 0)];
    let v01 = oracle.hc1_vce[(0, 1)];
    let v11 = oracle.hc1_vce[(1, 1)];
    let determinant = v00 * v11 - v01 * v01;
    assert!(determinant > 0.0);
    let quadratic =
        (first * first * v11 - 2.0 * first * second * v01 + second * second * v00) / determinant;
    quadratic / 2.0
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
            assert_close(report.solver_weight_divisor, 1.0, ROBUST_TOLERANCE);
            assert_close(report.reference_weight_multiplier, 1.0, ROBUST_TOLERANCE);
        }
        CaseWeights::Analytic(values) => {
            assert_weight_report_for_continuous(report, values, nobs, WeightKind::Analytic);
        }
        CaseWeights::Probability(values) => {
            assert_weight_report_for_continuous(report, values, nobs, WeightKind::Probability);
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
            assert_close(report.solver_weight_divisor, maximum, ROBUST_TOLERANCE);
            assert_close(
                report.reference_weight_multiplier,
                maximum,
                ROBUST_TOLERANCE,
            );
        }
    }
}

fn assert_weight_report_for_continuous(
    report: fereg_core::PreparedOlsReferenceWeightReport,
    values: &[f64],
    nobs: usize,
    kind: WeightKind,
) {
    let sum = values.iter().sum::<f64>();
    let maximum = values.iter().copied().fold(0.0_f64, f64::max);
    assert_eq!(report.kind, Some(kind));
    assert_optional_close(report.raw_weight_sum, Some(sum));
    assert_close(report.solver_weight_divisor, maximum, ROBUST_TOLERANCE);
    assert_close(
        report.reference_weight_multiplier,
        nobs as f64 * maximum / sum,
        ROBUST_TOLERANCE,
    );
}

fn prepare_case(
    case: &InteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    reverse_absorb_sources: bool,
) -> PreparedOlsData {
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
    prepare_model(&request, &[], &loaded, true).expect("prepared categorical robust case")
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

fn effective_observations(weights: CaseWeights<'_>, nobs: usize) -> usize {
    match weights {
        CaseWeights::Frequency(values) => values
            .iter()
            .map(|value| usize::try_from(*value).expect("frequency fits usize"))
            .sum(),
        CaseWeights::Unit | CaseWeights::Analytic(_) | CaseWeights::Probability(_) => nobs,
    }
}

fn assert_optional_close(actual: Option<f64>, expected: Option<f64>) {
    match (actual, expected) {
        (Some(actual), Some(expected)) => assert_close(actual, expected, ROBUST_TOLERANCE),
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

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|row| values[*row]).collect()
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
