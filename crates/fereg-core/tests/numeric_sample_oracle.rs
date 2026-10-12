use std::cell::RefCell;

use fereg_core::{
    prepare_numeric_sample, prepare_numeric_sample_with_source_uses, AdjustmentInput,
    DomainDecision, DropReason, ErrorCode, FeregError, NumericColumnView, NumericDomainPolicy,
    NumericPreparationInput, NumericSourceUse, PreparedAdjustment, PreparedWeights, Result,
    WeightInput, WeightKind,
};

#[test]
fn unified_source_roles_lock_regressor_weight_and_fixed_effect_precedence() {
    let candidate = [false, true, true, true, true, true];
    let outcome = [0.0, f64::NAN, 2.0, 3.0, 4.0, 5.0];
    let ordinary_and_fe = [f64::NAN, 1.0, f64::NAN, 1.0, 1.0, -1.5];
    let fe_only = [f64::NAN, f64::NAN, f64::NAN, f64::NAN, 2.5, -3.25];
    let weight = [1.0, 1.0, 1.0, f64::NAN, 1.0, 2.0];
    let regressors = [
        NumericColumnView::new(&ordinary_and_fe),
        NumericColumnView::new(&fe_only),
    ];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &regressors,
        weights: WeightInput::Provided {
            kind: WeightKind::Analytic,
            column: NumericColumnView::new(&weight),
        },
        adjustment: AdjustmentInput::None,
    };
    let uses = [
        NumericSourceUse::ordinary_and_fixed_effect(),
        NumericSourceUse::fixed_effect(),
    ];
    let prepared =
        prepare_numeric_sample_with_source_uses(&input, &uses, &LiteralPolicy::default()).unwrap();
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        vec![
            DropReason::ExcludedByIfIn,
            DropReason::MissingOutcome,
            DropReason::MissingRegressor,
            DropReason::InvalidOrMissingWeight,
            DropReason::Retained,
            DropReason::Retained,
        ]
    );
    assert_eq!(prepared.retained_rows().iter().collect::<Vec<_>>(), &[4, 5]);
    assert_eq!(prepared.regressors[1], vec![2.5, -3.25]);

    let no_weight = NumericPreparationInput {
        weights: WeightInput::None,
        ..input
    };
    let prepared =
        prepare_numeric_sample_with_source_uses(&no_weight, &uses, &LiteralPolicy::default())
            .unwrap();
    assert_eq!(prepared.drop_reasons[3], DropReason::MissingFixedEffect);
}

#[derive(Default)]
struct LiteralPolicy {
    outcome_drop: Option<usize>,
    weight_drop: Option<usize>,
    offset_drop: Option<usize>,
    exposure_drop: Option<usize>,
    fatal: Option<(&'static str, usize)>,
    calls: RefCell<Vec<(&'static str, usize, Option<WeightKind>)>>,
}

impl LiteralPolicy {
    fn decide(&self, hook: &'static str, row: usize) -> Result<DomainDecision> {
        if self.fatal == Some((hook, row)) {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "injected domain-policy error",
            ));
        }
        let drop = match hook {
            "outcome" => self.outcome_drop,
            "weight" => self.weight_drop,
            "offset" => self.offset_drop,
            "exposure" => self.exposure_drop,
            _ => unreachable!(),
        };
        Ok(if drop == Some(row) {
            DomainDecision::Drop
        } else {
            DomainDecision::Retain
        })
    }
}

impl NumericDomainPolicy for LiteralPolicy {
    fn outcome(&self, row: usize, _value: f64) -> Result<DomainDecision> {
        self.calls.borrow_mut().push(("outcome", row, None));
        self.decide("outcome", row)
    }

    fn weight(&self, row: usize, kind: WeightKind, _value: f64) -> Result<DomainDecision> {
        self.calls.borrow_mut().push(("weight", row, Some(kind)));
        self.decide("weight", row)
    }

    fn offset(&self, row: usize, _value: f64) -> Result<DomainDecision> {
        self.calls.borrow_mut().push(("offset", row, None));
        self.decide("offset", row)
    }

    fn exposure(&self, row: usize, _value: f64) -> Result<DomainDecision> {
        self.calls.borrow_mut().push(("exposure", row, None));
        self.decide("exposure", row)
    }
}

#[test]
fn literal_oracle_locks_precedence_masks_and_policy_order() {
    let candidate = [false, true, true, true, true, true, true, true, true];
    let outcome = [f64::NAN, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
    let outcome_missing = [false, true, false, false, false, false, false, false, false];
    let first_regressor = [f64::NAN, 1.0, f64::NAN, f64::NAN, 4.0, 5.0, 6.0, 7.0, 8.0];
    let second_regressor = [0.0; 9];
    let weight = [1.0; 9];
    let weight_missing = [false, false, false, false, true, false, false, false, false];
    let offset = [0.0, 0.0, 0.0, 0.0, 0.0, f64::NAN, f64::INFINITY, 0.0, -0.0];
    let regressors = [
        NumericColumnView::new(&first_regressor),
        NumericColumnView::new(&second_regressor),
    ];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::with_missing(&outcome, &outcome_missing),
        regressors: &regressors,
        weights: WeightInput::Provided {
            kind: WeightKind::Analytic,
            column: NumericColumnView::with_missing(&weight, &weight_missing),
        },
        adjustment: AdjustmentInput::Offset(NumericColumnView::new(&offset)),
    };
    let policy = LiteralPolicy {
        outcome_drop: Some(2),
        weight_drop: Some(5),
        offset_drop: Some(7),
        ..LiteralPolicy::default()
    };

    let prepared = prepare_numeric_sample(&input, &policy).unwrap();
    assert_eq!(prepared.n_original, 9);
    assert_eq!(prepared.n_candidate, 8);
    assert_eq!(prepared.n_active(), 1);
    assert_eq!(prepared.retained_rows().iter().collect::<Vec<_>>(), &[8]);
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [
            DropReason::ExcludedByIfIn,
            DropReason::MissingOutcome,
            DropReason::OtherModelExclusion,
            DropReason::MissingRegressor,
            DropReason::InvalidOrMissingWeight,
            DropReason::InvalidOrMissingWeight,
            DropReason::MissingOffsetOrExposure,
            DropReason::MissingOffsetOrExposure,
            DropReason::Retained,
        ]
    );
    assert!(prepared.is_retained(8));
    assert!(!prepared.is_retained(9));
    assert_eq!(prepared.drop_reason(9), None);
    assert_eq!(
        *policy.calls.borrow(),
        [
            ("outcome", 2, None),
            ("outcome", 3, None),
            ("outcome", 4, None),
            ("outcome", 5, None),
            ("weight", 5, Some(WeightKind::Analytic)),
            ("outcome", 6, None),
            ("weight", 6, Some(WeightKind::Analytic)),
            ("outcome", 7, None),
            ("weight", 7, Some(WeightKind::Analytic)),
            ("offset", 7, None),
            ("outcome", 8, None),
            ("weight", 8, Some(WeightKind::Analytic)),
            ("offset", 8, None),
        ]
    );
}

#[test]
fn every_nonfinite_value_is_unavailable_before_its_policy_hook() {
    let candidate = [true; 7];
    let outcome = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        4.0,
        5.0,
        6.0,
        7.0,
    ];
    let regressor = [1.0, 2.0, 3.0, f64::NEG_INFINITY, 5.0, 6.0, 7.0];
    let weight = [1.0, 1.0, 1.0, 1.0, f64::NAN, 1.0, 1.0];
    let exposure = [1.0, 1.0, 1.0, 1.0, 1.0, f64::INFINITY, 1.0];
    let regressors = [NumericColumnView::new(&regressor)];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &regressors,
        weights: WeightInput::Provided {
            kind: WeightKind::Probability,
            column: NumericColumnView::new(&weight),
        },
        adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&exposure)),
    };
    let policy = LiteralPolicy::default();
    let prepared = prepare_numeric_sample(&input, &policy).unwrap();
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [
            DropReason::MissingOutcome,
            DropReason::MissingOutcome,
            DropReason::MissingOutcome,
            DropReason::MissingRegressor,
            DropReason::InvalidOrMissingWeight,
            DropReason::MissingOffsetOrExposure,
            DropReason::Retained,
        ]
    );
    assert_eq!(
        *policy.calls.borrow(),
        [
            ("outcome", 3, None),
            ("outcome", 4, None),
            ("outcome", 5, None),
            ("weight", 5, Some(WeightKind::Probability)),
            ("outcome", 6, None),
            ("weight", 6, Some(WeightKind::Probability)),
            ("exposure", 6, None),
        ]
    );
}

#[test]
fn finite_missing_masks_are_authoritative_for_regressors_and_adjustments() {
    let candidate = [true, true];
    let outcome = [1.0, 2.0];
    let regressor = [10.0, 20.0];
    let regressor_missing = [true, false];
    let offset = [30.0, 40.0];
    let offset_missing = [false, true];
    let regressors = [NumericColumnView::with_missing(
        &regressor,
        &regressor_missing,
    )];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &regressors,
        weights: WeightInput::None,
        adjustment: AdjustmentInput::Offset(NumericColumnView::with_missing(
            &offset,
            &offset_missing,
        )),
    };
    let policy = LiteralPolicy::default();
    let prepared = prepare_numeric_sample(&input, &policy).unwrap();
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [
            DropReason::MissingRegressor,
            DropReason::MissingOffsetOrExposure
        ]
    );
    assert_eq!(
        *policy.calls.borrow(),
        [("outcome", 0, None), ("outcome", 1, None)]
    );
}

#[test]
fn candidate_exclusion_never_invokes_a_fatal_policy() {
    let candidate = [false];
    let outcome = [f64::NAN];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &[],
        weights: WeightInput::None,
        adjustment: AdjustmentInput::None,
    };
    let policy = LiteralPolicy {
        fatal: Some(("outcome", 0)),
        ..LiteralPolicy::default()
    };
    let prepared = prepare_numeric_sample(&input, &policy).unwrap();
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [DropReason::ExcludedByIfIn]
    );
    assert!(policy.calls.borrow().is_empty());

    let candidate = [true];
    let outcome = [1.0];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &[],
        weights: WeightInput::None,
        adjustment: AdjustmentInput::None,
    };
    let error = prepare_numeric_sample(&input, &policy).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidData);
}

#[test]
fn a_missing_later_regressor_drops_the_row_without_losing_column_count() {
    let candidate = [true, true];
    let outcome = [1.0, 2.0];
    let x1 = [10.0, 20.0];
    let x2 = [30.0, f64::NAN];
    let regressors = [NumericColumnView::new(&x1), NumericColumnView::new(&x2)];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &regressors,
        weights: WeightInput::None,
        adjustment: AdjustmentInput::None,
    };
    let prepared = prepare_numeric_sample(&input, &LiteralPolicy::default()).unwrap();
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [DropReason::Retained, DropReason::MissingRegressor]
    );
    assert_eq!(prepared.regressors, [vec![10.0], vec![30.0]]);
}

#[test]
fn retained_values_are_bit_exact_without_weight_or_adjustment_transforms() {
    let candidate = [true, true, true, true];
    let values = [-0.0, f64::from_bits(1), f64::MAX, -f64::MAX];
    let x1 = [f64::MIN_POSITIVE, -0.0, f64::from_bits(2), 17.0];
    let x2 = [3.0, 4.0, 5.0, 6.0];
    let weight = [f64::from_bits(3), -0.0, f64::MAX, -f64::MAX];
    let offset = [-0.0, f64::from_bits(4), f64::MAX, -f64::MAX];
    let regressors = [NumericColumnView::new(&x1), NumericColumnView::new(&x2)];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&values),
        regressors: &regressors,
        weights: WeightInput::Provided {
            kind: WeightKind::Importance,
            column: NumericColumnView::new(&weight),
        },
        adjustment: AdjustmentInput::Offset(NumericColumnView::new(&offset)),
    };
    let prepared = prepare_numeric_sample(&input, &LiteralPolicy::default()).unwrap();
    assert_bits(&prepared.outcome, &values);
    assert_bits(&prepared.regressors[0], &x1);
    assert_bits(&prepared.regressors[1], &x2);
    match prepared.weights {
        PreparedWeights::Provided { kind, values } => {
            assert_eq!(kind, WeightKind::Importance);
            assert_bits(&values, &weight);
        }
        PreparedWeights::None => panic!("provided weights were lost"),
    }
    match prepared.adjustment {
        PreparedAdjustment::Offset(values) => assert_bits(&values, &offset),
        _ => panic!("raw offset was lost"),
    }

    let exposure = [f64::from_bits(5), -0.0, f64::MAX, -f64::MAX];
    let exposure_input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&values),
        regressors: &regressors,
        weights: WeightInput::None,
        adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&exposure)),
    };
    let exposure_prepared =
        prepare_numeric_sample(&exposure_input, &LiteralPolicy::default()).unwrap();
    match exposure_prepared.adjustment {
        PreparedAdjustment::Exposure(values) => assert_bits(&values, &exposure),
        _ => panic!("raw exposure was lost or transformed"),
    }
}

#[test]
fn all_weight_kinds_and_both_adjustment_kinds_reach_the_policy_unchanged() {
    let candidate = [true];
    let outcome = [1.0];
    let weight = [2.5];
    let adjustment = [-3.0];
    for kind in [
        WeightKind::Frequency,
        WeightKind::Analytic,
        WeightKind::Probability,
        WeightKind::Importance,
    ] {
        for use_exposure in [false, true] {
            let input = NumericPreparationInput {
                candidate: (&candidate[..]).into(),
                outcome: NumericColumnView::new(&outcome),
                regressors: &[],
                weights: WeightInput::Provided {
                    kind,
                    column: NumericColumnView::new(&weight),
                },
                adjustment: if use_exposure {
                    AdjustmentInput::Exposure(NumericColumnView::new(&adjustment))
                } else {
                    AdjustmentInput::Offset(NumericColumnView::new(&adjustment))
                },
            };
            let policy = LiteralPolicy::default();
            let prepared = prepare_numeric_sample(&input, &policy).unwrap();
            assert_eq!(prepared.n_active(), 1);
            assert_eq!(policy.calls.borrow()[1], ("weight", 0, Some(kind)));
            assert_eq!(
                policy.calls.borrow()[2].0,
                if use_exposure { "exposure" } else { "offset" }
            );
        }
    }
}

#[test]
fn exposure_drop_and_error_are_distinct_from_availability() {
    let candidate = [true, true];
    let outcome = [1.0, 2.0];
    let exposure = [0.0, -1.0];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &[],
        weights: WeightInput::None,
        adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&exposure)),
    };
    let drop_policy = LiteralPolicy {
        exposure_drop: Some(0),
        ..LiteralPolicy::default()
    };
    let prepared = prepare_numeric_sample(&input, &drop_policy).unwrap();
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [DropReason::MissingOffsetOrExposure, DropReason::Retained]
    );

    let policy = LiteralPolicy {
        fatal: Some(("exposure", 1)),
        ..LiteralPolicy::default()
    };
    let error = prepare_numeric_sample(&input, &policy).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidData);
    assert_eq!(
        policy.calls.borrow().last().copied(),
        Some(("exposure", 1, None))
    );
}

#[test]
fn weight_and_offset_policy_errors_are_fatal() {
    let candidate = [true];
    let outcome = [1.0];
    let weight = [2.0];
    let offset = [3.0];
    let input = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &[],
        weights: WeightInput::Provided {
            kind: WeightKind::Frequency,
            column: NumericColumnView::new(&weight),
        },
        adjustment: AdjustmentInput::Offset(NumericColumnView::new(&offset)),
    };
    for hook in ["weight", "offset"] {
        let policy = LiteralPolicy {
            fatal: Some((hook, 0)),
            ..LiteralPolicy::default()
        };
        let error = prepare_numeric_sample(&input, &policy).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidData);
    }
}

#[test]
fn zero_sized_zero_candidate_and_all_dropped_inputs_are_valid() {
    let empty = NumericPreparationInput {
        candidate: (&[][..]).into(),
        outcome: NumericColumnView::new(&[]),
        regressors: &[],
        weights: WeightInput::None,
        adjustment: AdjustmentInput::None,
    };
    let prepared = prepare_numeric_sample(&empty, &LiteralPolicy::default()).unwrap();
    assert_eq!(prepared.n_original, 0);
    assert_eq!(prepared.n_candidate, 0);
    assert_eq!(prepared.n_active(), 0);
    assert!(prepared.regressors.is_empty());
    assert_eq!(prepared.weights, PreparedWeights::None);
    assert_eq!(prepared.adjustment, PreparedAdjustment::None);

    let candidate = [false, false];
    let outcome = [1.0, 2.0];
    let no_candidate = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&outcome),
        regressors: &[],
        weights: WeightInput::None,
        adjustment: AdjustmentInput::None,
    };
    let prepared = prepare_numeric_sample(&no_candidate, &LiteralPolicy::default()).unwrap();
    assert_eq!(prepared.n_candidate, 0);
    assert_eq!(prepared.n_active(), 0);

    let candidate = [true, true];
    let missing = [f64::NAN, f64::INFINITY];
    let regressor = [10.0, 20.0];
    let regressors = [NumericColumnView::new(&regressor)];
    let weights = [1.0, 2.0];
    let exposure = [3.0, 4.0];
    let all_dropped = NumericPreparationInput {
        candidate: (&candidate[..]).into(),
        outcome: NumericColumnView::new(&missing),
        regressors: &regressors,
        weights: WeightInput::Provided {
            kind: WeightKind::Importance,
            column: NumericColumnView::new(&weights),
        },
        adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&exposure)),
    };
    let prepared = prepare_numeric_sample(&all_dropped, &LiteralPolicy::default()).unwrap();
    assert_eq!(prepared.n_candidate, 2);
    assert_eq!(prepared.n_active(), 0);
    assert_eq!(
        prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
        [DropReason::MissingOutcome; 2]
    );
    assert_eq!(prepared.regressors, [Vec::<f64>::new()]);
    assert_eq!(
        prepared.weights,
        PreparedWeights::Provided {
            kind: WeightKind::Importance,
            values: Vec::new(),
        }
    );
    assert_eq!(
        prepared.adjustment,
        PreparedAdjustment::Exposure(Vec::new())
    );
}

#[test]
fn every_value_and_mask_length_mismatch_is_rejected_before_policy_calls() {
    let candidate = [true, true];
    let good = [1.0, 2.0];
    let short = [1.0];
    let short_mask = [false];
    let cases = [
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&short),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::with_missing(&good, &short_mask),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[NumericColumnView::new(&short)],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[NumericColumnView::with_missing(&good, &short_mask)],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[],
            weights: WeightInput::Provided {
                kind: WeightKind::Frequency,
                column: NumericColumnView::new(&short),
            },
            adjustment: AdjustmentInput::None,
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[],
            weights: WeightInput::Provided {
                kind: WeightKind::Frequency,
                column: NumericColumnView::with_missing(&good, &short_mask),
            },
            adjustment: AdjustmentInput::None,
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::Offset(NumericColumnView::new(&short)),
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::Offset(NumericColumnView::with_missing(
                &good,
                &short_mask,
            )),
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::Exposure(NumericColumnView::new(&short)),
        },
        NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&good),
            regressors: &[],
            weights: WeightInput::None,
            adjustment: AdjustmentInput::Exposure(NumericColumnView::with_missing(
                &good,
                &short_mask,
            )),
        },
    ];
    for input in cases {
        let policy = LiteralPolicy::default();
        let error = prepare_numeric_sample(&input, &policy).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(policy.calls.borrow().is_empty());
    }
}

#[test]
fn deterministic_permutations_preserve_original_order_and_literal_oracle() {
    let base_y = [11.0, f64::NAN, 13.0, 14.0, 15.0, 16.0];
    let base_x = [21.0, 22.0, 23.0, f64::INFINITY, 25.0, 26.0];
    let base_candidate = [true, true, false, true, true, true];
    let permutations = [
        [0, 1, 2, 3, 4, 5],
        [5, 4, 3, 2, 1, 0],
        [2, 0, 5, 1, 4, 3],
        [1, 3, 5, 0, 2, 4],
    ];
    for permutation in permutations {
        let y = permutation.map(|index| base_y[index]);
        let x = permutation.map(|index| base_x[index]);
        let candidate = permutation.map(|index| base_candidate[index]);
        let regressors = [NumericColumnView::new(&x)];
        let input = NumericPreparationInput {
            candidate: (&candidate[..]).into(),
            outcome: NumericColumnView::new(&y),
            regressors: &regressors,
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        };
        let prepared = prepare_numeric_sample(&input, &LiteralPolicy::default()).unwrap();

        let mut expected_rows = Vec::new();
        let mut expected_reasons = Vec::new();
        for row in 0..candidate.len() {
            let reason = if !candidate[row] {
                DropReason::ExcludedByIfIn
            } else if !y[row].is_finite() {
                DropReason::MissingOutcome
            } else if !x[row].is_finite() {
                DropReason::MissingRegressor
            } else {
                expected_rows.push(row);
                DropReason::Retained
            };
            expected_reasons.push(reason);
        }
        assert_eq!(
            prepared.retained_rows().iter().collect::<Vec<_>>(),
            expected_rows
        );
        assert_eq!(
            prepared.drop_reasons.iter().copied().collect::<Vec<_>>(),
            expected_reasons
        );
        assert!(prepared
            .retained_rows()
            .iter()
            .zip(prepared.retained_rows().iter().skip(1))
            .all(|(left, right)| left < right));
    }
}

fn assert_bits(actual: &[f64], expected: &[f64]) {
    assert_eq!(
        actual
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
}

#[test]
fn candidate_uniform_raw_masks_preserve_domains_shapes_and_source_use_errors() {
    use fereg_core::data::UniformValues;
    let good = [1.0, 2.0, 3.0];
    let invalid = [f64::NAN, f64::INFINITY, -1.0];
    let sources = [NumericColumnView::new(&invalid)];
    for value in [false, true] {
        for case in 0..6 {
            let repeated = UniformValues::Repeated { len: 3, value };
            let explicit = vec![value; 3];
            let input = NumericPreparationInput {
                candidate: repeated.view(),
                outcome: NumericColumnView::new(if case == 1 { &good[..2] } else { &invalid }),
                regressors: &sources,
                weights: WeightInput::Provided {
                    kind: WeightKind::Analytic,
                    column: NumericColumnView::new(if case == 2 { &good[..2] } else { &invalid }),
                },
                adjustment: AdjustmentInput::Exposure(NumericColumnView::new(if case == 3 {
                    &good[..2]
                } else {
                    &invalid
                })),
            };
            let roles = [NumericSourceUse::ordinary_and_fixed_effect()];
            let uses = if case == 4 { &[][..] } else { &roles[..] };
            let first_policy = LiteralPolicy::default();
            let first = prepare_numeric_sample_with_source_uses(&input, uses, &first_policy);
            let second_policy = LiteralPolicy::default();
            let second = prepare_numeric_sample_with_source_uses(
                &NumericPreparationInput {
                    candidate: explicit.as_slice().into(),
                    ..input
                },
                uses,
                &second_policy,
            );
            assert_eq!(first, second, "value={value}, case={case}");
            assert_eq!(*first_policy.calls.borrow(), *second_policy.calls.borrow());
            if !value && (case == 0 || case == 5) {
                let sample = first.unwrap();
                assert_eq!(sample.n_candidate, 0);
                assert!(sample
                    .drop_reasons
                    .iter()
                    .all(|r| *r == DropReason::ExcludedByIfIn));
                assert!(first_policy.calls.borrow().is_empty());
            }
        }
    }
}
