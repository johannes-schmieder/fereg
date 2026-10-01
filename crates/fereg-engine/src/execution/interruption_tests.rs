use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn nested_work_polls_when_each_loop_is_short_and_preserves_value_and_error_identity() {
    for batched in [false, true] {
        let checkpoints = every_checkpoint(|poll| {
            let mut work = WorkPoll::new(poll)?;
            let mut value = 0;
            for row in 0..37 {
                for column in 0..31 {
                    value += row + column;
                    if !batched {
                        work.step()?;
                    }
                }
                if batched {
                    work.advance(31)?;
                }
            }
            check(poll)?;
            Ok(value)
        });
        assert_eq!(checkpoints, 3);
    }
    let calls = AtomicUsize::new(0);
    let observe = || {
        calls.fetch_add(1, Ordering::Relaxed);
        true
    };
    let mut work = WorkPoll::new(Some(&observe)).unwrap();
    work.advance(usize::MAX).unwrap();
    work.step().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

pub(crate) fn every_checkpoint<T: std::fmt::Debug + PartialEq>(
    run: impl Fn(Poll<'_>) -> Result<T>,
) -> usize {
    let expected = run(None).unwrap();
    let count = AtomicUsize::new(0);
    let observe = || {
        count.fetch_add(1, Ordering::Relaxed);
        true
    };
    assert_eq!(run(Some(&observe)).unwrap(), expected);
    let total = count.load(Ordering::Relaxed);
    assert!(total > 0);
    for stop in 1..=total {
        count.store(0, Ordering::Relaxed);
        let cancel = || count.fetch_add(1, Ordering::Relaxed) + 1 < stop;
        assert_eq!(run(Some(&cancel)).unwrap_err().code, ErrorCode::Interrupted);
        assert_eq!(count.load(Ordering::Relaxed), stop);
        assert_eq!(run(None).unwrap(), expected);
    }
    total
}

#[test]
fn fallible_blocked_sums_preserve_fixed_partitions_and_propagate_value_errors() {
    let values: Vec<_> = (0..4099)
        .map(|row| [1e16, 1., -1e16, -0.25, 4.][row % 5])
        .collect();
    let expected = crate::threads::compensated_sum(
        values
            .chunks(REDUCTION_CHUNK)
            .map(|chunk| crate::threads::compensated_sum(chunk.iter().copied())),
    );
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        assert_eq!(
            team.install(|| every_checkpoint(|poll| {
                let result = try_blocked_sum(values.len(), |row| Ok(values[row]), poll)?;
                assert_eq!(result.to_bits(), expected.to_bits());
                Ok(result)
            })),
            values.len().div_ceil(REDUCTION_CHUNK)
        );
    }
    let failure = FeregError::new(ErrorCode::NonConvergence, "fallible mean value r(430)");
    assert_eq!(
        try_blocked_sum(
            values.len(),
            |row| {
                if row == 1025 {
                    Err(failure.clone())
                } else {
                    Ok(values[row])
                }
            },
            None
        )
        .unwrap_err(),
        failure
    );
}
