use fereg_core::{
    absorb_borrowed_intercept_map_certified, absorb_prepared_ols_rhs_map, build_prepared_ols_rhs,
    prepare_ols_data, AbsorbTermId, AbsorbedTerm, BorrowedMapWeights, CoefficientSlot,
    CompiledTermGraph, ComponentKind, ComponentOrder, DenseMatrix, EffectTerm, ExpandedTerm,
    FeDesign, LevelState, LoadedNumericColumn, LoadedOlsInput, MapConfig, MetadataFlag,
    OlsInputBinding, OlsInputRole, OlsRequestV1, PreparedOlsData, PreparedWeights, RawTermKind,
    SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use fereg_testkit::explicit_dummy::fe_projection_certificate_with_reference;
use fereg_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const TOLERANCE: f64 = 2e-8;

#[derive(Clone, Copy)]
enum CaseWeights<'a> {
    Unit,
    Analytic(&'a [f64]),
    Probability(&'a [f64]),
    Frequency(&'a [u32]),
}

fn map_config() -> MapConfig {
    MapConfig {
        tolerance: 1e-10,
        max_iterations: 20_000,
        check_every: 1,
        symmetric: true,
    }
}

#[test]
fn prepared_borrowed_map_matches_explicit_dummies_for_multiple_rhs_and_weights() {
    let codes = vec![
        vec![0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2],
        vec![0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
        vec![0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
    ];
    let effects = [0, 1, 2, 0];
    let columns = vec![
        vec![
            1.0, 4.0, -2.0, 8.0, 0.5, 3.0, 9.0, -1.0, 6.0, 2.0, 7.0, -4.0,
        ],
        vec![
            2.0, -1.0, 5.0, 0.0, 3.5, -2.5, 8.0, 1.0, -3.0, 4.0, 6.0, 9.0,
        ],
        vec![
            0.0, 1.0, 4.0, 9.0, 16.0, 25.0, 36.0, 49.0, 64.0, 81.0, 100.0, 121.0,
        ],
    ];
    let weights = [0.5, 2.0, 1.25, 3.0, 0.75, 4.0, 1.5, 2.5, 5.0, 0.8, 3.5, 1.1];
    let frequencies = [1_u32, 2, 3, 1, 4, 2, 1, 3, 2, 5, 1, 2];

    for case in [
        CaseWeights::Unit,
        CaseWeights::Analytic(&weights),
        CaseWeights::Probability(&weights),
        CaseWeights::Frequency(&frequencies),
    ] {
        run_case(&codes, &effects, &columns, case);
    }
}

#[test]
fn prepared_yx_builder_and_integrated_map_match_explicit_dummies_for_all_weight_kinds() {
    let codes = vec![
        vec![0, 0, 0, 1, 1, 1, 2, 2, 2],
        vec![0, 1, 2, 0, 1, 2, 0, 1, 2],
    ];
    let effects = [0, 1, 0];
    let outcome = vec![1.0, 7.0, -2.0, 5.0, 3.0, 8.0, 4.0, -1.0, 6.0];
    let weights = [0.5, 2.0, 3.0, 1.0, 4.0, 1.5, 2.5, 0.75, 5.0];
    let frequencies = [1_u32, 2, 3, 1, 4, 2, 1, 3, 2];

    for case in [
        CaseWeights::Unit,
        CaseWeights::Analytic(&weights),
        CaseWeights::Probability(&weights),
        CaseWeights::Frequency(&frequencies),
    ] {
        let prepared = prepare_case(&codes, &effects, outcome.clone(), case);
        let raw = build_prepared_ols_rhs(&prepared).expect("prepared Y/X");
        assert_eq!((raw.values().nrows(), raw.values().ncols()), (9, 3));
        assert_close(&raw.values().column(0).expect("outcome RHS"), &outcome, 0.0);
        for (rhs, expected) in codes.iter().enumerate() {
            let expected = expected
                .iter()
                .map(|code| f64::from(*code))
                .collect::<Vec<_>>();
            assert_close(
                &raw.values().column(rhs + 1).expect("model RHS"),
                &expected,
                0.0,
            );
        }

        let result = absorb_prepared_ols_rhs_map(&prepared, map_config()).expect("integrated MAP");
        assert!(result.report().certified());
        let dummy_effects = dummy_effects(&codes, &effects);
        for rhs in 0..raw.values().ncols() {
            let raw_column = raw.values().column(rhs).expect("raw column");
            let oracle = fit_explicit_ols(&ExplicitOlsProblem {
                outcome: raw_column,
                regressors: Vec::new(),
                effects: dummy_effects.clone(),
                weights: oracle_weights(case),
                clusters: Vec::new(),
            })
            .expect("explicit dummy projection");
            assert_close(
                &result.residualized().column(rhs).expect("residualized RHS"),
                &oracle.residuals,
                TOLERANCE,
            );
        }
    }
}

#[test]
fn disconnected_components_match_explicit_dummies() {
    let codes = vec![vec![0, 0, 1, 1, 2, 2, 3, 3], vec![0, 1, 0, 1, 2, 3, 2, 3]];
    let columns = vec![vec![2.0, -1.0, 4.0, 7.0, 3.0, 8.0, -2.0, 6.0]];
    run_case(&codes, &[0, 1], &columns, CaseWeights::Unit);
}

#[test]
fn common_weight_scaling_and_row_permutation_preserve_identified_residuals() {
    let codes = vec![
        vec![0, 0, 0, 1, 1, 1, 2, 2, 2],
        vec![0, 1, 2, 0, 1, 2, 0, 1, 2],
    ];
    let columns = vec![vec![1.0, 7.0, -2.0, 5.0, 3.0, 8.0, 4.0, -1.0, 6.0]];
    let weights = vec![0.5, 2.0, 3.0, 1.0, 4.0, 1.5, 2.5, 0.75, 5.0];
    let scaled = weights
        .iter()
        .map(|weight| 97.0 * weight)
        .collect::<Vec<_>>();
    let baseline = solve_case(&codes, &[0, 1], &columns, CaseWeights::Analytic(&weights));
    let rescaled = solve_case(&codes, &[0, 1], &columns, CaseWeights::Analytic(&scaled));
    assert_close(&baseline, &rescaled, 5e-12);

    let permutation = [8_usize, 1, 5, 0, 7, 3, 2, 6, 4];
    let permuted_codes = codes
        .iter()
        .map(|source| permutation.iter().map(|row| source[*row]).collect())
        .collect::<Vec<Vec<u32>>>();
    let permuted_columns = columns
        .iter()
        .map(|column| permutation.iter().map(|row| column[*row]).collect())
        .collect::<Vec<Vec<f64>>>();
    let permuted_weights = permutation
        .iter()
        .map(|row| weights[*row])
        .collect::<Vec<_>>();
    let permuted = solve_case(
        &permuted_codes,
        &[0, 1],
        &permuted_columns,
        CaseWeights::Analytic(&permuted_weights),
    );
    let mut restored = vec![0.0; baseline.len()];
    for (new_row, old_row) in permutation.iter().copied().enumerate() {
        restored[old_row] = permuted[new_row];
    }
    assert_close(&baseline, &restored, 5e-11);
}

#[test]
fn randomized_well_conditioned_designs_match_explicit_dummies() {
    let mut generator = Generator(0x7f33_2a19_d08c_5b61);
    for _ in 0..16 {
        let n = 24;
        let shift_a = generator.usize(3);
        let shift_b = generator.usize(4);
        let codes = vec![
            (0..n).map(|row| ((row + shift_a) % 3) as u32).collect(),
            (0..n)
                .map(|row| (((row / 3) + shift_b) % 4) as u32)
                .collect(),
        ];
        let columns = (0..3)
            .map(|_| (0..n).map(|_| generator.f64(-5.0, 5.0)).collect())
            .collect::<Vec<Vec<f64>>>();
        let weights = (0..n).map(|_| generator.f64(0.25, 4.0)).collect::<Vec<_>>();
        run_case(&codes, &[0, 1], &columns, CaseWeights::Analytic(&weights));
    }
}

fn run_case(codes: &[Vec<u32>], effects: &[usize], columns: &[Vec<f64>], case: CaseWeights<'_>) {
    let prepared = prepare_case(codes, effects, columns[0].clone(), case);
    let design = prepared
        .borrowed_intercept_fe_design()
        .expect("borrowed prepared design");
    assert_eq!(design.effects().len(), effects.len());
    assert_eq!(
        prepared.fe_topology().registry().sources().len(),
        unique_source_count(effects)
    );
    for (effect_index, effect) in design.effects().iter().enumerate() {
        let categorical = &prepared.categorical_sources()[prepared
            .categorical_sources()
            .iter()
            .position(|s| s.source() == prepared.fe_effects()[effect_index].sources[0])
            .unwrap()];
        assert!(std::ptr::eq(
            effect.codes(),
            categorical
                .codes(prepared.fe_topology())
                .expect("topology-backed source")
        ));
    }

    let reference = DenseMatrix::from_columns(columns).expect("reference matrix");
    let result = absorb_borrowed_intercept_map_certified(
        &design,
        prepared_map_weights(&prepared),
        DenseMatrix::from_columns(columns).expect("solver matrix"),
        map_config(),
    )
    .expect("certified borrowed MAP");
    assert!(result.report().certified());

    let dummy_effects = dummy_effects(codes, effects);
    for (rhs, column) in columns.iter().enumerate() {
        let oracle = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: column.clone(),
            regressors: Vec::new(),
            effects: dummy_effects.clone(),
            weights: oracle_weights(case),
            clusters: Vec::new(),
        })
        .expect("explicit dummy oracle");
        let residuals = result.residualized().column(rhs).expect("residualized RHS");
        assert_close(&residuals, &oracle.residuals, TOLERANCE);
        let fitted = column
            .iter()
            .zip(&residuals)
            .map(|(value, residual)| value - residual)
            .collect::<Vec<_>>();
        assert_close(&fitted, &oracle.absorbed_fitted_values, TOLERANCE);
    }

    let owned_effects = effects
        .iter()
        .map(|source| {
            EffectTerm::new(
                codes[*source].clone(),
                level_count(&codes[*source]),
                true,
                Vec::new(),
            )
            .expect("owned oracle effect")
        })
        .collect::<Vec<_>>();
    let owned_design = FeDesign::new(columns[0].len(), owned_effects).expect("owned oracle design");
    let raw_weights = raw_weights(case, columns[0].len());
    let oracle_certificate = fe_projection_certificate_with_reference(
        &owned_design,
        &raw_weights,
        result.residualized(),
        &reference,
    )
    .expect("explicit certificate");
    assert!(
        (result.report().certificate.relative_infinity_norm
            - oracle_certificate.relative_infinity_norm)
            .abs()
            < 5e-12
    );
    assert!(
        (result.report().certificate.relative_two_norm - oracle_certificate.relative_two_norm)
            .abs()
            < 5e-12
    );
}

fn solve_case(
    codes: &[Vec<u32>],
    effects: &[usize],
    columns: &[Vec<f64>],
    case: CaseWeights<'_>,
) -> Vec<f64> {
    let prepared = prepare_case(codes, effects, columns[0].clone(), case);
    let design = prepared
        .borrowed_intercept_fe_design()
        .expect("borrowed design");
    absorb_borrowed_intercept_map_certified(
        &design,
        prepared_map_weights(&prepared),
        DenseMatrix::from_columns(columns).expect("matrix"),
        map_config(),
    )
    .expect("solve")
    .residualized()
    .column(0)
    .expect("column")
}

fn prepare_case(
    codes: &[Vec<u32>],
    effects: &[usize],
    outcome: Vec<f64>,
    weights: CaseWeights<'_>,
) -> PreparedOlsData {
    let n = outcome.len();
    assert!(codes.iter().all(|source| source.len() == n));
    let sources = (0..codes.len())
        .map(|index| SourceEntry {
            id: source_id(index),
            canonical_name: format!("g{}", index + 1),
        })
        .collect::<Vec<_>>();
    let terms = sources
        .iter()
        .enumerate()
        .map(|(index, source)| continuous_term(index, source.id))
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
        sources
            .iter()
            .enumerate()
            .map(|(index, source)| OlsInputBinding {
                stata_var_position: u32::try_from(index + 3).expect("small binding"),
                role: OlsInputRole::NumericSource(source.id),
            }),
    );
    if let Some(role) = weight_role(weights) {
        bindings.push(OlsInputBinding {
            stata_var_position: u32::try_from(codes.len() + 3).expect("small weight binding"),
            role,
        });
    }
    let request = OlsRequestV1 {
        model: CompiledTermGraph { sources, terms },
        absorbed_terms: effects
            .iter()
            .enumerate()
            .map(|(index, source)| AbsorbedTerm {
                id: AbsorbTermId::new(u32::try_from(index + 1).expect("small effect"))
                    .expect("effect id"),
                intercept: true,
                categorical_sources: vec![source_id(*source)],
                slope_sources: Vec::new(),
                label: format!("g{}_effect_{}", source + 1, index + 1),
                saved_name: String::new(),
            })
            .collect(),
        bindings,
        include_intercept: false,
        keep_singletons: true,
    };
    let raw_weight = weight_values(weights);
    let loaded = LoadedOlsInput {
        candidate: vec![true; n],
        outcome: loaded_column(outcome),
        numeric_sources: codes
            .iter()
            .enumerate()
            .map(|(index, source)| {
                (
                    source_id(index),
                    loaded_column(source.iter().map(|code| f64::from(*code)).collect()),
                )
            })
            .collect(),
        analytic_weight: matches!(weights, CaseWeights::Analytic(_))
            .then(|| loaded_column(raw_weight.clone())),
        frequency_weight: matches!(weights, CaseWeights::Frequency(_))
            .then(|| loaded_column(raw_weight.clone())),
        probability_weight: matches!(weights, CaseWeights::Probability(_))
            .then(|| loaded_column(raw_weight)),
    };
    prepare_ols_data(&request, &loaded).expect("prepared case")
}

fn continuous_term(index: usize, source: SourceId) -> ExpandedTerm {
    let one_based = u32::try_from(index + 1).expect("small term");
    ExpandedTerm {
        id: TermId::new(one_based).expect("term id"),
        order: TermOrder::new(one_based).expect("term order"),
        coefficient_slot: CoefficientSlot::new(one_based).expect("slot"),
        raw_kind: RawTermKind::Variable,
        whole_column_omitted: false,
        canonical_token: format!("c.g{one_based}"),
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

fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

fn prepared_map_weights(prepared: &PreparedOlsData) -> BorrowedMapWeights<'_> {
    match &prepared.sample().weights {
        PreparedWeights::None => BorrowedMapWeights::Unit,
        PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),
    }
}

fn dummy_effects(codes: &[Vec<u32>], effects: &[usize]) -> Vec<DummyEffect> {
    effects
        .iter()
        .map(|source| {
            DummyEffect::new(
                codes[*source].clone(),
                level_count(&codes[*source]),
                true,
                Vec::new(),
            )
            .expect("dummy effect")
        })
        .collect()
}

fn level_count(codes: &[u32]) -> usize {
    codes
        .iter()
        .copied()
        .max()
        .map_or(0, |value| value as usize + 1)
}

fn source_id(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source id")
}

fn unique_source_count(effects: &[usize]) -> usize {
    let mut unique = effects.to_vec();
    unique.sort_unstable();
    unique.dedup();
    unique.len()
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

fn raw_weights(weights: CaseWeights<'_>, n: usize) -> Vec<f64> {
    match weights {
        CaseWeights::Unit => vec![1.0; n],
        _ => weight_values(weights),
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

fn assert_close(left: &[f64], right: &[f64], tolerance: f64) {
    assert_eq!(left.len(), right.len());
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        assert!(
            (left - right).abs() <= tolerance * left.abs().max(right.abs()).max(1.0),
            "index={index} left={left:.17e} right={right:.17e}"
        );
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

    fn usize(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper
    }

    fn f64(&mut self, lower: f64, upper: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / ((1_u64 << 53) as f64);
        lower + (upper - lower) * unit
    }
}
