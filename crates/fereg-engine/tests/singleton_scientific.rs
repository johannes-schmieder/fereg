#![forbid(unsafe_code)]
#![deny(clippy::too_many_lines)]
use fereg_engine::{topology::*, ErrorCode};
#[test]
fn drops_singletons_to_fixed_point() {
    let first = vec![0, 0, 1, 2];
    let second = vec![0, 1, 1, 2];
    let result = prune_singletons(
        &[&first, &second],
        &[3, 3],
        &[true, true, true, true],
        false,
    )
    .expect("singleton pruning");
    assert_eq!(result.active, vec![false, false, false, false]);
    assert_eq!(result.dropped_observations.len(), 4);
    assert_eq!(result.cascade_events, 4);
}
#[test]
fn respects_keep_singletons() {
    let codes = vec![0, 1];
    let result = prune_singletons(&[&codes], &[2], &[true, true], true).expect("kept");
    assert_eq!(result.active, vec![true, true]);
    assert!(result.dropped_observations.is_empty());
}
#[test]
fn rejects_invalid_codes() {
    let codes = vec![0, 2];
    let error = prune_singletons(&[&codes], &[2], &[true, true], false).expect_err("invalid code");
    assert_eq!(error.code, ErrorCode::InvalidData);
}
#[test]
fn frequency_multiplicity_changes_singleton_status() {
    let codes = vec![0, 1];
    let frequency = vec![2.0, 1.0];
    let result = prune_singletons_with_multiplicity(
        &[&codes],
        &[2],
        &[true, true],
        SingletonMultiplicity::Frequency(&frequency),
        false,
    )
    .expect("frequency singleton pruning");
    assert_eq!(result.active, vec![true, false]);
    assert_eq!(result.dropped_observations, vec![1]);
}
#[test]
fn inactive_frequency_rows_do_not_contribute() {
    let codes = vec![0, 0, 1];
    let frequency = vec![1.0, 100.0, 2.0];
    let result = prune_singletons_with_multiplicity(
        &[&codes],
        &[2],
        &[true, false, true],
        SingletonMultiplicity::Frequency(&frequency),
        false,
    )
    .expect("frequency singleton pruning");
    assert_eq!(result.active, vec![false, false, true]);
    assert_eq!(result.dropped_observations, vec![0]);
}
#[test]
fn rejects_noninteger_frequency_multiplicity() {
    let codes = vec![0, 0];
    let frequency = vec![1.5, 1.0];
    let error = prune_singletons_with_multiplicity(
        &[&codes],
        &[1],
        &[true, true],
        SingletonMultiplicity::Frequency(&frequency),
        false,
    )
    .expect_err("invalid frequency multiplicity");
    assert_eq!(error.code, ErrorCode::InvalidData);
}
#[test]
fn rejects_overflowing_level_totals_before_allocation() {
    let active = [true];
    let codes = [0_u32];
    let effects: [&[u32]; 2] = [&codes, &codes];
    let error = prune_singletons_with_multiplicity(
        &effects,
        &[usize::MAX, 1],
        &active,
        SingletonMultiplicity::Unit,
        false,
    )
    .expect_err("overflowing singleton level totals");
    assert_eq!(error.code, ErrorCode::Allocation);
}
