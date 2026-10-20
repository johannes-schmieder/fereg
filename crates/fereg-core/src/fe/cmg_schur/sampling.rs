//! One-time weighted clique-tree sampling for the CMG preconditioning graph.

use crate::error::{allocation, invalid_argument, invalid_data, Result};
use crate::linalg::neumaier_add;
use crate::memory::vec_capacity_bytes;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(in crate::fe) struct Sampling {
    pub(in crate::fe) samples: u32,
    pub(in crate::fe) seed: u64,
}

impl Sampling {
    pub(in crate::fe) fn validate(self) -> Result<()> {
        if self.seed > (1_u64 << 53) - 1 || (self.samples == 0 && self.seed != 0) {
            return Err(invalid_argument(
                "CMG sampling seed must be an integer in 0..2^53-1 and requires positive samples",
            ));
        }
        Ok(())
    }
}

// SplitMix64's standard counter increment and output permutation. Derivation
// domains pin independent worker/tree streams without dependence on traversal
// scheduling, other workers' degrees, or how many draws earlier trees used.
const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
const WORKER_DOMAIN: u64 = 0x434d_4753_574f_524b;
const TREE_DOMAIN: u64 = 0x434d_4753_5452_4545;

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn stream(seed: u64, worker: u64, tree: u32) -> Self {
        Self(mix(mix(seed ^ WORKER_DOMAIN) ^ worker) ^ mix(TREE_DOMAIN ^ u64::from(tree)))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GOLDEN_GAMMA);
        mix(self.0)
    }

    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }

    fn indexed_uniform(seed: u64, worker: u64, tree: u32, vertex: usize) -> f64 {
        let initial = Self::stream(seed, worker, tree).0;
        let counter = GOLDEN_GAMMA.wrapping_mul((vertex as u64).wrapping_add(1));
        (mix(initial.wrapping_add(counter)) >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }
}

/// One independent draw, with exactly the serial stream's counter and arithmetic.
/// The caller has already sorted the row and formed its compensated suffix.
pub(super) fn indexed_edge(
    sampling: Sampling,
    worker: usize,
    cells: &[(u32, f64)],
    suffix: &[f64],
    ordinal: usize,
) -> (usize, usize, f64) {
    let tree = (ordinal / (cells.len() - 1)) as u32;
    let vertex = ordinal % (cells.len() - 1);
    let tail = suffix[vertex + 1];
    let uniform = SplitMix64::indexed_uniform(sampling.seed, worker as u64, tree, vertex);
    let target = (uniform * tail).min(f64::from_bits(tail.to_bits() - 1));
    let mut lower = vertex + 1;
    let mut upper = cells.len() - 1;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if target < suffix[middle + 1] {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    let weight = cells[vertex].1 * (tail / suffix[0]) / f64::from(sampling.samples);
    (cells[vertex].0 as usize, cells[lower].0 as usize, weight)
}

/// Sort one sampled worker's cells by weight and form their compensated
/// suffix masses in reverse order (`suffix[cells.len()]` is zero). The serial
/// sampler and team lanes share this one implementation.
pub(super) fn prepare_row<C: super::bounded::Counter>(
    cells: &mut [(u32, f64)],
    suffix: &mut [f64],
    counter: &mut C,
) -> std::result::Result<(), C::Error> {
    super::bounded::counted_sort(counter, cells, |left, right| {
        left.1.total_cmp(&right.1).then(left.0.cmp(&right.0))
    })?;
    suffix[cells.len()] = 0.0;
    let mut sum = 0.0;
    let mut correction = 0.0;
    for (index, &(_, weight)) in cells.iter().enumerate().rev() {
        counter.step(64)?;
        if !weight.is_finite() || weight <= 0.0 {
            return Err(invalid_data("CMG sampled worker weight is invalid").into());
        }
        neumaier_add(weight, &mut sum, &mut correction);
        suffix[index] = sum + correction;
        if !suffix[index].is_finite() || suffix[index] <= 0.0 {
            return Err(invalid_data("CMG sampled suffix mass is invalid").into());
        }
    }
    Ok(())
}

pub(super) struct SampledTrees {
    sampling: Sampling,
    suffix: Vec<f64>,
    worker: u64,
    tree: u32,
    vertex: usize,
    random: SplitMix64,
}

impl SampledTrees {
    #[cfg(test)]
    pub(super) fn suffix(&self, count: usize) -> &[f64] {
        &self.suffix[..=count]
    }

    pub(super) fn allocation_bytes(max_rows: usize) -> Result<usize> {
        super::bytes::<f64>(
            max_rows
                .checked_add(1)
                .ok_or_else(|| allocation("CMG sampler suffix length overflows"))?,
        )
    }

    /// The caller admits this exact suffix bank before constructing the sampler.
    pub(super) fn new(
        sampling: Sampling,
        max_rows: usize,
        work: &mut super::bounded::Work,
    ) -> super::bounded::Attempt<Self> {
        work.step(64)?;
        sampling.validate()?;
        let length = max_rows
            .checked_add(1)
            .ok_or_else(|| allocation("CMG sampler suffix length overflows"))?;
        Ok(Self {
            sampling,
            suffix: work.filled(length, 0.0)?,
            worker: 0,
            tree: 0,
            vertex: 0,
            random: SplitMix64(0),
        })
    }

    pub(super) fn prepare(
        &mut self,
        cells: &mut [(u32, f64)],
        worker: usize,
        work: &mut super::bounded::Work,
    ) -> super::bounded::Attempt<()> {
        work.step(64)?;
        if cells.len() < 2 || cells.len() >= self.suffix.len() {
            return Err(invalid_argument("CMG sampled worker exceeds its admitted scratch").into());
        }
        let length = cells.len();
        prepare_row(cells, &mut self.suffix[..=length], work)?;
        work.step(64)?;
        self.worker = u64::try_from(worker)
            .map_err(|_| allocation("CMG sampled worker identifier overflows"))?;
        self.tree = 0;
        self.vertex = 0;
        self.random = SplitMix64::stream(self.sampling.seed, self.worker, 0);
        Ok(())
    }

    pub(super) fn next(
        &mut self,
        cells: &[(u32, f64)],
        work: &mut super::bounded::Work,
    ) -> super::bounded::Attempt<Option<(usize, usize, f64)>> {
        work.step(128)?;
        if self.tree == self.sampling.samples {
            return Ok(None);
        }
        let tail = self.suffix[self.vertex + 1];
        let target = (self.random.uniform() * tail).min(f64::from_bits(tail.to_bits() - 1));
        let mut lower = self.vertex + 1;
        let mut upper = cells.len() - 1;
        while lower < upper {
            work.step(32)?;
            let middle = lower + (upper - lower) / 2;
            if target < self.suffix[middle + 1] {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        work.step(128)?;
        let weight =
            cells[self.vertex].1 * (tail / self.suffix[0]) / f64::from(self.sampling.samples);
        if !weight.is_finite() || weight <= 0.0 {
            return Err(invalid_data("CMG sampled edge weight is nonpositive or nonfinite").into());
        }
        let edge = (
            cells[self.vertex].0 as usize,
            cells[lower].0 as usize,
            weight,
        );
        self.vertex += 1;
        if self.vertex + 1 == cells.len() {
            self.vertex = 0;
            self.tree += 1;
            self.random = SplitMix64::stream(self.sampling.seed, self.worker, self.tree);
        }
        Ok(Some(edge))
    }

    pub(super) fn bytes(&self) -> u128 {
        vec_capacity_bytes(&self.suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::super::bounded::Work;
    use super::*;

    #[test]
    fn indexed_draws_and_edges_match_the_serial_stream_exactly() {
        for seed in [0, 17, (1_u64 << 53) - 1] {
            for worker in [0, 1, 913] {
                for tree in [0, 1, 31] {
                    let mut random = SplitMix64::stream(seed, worker, tree);
                    for vertex in 0..513 {
                        assert_eq!(
                            random.uniform().to_bits(),
                            SplitMix64::indexed_uniform(seed, worker, tree, vertex).to_bits(),
                        );
                    }
                }
                for count in [2, 3, 6, 17, 257] {
                    let sampling = Sampling { samples: 3, seed };
                    let mut cells: Vec<_> = (0..count)
                        .rev()
                        .map(|i| (i as u32, [1.0, 1.0, 1.0e-12, 1.0e12][i % 4]))
                        .collect();
                    let mut work = Work::new(false);
                    let mut serial = SampledTrees::new(sampling, count, &mut work).unwrap();
                    serial
                        .prepare(&mut cells, worker as usize, &mut work)
                        .unwrap();
                    for ordinal in 0..3 * (count - 1) {
                        let expected = serial.next(&cells, &mut work).unwrap().unwrap();
                        let actual = indexed_edge(
                            sampling,
                            worker as usize,
                            &cells,
                            serial.suffix(count),
                            ordinal,
                        );
                        assert_eq!((actual.0, actual.1), (expected.0, expected.1));
                        assert_eq!(actual.2.to_bits(), expected.2.to_bits());
                    }
                    assert!(serial.next(&cells, &mut work).unwrap().is_none());
                }
            }
        }
    }

    #[test]
    fn splitmix64_known_answers_and_sampling_validation() {
        let mut random = SplitMix64(0);
        assert_eq!(random.next(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(random.next(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(random.next(), 0x06c4_5d18_8009_454f);
        assert!(Sampling::default().validate().is_ok());
        assert!(Sampling {
            samples: 1,
            seed: (1 << 53) - 1
        }
        .validate()
        .is_ok());
        assert!(Sampling {
            samples: 0,
            seed: 1
        }
        .validate()
        .is_err());
        assert!(Sampling {
            samples: 1,
            seed: 1 << 53
        }
        .validate()
        .is_err());
        assert!(SampledTrees::allocation_bytes(usize::MAX).is_err());
    }

    #[test]
    fn suffixes_are_direct_compensated_sums_and_streams_repeat() {
        let mut work = Work::new(false);
        let mut sampler = SampledTrees::new(
            Sampling {
                samples: 3,
                seed: 9,
            },
            4,
            &mut work,
        )
        .unwrap();
        let mut cells = [(3, 4.0), (1, 1.0), (0, 0.25), (2, 2.0)];
        sampler.prepare(&mut cells, 19, &mut work).unwrap();
        assert_eq!(sampler.suffix, vec![7.25, 7.0, 6.0, 4.0, 0.0]);
        let mut first = Vec::new();
        while let Some(edge) = sampler.next(&cells, &mut work).unwrap() {
            first.push(edge);
        }
        assert_eq!(first.len(), 9);
        sampler.prepare(&mut cells, 19, &mut work).unwrap();
        for edge in first {
            assert_eq!(sampler.next(&cells, &mut work).unwrap(), Some(edge));
        }
        assert!(sampler.next(&cells, &mut work).unwrap().is_none());
    }

    #[test]
    fn unrepresentable_sampled_edge_is_rejected() {
        let mut work = Work::new(false);
        let mut sampler = SampledTrees::new(
            Sampling {
                samples: 2,
                seed: 0,
            },
            2,
            &mut work,
        )
        .unwrap();
        let mut cells = [(0, f64::from_bits(1)), (1, 1.0)];
        sampler.prepare(&mut cells, 0, &mut work).unwrap();
        assert!(sampler.next(&cells, &mut work).is_err());
    }

    #[test]
    fn sampled_edge_generation_polls_cancellation() {
        // Cancellation is process-wide. Isolate this witness from concurrent
        // scientific tests, then request it only after tree emission has begun.
        const CHILD: &str = "FEREG_CMG_SAMPLED_TREE_INTERRUPT_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args([
                    "fe::cmg_schur::sampling::tests::sampled_edge_generation_polls_cancellation",
                    "--exact",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let mut work = Work::new(false);
        let mut sampler = SampledTrees::new(
            Sampling {
                samples: 2,
                seed: 0,
            },
            2048,
            &mut work,
        )
        .unwrap();
        let mut cells: Vec<_> = (0..2048).map(|firm| (firm, 1.0)).collect();
        sampler.prepare(&mut cells, 0, &mut work).unwrap();
        assert!(sampler.next(&cells, &mut work).unwrap().is_some());
        crate::interrupt::request_cancellation();
        let mut emitted = 1;
        let failure = loop {
            match sampler.next(&cells, &mut work) {
                Ok(Some(_)) => emitted += 1,
                Ok(None) => panic!("sampled tree ignored cancellation"),
                Err(error) => break error,
            }
        };
        crate::interrupt::reset_cancellation();
        let super::super::bounded::AttemptError::Fatal(failure) = failure else {
            panic!("cancellation is never a refusal");
        };
        assert_eq!(failure.code, crate::error::ErrorCode::Interrupted);
        // Every emitted edge counts at least 256 units; the flag is polled
        // whenever the running count crosses the fixed 2^16-unit cadence.
        assert!(
            emitted <= (1 << 16) / 256 + 1,
            "{emitted} edges between polls"
        );
    }
}
