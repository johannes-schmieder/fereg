use super::*;
use crate::{
    data::{DomainDecision, DropReason, LoadedAdjustment, NumericColumn, WeightKind},
    execution::interruption_tests::every_checkpoint,
    threads::ThreadTeam,
};
use std::{cell::Cell, rc::Rc};

struct Policy;
impl NumericDomainPolicy for Policy {
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
fn fixture(n: usize) -> (SourcePlan, Loaded) {
    let plan = SourcePlan {
        uses: vec![
            NumericSourceUse::ordinary(),
            NumericSourceUse::fixed_effect(),
        ],
        ordinary_categorical: vec![true, false],
        numeric_required: vec![true, false],
    };
    let loaded = Loaded {
        candidate: (0..n).map(|i| i % 17 != 0).collect(),
        outcome: NumericColumn {
            values: vec![1.; n],
            missing: None,
        },
        sources: vec![
            LoadedSource::Numeric(NumericColumn {
                values: (0..n).map(|i| (i % 3) as f64).collect(),
                missing: None,
            }),
            LoadedSource::Strings(
                (0..n)
                    .map(|i| {
                        if i % 31 == 0 {
                            String::new()
                        } else {
                            format!("Ω{:05}", (i * 37) % 2053)
                        }
                    })
                    .collect(),
            ),
        ],
        weights: None,
        adjustment: LoadedAdjustment::Offset(NumericColumn {
            values: vec![0.; n],
            missing: None,
        }),
    };
    (plan, loaded)
}

#[test]
fn shared_source_preparation_cancels_string_encoding_compaction_and_factor_validation() {
    let (plan, loaded) = fixture(4099);
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let count =
            team.install(|| every_checkpoint(|poll| plan.prepare_polled(&loaded, &Policy, poll)));
        assert!(count > 30);
        assert_eq!(*reference.get_or_insert(count), count);
    }
    let sample = plan.prepare(&loaded, &Policy).unwrap();
    let LoadedSource::Strings(strings) = &loaded.sources[1] else {
        unreachable!()
    };
    let mut levels = strings.clone();
    levels.sort_unstable();
    levels.dedup();
    for (row, &original) in sample.original_row_by_active.iter().enumerate() {
        assert_eq!(
            sample.regressors[1][row] as usize,
            levels.binary_search(&strings[original]).unwrap()
        );
    }
    assert_eq!(sample.drop_reasons[0], DropReason::ExcludedByIfIn);
    assert_eq!(sample.drop_reasons[31], DropReason::MissingFixedEffect);
}

#[test]
fn factor_errors_remain_after_availability_and_caller_cancellation_retains_host_error() {
    let (plan, mut loaded) = fixture(2053);
    let LoadedSource::Numeric(column) = &mut loaded.sources[0] else {
        unreachable!()
    };
    column.values[0] = -1.;
    column.values[1] = -1.;
    let original = plan.prepare(&loaded, &Policy).unwrap_err();
    assert_eq!(original.code, ErrorCode::InvalidFactor);
    assert!(original.message.contains("observation 2"));
    let observe = || true;
    assert_eq!(
        plan.prepare_polled(&loaded, &Policy, Some(&observe))
            .unwrap_err(),
        original
    );
    let (plan, loaded) = fixture(4099);
    let expected = plan.prepare(&loaded, &Policy).unwrap();
    let caller = std::thread::current().id();
    let seen = Rc::new(Cell::new(0));
    let sentinel = FeregError::new(ErrorCode::NonConvergence, "preparation host r(430)");
    for workers in [1, 8] {
        seen.set(0);
        let team = ThreadTeam::new(workers).unwrap();
        let error = execution::run(
            &team,
            |relay| {
                let poll = || relay(());
                plan.prepare_polled(&loaded, &Policy, Some(&poll))
            },
            &mut |_| {
                assert_eq!(std::thread::current().id(), caller);
                seen.set(seen.get() + 1);
                if seen.get() == 4 {
                    Err(sentinel.clone())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error, sentinel);
        assert_eq!(seen.get(), 4);
        assert_eq!(plan.prepare(&loaded, &Policy).unwrap(), expected);
    }
}
