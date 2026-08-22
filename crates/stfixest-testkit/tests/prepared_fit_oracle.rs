use stfixest_core::{
    fit_prepared_ols_coefficients_map, prepare_ols_data, AbsorbTermId, AbsorbedTerm,
    CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ExpandedTerm, LevelState,
    LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding, OlsInputRole, OlsRequestV1,
    PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatus, RawTermKind, SourceEntry,
    SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use stfixest_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const COEFFICIENT_TOLERANCE: f64 = 2e-8;
const RESIDUAL_TOLERANCE: f64 = 2e-8;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

#[test]
fn prepared_coefficients_and_residuals_match_explicit_dummies_for_all_weights() {
    let first_codes = vec![0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
    let second_codes = vec![0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1];
    let regressors = vec![
        vec![
            -2.0, -0.5, 1.0, 3.0, -1.0, 0.25, 2.0, 4.0, -3.0, 0.75, 2.5, 5.0,
        ],
        vec![
            1.0, -2.0, 0.5, 4.0, 2.0, -1.0, 3.5, 0.0, -0.5, 2.5, 1.5, 5.0,
        ],
    ];
    let outcome = (0..first_codes.len())
        .map(|row| {
            1.7 * regressors[0][row] - 0.6 * regressors[1][row]
                + [0.5, -1.0, 2.0][first_codes[row] as usize]
                + [0.2, -0.4][second_codes[row] as usize]
                + (row as f64 * 0.13).sin() * 0.2
        })
        .collect::<Vec<_>>();
    let weights = [0.5, 2.0, 1.25, 3.0, 0.75, 4.0, 1.5, 2.5, 5.0, 0.8, 3.5, 1.1];
    let frequencies = [1_u32, 2, 3, 1, 4, 2, 1, 3, 2, 5, 1, 2];

    for case in [
        CaseWeights::Unit,
        CaseWeights::Analytic(&weights),
        CaseWeights::Probability(&weights),
        CaseWeights::Frequency(&frequencies),
    ] {
        run_oracle_case(
            &outcome,
            &regressors,
            &[first_codes.clone(), second_codes.clone()],
            &[0, 1, 0],
            true,
            case,
        );
    }
}

#[test]
fn randomized_well_conditioned_prepared_fits_match_independent_oracle() {
    let mut generator = Generator(0x8a13_72ce_4d9b_f061);
    for replication in 0..20 {
        let n = 30;
        let first_codes = (0..n).map(|row| (row % 3) as u32).collect::<Vec<_>>();
        let second_codes = (0..n)
            .map(|row| ((row * 5 + row / 3) % 4) as u32)
            .collect::<Vec<_>>();
        let first_x = (0..n).map(|_| generator.f64(-3.0, 3.0)).collect::<Vec<_>>();
        let second_x = (0..n)
            .map(|row| 0.2 * first_x[row] + generator.f64(-2.0, 2.0))
            .collect::<Vec<_>>();
        let weights = (0..n).map(|_| generator.f64(0.2, 5.0)).collect::<Vec<_>>();
        let outcome = (0..n)
            .map(|row| {
                2.3 * first_x[row] - 0.8 * second_x[row]
                    + [0.5, -1.0, 1.7][first_codes[row] as usize]
                    + [0.1, -0.4, 0.9, 1.2][second_codes[row] as usize]
                    + generator.f64(-0.15, 0.15)
            })
            .collect::<Vec<_>>();
        let regressors = vec![first_x, second_x];
        let effects = vec![first_codes, second_codes];
        run_oracle_case(
            &outcome,
            &regressors,
            &effects,
            &[0, 1],
            replication % 2 == 0,
            CaseWeights::Analytic(&weights),
        );
    }
}

#[test]
fn observation_permutation_preserves_identified_fit() {
    let first_codes = vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let second_codes = vec![0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];
    let x = vec![
        -2.0, 0.5, 3.0, -1.0, 1.5, 4.0, -3.0, 2.0, 5.0, -0.5, 2.5, 6.0,
    ];
    let weights = vec![0.5, 2.0, 3.0, 1.0, 4.0, 1.5, 2.5, 0.75, 5.0, 1.2, 3.3, 0.9];
    let outcome = (0..x.len())
        .map(|row| {
            1.9 * x[row]
                + [0.5, -1.0, 2.0, 0.2][first_codes[row] as usize]
                + [0.3, -0.6, 0.8][second_codes[row] as usize]
                + 0.1 * (row as f64).cos()
        })
        .collect::<Vec<_>>();
    let effects = vec![first_codes, second_codes];
    let baseline = solve_case(
        &outcome,
        std::slice::from_ref(&x),
        &effects,
        &[0, 1],
        false,
        CaseWeights::Analytic(&weights),
    );
    let permutation = [11_usize, 2, 7, 0, 9, 4, 6, 1, 10, 5, 8, 3];
    let permuted_outcome = permute(&outcome, &permutation);
    let permuted_x = permute(&x, &permutation);
    let permuted_effects = effects
        .iter()
        .map(|codes| permute(codes, &permutation))
        .collect::<Vec<_>>();
    let permuted_weights = permute(&weights, &permutation);
    let permuted = solve_case(
        &permuted_outcome,
        &[permuted_x],
        &permuted_effects,
        &[0, 1],
        false,
        CaseWeights::Analytic(&permuted_weights),
    );
    assert_close(baseline.0, permuted.0, COEFFICIENT_TOLERANCE);
    let mut restored = vec![0.0; baseline.1.len()];
    for (new_row, old_row) in permutation.iter().copied().enumerate() {
        restored[old_row] = permuted.1[new_row];
    }
    assert_slice_close(&baseline.1, &restored, RESIDUAL_TOLERANCE);
}

fn run_oracle_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effect_sources: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) {
    let prepared = prepare_case(
        outcome,
        regressors,
        effect_sources,
        effect_order,
        include_intercept,
        weights,
    );
    let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
        .expect("certified prepared fit");
    assert!(fit.absorption_report().certified());
    assert!(fit.residual_absorption_report().certified());
    assert!(fit.rank_certificate().passed);
    assert!(fit.score_certificate().passed);

    let mut oracle_regressors = regressors.to_vec();
    if include_intercept {
        oracle_regressors.push(vec![1.0; outcome.len()]);
    }
    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: outcome.to_vec(),
        regressors: oracle_regressors,
        effects: effect_order
            .iter()
            .map(|source| {
                let codes = effect_sources[*source].clone();
                DummyEffect::new(codes.clone(), level_count(&codes), true, Vec::new())
                    .expect("dummy effect")
            })
            .collect(),
        weights: oracle_weights(weights),
        clusters: Vec::new(),
    })
    .expect("explicit-dummy oracle");

    assert_eq!(fit.coefficients().len(), oracle.coefficients.len());
    for (index, (actual, expected)) in fit
        .coefficients()
        .iter()
        .zip(&oracle.coefficients)
        .enumerate()
    {
        match expected {
            Some(expected) => {
                assert_eq!(actual.status, PreparedOlsFitStatus::Kept, "slot {index}");
                assert_close(
                    actual.value.expect("identified coefficient"),
                    *expected,
                    COEFFICIENT_TOLERANCE,
                );
            }
            None if include_intercept && index + 1 == oracle.coefficients.len() => {
                assert_eq!(
                    actual.status,
                    PreparedOlsFitStatus::AbsorbedInterceptDeferred
                );
                assert!(actual.value.is_none());
            }
            None => assert_ne!(actual.status, PreparedOlsFitStatus::Kept),
        }
    }
    assert_eq!(fit.finite_rank(), oracle.finite_regressor_rank);
    assert_slice_close(fit.residuals(), &oracle.residuals, RESIDUAL_TOLERANCE);
}

fn solve_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effect_sources: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> (f64, Vec<f64>) {
    let prepared = prepare_case(
        outcome,
        regressors,
        effect_sources,
        effect_order,
        include_intercept,
        weights,
    );
    let fit = fit_prepared_ols_coefficients_map(&prepared, PreparedOlsFitConfig::default())
        .expect("certified prepared fit");
    (
        fit.coefficients()[0].value.expect("identified slope"),
        fit.residuals().to_vec(),
    )
}

fn prepare_case(
    outcome: &[f64],
    regressors: &[Vec<f64>],
    effect_sources: &[Vec<u32>],
    effect_order: &[usize],
    include_intercept: bool,
    weights: CaseWeights<'_>,
) -> PreparedOlsData {
    let n = outcome.len();
    assert!(regressors.iter().all(|values| values.len() == n));
    assert!(effect_sources.iter().all(|values| values.len() == n));
    let source_count = regressors.len() + effect_sources.len();
    let sources = (0..source_count)
        .map(|index| SourceEntry {
            id: source_id(index),
            canonical_name: format!("s{}", index + 1),
        })
        .collect::<Vec<_>>();
    let terms = (0..regressors.len())
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
        candidate: vec![true; n],
        outcome: loaded_column(outcome.to_vec()),
        numeric_sources: regressors
            .iter()
            .cloned()
            .chain(effect_sources.iter().map(|codes| {
                codes
                    .iter()
                    .map(|code| f64::from(*code))
                    .collect::<Vec<_>>()
            }))
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
    let request = OlsRequestV1 {
        model: CompiledTermGraph { sources, terms },
        absorbed_terms: effect_order
            .iter()
            .enumerate()
            .map(|(index, source)| AbsorbedTerm {
                id: AbsorbTermId::new(u32::try_from(index + 1).expect("small effect"))
                    .expect("effect id"),
                intercept: true,
                categorical_sources: vec![source_id(regressors.len() + *source)],
                slope_sources: Vec::new(),
                label: format!("g{}_effect_{}", source + 1, index + 1),
                saved_name: String::new(),
            })
            .collect(),
        bindings,
        include_intercept,
        keep_singletons: true,
    };
    prepare_ols_data(&request, &loaded).expect("prepared oracle case")
}

fn continuous_term(index: usize, source: SourceId) -> ExpandedTerm {
    let one_based = u32::try_from(index + 1).expect("small term");
    ExpandedTerm {
        id: TermId::new(one_based).expect("term id"),
        order: TermOrder::new(one_based).expect("term order"),
        coefficient_slot: CoefficientSlot::new(one_based).expect("slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.x{one_based}"),
        components: vec![TermComponent {
            order: ComponentOrder::new(1).expect("component"),
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

fn source_id(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source id")
}

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

fn level_count(codes: &[u32]) -> usize {
    codes
        .iter()
        .copied()
        .max()
        .map_or(0, |value| value as usize + 1)
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

struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn f64(&mut self, lower: f64, upper: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / (1_u64 << 53) as f64;
        (upper - lower).mul_add(unit, lower)
    }
}
