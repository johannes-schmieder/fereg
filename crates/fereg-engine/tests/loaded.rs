#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
use fereg_engine::{data::*, ErrorCode, Result};
struct Retain;
impl NumericDomainPolicy for Retain {
    fn outcome(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn weight(&self, _: usize, _: WeightKind, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn offset(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
    fn exposure(&self, _: usize, _: f64) -> Result<DomainDecision> {
        Ok(DomainDecision::Retain)
    }
}
fn column(values: &[f64]) -> NumericColumn {
    NumericColumn {
        values: values.to_vec(),
        missing: None,
    }
}
fn fixture() -> Loaded {
    Loaded {
        candidate: vec![false, true, true, true, true],
        outcome: column(&[0., 1., 2., 3., 4.]),
        sources: vec![
            LoadedSource::Strings(["", "b", "", "a#b", "a"].map(str::to_owned).to_vec()),
            LoadedSource::Numeric(column(&[0., 10., 20., 30., 40.])),
        ],
        weights: Some((WeightKind::Probability, column(&[1., 2., 3., 4., 5.]))),
        adjustment: LoadedAdjustment::Exposure(column(&[1., 2., 3., 4., 5.])),
    }
}
#[test]
fn string_and_numeric_banks_share_the_ledger_and_stable_compaction() {
    let input = fixture();
    let uses = [
        NumericSourceUse::fixed_effect(),
        NumericSourceUse::ordinary(),
    ];
    let sample = input.prepare(&uses, &Retain).unwrap();
    assert_eq!(sample.retained_rows(), &[1, 3, 4]);
    assert_eq!(sample.drop_reasons[2], DropReason::MissingFixedEffect);
    assert_eq!(
        sample.regressors,
        vec![vec![3., 2., 1.], vec![10., 30., 40.]]
    );
    let sample = sample
        .compact_active_rows(&[true, false, true], DropReason::Singleton)
        .unwrap()
        .compact_active_rows(&[false, true], DropReason::MuSeparation)
        .unwrap();
    assert_eq!(sample.retained_rows(), &[4]);
    assert_eq!(sample.outcome, vec![4.]);
    assert_eq!(sample.regressors, vec![vec![1.], vec![40.]]);
    assert_eq!(sample.n_candidate, 4);
    assert_eq!(
        sample.drop_reasons,
        vec![
            DropReason::ExcludedByIfIn,
            DropReason::MuSeparation,
            DropReason::MissingFixedEffect,
            DropReason::Singleton,
            DropReason::Retained
        ]
    );
    assert_eq!(
        sample.weights,
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            values: vec![5.]
        }
    );
    assert_eq!(sample.adjustment, PreparedAdjustment::Exposure(vec![5.]));
    sample.validate_alignment().unwrap();
    assert_eq!(input.outcome.values.len(), 5);
}
#[test]
fn loaded_rejects_wrong_lengths_roles_and_string_regressors() {
    let mut input = fixture();
    assert_eq!(
        input.prepare(&[], &Retain).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        input
            .prepare(&[NumericSourceUse::ordinary(); 2], &Retain)
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    let uses = [
        NumericSourceUse::fixed_effect(),
        NumericSourceUse::ordinary(),
    ];
    input.sources[0] = LoadedSource::Strings(vec![]);
    assert_eq!(
        input.prepare(&uses, &Retain).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}
#[test]
fn compaction_rejects_misaligned_or_inconsistent_ledgers() {
    let input = fixture();
    let uses = [
        NumericSourceUse::fixed_effect(),
        NumericSourceUse::ordinary(),
    ];
    for corrupt in 0..7 {
        let mut sample = input.prepare(&uses, &Retain).unwrap();
        match corrupt {
            0 => {
                sample.outcome.pop();
            }
            1 => {
                sample.original_row_by_active[2] = 1;
            }
            2 => {
                sample.original_row_by_active[2] = 9;
            }
            3 => {
                sample.regressors[0].clear();
            }
            4 => {
                sample.drop_reasons[0] = DropReason::Retained;
            }
            5 => {
                sample.n_candidate = 0;
            }
            _ => {
                sample.drop_reasons.pop();
            }
        }
        assert_eq!(
            sample
                .compact_active_rows(&[true; 3], DropReason::Singleton)
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
    }
}
#[test]
fn intercept_only_and_overflowing_shapes_are_explicit() {
    let builder = ModelMatrixBuilder::with_nrows(3, &[], &[]).unwrap();
    assert_eq!(
        builder
            .generate(&[ColumnRecipe::Intercept])
            .unwrap()
            .as_slice(),
        &[1.; 3]
    );
    assert!(ModelMatrixBuilder::with_nrows(3, &[vec![1.]], &[]).is_err());
    assert_eq!(
        DenseMatrix::zeros(usize::MAX, 2).unwrap_err().code,
        ErrorCode::Allocation
    );
}
