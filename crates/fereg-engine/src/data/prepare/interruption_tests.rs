use super::*;
use crate::data::WeightKind;
use crate::{execution::interruption_tests::every_checkpoint, threads::ThreadTeam};

struct Policy;
impl NumericDomainPolicy for Policy {
    fn outcome(&self, _: usize, y: f64) -> Result<DomainDecision> {
        Ok(if y == -1. {
            DomainDecision::Drop
        } else {
            DomainDecision::Retain
        })
    }
    fn weight(&self, row: usize, _: WeightKind, w: f64) -> Result<DomainDecision> {
        if w < 0. {
            Err(FeregError::new(
                ErrorCode::NegativeWeight,
                format!("negative weight at {row}"),
            ))
        } else {
            Ok(if w == 0. {
                DomainDecision::Drop
            } else {
                DomainDecision::Retain
            })
        }
    }
    fn offset(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn exposure(&self, _: usize, e: f64) -> Result<DomainDecision> {
        Ok(if e == 0. {
            DomainDecision::Drop
        } else {
            DomainDecision::Retain
        })
    }
}

#[test]
fn numeric_preparation_cancels_every_checkpoint_with_exact_availability_and_policy_precedence() {
    let n = 4099;
    let mut candidate = vec![true; n];
    candidate[0] = false;
    let mut outcome = vec![1.; n];
    outcome[1] = f64::NAN;
    outcome[2] = -1.;
    let mut x = vec![2.; n];
    x[3] = f64::NAN;
    let mut fe = vec![3.; n];
    fe[4] = f64::NAN;
    fe[5] = f64::NAN;
    let mut cluster = vec![4.; n];
    cluster[5] = f64::NAN;
    cluster[6] = f64::NAN;
    let mut w = vec![1.; n];
    w[3] = -1.;
    w[4] = f64::NAN;
    let mut offset = vec![0.; n];
    offset[6] = f64::NAN;
    offset[7] = f64::NAN;
    let sources = [
        NumericColumnView::new(&x),
        NumericColumnView::new(&fe),
        NumericColumnView::new(&cluster),
    ];
    let uses = [
        NumericSourceUse::ordinary(),
        NumericSourceUse::fixed_effect(),
        NumericSourceUse {
            ordinary: false,
            fixed_effect: false,
            cluster: true,
        },
    ];
    let input = NumericPreparationInput {
        candidate: &candidate,
        outcome: NumericColumnView::new(&outcome),
        regressors: &sources,
        weights: WeightInput::Provided {
            kind: WeightKind::Analytic,
            column: NumericColumnView::new(&w),
        },
        adjustment: AdjustmentInput::Offset(NumericColumnView::new(&offset)),
    };
    let expected = prepare_sample(&input, Some(&uses), &Policy, None).unwrap();
    assert_eq!(
        &expected.drop_reasons[..8],
        &[
            DropReason::ExcludedByIfIn,
            DropReason::MissingOutcome,
            DropReason::OtherModelExclusion,
            DropReason::MissingRegressor,
            DropReason::InvalidOrMissingWeight,
            DropReason::MissingFixedEffect,
            DropReason::MissingCluster,
            DropReason::MissingOffsetOrExposure
        ]
    );
    assert_eq!(expected.original_row_by_active, (8..n).collect::<Vec<_>>());
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let count = team.install(|| {
            every_checkpoint(|poll| prepare_sample(&input, Some(&uses), &Policy, poll))
        });
        assert!(count > 5);
        let wide_values = vec![2.; 3];
        let wide = vec![NumericColumnView::new(&wide_values); 2053];
        let wide_uses = vec![NumericSourceUse::ordinary_and_fixed_effect(); wide.len()];
        let input = NumericPreparationInput {
            candidate: &[true; 3],
            outcome: NumericColumnView::new(&[1.; 3]),
            regressors: &wide,
            weights: WeightInput::None,
            adjustment: AdjustmentInput::None,
        };
        let columns = team.install(|| {
            every_checkpoint(|poll| prepare_sample(&input, Some(&wide_uses), &Policy, poll))
        });
        assert!(columns > 20);
        assert_eq!(*reference.get_or_insert((count, columns)), (count, columns));
    }
}

#[test]
fn malformed_preparation_and_policy_errors_keep_their_original_identity() {
    let n = 2053;
    let observe = || true;
    for (candidate, y, w) in [
        (vec![true; n], vec![1.; n], vec![-1.; n]),
        (vec![true; n], vec![1.; 2], vec![1.; n]),
    ] {
        let input = NumericPreparationInput {
            candidate: &candidate,
            outcome: NumericColumnView::new(&y),
            regressors: &[],
            weights: WeightInput::Provided {
                kind: WeightKind::Analytic,
                column: NumericColumnView::new(&w),
            },
            adjustment: AdjustmentInput::None,
        };
        assert_eq!(
            prepare_sample(&input, None, &Policy, None).unwrap_err(),
            prepare_sample(&input, None, &Policy, Some(&observe)).unwrap_err()
        );
    }
    every_checkpoint(|poll| {
        prepare_sample(
            &NumericPreparationInput {
                candidate: &[],
                outcome: NumericColumnView::new(&[]),
                regressors: &[],
                weights: WeightInput::None,
                adjustment: AdjustmentInput::None,
            },
            None,
            &Policy,
            poll,
        )
    });
}
