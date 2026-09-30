//! Cooperative cancellation inside the existing standard-library sort.
use super::{check, check_chunk, interrupted, Poll};
use crate::Result;
use std::{
    cmp::Ordering,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
};

struct SortInterrupted;

pub(crate) fn sort_unstable_by<T>(
    values: &mut [T],
    mut compare: impl FnMut(&T, &T) -> Ordering,
    poll: Poll<'_>,
) -> Result<()> {
    check(poll)?;
    let mut comparisons = 0usize;
    // Slice sort preserves every element on unwind. A private zero-sized tag
    // exits its infallible comparator without allocating a second order bank
    // or invoking a host panic hook; unrelated panics retain their payload.
    let result = catch_unwind(AssertUnwindSafe(|| {
        values.sort_unstable_by(|a, b| {
            comparisons = comparisons.wrapping_add(1);
            if check_chunk(poll, comparisons).is_err() {
                resume_unwind(Box::new(SortInterrupted));
            }
            compare(a, b)
        });
    }));
    match result {
        Ok(()) => check(poll),
        Err(payload) if payload.is::<SortInterrupted>() => Err(interrupted()),
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
mod tests;
