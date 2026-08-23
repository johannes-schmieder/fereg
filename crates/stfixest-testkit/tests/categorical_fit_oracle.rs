use std::collections::{BTreeMap, BTreeSet};

use stfixest_core::{
    fit_prepared_categorical_ols_coefficients_map, prepare_categorical_ols_data, AbsorbTermId,
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm,
    LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding, OlsInputRole,
    OlsRequestV1, PreparedCategoricalOlsData, PreparedOlsBreadCoordinateKind, PreparedOlsFitConfig,
    PreparedOlsFitStatus, RawTermKind, SourceEntry, SourceId, TermComponent, TermId, TermOrder,
    TimeSeriesState,
};
use stfixest_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const COEFFICIENT_TOLERANCE: f64 = 3e-8;
const BREAD_TOLERANCE: f64 = 6e-8;
const RESIDUAL_TOLERANCE: f64 = 3e-8;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn firm_year_coefficients_bread_and_residuals_match_explicit_dummies() {
    let case = balanced_case();
    let analytic = case
        .x1
        .iter()
        .enumerate()
        .map(|(row, value)| 0.35 + value.abs() * 0.2 + (row % 5) as f64 * 0.31)
        .collect::<Vec<_>>();
    let frequencies = (0..case.outcome.len())
        .map(|row| u32::try_from(1 + (row * 7) % 5).expect("bounded frequency"))
        .collect::<Vec<_>>();

    for include_intercept in [false, true] {
        for weights in [
            CaseWeights::Unit,
            CaseWeights::Analytic(&analytic),
            CaseWeights::Probability(&analytic),
            CaseWeights::Frequency(&frequencies),
        ] {
            assert_case_matches_oracle(&case, include_intercept, weights, false);
        }
    }
}

#[test]
fn reordered_absorb_sources_and_row_permutation_preserve_the_fit() {
    let case = balanced_case();
    let weights = case
        .x2
        .iter()
        .enumerate()
        .map(|(row, value)| 0.4 + value.abs() * 0.15 + (row % 4) as f64 * 0.27)
        .collect::<Vec<_>>();
    let baseline = fit_case(&case, true, CaseWeights::Analytic(&weights), false);
    let reversed = fit_case(&case, true, CaseWeights::Analytic(&weights), true);
    assert_fit_equivalent(&baseline, &reversed, None);

    let permutation = [
        23_usize, 2, 17, 0, 11, 8, 19, 5, 14, 21, 3, 10, 6, 18, 1, 22, 9, 15, 4, 20, 7, 12, 16, 13,
    ];
    let permuted_case = case.permute(&permutation);
    let permuted_weights = permute(&weights, &permutation);
    let permuted = fit_case(
        &permuted_case,
        true,
        CaseWeights::Analytic(&permuted_weights),
        true,
    );
    assert_fit_equivalent(&baseline, &permuted, Some(&permutation));
}

#[test]
fn cell_constant_regressor_is_rejected_as_absorbed_collinear() {
    let mut case = balanced_case();
    case.regressors.push(
        case.firm
            .iter()
            .zip(&case.year)
            .map(|(firm, year)| 3.0 * f64::from(*firm) - 0.75 * f64::from(*year))
            .collect(),
    );
    let prepared = prepare_case(&case, false, CaseWeights::Unit, false);
    let fit =
        fit_prepared_categorical_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("categorical fit with an absorbed-collinear regressor");
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);
    assert_eq!(fit.finite_rank(), 2);
    assert_eq!(fit.coefficients().len(), 3);
    assert!(matches!(
        fit.coefficients()[2].status,
        PreparedOlsFitStatus::CollinearWithFixedEffects
            | PreparedOlsFitStatus::NumericallyRankDeficient
    ));
    assert!(fit.coefficients()[2].value.is_none());

    let (interaction, levels) = independent_interaction_codes(&case.firm, &case.year);
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: case.outcome.clone(),
        regressors: case.regressors.clone(),
        effects: vec![
            DummyEffect::new(interaction, levels, true, Vec::new()).expect("interaction dummy")
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("explicit-dummy collinearity oracle");
    assert_eq!(oracle.finite_regressor_rank, 2);
    assert!(oracle.coefficients[2].is_none());
    assert_slice_close(fit.residuals(), &oracle.residuals, RESIDUAL_TOLERANCE);
}

fn assert_case_matches_oracle(
    case: &InteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    reverse_absorb_sources: bool,
) {
    let prepared = prepare_case(case, include_intercept, weights, reverse_absorb_sources);
    let fit =
        fit_prepared_categorical_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
            .expect("certified categorical fit");
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);
    assert!(fit.bread().certificate().passed);

    let (interaction, n_levels) = independent_interaction_codes(&case.firm, &case.year);
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: case.outcome.clone(),
        regressors: case.regressors.clone(),
        effects: vec![DummyEffect::new(interaction, n_levels, true, Vec::new())
            .expect("explicit interaction effect")],
        weights: oracle_weights(weights),
        clusters: Vec::new(),
    })
    .expect("explicit-dummy interaction oracle");

    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_eq!(
        oracle.kept_regressors,
        (0..case.regressors.len()).collect::<Vec<_>>()
    );
    assert_eq!(
        fit.coefficients().len(),
        case.regressors.len() + usize::from(include_intercept)
    );
    for (index, expected) in oracle.coefficients.iter().enumerate() {
        let expected = expected.expect("well-conditioned interaction coefficient");
        let actual = &fit.coefficients()[index];
        assert_eq!(actual.status, PreparedOlsFitStatus::Kept, "slot {index}");
        assert_close(
            actual.value.expect("identified interaction coefficient"),
            expected,
            COEFFICIENT_TOLERANCE,
        );
    }
    assert_slice_close(fit.residuals(), &oracle.residuals, RESIDUAL_TOLERANCE);

    let expected_bread = expected_reported_bread(case, &oracle, weights, include_intercept);
    assert_eq!(fit.bread().dimension(), expected_bread.0);
    assert_slice_close(fit.bread().values(), &expected_bread.1, BREAD_TOLERANCE);
    assert!(fit.bread().coordinates()[..oracle.kept_regressors.len()]
        .iter()
        .all(|coordinate| coordinate.kind == PreparedOlsBreadCoordinateKind::FiniteRegressor));

    if include_intercept {
        let normalized = normalized_weights(weights, case.outcome.len());
        let finite_residual = (0..case.outcome.len())
            .map(|row| {
                oracle.coefficients.iter().enumerate().fold(
                    case.outcome[row],
                    |value, (column, coefficient)| {
                        coefficient.map_or(value, |coefficient| {
                            (-coefficient).mul_add(case.regressors[column][row], value)
                        })
                    },
                )
            })
            .collect::<Vec<_>>();
        let expected_intercept = weighted_mean(&finite_residual, &normalized);
        let actual = fit.coefficients().last().expect("reported intercept");
        assert_eq!(
            actual.status,
            PreparedOlsFitStatus::NormalizedAbsorbedIntercept
        );
        assert_close(
            actual.value.expect("normalized intercept"),
            expected_intercept,
            COEFFICIENT_TOLERANCE,
        );
        assert_eq!(
            fit.bread()
                .coordinates()
                .last()
                .expect("intercept coordinate")
                .kind,
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
        );
    }
}

fn expected_reported_bread(
    case: &InteractionCase,
    oracle: &stfixest_testkit::explicit_ols::ExplicitOlsResult,
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

    let normalized = normalized_weights(weights, case.outcome.len());
    let means = oracle
        .kept_regressors
        .iter()
        .map(|column| weighted_mean(&case.regressors[*column], &normalized))
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
    let reference_weight_sum = match weights {
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).sum(),
        CaseWeights::Unit | CaseWeights::Analytic(_) | CaseWeights::Probability(_) => {
            case.outcome.len() as f64
        }
    };
    expected[intercept_position * dimension + intercept_position] = reference_weight_sum.recip()
        - means
            .iter()
            .zip(&side)
            .map(|(mean, side)| mean * side)
            .sum::<f64>();
    (dimension, expected)
}

fn fit_case(
    case: &InteractionCase,
    include_intercept: bool,
    weights: CaseWeights<'_>,
    reverse_absorb_sources: bool,
) -> stfixest_core::CertifiedPreparedOlsFit {
    let prepared = prepare_case(case, include_intercept, weights, reverse_absorb_sources);
    fit_prepared_categorical_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
        .expect("categorical fit")
}

fn assert_fit_equivalent(
    baseline: &stfixest_core::CertifiedPreparedOlsFit,
    comparison: &stfixest_core::CertifiedPreparedOlsFit,
    permutation: Option<&[usize]>,
) {
    assert_eq!(baseline.finite_rank(), comparison.finite_rank());
    assert_eq!(
        baseline.coefficients().len(),
        comparison.coefficients().len()
    );
    for (left, right) in baseline
        .coefficients()
        .iter()
        .zip(comparison.coefficients())
    {
        assert_eq!(left.status, right.status);
        match (left.value, right.value) {
            (Some(left), Some(right)) => assert_close(left, right, COEFFICIENT_TOLERANCE),
            (None, None) => {}
            values => panic!("coefficient availability differs: {values:?}"),
        }
    }
    assert_slice_close(
        baseline.bread().values(),
        comparison.bread().values(),
        BREAD_TOLERANCE,
    );
    let comparison_residuals = if let Some(permutation) = permutation {
        let mut restored = vec![0.0_f64; comparison.residuals().len()];
        for (new_row, old_row) in permutation.iter().copied().enumerate() {
            restored[old_row] = comparison.residuals()[new_row];
        }
        restored
    } else {
        comparison.residuals().to_vec()
    };
    assert_slice_close(
        baseline.residuals(),
        &comparison_residuals,
        RESIDUAL_TOLERANCE,
    );
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
    prepare_categorical_ols_data(&request, &loaded).expect("prepared categorical oracle case")
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

fn normalized_weights(weights: CaseWeights<'_>, nobs: usize) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => vec![1.0; nobs],
        CaseWeights::Analytic(values) | CaseWeights::Probability(values) => values.to_vec(),
        CaseWeights::Frequency(values) => values.iter().map(|value| f64::from(*value)).collect(),
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

fn permute<T: Copy>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|row| values[*row]).collect()
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

#[derive(Clone)]
struct InteractionCase {
    outcome: Vec<f64>,
    regressors: Vec<Vec<f64>>,
    x1: Vec<f64>,
    x2: Vec<f64>,
    firm: Vec<u32>,
    year: Vec<u32>,
}

impl InteractionCase {
    fn permute(&self, permutation: &[usize]) -> Self {
        Self {
            outcome: permute(&self.outcome, permutation),
            regressors: self
                .regressors
                .iter()
                .map(|values| permute(values, permutation))
                .collect(),
            x1: permute(&self.x1, permutation),
            x2: permute(&self.x2, permutation),
            firm: permute(&self.firm, permutation),
            year: permute(&self.year, permutation),
        }
    }
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
        regressors: vec![x1.clone(), x2.clone()],
        x1,
        x2,
        firm,
        year,
    }
}
