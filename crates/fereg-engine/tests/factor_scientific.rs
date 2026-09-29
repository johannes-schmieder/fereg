#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
// Scientific assertions harvested from legacy; admission-only tests remain there.
use fereg_engine::{factor::*, threads::ThreadTeam, ErrorCode, FeregError};
#[test]
fn two_way_codes_are_compact_collision_free_and_lexicographic() {
    let firm = [2, 0, 2, 1, 0, 2, 1];
    let year = [1, 2, 1, 0, 2, 0, 0];
    let encoded = factorize_code_interaction(&[
        CategoricalCodeView::new(&firm, 3),
        CategoricalCodeView::new(&year, 3),
    ])
    .expect("factorization");
    assert_eq!(encoded.n_levels(), 4);
    // Tuples in lexical order are (0,2), (1,0), (2,0), (2,1).
    assert_eq!(encoded.codes(), &[3, 0, 3, 1, 0, 2, 1]);
}

#[test]
fn three_way_and_repeated_components_are_exact() {
    let a = [0, 0, 1, 1, 1, 0];
    let b = [0, 1, 0, 1, 1, 0];
    let c = [1, 1, 0, 0, 1, 1];
    let encoded = factorize_code_interaction(&[
        CategoricalCodeView::new(&a, 2),
        CategoricalCodeView::new(&b, 2),
        CategoricalCodeView::new(&c, 2),
    ])
    .expect("three-way factorization");
    assert_eq!(encoded.n_levels(), 5);
    assert_eq!(encoded.codes(), &[0, 1, 2, 3, 4, 0]);

    let repeated = factorize_code_interaction(&[
        CategoricalCodeView::new(&a, 2),
        CategoricalCodeView::new(&a, 2),
    ])
    .expect("repeated component");
    assert_eq!(repeated.n_levels(), 2);
    assert_eq!(repeated.codes(), a);
}

#[test]
fn row_permutation_preserves_tuple_to_code_mapping() {
    let left = [0, 1, 0, 2, 1, 2, 0];
    let right = [2, 0, 1, 1, 0, 1, 2];
    let original = factorize_code_interaction(&[
        CategoricalCodeView::new(&left, 3),
        CategoricalCodeView::new(&right, 3),
    ])
    .expect("original");
    let permutation = [5, 2, 6, 1, 4, 0, 3];
    let permuted_left = permutation.map(|row| left[row]);
    let permuted_right = permutation.map(|row| right[row]);
    let permuted = factorize_code_interaction(&[
        CategoricalCodeView::new(&permuted_left, 3),
        CategoricalCodeView::new(&permuted_right, 3),
    ])
    .expect("permuted");
    assert_eq!(original.n_levels(), permuted.n_levels());
    for (new_row, old_row) in permutation.into_iter().enumerate() {
        assert_eq!(permuted.codes()[new_row], original.codes()[old_row]);
    }
}

#[test]
fn unused_component_levels_are_recompacted() {
    let left = [4, 4, 1, 1];
    let right = [3, 3, 2, 2];
    let encoded = factorize_code_interaction(&[
        CategoricalCodeView::new(&left, 5),
        CategoricalCodeView::new(&right, 4),
    ])
    .expect("factorization with holes");
    assert_eq!(encoded.n_levels(), 2);
    assert_eq!(encoded.codes(), &[1, 1, 0, 0]);
}

#[test]
fn empty_input_has_no_levels() {
    let encoded = factorize_code_interaction(&[
        CategoricalCodeView::new(&[], 0),
        CategoricalCodeView::new(&[], 0),
    ])
    .expect("empty interaction");
    assert!(encoded.codes().is_empty());
    assert_eq!(encoded.n_levels(), 0);
}

#[test]
fn malformed_components_fail_before_allocation() {
    let valid = [0, 1];
    let short = [0];
    let invalid_code = [0, 2];
    let cases = [
        factorize_code_interaction(&[]).expect_err("zero arity"),
        factorize_code_interaction(&[
            CategoricalCodeView::new(&valid, 2),
            CategoricalCodeView::new(&short, 1),
        ])
        .expect_err("row mismatch"),
        factorize_code_interaction(&[CategoricalCodeView::new(&valid, 0)])
            .expect_err("zero levels"),
        factorize_code_interaction(&[CategoricalCodeView::new(&invalid_code, 2)])
            .expect_err("invalid code"),
        factorize_code_interaction(&[CategoricalCodeView::new(&[], 1)])
            .expect_err("empty with levels"),
    ];
    assert_eq!(cases[0].code, ErrorCode::InvalidArgument);
    assert_eq!(cases[1].code, ErrorCode::InvalidArgument);
    assert_eq!(cases[2].code, ErrorCode::InvalidArgument);
    assert_eq!(cases[3].code, ErrorCode::InvalidData);
    assert_eq!(cases[4].code, ErrorCode::InvalidArgument);
}

#[test]
fn interactions_match_an_independent_tuple_map() {
    use std::collections::BTreeMap;
    for arity in 1..=6 {
        for nobs in [1, 7, 32, 257, 4096] {
            for extra_levels in [0, 19] {
                let columns: Vec<Vec<u32>> = (0..arity)
                    .map(|column| {
                        (0..nobs)
                            .map(|row| ((row * 17 + row / 7 + column * 3) % (column + 2)) as u32)
                            .collect()
                    })
                    .collect();
                let components: Vec<_> = columns
                    .iter()
                    .enumerate()
                    .map(|(index, codes)| CategoricalCodeView::new(codes, index + 2 + extra_levels))
                    .collect();
                let tuples: Vec<Vec<u32>> = (0..nobs)
                    .map(|row| columns.iter().map(|codes| codes[row]).collect())
                    .collect();
                let mut canonical: BTreeMap<Vec<u32>, u32> =
                    tuples.iter().cloned().map(|tuple| (tuple, 0)).collect();
                for (code, (_, value)) in canonical.iter_mut().enumerate() {
                    *value = code as u32;
                }
                let expected: Vec<u32> = tuples.iter().map(|tuple| canonical[tuple]).collect();
                let actual = factorize_code_interaction(&components).expect("factorization");
                assert_eq!(actual.n_levels(), canonical.len());
                assert_eq!(
                    actual.codes(),
                    expected,
                    "arity={arity}, rows={nobs}, extra={extra_levels}"
                );
            }
        }
    }
}

#[test]
fn moderate_large_case_retains_exact_partition() {
    const N: usize = 50_000;
    let left = (0..N)
        .map(|row| u32::try_from((row * 17) % 997).expect("bounded"))
        .collect::<Vec<_>>();
    let right = (0..N)
        .map(|row| u32::try_from((row * 31) % 101).expect("bounded"))
        .collect::<Vec<_>>();
    let encoded = factorize_code_interaction(&[
        CategoricalCodeView::new(&left, 997),
        CategoricalCodeView::new(&right, 101),
    ])
    .expect("large factorization");
    assert_eq!(encoded.codes().len(), N);
    for first in (0..N).step_by(137) {
        for second in (0..N).step_by(251) {
            assert_eq!(
                encoded.codes()[first] == encoded.codes()[second],
                left[first] == left[second] && right[first] == right[second]
            );
        }
    }
}

#[test]
fn integer_codes_are_row_order_invariant() {
    let encoded = factorize_i64(&[20, 10, 20, -1]).expect("factorize");
    assert_eq!(encoded.levels, vec![-1, 10, 20]);
    assert_eq!(encoded.codes, vec![2, 1, 2, 0]);
}

#[test]
fn floating_zero_is_canonicalized() {
    let encoded = factorize_f64(&[-0.0, 0.0, 2.5]).expect("factorize");
    assert_eq!(encoded.levels, vec![0.0, 2.5]);
    assert_eq!(encoded.codes, vec![0, 0, 1]);
}

#[test]
fn ordered_float_factorization_matches_independent_sorted_level_oracle() {
    for rows in [0, 1, 16_383, 16_384, 32_771] {
        for levels in [
            vec![-0.0, 0.0],
            vec![-7.0, -1.0, 0.0, 9.0],
            vec![-f64::MAX, -0.25, -0.0, 0.0, 1e-300, 0.5, f64::MAX],
        ] {
            let values: Vec<_> = (0..rows)
                .map(|row| levels[row * levels.len() / rows])
                .collect();
            let mut expected_levels: Vec<_> = values
                .iter()
                .map(|&v| if v == 0.0 { 0.0 } else { v })
                .collect();
            expected_levels.sort_unstable_by(f64::total_cmp);
            expected_levels.dedup();
            let expected_codes: Vec<_> = values
                .iter()
                .map(|v| {
                    expected_levels
                        .binary_search_by(|level| level.partial_cmp(v).unwrap())
                        .unwrap() as u32
                })
                .collect();
            {
                let actual = factorize_f64(&values).unwrap();
                assert_eq!(actual.codes, expected_codes);
                assert_eq!(
                    actual
                        .levels
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    expected_levels
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>()
                );
            }
            if rows > 1 && levels[0] != *levels.last().unwrap() {
                let mut near_ordered = values.clone();
                near_ordered[rows - 1] = levels[0];
                let actual = factorize_f64(&near_ordered).unwrap();
                for (&code, &value) in actual.codes.iter().zip(&near_ordered) {
                    assert_eq!(actual.levels[code as usize], value);
                }
            }
        }
    }
}

#[test]
fn nonfinite_rejection_is_preserved_for_ordered_and_unordered_values() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for row in [0, 4_095, 16_383, 16_384] {
            let mut values: Vec<_> = (0..16_385).map(|r| r as f64 / 3.0).collect();
            values[row] = invalid;
            let error = factorize_f64(&values).unwrap_err();
            assert_eq!(
                error,
                FeregError::new(
                    ErrorCode::InvalidData,
                    "categorical numeric source contains a nonfinite value"
                )
            );
            values[1] = -1.0;
            assert!(factorize_f64(&values).is_err());
        }
    }
    assert!(factorize_f64(&[2.0, 1.0, f64::NAN]).is_err());
}

#[test]
fn dense_integer_codes_are_canonical_with_gaps_and_negatives() {
    let encoded = factorize_f64(&[2.0, -1.0, 2.0, 0.0, -1.0]).expect("factorize");
    assert_eq!(encoded.levels, vec![-1.0, 0.0, 2.0]);
    assert_eq!(encoded.codes, vec![2, 0, 2, 1, 0]);
}

#[test]
fn factorization_preserves_codes_and_errors_across_threads() {
    for rows in [16383, 16384, 32771] {
        let mut values: Vec<_> = (0..rows)
            .map(|r| {
                if r % 59 == 0 {
                    -0.0
                } else {
                    ((r * 71) % 971) as f64 - 500.0
                }
            })
            .collect();
        let reference = factorize_f64(&values).unwrap();
        let team = ThreadTeam::new(4).unwrap();
        assert_eq!(reference, team.install(|| factorize_f64(&values)).unwrap());
        for first in [0, 4095, rows - 2] {
            for pair in [
                [0.5, f64::NAN],
                [f64::NAN, 0.5],
                [i64::MAX as f64, f64::INFINITY],
                [f64::NEG_INFINITY, -0.5],
            ] {
                let old = [values[first], values[first + 1]];
                values[first] = pair[0];
                values[first + 1] = pair[1];
                assert_eq!(
                    factorize_f64(&values),
                    team.install(|| factorize_f64(&values))
                );
                values[first] = old[0];
                values[first + 1] = old[1];
            }
        }
    }
}

#[test]
fn string_codes_are_collision_safe() {
    let encoded = factorize_strings(&["a#b", "a", "a#b", "b"]).expect("factorize");
    assert_eq!(encoded.levels, vec!["a", "a#b", "b"]);
    assert_eq!(encoded.codes, vec![1, 0, 1, 2]);
}
