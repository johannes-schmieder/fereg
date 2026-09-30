use super::*;
use crate::{threads::ThreadTeam, ErrorCode};
use std::sync::{
    atomic::{AtomicUsize, Ordering as AtomicOrdering},
    Arc,
};

struct Item {
    key: usize,
    dropped: Arc<AtomicUsize>,
}
impl Drop for Item {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

#[test]
fn cancellation_inside_sort_preserves_every_owned_element_and_fresh_retries() {
    let n = 2053;
    let dropped = Arc::new(AtomicUsize::new(0));
    let make = || {
        (0..n)
            .map(|i| Item {
                key: (i * 37) % n,
                dropped: dropped.clone(),
            })
            .collect::<Vec<_>>()
    };
    let count = AtomicUsize::new(0);
    let observe = || {
        count.fetch_add(1, AtomicOrdering::Relaxed);
        true
    };
    let mut original = make();
    sort_unstable_by(&mut original, |a, b| a.key.cmp(&b.key), Some(&observe)).unwrap();
    assert!(original.iter().enumerate().all(|(i, value)| i == value.key));
    let total = count.load(AtomicOrdering::Relaxed);
    assert!(total > 4);
    drop(original);
    for stop in 1..=total {
        let mut values = make();
        count.store(0, AtomicOrdering::Relaxed);
        let cancel = || count.fetch_add(1, AtomicOrdering::Relaxed) + 1 < stop;
        assert_eq!(
            sort_unstable_by(&mut values, |a, b| a.key.cmp(&b.key), Some(&cancel))
                .unwrap_err()
                .code,
            ErrorCode::Interrupted
        );
        assert_eq!(count.load(AtomicOrdering::Relaxed), stop);
        // Unwind may reorder the private work buffer but must lose no owner.
        sort_unstable_by(&mut values, |a, b| a.key.cmp(&b.key), None).unwrap();
        assert!(values.iter().enumerate().all(|(i, value)| i == value.key));
        drop(values);
        assert_eq!(dropped.load(AtomicOrdering::Relaxed), n * (stop + 1));
    }
}

#[test]
fn real_comparator_and_poll_panics_keep_the_original_payload() {
    let sentinel = Arc::new(17usize);
    for from_poll in [false, true] {
        let mut values: Vec<_> = (0..4099).map(|i| (i * 31) % 4099).collect();
        let count = AtomicUsize::new(0);
        let poll = || {
            if from_poll && count.fetch_add(1, AtomicOrdering::Relaxed) == 1 {
                resume_unwind(Box::new(sentinel.clone()));
            }
            true
        };
        let payload = catch_unwind(AssertUnwindSafe(|| {
            sort_unstable_by(
                &mut values,
                |a, b| {
                    if !from_poll {
                        resume_unwind(Box::new(sentinel.clone()));
                    }
                    a.cmp(b)
                },
                Some(&poll),
            )
        }))
        .unwrap_err();
        assert!(Arc::ptr_eq(
            &sentinel,
            &payload.downcast::<Arc<usize>>().unwrap()
        ));
        sort_unstable_by(&mut values, Ord::cmp, None).unwrap();
        assert!(values.iter().enumerate().all(|(i, &value)| i == value));
    }
}

#[test]
fn sort_relay_cancels_on_the_caller_and_preserves_host_error_identity() {
    use std::{cell::Cell, rc::Rc};
    let caller = std::thread::current().id();
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let calls = Rc::new(Cell::new(0));
        let sentinel = crate::FeregError::new(ErrorCode::NonConvergence, "sort host r(430)");
        let result = crate::execution::run(
            &team,
            |relay| {
                let mut values: Vec<_> = (0..4099).map(|i| (i * 31) % 4099).collect();
                sort_unstable_by(&mut values, Ord::cmp, Some(&|| relay(())))?;
                Ok(values)
            },
            &mut |()| {
                assert_eq!(std::thread::current().id(), caller);
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    Err(sentinel.clone())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err(), sentinel);
        assert_eq!(calls.get(), 2);
        assert_eq!(team.install(|| 23), 23);
    }
}
