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
    pub(super) fn allocation_bytes(max_rows: usize) -> Result<usize> {
        super::bytes::<f64>(
            max_rows
                .checked_add(1)
                .ok_or_else(|| allocation("CMG sampler suffix length overflows"))?,
        )
    }

    /// The caller admits this exact suffix bank before constructing the sampler.
    pub(super) fn new(sampling: Sampling, max_rows: usize) -> Result<Self> {
        sampling.validate()?;
        let length = max_rows
            .checked_add(1)
            .ok_or_else(|| allocation("CMG sampler suffix length overflows"))?;
        Ok(Self {
            sampling,
            suffix: super::filled(length, 0.0)?,
            worker: 0,
            tree: 0,
            vertex: 0,
            random: SplitMix64(0),
        })
    }

    pub(super) fn bytes(&self) -> u128 {
        vec_capacity_bytes(&self.suffix)
    }

    pub(super) fn prepare(&mut self, cells: &mut [(u32, f64)], worker: usize) -> Result<()> {
        if cells.len() < 2 || cells.len() >= self.suffix.len() {
            return Err(invalid_argument(
                "CMG sampled worker exceeds its admitted scratch",
            ));
        }
        cells.sort_unstable_by(|left, right| left.1.total_cmp(&right.1).then(left.0.cmp(&right.0)));
        self.suffix[cells.len()] = 0.0;
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (index, &(_, weight)) in cells.iter().enumerate().rev() {
            crate::interrupt::row_checkpoint(index)?;
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data("CMG sampled worker weight is invalid"));
            }
            neumaier_add(weight, &mut sum, &mut correction);
            self.suffix[index] = sum + correction;
            if !self.suffix[index].is_finite() || self.suffix[index] <= 0.0 {
                return Err(invalid_data("CMG sampled suffix mass is invalid"));
            }
        }
        self.worker = u64::try_from(worker)
            .map_err(|_| allocation("CMG sampled worker identifier overflows"))?;
        self.tree = 0;
        self.vertex = 0;
        self.random = SplitMix64::stream(self.sampling.seed, self.worker, 0);
        Ok(())
    }

    pub(super) fn next(&mut self, cells: &[(u32, f64)]) -> Result<Option<(usize, usize, f64)>> {
        if self.tree == self.sampling.samples {
            return Ok(None);
        }
        crate::interrupt::row_checkpoint(self.vertex)?;
        let tail = self.suffix[self.vertex + 1];
        // Multiplication can round the largest uniform draw to the endpoint.
        // Clamp to its predecessor so every selected target is a later vertex.
        let target = (self.random.uniform() * tail).min(f64::from_bits(tail.to_bits() - 1));
        let mut lower = self.vertex + 1;
        let mut upper = cells.len() - 1;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if target < self.suffix[middle + 1] {
                lower = middle + 1;
            } else {
                upper = middle;
            }
        }
        let weight =
            cells[self.vertex].1 * (tail / self.suffix[0]) / f64::from(self.sampling.samples);
        if !weight.is_finite() || weight <= 0.0 {
            return Err(invalid_data(
                "CMG sampled edge weight is nonpositive or nonfinite",
            ));
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut sampler = SampledTrees::new(
            Sampling {
                samples: 3,
                seed: 9,
            },
            4,
        )
        .unwrap();
        let mut cells = [(3, 4.0), (1, 1.0), (0, 0.25), (2, 2.0)];
        sampler.prepare(&mut cells, 19).unwrap();
        assert_eq!(sampler.suffix, vec![7.25, 7.0, 6.0, 4.0, 0.0]);
        let mut first = Vec::new();
        while let Some(edge) = sampler.next(&cells).unwrap() {
            first.push(edge);
        }
        assert_eq!(first.len(), 9);
        sampler.prepare(&mut cells, 19).unwrap();
        for edge in first {
            assert_eq!(sampler.next(&cells).unwrap(), Some(edge));
        }
        assert!(sampler.next(&cells).unwrap().is_none());
    }

    #[test]
    fn unrepresentable_sampled_edge_is_rejected() {
        let mut sampler = SampledTrees::new(
            Sampling {
                samples: 2,
                seed: 0,
            },
            2,
        )
        .unwrap();
        let mut cells = [(0, f64::from_bits(1)), (1, 1.0)];
        sampler.prepare(&mut cells, 0).unwrap();
        assert!(sampler.next(&cells).is_err());
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
        let mut sampler = SampledTrees::new(
            Sampling {
                samples: 2,
                seed: 0,
            },
            2048,
        )
        .unwrap();
        let mut cells: Vec<_> = (0..2048).map(|firm| (firm, 1.0)).collect();
        sampler.prepare(&mut cells, 0).unwrap();
        assert!(sampler.next(&cells).unwrap().is_some());
        crate::interrupt::request_cancellation();
        let mut emitted = 1;
        let failure = loop {
            match sampler.next(&cells) {
                Ok(Some(_)) => emitted += 1,
                Ok(None) => panic!("sampled tree ignored cancellation"),
                Err(error) => break error,
            }
        };
        crate::interrupt::reset_cancellation();
        assert_eq!(failure.code, crate::error::ErrorCode::Interrupted);
        assert_eq!(emitted, crate::interrupt::CHECKPOINT_ROW_STRIDE);
    }
}
