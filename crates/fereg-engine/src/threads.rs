//! Call-owned worker lifecycle harvested from legacy parallel.rs at b1700e6d.
//! Reduction partitions are fixed by the input length, never the worker count.
use crate::error::{reserved, ErrorCode, FeregError, Result};
use rayon::prelude::*;
use std::ops::Range;

pub(crate) const REDUCTION_CHUNK: usize = 1024;

#[derive(Debug, Default)]
struct JoinedWorkers(Vec<std::thread::JoinHandle<()>>);

impl Drop for JoinedWorkers {
    fn drop(&mut self) {
        for handle in self.0.drain(..) {
            let _ = handle.join();
        }
    }
}

#[derive(Debug)]
pub struct ThreadTeam {
    pool: Option<rayon::ThreadPool>,
    workers: JoinedWorkers,
}

impl ThreadTeam {
    /// Zero resolves the native default, independent of Stata's licensed processor count.
    pub fn new(requested: usize) -> Result<Self> {
        let threads = if requested == 0 {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        } else {
            requested
        };
        let mut workers = JoinedWorkers(reserved(threads)?);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .spawn_handler(|thread| {
                workers
                    .0
                    .push(std::thread::Builder::new().spawn(|| thread.run())?);
                Ok(())
            })
            .build()
            .map_err(|error| {
                FeregError::new(
                    ErrorCode::Allocation,
                    format!("unable to build worker team: {error}"),
                )
            })?;
        Ok(Self {
            pool: Some(pool),
            workers,
        })
    }

    #[must_use]
    pub fn threads(&self) -> usize {
        self.pool.as_ref().expect("live team").current_num_threads()
    }

    pub(crate) fn is_worker(&self) -> bool {
        self.pool
            .as_ref()
            .expect("live team")
            .current_thread_index()
            .is_some()
    }

    pub fn install<T: Send>(&self, function: impl FnOnce() -> T + Send) -> T {
        self.pool.as_ref().expect("live team").install(function)
    }

    /// The scope's caller remains on its current thread; only spawned work enters the team.
    pub fn in_place_scope<'scope, T>(
        &self,
        function: impl FnOnce(&rayon::Scope<'scope>) -> T,
    ) -> T {
        self.pool
            .as_ref()
            .expect("live team")
            .in_place_scope(function)
    }

    /// Results have canonical partition order even when workers finish out of order.
    pub fn map_ranges<T: Default + Send, F>(
        &self,
        length: usize,
        chunk: usize,
        function: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(Range<usize>) -> T + Sync,
    {
        if chunk == 0 {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "partition width must be positive",
            ));
        }
        let count = length.div_ceil(chunk);
        let mut results = reserved(count)?;
        results.resize_with(count, T::default);
        self.install(|| {
            results
                .par_iter_mut()
                .enumerate()
                .for_each(|(index, value)| {
                    let start = index * chunk;
                    *value = function(start..start.saturating_add(chunk).min(length));
                });
        });
        Ok(results)
    }

    pub fn sum(&self, values: &[f64]) -> Result<f64> {
        let partials = self.map_ranges(values.len(), REDUCTION_CHUNK, |range| {
            compensated_sum(values[range].iter().copied())
        })?;
        Ok(compensated_sum(partials))
    }

    pub fn dot(&self, left: &[f64], right: &[f64]) -> Result<f64> {
        if left.len() != right.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "dot-product lengths differ",
            ));
        }
        let partials = self.map_ranges(left.len(), REDUCTION_CHUNK, |range| {
            compensated_sum(range.map(|index| left[index] * right[index]))
        })?;
        Ok(compensated_sum(partials))
    }
}

impl Drop for ThreadTeam {
    fn drop(&mut self) {
        let own_worker = self
            .pool
            .as_ref()
            .is_some_and(|pool| pool.current_thread_index().is_some());
        drop(self.pool.take());
        if own_worker {
            // External unscoped Rayon work may own the last handle. It cannot join itself.
            // Estimation uses scoped work and drops its call-owned team on the host thread.
            self.workers.0.clear();
        }
    }
}

#[derive(Default)]
pub(crate) struct CompensatedSum {
    sum: f64,
    correction: f64,
}
impl CompensatedSum {
    pub(crate) fn add(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
    }
    pub(crate) fn total(self) -> f64 {
        self.sum + self.correction
    }
}
pub(crate) fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = CompensatedSum::default();
    for value in values {
        sum.add(value);
    }
    sum.total()
}

pub(crate) fn blocked_sum(length: usize, value: impl Fn(usize) -> f64) -> f64 {
    compensated_sum((0..length).step_by(REDUCTION_CHUNK).map(|start| {
        compensated_sum((start..start.saturating_add(REDUCTION_CHUNK).min(length)).map(&value))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reductions_and_partition_order_are_identical_across_worker_counts() {
        let values = (0..10_001)
            .map(|index| match index % 4 {
                0 => 1e16,
                1 => 1.0,
                2 => -1e16,
                _ => 1.0,
            })
            .collect::<Vec<_>>();
        let ones = vec![1.0; values.len()];
        let mut expected = None;
        for threads in [1, 2, 4, 8] {
            let team = ThreadTeam::new(threads).unwrap();
            let observed = (
                team.sum(&values).unwrap().to_bits(),
                team.dot(&values, &ones).unwrap().to_bits(),
            );
            assert_eq!(observed.0, observed.1);
            assert_eq!(observed.0, (1e16_f64 + 5000.0).to_bits());
            assert_eq!(team.sum(&[]).unwrap(), 0.0);
            assert!(team.dot(&values, &[]).is_err());
            assert_eq!(*expected.get_or_insert(observed), observed);
            let partitions = team
                .map_ranges(13, 4, |range| range.collect::<Vec<_>>())
                .unwrap();
            assert_eq!(
                partitions,
                vec![
                    vec![0, 1, 2, 3],
                    vec![4, 5, 6, 7],
                    vec![8, 9, 10, 11],
                    vec![12]
                ]
            );
        }
    }

    #[test]
    fn scope_stays_on_caller_and_team_joins_after_panic() {
        let caller = std::thread::current().id();
        let outcome = std::panic::catch_unwind(|| {
            let team = ThreadTeam::new(2).unwrap();
            team.in_place_scope(|scope| {
                assert_eq!(std::thread::current().id(), caller);
                scope.spawn(|_| panic!("injected worker panic"));
            });
        });
        assert!(outcome.is_err());
        let team = ThreadTeam::new(3).unwrap();
        assert_eq!(team.install(rayon::current_num_threads), 3);
        assert!(team.map_ranges(4, 0, |_| 0usize).is_err());
    }
}
