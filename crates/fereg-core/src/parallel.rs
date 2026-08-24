use crate::options::ThreadConfig;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadTeam {
    threads: usize,
    deterministic: bool,
}

impl ThreadTeam {
    #[must_use]
    pub fn new(config: ThreadConfig) -> Self {
        Self {
            threads: config.resolved(),
            deterministic: config.deterministic,
        }
    }

    #[must_use]
    pub const fn threads(self) -> usize {
        self.threads
    }

    #[must_use]
    pub const fn deterministic(self) -> bool {
        self.deterministic
    }

    #[must_use]
    pub fn ranges(self, len: usize, minimum_chunk: usize) -> Vec<Range<usize>> {
        if len == 0 {
            return Vec::new();
        }
        let minimum_chunk = minimum_chunk.max(1);
        let useful_workers = len.div_ceil(minimum_chunk).max(1);
        let workers = self.threads.min(useful_workers).min(len).max(1);
        let base = len / workers;
        let remainder = len % workers;
        let mut start = 0usize;
        (0..workers)
            .map(|worker| {
                let width = base + usize::from(worker < remainder);
                let range = start..start + width;
                start += width;
                range
            })
            .collect()
    }

    pub fn map_ranges<T, F>(self, len: usize, minimum_chunk: usize, function: F) -> Vec<T>
    where
        T: Send,
        F: Fn(usize, Range<usize>) -> T + Sync,
    {
        let ranges = self.ranges(len, minimum_chunk);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(ranges.len());
            for (worker, range) in ranges.into_iter().enumerate() {
                let function = &function;
                handles.push(scope.spawn(move || function(worker, range)));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().expect("fereg worker thread panicked"))
                .collect()
        })
    }

    #[must_use]
    pub fn xor_checksum(self, values: &[u64]) -> u64 {
        let partials = self.map_ranges(values.len(), 1_024, |worker, range| {
            let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ worker as u64;
            let start = range.start;
            for (index, value) in values[range].iter().enumerate() {
                let absolute = start + index;
                state ^= value
                    .wrapping_add((absolute as u64).rotate_left((absolute % 63) as u32))
                    .rotate_left((absolute % 61) as u32);
            }
            (worker, state)
        });
        partials.into_iter().fold(0_u64, |state, (worker, value)| {
            state ^ value.rotate_left((worker % 63) as u32)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::ThreadConfig;

    #[test]
    fn ranges_cover_exactly_once() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(32, true).expect("config"));
        let ranges = team.ranges(101, 1);
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(101));
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn requested_32_threads_are_exercised_on_large_work() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(32, true).expect("config"));
        assert_eq!(team.ranges(100_000, 1_024).len(), 32);
    }

    #[test]
    fn map_preserves_worker_order() {
        let team = ThreadTeam::new(ThreadConfig::from_requested(8, true).expect("config"));
        let result = team.map_ranges(80, 10, |worker, range| (worker, range));
        for (expected, (worker, _)) in result.into_iter().enumerate() {
            assert_eq!(expected, worker);
        }
    }
}
