use super::*;
use crate::threads::ThreadTeam;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

fn every_checkpoint<T: std::fmt::Debug + PartialEq>(run: impl Fn(Poll<'_>) -> Result<T>) -> usize {
    let expected = run(None).unwrap();
    let count = AtomicUsize::new(0);
    let observe = || {
        count.fetch_add(1, AtomicOrdering::Relaxed);
        true
    };
    assert_eq!(run(Some(&observe)).unwrap(), expected);
    let total = count.load(AtomicOrdering::Relaxed);
    for stop in 1..=total {
        count.store(0, AtomicOrdering::Relaxed);
        let cancel = || count.fetch_add(1, AtomicOrdering::Relaxed) + 1 < stop;
        assert_eq!(run(Some(&cancel)).unwrap_err().code, ErrorCode::Interrupted);
        assert_eq!(count.load(AtomicOrdering::Relaxed), stop);
        assert_eq!(run(None).unwrap(), expected);
    }
    total
}

#[test]
fn numeric_and_tuple_coding_cancel_every_checkpoint_without_changing_canonical_codes() {
    let n = 4099;
    let values: Vec<_> = (0..n)
        .map(|i| match i % 11 {
            0 => -0.,
            1 => 0.,
            _ => ((i * 31) % n) as f64 / 8. - 256.,
        })
        .collect();
    let first: Vec<_> = (0..n).map(|i| ((i * 31) % 2053) as u32).collect();
    let second: Vec<_> = (0..n).map(|i| ((i * 17) % 31) as u32).collect();
    let views = [
        CategoricalCodeView::new(&first, 2053),
        CategoricalCodeView::new(&second, 31),
        CategoricalCodeView::new(&first, 2053),
    ];
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let numeric = team.install(|| every_checkpoint(|poll| factorize_f64_polled(&values, poll)));
        let tuples = team
            .install(|| every_checkpoint(|poll| factorize_code_interaction_polled(&views, poll)));
        assert!(numeric > 20 && tuples > 20);
        assert_eq!(
            *reference.get_or_insert((numeric, tuples)),
            (numeric, tuples)
        );
    }
    let encoded = factorize_f64(&values).unwrap();
    assert_eq!(encoded.codes[0], encoded.codes[1]);
    assert!(encoded.levels.windows(2).all(|v| v[0] < v[1]));
    let encoded = factorize_code_interaction(&views).unwrap();
    let pairs: Vec<_> = first.iter().copied().zip(second.iter().copied()).collect();
    let mut levels = pairs.clone();
    levels.sort_unstable();
    levels.dedup();
    assert_eq!(encoded.n_levels, levels.len());
    for (row, pair) in pairs.iter().enumerate() {
        assert_eq!(
            encoded.codes[row] as usize,
            levels.binary_search(pair).unwrap()
        );
    }
    every_checkpoint(|poll| factorize_f64_polled(&[], poll));
    every_checkpoint(|poll| {
        factorize_code_interaction_polled(&[CategoricalCodeView::new(&[], 0)], poll)
    });
}

#[test]
fn string_coding_cancels_every_checkpoint_with_identical_owned_canonical_levels() {
    let values: Vec<_> = (0..4099)
        .map(|i| {
            if i % 13 == 0 {
                String::new()
            } else {
                format!("Ω{:05}", (i * 37) % 2053)
            }
        })
        .collect();
    let mut reference = None;
    for workers in [1, 8] {
        let team = ThreadTeam::new(workers).unwrap();
        let count =
            team.install(|| every_checkpoint(|poll| factorize_strings_polled(&values, poll)));
        assert!(count > 20);
        assert_eq!(*reference.get_or_insert(count), count);
    }
    let encoded = factorize_strings(&values).unwrap();
    assert!(encoded
        .levels
        .windows(2)
        .all(|levels| levels[0] < levels[1]));
    for (row, value) in values.iter().enumerate() {
        assert_eq!(
            encoded.codes[row] as usize,
            encoded.levels.binary_search(value).unwrap()
        );
    }
}
