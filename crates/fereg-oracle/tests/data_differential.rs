#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
//! Exact harvest parity, including the first fatal domain error and missingness precedence.
use fereg_core as old;
use fereg_engine::data as new;
use fereg_oracle::designs::{generate, DesignSpec, Graph};

struct Policy;
macro_rules! policy {
    ($data:ident, $error:path, $code:path, $result:path) => {
        impl $data::NumericDomainPolicy for Policy {
            fn outcome(&self, _: usize, value: f64) -> $result {
                Ok(if value < 0.0 {
                    $data::DomainDecision::Drop
                } else {
                    $data::DomainDecision::Retain
                })
            }
            fn weight(&self, row: usize, _: $data::WeightKind, value: f64) -> $result {
                if value < 0.0 {
                    return Err(<$error>::new(
                        $code,
                        format!("negative weight at row {row}"),
                    ));
                }
                Ok(if value == 0.0 {
                    $data::DomainDecision::Drop
                } else {
                    $data::DomainDecision::Retain
                })
            }
            fn offset(&self, _: usize, _: f64) -> $result {
                Ok($data::DomainDecision::Retain)
            }
            fn exposure(&self, _: usize, value: f64) -> $result {
                Ok(if value <= 0.0 {
                    $data::DomainDecision::Drop
                } else {
                    $data::DomainDecision::Retain
                })
            }
        }
    };
}
policy!(
    old,
    old::FeregError,
    old::ErrorCode::NegativeWeight,
    old::Result<old::DomainDecision>
);
policy!(
    new,
    fereg_engine::FeregError,
    fereg_engine::ErrorCode::NegativeWeight,
    fereg_engine::Result<new::DomainDecision>
);

struct Inputs {
    candidate: Vec<bool>,
    outcome: Vec<f64>,
    sources: Vec<Vec<f64>>,
    masks: Vec<Vec<bool>>,
    weights: Vec<f64>,
    adjustment: Vec<f64>,
}
fn inputs(seed: usize) -> Inputs {
    let n = [1, 7, 31, 257][seed % 4];
    let design = generate(
        seed as u64,
        &DesignSpec {
            rows: n,
            graph: [Graph::DensePanel, Graph::AkmWeak, Graph::Disconnected][seed % 3],
            missing_fraction: 0.1,
            ..DesignSpec::default()
        },
    )
    .unwrap();
    let mut sources = design.regressors;
    sources.extend(design.effects.iter().map(|effect| {
        effect
            .codes()
            .iter()
            .map(|&code| code as f64)
            .collect::<Vec<_>>()
    }));
    let masks: Vec<Vec<_>> = (0..sources.len() + 1)
        .map(|j| {
            (0..n)
                .map(|i| (i * 71 + seed * 31 + j * 13) % 23 == 0)
                .collect()
        })
        .collect();
    for (j, source) in sources.iter_mut().enumerate() {
        for (i, value) in source.iter_mut().enumerate() {
            *value = match (seed * 43 + i * 11 + j * 17) % 59 {
                0 => f64::NAN,
                1 => f64::INFINITY,
                2 => f64::NEG_INFINITY,
                3 => -0.0,
                _ => *value,
            };
        }
    }
    Inputs {
        candidate: (0..n).map(|i| (i + seed) % 13 != 0).collect(),
        outcome: design.outcome,
        sources,
        masks,
        weights: (0..n)
            .map(|i| match (i + seed * 17) % 47 {
                0 => -1.,
                1 => 0.,
                2 => f64::NAN,
                _ => (i % 7) as f64 + 0.5,
            })
            .collect(),
        adjustment: (0..n)
            .map(|i| match (i + seed * 7) % 19 {
                0 => f64::NAN,
                1 => -1.,
                2 => 0.,
                _ => (i % 5) as f64,
            })
            .collect(),
    }
}
fn compare(seed: usize) {
    let input = inputs(seed);
    let old_columns: Vec<_> = input
        .sources
        .iter()
        .zip(&input.masks)
        .map(|(values, missing)| old::NumericColumnView::with_missing(values, missing))
        .collect();
    let new_columns: Vec<_> = input
        .sources
        .iter()
        .zip(&input.masks)
        .map(|(values, missing)| new::NumericColumnView::with_missing(values, missing))
        .collect();
    let old_uses = [
        old::NumericSourceUse::ordinary(),
        old::NumericSourceUse::ordinary_and_fixed_effect(),
        old::NumericSourceUse::fixed_effect(),
        old::NumericSourceUse {
            ordinary: false,
            fixed_effect: seed % 2 == 0,
            cluster: true,
        },
    ];
    let new_uses: Vec<_> = old_uses
        .iter()
        .map(|usage| new::NumericSourceUse {
            ordinary: usage.ordinary,
            fixed_effect: usage.fixed_effect,
            cluster: usage.cluster,
        })
        .collect();
    let old_weight = [
        old::WeightKind::Frequency,
        old::WeightKind::Analytic,
        old::WeightKind::Probability,
        old::WeightKind::Importance,
    ][seed % 4];
    let new_weight = [
        new::WeightKind::Frequency,
        new::WeightKind::Analytic,
        new::WeightKind::Probability,
        new::WeightKind::Importance,
    ][seed % 4];
    let old_adjust = old::NumericColumnView::new(&input.adjustment);
    let new_adjust = new::NumericColumnView::new(&input.adjustment);
    let previous = old::prepare_numeric_sample_with_source_uses(
        &old::NumericPreparationInput {
            candidate: &input.candidate,
            outcome: old::NumericColumnView::with_missing(
                &input.outcome,
                input.masks.last().unwrap(),
            ),
            regressors: &old_columns,
            weights: if seed % 5 == 0 {
                old::WeightInput::None
            } else {
                old::WeightInput::Provided {
                    kind: old_weight,
                    column: old::NumericColumnView::new(&input.weights),
                }
            },
            adjustment: match seed % 3 {
                0 => old::AdjustmentInput::None,
                1 => old::AdjustmentInput::Offset(old_adjust),
                _ => old::AdjustmentInput::Exposure(old_adjust),
            },
        },
        &old_uses,
        &Policy,
    );
    let current = new::prepare_numeric_sample_with_source_uses(
        &new::NumericPreparationInput {
            candidate: &input.candidate,
            outcome: new::NumericColumnView::with_missing(
                &input.outcome,
                input.masks.last().unwrap(),
            ),
            regressors: &new_columns,
            weights: if seed % 5 == 0 {
                new::WeightInput::None
            } else {
                new::WeightInput::Provided {
                    kind: new_weight,
                    column: new::NumericColumnView::new(&input.weights),
                }
            },
            adjustment: match seed % 3 {
                0 => new::AdjustmentInput::None,
                1 => new::AdjustmentInput::Offset(new_adjust),
                _ => new::AdjustmentInput::Exposure(new_adjust),
            },
        },
        &new_uses,
        &Policy,
    );
    assert_prepared(previous, current, seed);
}
fn assert_prepared(
    previous: old::Result<old::PreparedNumericSample>,
    current: fereg_engine::Result<new::Prepared>,
    seed: usize,
) {
    match (previous, current) {
        (Ok(a), Ok(b)) => {
            b.validate_alignment().unwrap();
            assert_eq!(a.n_original, b.n_original, "seed={seed}");
            assert_eq!(a.n_candidate, b.n_candidate, "seed={seed}");
            assert_eq!(
                a.original_row_by_active, b.original_row_by_active,
                "seed={seed}"
            );
            assert_eq!(
                a.drop_reasons.iter().map(|r| r.code()).collect::<Vec<_>>(),
                b.drop_reasons.iter().map(|r| r.code()).collect::<Vec<_>>(),
                "seed={seed}"
            );
            assert_bits(&a.outcome, &b.outcome);
            for (old, new) in a.regressors.iter().zip(&b.regressors) {
                assert_bits(old, new);
            }
            match (a.weights, b.weights) {
                (old::PreparedWeights::None, new::PreparedWeights::None) => (),
                (
                    old::PreparedWeights::Provided { kind: a, values: x },
                    new::PreparedWeights::Provided { kind: b, values: y },
                ) => {
                    assert_eq!(a.label(), b.label());
                    assert_bits(&x, &y);
                }
                pair => panic!("weight mismatch {pair:?}"),
            }
            match (a.adjustment, b.adjustment) {
                (old::PreparedAdjustment::None, new::PreparedAdjustment::None) => (),
                (old::PreparedAdjustment::Offset(x), new::PreparedAdjustment::Offset(y))
                | (old::PreparedAdjustment::Exposure(x), new::PreparedAdjustment::Exposure(y)) => {
                    assert_bits(&x, &y)
                }
                pair => panic!("adjustment mismatch {pair:?}"),
            }
        }
        (Err(a), Err(b)) => {
            assert_eq!(a.code.label(), b.code.label(), "seed={seed}");
            assert_eq!(a.message, b.message, "seed={seed}");
        }
        pair => panic!("sample mismatch for seed {seed}: {pair:?}"),
    }
}
fn assert_bits(a: &[f64], b: &[f64]) {
    assert_eq!(
        a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}
#[test]
fn numeric_sample_harvest_is_exact_on_random_designs() {
    for seed in 0..400 {
        compare(seed);
    }
}
#[test]
#[ignore = "long seeded sample differential"]
fn long_numeric_sample_harvest_is_exact() {
    for seed in 400..2400 {
        compare(seed);
    }
}

#[test]
fn factor_codes_and_levels_match_legacy_for_random_permutations() {
    for seed in 0..100 {
        let inputs = inputs(seed);
        let values: Vec<_> = inputs
            .sources
            .into_iter()
            .flatten()
            .filter(|value| value.is_finite())
            .collect();
        let a = old::factorize_f64(&values).unwrap();
        let b = fereg_engine::factor::factorize_f64(&values).unwrap();
        assert_eq!(a.codes, b.codes);
        assert_bits(&a.levels, &b.levels);
        let strings: Vec<_> = values
            .iter()
            .map(|value| format!("label#{value}"))
            .collect();
        let a = old::factorize_strings(&strings).unwrap();
        let b = fereg_engine::factor::factorize_strings(&strings).unwrap();
        assert_eq!(a.codes, b.codes);
        assert_eq!(a.levels, b.levels);
    }
}
