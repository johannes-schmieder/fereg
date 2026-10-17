//! Independent, current-weight two-FE kernel experiments.
//!
//! Scope: cell regrouping and Schur-diagonal arithmetic may change low bits.
//! Sample, original factor slots, rank and omission decisions are untouched.
//! The caller retains the original observation operator for certification.

mod compact;
mod membership;
mod occurrence;
mod twin;
pub(crate) use compact::build_v2;
pub(crate) use membership::build_membership;
pub(crate) use occurrence::{build_occurrence, build_occurrence_gated};
pub(crate) use twin::build_engine;

use crate::error::{allocation, invalid_argument, invalid_data, FeregError, Result};
use crate::memory::{FeSolverMemoryBudget, SolverWorkspacePeak};

use super::{BorrowedInterceptFeDesign, BorrowedMapWeights};

#[derive(Clone, Copy, Debug)]
pub(crate) struct KernelLimits {
    pub work_budget: u64,
    pub retained_live_bytes: u128,
    pub memory: Option<FeSolverMemoryBudget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KernelRefusal {
    WorkBudget,
    MemoryBudget,
    Numerical,
    Unprofitable { cells: usize, limit: usize },
}

pub(crate) struct OptionalSetup {
    pub kernel: Option<CellSchur>,
    pub work_used: u64,
    pub peak: SolverWorkspacePeak,
    pub refusal: Option<KernelRefusal>,
}

#[derive(Clone, Copy, Default)]
struct Cell {
    worker: u32,
    firm: u32,
    weight: f64,
}

pub(crate) struct CellSchur {
    cells: Vec<Cell>,
    offsets: Vec<usize>,
    worker_weights: Vec<f64>,
    firm_weights: Vec<f64>,
    diagonal: Vec<f64>,
}

impl CellSchur {
    pub(crate) fn cell_count(&self) -> usize {
        if self.two_pass() {
            self.offsets[self.worker_weights.len()]
        } else {
            self.cells.len()
        }
    }

    pub(super) fn compact(&self) -> bool {
        self.offsets.len() > self.worker_weights.len() + 1 && !self.two_pass()
    }

    /// True Schur diagonal, with exactly-zero entries replaced by D_f solely
    /// for preconditioning. This is never a numerical-rank classification.
    pub fn diagonal(&self) -> &[f64] {
        &self.diagonal
    }

    pub fn retained_bytes(&self) -> u128 {
        self.cells.capacity() as u128 * std::mem::size_of::<Cell>() as u128
            + self.offsets.capacity() as u128 * std::mem::size_of::<usize>() as u128
            + (self.worker_weights.capacity() as u128
                + self.firm_weights.capacity() as u128
                + self.diagonal.capacity() as u128)
                * 8
    }

    /// S = D_f - C^T D_w^-1 C, using deterministic worker/firm CSR order.
    /// Only active RHS lanes are written. All buffers are borrowed; this call
    /// creates no executor, plan, allocation, or observation-scale workspace.
    pub fn apply(
        &self,
        input: &[f64],
        output: &mut [f64],
        left_scratch: &mut [f64],
        rhs_count: usize,
        stride: usize,
    ) -> Result<()> {
        let right_len = self
            .firm_weights
            .len()
            .checked_mul(stride)
            .ok_or_else(|| allocation("cell Schur right workspace size overflows"))?;
        let left_len = self
            .worker_weights
            .len()
            .checked_mul(stride)
            .ok_or_else(|| allocation("cell Schur left workspace size overflows"))?;
        if rhs_count > stride
            || input.len() != right_len
            || output.len() != right_len
            || left_scratch.len() != left_len
        {
            return Err(invalid_argument(
                "cell Schur input/workspace dimensions differ",
            ));
        }
        for (worker, &total) in self.worker_weights.iter().enumerate() {
            crate::interrupt::checkpoint()?;
            let work = &mut left_scratch[worker * stride..worker * stride + rhs_count];
            work.fill(0.0);
            for cell in &self.cells[self.offsets[worker]..self.offsets[worker + 1]] {
                crate::interrupt::checkpoint()?;
                let source =
                    &input[cell.firm as usize * stride..cell.firm as usize * stride + rhs_count];
                for (sum, &value) in work.iter_mut().zip(source) {
                    *sum += cell.weight * value;
                }
            }
            if total > 0.0 {
                for value in work {
                    *value /= total;
                }
            }
        }
        for (firm, &total) in self.firm_weights.iter().enumerate() {
            crate::interrupt::checkpoint()?;
            for rhs in 0..rhs_count {
                output[firm * stride + rhs] = total * input[firm * stride + rhs];
            }
        }
        for cell in &self.cells {
            crate::interrupt::checkpoint()?;
            for rhs in 0..rhs_count {
                output[cell.firm as usize * stride + rhs] -=
                    cell.weight * left_scratch[cell.worker as usize * stride + rhs];
            }
        }
        for firm in 0..self.firm_weights.len() {
            crate::interrupt::checkpoint()?;
            for rhs in 0..rhs_count {
                if !output[firm * stride + rhs].is_finite() {
                    return Err(invalid_data("cell Schur product is nonfinite"));
                }
            }
        }
        Ok(())
    }
}

enum Stop {
    Refused(KernelRefusal),
    Fatal(FeregError),
}
impl From<FeregError> for Stop {
    fn from(error: FeregError) -> Self {
        Self::Fatal(error)
    }
}
type Attempt<T> = std::result::Result<T, Stop>;

struct Work {
    limit: u64,
    used: u64,
}
impl Work {
    fn charge(&mut self, units: u16) -> Attempt<()> {
        crate::interrupt::checkpoint()?;
        if units == 0 || units > 1024 {
            return Err(invalid_argument("invalid cell-kernel work chunk").into());
        }
        if u64::from(units) > self.limit - self.used {
            return Err(Stop::Refused(KernelRefusal::WorkBudget));
        }
        self.used += u64::from(units);
        Ok(())
    }
    fn capacity<T>(&mut self, count: usize) -> Attempt<Vec<T>> {
        self.charge(16)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| allocation("cell kernel allocation failed"))?;
        Ok(result)
    }
    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Attempt<Vec<T>> {
        let mut result = self.capacity(count)?;
        for _ in 0..count {
            self.charge(4)?;
            result.push(value);
        }
        Ok(result)
    }
}

impl Work {
    // Charge before a bounded tile. Unlike per-element checkpoints, the tile
    // boundary amortizes cancellation/work overhead without hiding a long sort.
    fn tiles(
        &mut self,
        count: usize,
        cost: u16,
        mut action: impl FnMut(std::ops::Range<usize>) -> Attempt<()>,
    ) -> Attempt<()> {
        let width = usize::from(1024 / cost);
        for first in (0..count).step_by(width) {
            let end = (first + width).min(count);
            self.charge(((end - first) as u16) * cost)?;
            action(first..end)?;
        }
        Ok(())
    }

    fn bank<T: Copy>(&mut self, count: usize, value: T) -> Attempt<Vec<T>> {
        let mut bank = self.capacity(count)?;
        self.tiles(count, 4, |range| {
            bank.resize(range.end, value);
            Ok(())
        })?;
        Ok(bank)
    }
}

/// Use the incumbent's validated normalization scale without recomputing it.
/// Positive row weights are divided exactly once, as in ValidatedWeights.
/// Work denial discards private partial storage; allocator failure is fatal.
pub(crate) fn build(
    design: &BorrowedInterceptFeDesign<'_>,
    left_effect: usize,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    limits: KernelLimits,
) -> Result<OptionalSetup> {
    let mut work = Work {
        limit: limits.work_budget,
        used: 0,
    };
    match work.charge(128) {
        Ok(()) => {}
        Err(Stop::Refused(reason)) => {
            return Ok(OptionalSetup {
                kernel: None,
                work_used: 0,
                peak: SolverWorkspacePeak::ZERO,
                refusal: Some(reason),
            })
        }
        Err(Stop::Fatal(error)) => return Err(error),
    }
    if design.effects().len() != 2
        || left_effect >= 2
        || !weight_scale.is_finite()
        || weight_scale <= 0.0
    {
        return Err(invalid_argument(
            "cell kernel requires two effects and a positive normalization scale",
        ));
    }
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != design.nobs() {
            return Err(invalid_argument("cell kernel weight length differs"));
        }
    }
    let p = design.effects()[left_effect].n_levels();
    let q = design.effects()[1 - left_effect].n_levels();
    let n = design.nobs();
    // Two radix banks dominate sort; one cell bank plus suffix values is
    // smaller. All final dimension banks may coexist, so reserve them here.
    // Include fixed stack buckets/descriptors separately from heap payloads.
    let bytes = (n as u128)
        .checked_mul(2 * std::mem::size_of::<Cell>() as u128)
        .and_then(|b| b.checked_add((p as u128 + 1) * std::mem::size_of::<usize>() as u128))
        .and_then(|b| b.checked_add((p as u128 + 2 * q as u128) * 8))
        .and_then(|b| b.checked_add(4096));
    let Some(bytes) = bytes else {
        return Ok(OptionalSetup {
            kernel: None,
            work_used: work.used,
            peak: SolverWorkspacePeak::ZERO,
            refusal: Some(KernelRefusal::MemoryBudget),
        });
    };
    let fits = limits
        .retained_live_bytes
        .checked_add(bytes)
        .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)));
    if !fits {
        return Ok(OptionalSetup {
            kernel: None,
            work_used: work.used,
            peak: SolverWorkspacePeak::ZERO,
            refusal: Some(KernelRefusal::MemoryBudget),
        });
    }
    let result = build_inner(design, left_effect, weights, weight_scale, &mut work);
    let (kernel, refusal) = match result {
        Ok(kernel) => (Some(kernel), None),
        Err(Stop::Refused(reason)) => (None, Some(reason)),
        Err(Stop::Fatal(error)) => return Err(error),
    };
    Ok(OptionalSetup {
        kernel,
        refusal,
        work_used: work.used,
        peak: SolverWorkspacePeak::from_bytes(bytes)?,
    })
}

fn build_inner(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    work: &mut Work,
) -> Attempt<CellSchur> {
    work.charge(64)?;
    let (left_codes, right_codes) = (
        design.effects()[left].codes(),
        design.effects()[1 - left].codes(),
    );
    let (p, q) = (
        design.effects()[left].n_levels(),
        design.effects()[1 - left].n_levels(),
    );
    let mut cells = work.capacity(design.nobs())?;
    for row in 0..design.nobs() {
        work.charge(16)?;
        let weight = match weights {
            BorrowedMapWeights::Unit => 1.0,
            BorrowedMapWeights::Provided(values) => values[row] / scale,
        };
        if !weight.is_finite() || weight <= 0.0 {
            return Err(
                invalid_data("cell kernel normalized weight must be finite and positive").into(),
            );
        }
        cells.push(Cell {
            worker: left_codes[row],
            firm: right_codes[row],
            weight,
        });
    }
    radix(&mut cells, work)?;
    let mut write = 0;
    let mut read = 0;
    while read < cells.len() {
        work.charge(16)?;
        let mut cell = cells[read];
        read += 1;
        while read < cells.len()
            && cells[read].worker == cell.worker
            && cells[read].firm == cell.firm
        {
            work.charge(16)?;
            cell.weight += cells[read].weight;
            read += 1;
        }
        work.charge(8)?;
        if !cell.weight.is_finite() {
            return Err(Stop::Refused(KernelRefusal::Numerical));
        }
        cells[write] = cell;
        write += 1;
    }
    cells.truncate(write);
    let mut offsets = work.filled(
        p.checked_add(1)
            .ok_or_else(|| allocation("cell offsets overflow"))?,
        0usize,
    )?;
    let mut worker_weights = work.filled(p, 0.0)?;
    let mut firm_weights = work.filled(q, 0.0)?;
    let mut diagonal = work.filled(q, 0.0)?;
    for cell in &cells {
        work.charge(16)?;
        offsets[cell.worker as usize + 1] += 1;
        worker_weights[cell.worker as usize] += cell.weight;
        firm_weights[cell.firm as usize] += cell.weight;
    }
    for i in 0..p {
        work.charge(4)?;
        offsets[i + 1] += offsets[i];
    }
    let mut suffix = work.filled(
        cells
            .len()
            .checked_add(1)
            .ok_or_else(|| allocation("cell suffix overflows"))?,
        0.0,
    )?;
    for worker in 0..p {
        work.charge(16)?;
        let (start, end) = (offsets[worker], offsets[worker + 1]);
        let total = worker_weights[worker];
        if !total.is_finite() {
            return Err(Stop::Refused(KernelRefusal::Numerical));
        }
        suffix[end] = 0.0;
        for i in (start..end).rev() {
            work.charge(8)?;
            suffix[i] = suffix[i + 1] + cells[i].weight;
        }
        let mut prefix = 0.0;
        for i in start..end {
            work.charge(32)?;
            let cell = cells[i];
            let other = prefix + suffix[i + 1];
            // Since c + other = total mathematically, dividing the larger
            // term first avoids a premature tiny-ratio underflow.
            let contribution = if cell.weight >= other {
                (cell.weight / total) * other
            } else {
                (other / total) * cell.weight
            };
            diagonal[cell.firm as usize] += contribution;
            prefix += cell.weight;
        }
    }
    for firm in 0..q {
        work.charge(16)?;
        if !firm_weights[firm].is_finite() || !diagonal[firm].is_finite() || diagonal[firm] < 0.0 {
            return Err(Stop::Refused(KernelRefusal::Numerical));
        }
        if diagonal[firm] == 0.0 {
            diagonal[firm] = firm_weights[firm];
        }
    }
    work.charge(32)?;
    Ok(CellSchur {
        cells,
        offsets,
        worker_weights,
        firm_weights,
        diagonal,
    })
}

fn radix(cells: &mut Vec<Cell>, work: &mut Work) -> Attempt<()> {
    let mut scratch = work.filled(cells.len(), Cell::default())?;
    for byte in 0..8 {
        work.charge(16)?;
        work.charge(256)?;
        let mut buckets = [0usize; 256];
        for cell in cells.iter() {
            work.charge(16)?;
            let key = (u64::from(cell.worker) << 32) | u64::from(cell.firm);
            buckets[((key >> (byte * 8)) & 255) as usize] += 1;
        }
        let mut prefix = 0;
        for count in &mut buckets {
            work.charge(8)?;
            let n = *count;
            *count = prefix;
            prefix += n;
        }
        for &cell in cells.iter() {
            work.charge(16)?;
            let key = (u64::from(cell.worker) << 32) | u64::from(cell.firm);
            let bucket = ((key >> (byte * 8)) & 255) as usize;
            scratch[buckets[bucket]] = cell;
            buckets[bucket] += 1;
        }
        work.charge(16)?;
        std::mem::swap(cells, &mut scratch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{BorrowedInterceptEffect, LevelIndex};

    fn with_design(
        left: &[u32],
        right: &[u32],
        p: usize,
        q: usize,
        action: impl FnOnce(&BorrowedInterceptFeDesign<'_>),
    ) {
        let li = LevelIndex::from_codes(left, p).unwrap();
        let ri = LevelIndex::from_codes(right, q).unwrap();
        let design = BorrowedInterceptFeDesign::new(
            left.len(),
            vec![
                BorrowedInterceptEffect::new(left, p, &li).unwrap(),
                BorrowedInterceptEffect::new(right, q, &ri).unwrap(),
            ],
        )
        .unwrap();
        action(&design);
    }
    fn limits() -> KernelLimits {
        KernelLimits {
            work_budget: u64::MAX,
            retained_live_bytes: 0,
            memory: None,
        }
    }

    fn engine(
        design: &BorrowedInterceptFeDesign<'_>,
        weights: BorrowedMapWeights<'_>,
        limits: KernelLimits,
    ) -> OptionalSetup {
        build_engine(
            design,
            0,
            weights,
            1.,
            limits,
            super::super::RoutingPreset::ablation("cells_e").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn twin_operator_exact_s_and_grouped_contractions_have_observation_witnesses() {
        let left: Vec<u32> = (0..192).map(|i| (i % 7) as u32).collect();
        let right: Vec<u32> = (0..192).map(|i| ((i * 13 + i / 11) % 5) as u32).collect();
        let weights: Vec<f64> = (0..192).map(|i| 10f64.powi((i % 13) - 6)).collect();
        with_design(&left, &right, 7, 5, |design| {
            let original = build_occurrence(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.,
                limits(),
            )
            .unwrap()
            .kernel
            .unwrap();
            let built = engine(design, BorrowedMapWeights::Provided(&weights), limits());
            let twin = built.kernel.unwrap();
            assert!(twin.two_pass() && !twin.compact());
            assert_eq!(twin.cell_count(), original.cell_count());
            assert!(u128::from(built.peak.bytes().unwrap()) >= twin.retained_bytes());
            let width = 131;
            let rhs = width - 1;
            let input: Vec<f64> = (0..5 * width)
                .map(|i| ((i * 23 % 101) as f64 - 50.) / 64.)
                .collect();
            let mut expected = vec![99.; input.len()];
            let mut old_scratch = vec![99.; 7 * width];
            original
                .apply(&input, &mut expected, &mut old_scratch, rhs, width)
                .unwrap();
            let means: Vec<f64> = (0..7 * width).map(|i| (i % 29) as f64 / 32.).collect();
            let mut totals = vec![0.; 7];
            for (&w, &worker) in weights.iter().zip(&left) {
                totals[worker as usize] += w;
            }
            let mut baseline = None;
            for threads in [1, 2, 4, 4] {
                crate::ThreadTeam::new(crate::ThreadConfig::from_requested(threads, true).unwrap())
                    .install(|| {
                        let mut actual = vec![99.; input.len()];
                        let mut scratch = vec![99.; 7 * width];
                        twin.apply_two_pass(&input, &mut actual, &mut scratch, rhs, width)
                            .unwrap();
                        assert_eq!(
                            actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                            expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                        );
                        assert_eq!(scratch, old_scratch);
                        let mut grouped_rhs = vec![0.; 5 * width];
                        twin.subtract_left_means(&means, &mut grouped_rhs, rhs, width)
                            .unwrap();
                        twin.reconstruct_left(&input, &means, &totals, &mut scratch, rhs, width)
                            .unwrap();
                        let mut row_rhs = vec![0.; 5 * width];
                        let mut row_sums = vec![0.; 7 * width];
                        for row in 0..left.len() {
                            for r in 0..rhs {
                                row_rhs[right[row] as usize * width + r] -=
                                    weights[row] * means[left[row] as usize * width + r];
                                row_sums[left[row] as usize * width + r] +=
                                    weights[row] * input[right[row] as usize * width + r];
                            }
                        }
                        for (a, b) in grouped_rhs.iter().zip(&row_rhs) {
                            assert!((a - b).abs() <= 1e-13 * b.abs().max(1.));
                        }
                        for worker in 0..7 {
                            for r in 0..rhs {
                                let b = means[worker * width + r]
                                    - row_sums[worker * width + r] / totals[worker];
                                assert!(
                                    (scratch[worker * width + r] - b).abs()
                                        <= 1e-13 * b.abs().max(1.)
                                );
                            }
                            assert_eq!(scratch[worker * width + rhs], 99.);
                        }
                        let state = (grouped_rhs, scratch);
                        if let Some(previous) = &baseline {
                            assert_eq!(&state, previous);
                        } else {
                            baseline = Some(state);
                        }
                    })
                    .unwrap();
            }
        });
    }

    #[test]
    fn twin_empty_layout_and_invalid_values_are_checked() {
        let twin = CellSchur {
            cells: vec![],
            offsets: vec![0, 0, 0, 0],
            worker_weights: vec![0.],
            firm_weights: vec![0.],
            diagonal: vec![0.],
        };
        assert!(twin.two_pass() && !twin.compact());
        assert_eq!(twin.cell_count(), 0);
        twin.apply_two_pass(&[1.], &mut [0.], &mut [0.], 1, 1)
            .unwrap();
        assert!(twin
            .apply_two_pass(&[1.], &mut [0.], &mut [0.], 1, 0)
            .is_err());
        assert!(twin
            .apply_two_pass(&[f64::INFINITY], &mut [0.], &mut [0.], 1, 1)
            .is_err());
        with_design(&[0, 0], &[0, 1], 1, 2, |design| {
            let twin = engine(design, BorrowedMapWeights::Unit, limits())
                .kernel
                .unwrap();
            assert!(twin
                .apply_two_pass(&[f64::NAN, 1.], &mut [0.; 2], &mut [0.], 1, 1)
                .is_err());
            assert!(twin
                .subtract_left_means(&[f64::INFINITY], &mut [0.; 2], 1, 1)
                .is_err());
            assert!(twin
                .reconstruct_left(&[1., 1.], &[0.], &[0.], &mut [0.], 1, 1)
                .is_err());
        });
    }

    #[test]
    fn twin_large_groups_and_four_planned_lanes_preserve_s() {
        let left: Vec<u32> = (0..40000).map(|i| (i / 4) as u32).collect();
        let right: Vec<u32> = (0..40000).map(|i| ((i / 4 + i % 4) % 200) as u32).collect();
        with_design(&left, &right, 10000, 200, |design| {
            let old = build_occurrence(design, 0, BorrowedMapWeights::Unit, 1., limits())
                .unwrap()
                .kernel
                .unwrap();
            let twin = engine(design, BorrowedMapWeights::Unit, limits())
                .kernel
                .unwrap();
            let work = twin.cell_count() as u128 * 4 * 4;
            assert_eq!(crate::parallel::StagePlan::new(work, 200, 4).lanes, 4);
            let input: Vec<f64> = (0..800)
                .map(|i| ((i * 13 % 53) as f64 - 26.) / 8.)
                .collect();
            let mut expected = vec![0.; 800];
            let mut scratch = vec![0.; 40000];
            old.apply(&input, &mut expected, &mut scratch, 4, 4)
                .unwrap();
            for workers in [1, 2, 4, 4] {
                let mut got = vec![0.; 800];
                crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                    .install(|| {
                        twin.apply_two_pass(&input, &mut got, &mut scratch, 4, 4)
                            .unwrap();
                    })
                    .unwrap();
                assert_eq!(got, expected);
            }
        });
        // A single group crosses multiple bounded cancellation tiles.
        let p = vec![0; 4097];
        let q: Vec<u32> = (0..4097).collect();
        with_design(&p, &q, 1, 4097, |design| {
            let twin = engine(design, BorrowedMapWeights::Unit, limits())
                .kernel
                .unwrap();
            let input: Vec<f64> = q.iter().map(|&q| q as f64).collect();
            let mut output = vec![0.; q.len()];
            twin.apply_two_pass(&input, &mut output, &mut [0.], 1, 1)
                .unwrap();
            assert!(output
                .iter()
                .enumerate()
                .all(|(i, &v)| v == i as f64 - 2048.));
        });
    }

    #[test]
    fn twin_transactional_work_and_memory_refusals_preserve_s_banks() {
        let left: Vec<u32> = (0..64).map(|i| (i / 8) as u32).collect();
        let right: Vec<u32> = (0..64).map(|i| (i % 8) as u32).collect();
        with_design(&left, &right, 8, 8, |design| {
            let plain =
                build_occurrence(design, 0, BorrowedMapWeights::Unit, 1., limits()).unwrap();
            let original = plain.kernel.unwrap();
            let full = engine(design, BorrowedMapWeights::Unit, limits());
            assert_eq!(
                full.work_used - plain.work_used,
                36 * 64 + 8 * 8 + 12 * 8 + 204
            );
            for remaining in [
                0,
                127,
                full.work_used - plain.work_used + 127,
                full.work_used - plain.work_used + 128,
            ] {
                let got = engine(
                    design,
                    BorrowedMapWeights::Unit,
                    KernelLimits {
                        work_budget: plain.work_used + remaining,
                        ..limits()
                    },
                );
                let kernel = got.kernel.unwrap();
                assert!(got.work_used <= plain.work_used + remaining);
                if remaining < full.work_used - plain.work_used + 128 {
                    assert!(!kernel.two_pass());
                    equal_banks(&kernel, &original);
                    assert_eq!(got.refusal, Some(KernelRefusal::WorkBudget));
                } else {
                    assert!(kernel.two_pass());
                }
            }
            let peak = u128::from(full.peak.bytes().unwrap());
            assert!(peak > u128::from(plain.peak.bytes().unwrap()));
            for delta in [0, 1, 2] {
                let got = engine(
                    design,
                    BorrowedMapWeights::Unit,
                    KernelLimits {
                        retained_live_bytes: 512,
                        memory: Some(FeSolverMemoryBudget {
                            execution_bytes: 128,
                            usable_bytes: 512 + 128 + peak - 1 + delta,
                        }),
                        ..limits()
                    },
                );
                let kernel = got.kernel.unwrap();
                if delta == 0 {
                    equal_banks(&kernel, &original);
                    assert_eq!(got.refusal, Some(KernelRefusal::MemoryBudget));
                } else {
                    assert!(kernel.two_pass());
                }
            }
        });
    }

    fn equal_banks(a: &CellSchur, b: &CellSchur) {
        assert_eq!(a.cells.len(), b.cells.len());
        for (a, b) in a.cells.iter().zip(&b.cells) {
            assert_eq!(
                (a.worker, a.firm, a.weight.to_bits()),
                (b.worker, b.firm, b.weight.to_bits())
            );
        }
        assert_eq!(a.offsets, b.offsets);
        for (a, b) in [
            (&a.worker_weights, &b.worker_weights),
            (&a.firm_weights, &b.firm_weights),
            (&a.diagonal, &b.diagonal),
        ] {
            assert_eq!(
                a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                b.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn membership_builder_preserves_all_original_bank_bits() {
        for workers in [1, 4] {
            crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                .install(|| {
                    for n in [1, 2, 127, 4097] {
                        let left: Vec<_> = (0..n)
                            .map(|i| {
                                if i < 7 || i % 5 == 0 {
                                    (i % 7) as u32
                                } else {
                                    0
                                }
                            })
                            .collect();
                        let right: Vec<_> = (0..n)
                            .map(|i| ((i * 31 + i / 3) % n.min(19)) as u32)
                            .collect();
                        let weights: Vec<_> = (0..n)
                            .map(|i| [1.0, 1e-100, 3.0, 1e100, 2.0][i % 5])
                            .collect();
                        with_design(
                            &left,
                            &right,
                            (*left.iter().max().unwrap() + 1) as usize,
                            (*right.iter().max().unwrap() + 1) as usize,
                            |design| {
                                for side in [0, 1] {
                                    for weights in [
                                        BorrowedMapWeights::Unit,
                                        BorrowedMapWeights::Provided(&weights),
                                    ] {
                                        let original = build(design, side, weights, 7.0, limits())
                                            .unwrap()
                                            .kernel
                                            .unwrap();
                                        let new =
                                            build_membership(design, side, weights, 7.0, limits())
                                                .unwrap();
                                        let kernel = new.kernel.unwrap();
                                        let occurrence =
                                            build_occurrence(design, side, weights, 7.0, limits())
                                                .unwrap();
                                        let occurrence = occurrence.kernel.unwrap();
                                        let pairs = |kernel: &CellSchur| {
                                            let mut rows: Vec<_> = kernel
                                                .cells
                                                .iter()
                                                .map(|c| (c.worker, c.firm, c.weight.to_bits()))
                                                .collect();
                                            rows.sort_unstable();
                                            rows
                                        };
                                        assert_eq!(pairs(&original), pairs(&occurrence));
                                        assert_eq!(original.offsets, occurrence.offsets);
                                        assert_eq!(
                                            original
                                                .firm_weights
                                                .iter()
                                                .map(|v| v.to_bits())
                                                .collect::<Vec<_>>(),
                                            occurrence
                                                .firm_weights
                                                .iter()
                                                .map(|v| v.to_bits())
                                                .collect::<Vec<_>>()
                                        );
                                        assert_eq!(
                                            occurrence.cells.capacity(),
                                            occurrence.cells.len()
                                        );
                                        equal_banks(&original, &kernel);
                                        assert_eq!(kernel.cells.capacity(), kernel.cells.len());
                                        assert!(
                                            kernel.retained_bytes()
                                                <= u128::from(new.peak.bytes().unwrap())
                                        );
                                    }
                                }
                            },
                        );
                    }
                })
                .unwrap();
        }
    }

    #[test]
    fn membership_builder_work_and_joint_memory_boundaries() {
        let left: Vec<_> = (0..1025).map(|i| (i % 2) as u32).collect();
        let right: Vec<_> = (0..1025).map(|i| (i % 13) as u32).collect();
        with_design(&left, &right, 2, 13, |design| {
            let full = build_membership(design, 0, BorrowedMapWeights::Unit, 1., limits()).unwrap();
            for budget in [
                0,
                127,
                full.work_used / 2,
                full.work_used - 1,
                full.work_used,
                full.work_used + 1,
            ] {
                let got = build_membership(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.,
                    KernelLimits {
                        work_budget: budget,
                        ..limits()
                    },
                )
                .unwrap();
                assert_eq!(got.kernel.is_some(), budget >= full.work_used);
                assert!(got.work_used <= budget);
            }
            let bytes = u128::from(full.peak.bytes().unwrap());
            for extra in [0, 1, 2] {
                let got = build_membership(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.,
                    KernelLimits {
                        retained_live_bytes: 512,
                        memory: Some(FeSolverMemoryBudget {
                            execution_bytes: 128,
                            usable_bytes: 512 + 128 + bytes - 1 + extra,
                        }),
                        ..limits()
                    },
                )
                .unwrap();
                assert_eq!(got.kernel.is_some(), extra >= 1);
                if extra == 0 {
                    assert_eq!(got.refusal, Some(KernelRefusal::MemoryBudget));
                }
            }
        });
    }

    #[test]
    fn occurrence_first_order_charges_and_joint_memory_boundaries() {
        let left = [1, 0, 0, 1, 0, 1, 0, 1];
        let right = [2, 1, 2, 0, 1, 2, 0, 1];
        let weights = [1., 1e-100, 3., 2., 1., 1e100, 7., 11.];
        with_design(&left, &right, 2, 3, |design| {
            let full = build_occurrence(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1e100,
                limits(),
            )
            .unwrap();
            let kernel = full.kernel.as_ref().unwrap();
            assert_eq!(
                kernel
                    .cells
                    .iter()
                    .map(|c| (c.worker, c.firm))
                    .collect::<Vec<_>>(),
                vec![(0, 1), (0, 2), (0, 0), (1, 2), (1, 0), (1, 1)]
            );
            // Sum the implementation's operation classes independently: row
            // aggregation, cell finalization, level banks, allocations/preflight.
            assert_eq!(full.work_used, 32 * 8 + 64 * 6 + 44 * 2 + 32 * 3 + 312);
            for budget in [
                0,
                127,
                128,
                full.work_used / 2,
                full.work_used - 1,
                full.work_used,
            ] {
                let got = build_occurrence(
                    design,
                    0,
                    BorrowedMapWeights::Provided(&weights),
                    1e100,
                    KernelLimits {
                        work_budget: budget,
                        ..limits()
                    },
                )
                .unwrap();
                assert_eq!(got.kernel.is_some(), budget >= full.work_used);
                assert!(got.work_used <= budget);
                if got.kernel.is_none() {
                    assert_eq!(got.refusal, Some(KernelRefusal::WorkBudget));
                }
            }
            let bytes = u128::from(full.peak.bytes().unwrap());
            assert!(kernel.retained_bytes() <= bytes);
            for extra in [0, 1, 2] {
                let got = build_occurrence(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.,
                    KernelLimits {
                        retained_live_bytes: 512,
                        memory: Some(FeSolverMemoryBudget {
                            execution_bytes: 128,
                            usable_bytes: 512 + 128 + bytes - 1 + extra,
                        }),
                        ..limits()
                    },
                )
                .unwrap();
                assert_eq!(got.kernel.is_some(), extra >= 1);
                if extra == 0 {
                    assert_eq!(got.refusal, Some(KernelRefusal::MemoryBudget));
                }
            }
        });
    }

    #[test]
    fn occurrence_gate_matches_independent_affine_costs_and_overflow_is_not_refusal() {
        let mut seed = 731u64;
        for _ in 0..800 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let n = 1 + (seed % 160) as usize;
            let p = 1 + ((seed >> 9) % n as u64) as usize;
            let q = 1 + ((seed >> 21) % n as u64) as usize;
            let max_rhs = 1 + ((seed >> 35) % 12) as usize;
            for weighted in [false, true] {
                let limit = occurrence::cell_limit(n, p, q, max_rhs, weighted).unwrap();
                for c in 1..=n {
                    let pays = (1..=max_rhs).any(|r| {
                        c * (10 * r + 16) + p * (4 * r + 8) + q * (6 * r + 8) + 128
                            < n * (10 * r + if weighted { 8 } else { 4 })
                    });
                    assert_eq!(c <= limit, pays, "n={n} p={p} q={q} r={max_rhs} c={c}");
                }
            }
        }
        assert_eq!(occurrence::cell_limit(1, 1, 1, 1, false), Some(0));
        assert_eq!(occurrence::cell_limit(100, 2, 2, 0, false), None);
        assert_eq!(
            occurrence::cell_limit(usize::MAX, 1, 1, usize::MAX, false),
            None
        );
        // A u64 overflow at the largest width must not hide a paying width one.
        let n = (u64::MAX / 16) as usize;
        let c = n / 2;
        assert!((c as u128) * 56 + 24 + 32 + 128 > u64::MAX as u128);
        assert!((c as u128) * 26 + 12 + 14 + 128 < (n as u128) * 14);
        assert!(occurrence::cell_limit(n, 1, 1, 4, false).unwrap() >= c);
    }

    #[test]
    fn occurrence_gated_banks_and_exact_work_memory_boundaries() {
        let left: Vec<u32> = (0..1024).map(|i| (i % 2) as u32).collect();
        let right: Vec<u32> = (0..1024).map(|i| ((i / 2) % 2) as u32).collect();
        let weights: Vec<f64> = (0..1024)
            .map(|i| if i % 3 == 0 { 1e-100 } else { 1e100 })
            .collect();
        with_design(&left, &right, 2, 2, |design| {
            let plain = build_occurrence(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1e100,
                limits(),
            )
            .unwrap();
            let gated = build_occurrence_gated(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1e100,
                limits(),
                4,
            )
            .unwrap();
            let (a, b) = (plain.kernel.unwrap(), gated.kernel.as_ref().unwrap());
            assert_eq!(
                a.cells
                    .iter()
                    .map(|x| (x.worker, x.firm, x.weight.to_bits()))
                    .collect::<Vec<_>>(),
                b.cells
                    .iter()
                    .map(|x| (x.worker, x.firm, x.weight.to_bits()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(a.offsets, b.offsets);
            for (x, y) in [
                (&a.worker_weights, &b.worker_weights),
                (&a.firm_weights, &b.firm_weights),
                (&a.diagonal, &b.diagonal),
            ] {
                assert_eq!(
                    x.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    y.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
            }
            assert_eq!(gated.work_used, plain.work_used + 128 + 8 * 32);
            assert_eq!(gated.peak.bytes(), plain.peak.bytes());
            for budget in [0, 127, 255, gated.work_used - 1, gated.work_used] {
                let got = build_occurrence_gated(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.,
                    KernelLimits {
                        work_budget: budget,
                        ..limits()
                    },
                    4,
                )
                .unwrap();
                assert_eq!(got.kernel.is_some(), budget == gated.work_used);
                assert!(got.work_used <= budget);
            }
            let bytes = u128::from(gated.peak.bytes().unwrap());
            for extra in [0, 1, 2] {
                let got = build_occurrence_gated(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.,
                    KernelLimits {
                        retained_live_bytes: 512,
                        memory: Some(FeSolverMemoryBudget {
                            execution_bytes: 128,
                            usable_bytes: 512 + 128 + bytes - 1 + extra,
                        }),
                        ..limits()
                    },
                    4,
                )
                .unwrap();
                assert_eq!(got.kernel.is_some(), extra >= 1);
                if extra == 0 {
                    assert_eq!(got.refusal, Some(KernelRefusal::MemoryBudget));
                }
            }
        });
    }

    #[test]
    fn occurrence_gated_prepass_and_partial_tile_refusal_have_exact_charges() {
        with_design(&[0], &[0], 1, 1, |design| {
            let got = build_occurrence_gated(
                design,
                0,
                BorrowedMapWeights::Unit,
                1.,
                KernelLimits {
                    memory: Some(FeSolverMemoryBudget {
                        execution_bytes: 0,
                        usable_bytes: 0,
                    }),
                    ..limits()
                },
                1,
            )
            .unwrap();
            assert_eq!(got.work_used, 256);
            assert_eq!(got.peak.bytes(), Some(0));
            assert_eq!(
                got.refusal,
                Some(KernelRefusal::Unprofitable { cells: 1, limit: 0 })
            );
        });
        let left: Vec<u32> = (0..1024).map(|i| (i / 32) as u32).collect();
        let right: Vec<u32> = (0..1024).map(|i| (i % 32) as u32).collect();
        with_design(&left, &right, 32, 32, |design| {
            for budget in [26759, 26760, u64::MAX] {
                let got = build_occurrence_gated(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.,
                    KernelLimits {
                        work_budget: budget,
                        ..limits()
                    },
                    4,
                )
                .unwrap();
                assert!(got.kernel.is_none());
                assert!(got.work_used <= budget);
                if budget >= 26760 {
                    assert_eq!(got.work_used, 26760);
                    assert_eq!(
                        got.refusal,
                        Some(KernelRefusal::Unprofitable {
                            cells: 800,
                            limit: 770
                        })
                    );
                } else {
                    assert_eq!(got.refusal, Some(KernelRefusal::WorkBudget));
                }
            }
        });
    }

    #[test]
    fn occurrence_firm_totals_follow_cells_not_worker_major_rows() {
        let left = [1, 0, 0, 1];
        let right = [0, 0, 0, 1];
        let weights = [1., 2f64.powi(-53), 2f64.powi(-53), 1.];
        with_design(&left, &right, 2, 2, |design| {
            let kernel = build_occurrence(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.,
                limits(),
            )
            .unwrap()
            .kernel
            .unwrap();
            // Row-order incumbent firm total is 1. Cell aggregation groups the
            // two half-ulp rows before adding worker 1's contribution.
            let row_total = weights[..3].iter().fold(0., |sum, w| sum + w);
            assert_eq!(row_total, 1.);
            assert_eq!(kernel.firm_weights[0], 1. + f64::EPSILON);
            let old = build(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.,
                limits(),
            )
            .unwrap()
            .kernel
            .unwrap();
            assert_eq!(kernel.firm_weights, old.firm_weights);
        });
    }

    #[test]
    fn repeated_weighted_cells_match_independent_rectangle_conductance() {
        for builder in [build, build_occurrence] {
            let left = [0, 0, 0, 1, 1, 1, 2, 2];
            let right = [0, 0, 1, 0, 1, 1, 2, 2];
            let weights = [2.0, 1.0, 3.0, 4.0, 5.0, 2.0, 7.0, 1.0];
            with_design(&left, &right, 3, 3, |design| {
                let setup = builder(
                    design,
                    0,
                    BorrowedMapWeights::Provided(&weights),
                    7.0,
                    limits(),
                )
                .unwrap();
                let peak = setup.peak.bytes().unwrap();
                let kernel = setup.kernel.unwrap();
                // Parallel conductances of two independently eliminated stars.
                let h = (3.0 * 3.0 / 6.0 + 4.0 * 7.0 / 11.0) / 7.0;
                assert!((kernel.diagonal()[0] - h).abs() < 2e-15);
                assert!((kernel.diagonal()[1] - h).abs() < 2e-15);
                // Exactly-zero Schur diagonal keeps a positive D_f preconditioner;
                // the corresponding operator block remains zero.
                assert_eq!(kernel.diagonal()[2], 8.0 / 7.0);
                assert!(kernel.retained_bytes() <= u128::from(peak));
                let input = [1.0, 2.0, 99.0, -2.0, 4.0, 99.0, 3.0, -7.0, 99.0];
                let mut output = [99.0; 9];
                let mut scratch = [99.0; 9];
                kernel
                    .apply(&input, &mut output, &mut scratch, 2, 3)
                    .unwrap();
                for rhs in 0..2 {
                    let expected = h * (input[rhs] - input[3 + rhs]);
                    assert!((output[rhs] - expected).abs() < 4e-15);
                    assert!((output[3 + rhs] + expected).abs() < 4e-15);
                    assert!(output[6 + rhs].abs() < 4e-15);
                }
                assert_eq!([output[2], output[5], output[8]], [99.0; 3]);
                let second = builder(
                    design,
                    0,
                    BorrowedMapWeights::Provided(&weights),
                    7.0,
                    limits(),
                )
                .unwrap();
                assert_eq!(second.work_used, setup.work_used);
                assert_eq!(
                    second
                        .kernel
                        .unwrap()
                        .diagonal()
                        .iter()
                        .map(|x| x.to_bits())
                        .collect::<Vec<_>>(),
                    kernel
                        .diagonal()
                        .iter()
                        .map(|x| x.to_bits())
                        .collect::<Vec<_>>()
                );
            });
        }
    }

    #[test]
    fn cell_operator_matches_independent_dense_observation_elimination() {
        for builder in [build, build_occurrence] {
            let left = [2, 0, 1, 0, 2, 1, 0, 1, 2, 0, 2, 1];
            let right = [0, 1, 3, 0, 2, 1, 1, 3, 3, 2, 0, 0];
            let weights = [1.0, 2.0, 4.0, 3.0, 1.0, 5.0, 2.0, 1.0, 7.0, 3.0, 2.0, 4.0];
            with_design(&left, &right, 3, 4, |design| {
                for side in [0, 1] {
                    let kernel = builder(
                        design,
                        side,
                        BorrowedMapWeights::Provided(&weights),
                        7.0,
                        limits(),
                    )
                    .unwrap()
                    .kernel
                    .unwrap();
                    let (workers, firms, p, q) = if side == 0 {
                        (&left, &right, 3, 4)
                    } else {
                        (&right, &left, 4, 3)
                    };
                    let mut dw = vec![0.0; p];
                    let mut dense = vec![0.0; q * q];
                    for row in 0..weights.len() {
                        dw[workers[row] as usize] += weights[row] / 7.0;
                        dense[firms[row] as usize * q + firms[row] as usize] += weights[row] / 7.0;
                    }
                    // Direct observation-pair expansion: no compressed cells,
                    // sorted key, CSR row, or diagonal-complement implementation.
                    for i in 0..weights.len() {
                        for j in 0..weights.len() {
                            if workers[i] == workers[j] {
                                dense[firms[i] as usize * q + firms[j] as usize] -=
                                    (weights[i] / 7.0) * (weights[j] / 7.0)
                                        / dw[workers[i] as usize];
                            }
                        }
                    }
                    let input: Vec<_> = (0..q).map(|i| (i as f64 - 1.5) * 0.7).collect();
                    let mut actual = vec![0.0; q];
                    let mut scratch = vec![0.0; p];
                    kernel
                        .apply(&input, &mut actual, &mut scratch, 1, 1)
                        .unwrap();
                    for i in 0..q {
                        let expected = (0..q).map(|j| dense[i * q + j] * input[j]).sum::<f64>();
                        assert!((actual[i] - expected).abs() < 5e-15);
                        assert!((kernel.diagonal()[i] - dense[i * q + i]).abs() < 5e-15);
                    }
                }
            });
        }
    }

    #[test]
    fn diagonal_retains_tiny_positive_complements_without_subtraction() {
        for builder in [build, build_occurrence] {
            with_design(&[0, 0], &[0, 1], 1, 2, |design| {
                for tiny in [1e-100, 1e-300, f64::from_bits(1)] {
                    let weights = [1.0, tiny];
                    let kernel = builder(
                        design,
                        0,
                        BorrowedMapWeights::Provided(&weights),
                        1.0,
                        limits(),
                    )
                    .unwrap()
                    .kernel
                    .unwrap();
                    assert_eq!(kernel.diagonal()[0].to_bits(), tiny.to_bits());
                    assert_eq!(kernel.diagonal()[1].to_bits(), tiny.to_bits());
                }
            });
        }
    }

    #[test]
    fn setup_refusal_charges_work_and_preserves_inputs() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        with_design(&left, &right, 2, 2, |design| {
            let full = build(design, 0, BorrowedMapWeights::Unit, 1.0, limits()).unwrap();
            for budget in [0, 127, full.work_used - 1] {
                let stopped = build(
                    design,
                    0,
                    BorrowedMapWeights::Unit,
                    1.0,
                    KernelLimits {
                        work_budget: budget,
                        ..limits()
                    },
                )
                .unwrap();
                assert!(stopped.kernel.is_none());
                assert_eq!(stopped.refusal, Some(KernelRefusal::WorkBudget));
                assert!(stopped.work_used <= budget);
            }
            assert!(build(
                design,
                0,
                BorrowedMapWeights::Unit,
                1.0,
                KernelLimits {
                    work_budget: full.work_used,
                    ..limits()
                }
            )
            .unwrap()
            .kernel
            .is_some());
            let bytes = u128::from(full.peak.bytes().unwrap());
            let refused = build(
                design,
                0,
                BorrowedMapWeights::Unit,
                1.0,
                KernelLimits {
                    retained_live_bytes: 512,
                    memory: Some(FeSolverMemoryBudget {
                        execution_bytes: 128,
                        usable_bytes: 512 + 128 + bytes - 1,
                    }),
                    ..limits()
                },
            )
            .unwrap();
            assert_eq!(refused.refusal, Some(KernelRefusal::MemoryBudget));
            assert!(refused.kernel.is_none());
            assert_eq!(refused.peak.bytes(), Some(0));
            assert_eq!(left, [0, 0, 1, 1]);
            assert_eq!(right, [0, 1, 0, 1]);
        });
    }
    #[test]
    fn compact_cells_match_observation_pairs_energy_and_retire_lanes() {
        let left = [2, 0, 1, 0, 2, 1, 0, 1, 2, 0, 2, 1, 3, 3];
        let right = [0, 1, 3, 0, 2, 1, 1, 3, 3, 2, 0, 0, 4, 4];
        let weights = [1., 2., 4., 3., 1., 5., 2., 1., 7., 3., 2., 4., 3., 7.];
        with_design(&left, &right, 4, 5, |design| {
            for side in [0, 1] {
                let (workers, firms, p, q) = if side == 0 {
                    (&left, &right, 4, 5)
                } else {
                    (&right, &left, 5, 4)
                };
                let mut dense = vec![0.; q * q];
                let mut dw = vec![0.; p];
                for i in 0..weights.len() {
                    dw[workers[i] as usize] += weights[i] / 7.;
                }
                for i in 0..weights.len() {
                    let f = firms[i] as usize;
                    dense[f * q + f] += weights[i] / 7.;
                    for j in 0..weights.len() {
                        if workers[i] == workers[j] {
                            dense[f * q + firms[j] as usize] -=
                                (weights[i] / 7.) * (weights[j] / 7.) / dw[workers[i] as usize];
                        }
                    }
                }
                let width = 131; // crosses the RHS tile and includes padded lanes
                let input: Vec<_> = (0..q * width)
                    .map(|i| ((i * 23 % 101) as f64 - 50.) / 8.)
                    .collect();
                let mut activity = vec![1.; width - 1];
                for i in (0..activity.len()).step_by(3) {
                    activity[i] = 0.;
                }
                let mut reference = None;
                for threads in [1, 4, 4] {
                    let mut kernel = build_v2(
                        design,
                        side,
                        BorrowedMapWeights::Provided(&weights),
                        7.,
                        width,
                        limits(),
                    )
                    .unwrap()
                    .kernel
                    .unwrap();
                    let mut actual = vec![99.; q * width];
                    let mut scratch = vec![99.; p * width];
                    crate::ThreadTeam::new(
                        crate::ThreadConfig::from_requested(threads, true).unwrap(),
                    )
                    .install(|| {
                        kernel
                            .apply_compact(&input, &mut actual, &mut scratch, &activity, width)
                            .unwrap()
                    })
                    .unwrap();
                    for f in 0..q {
                        for r in 0..activity.len() {
                            let expected = if activity[r] == 0. {
                                0.
                            } else {
                                (0..q)
                                    .map(|g| dense[f * q + g] * input[g * width + r])
                                    .sum::<f64>()
                            };
                            assert!((actual[f * width + r] - expected).abs() < 3e-14);
                        }
                        assert_eq!(actual[f * width + width - 1], 99.);
                    }
                    if let Some(reference) = &reference {
                        assert_eq!(&actual, reference);
                    } else {
                        reference = Some(actual.clone());
                    }
                    let r = 1;
                    let mut means = vec![0.; p];
                    for i in 0..weights.len() {
                        means[workers[i] as usize] +=
                            weights[i] / 7. * input[firms[i] as usize * width + r];
                    }
                    for w in 0..p {
                        means[w] /= dw[w];
                    }
                    let energy: f64 = (0..weights.len())
                        .map(|i| {
                            weights[i] / 7.
                                * (input[firms[i] as usize * width + r]
                                    - means[workers[i] as usize])
                                    .powi(2)
                        })
                        .sum();
                    let product: f64 = (0..q)
                        .map(|f| input[f * width + r] * actual[f * width + r])
                        .sum();
                    assert!((energy - product).abs() < 1e-12);
                    assert!(kernel.cells.iter().all(|cell| if side == 0 {
                        cell.worker != 3
                    } else {
                        cell.worker != 4
                    }));
                    let constant = vec![1e100; q * width];
                    kernel
                        .apply_compact(&constant, &mut actual, &mut scratch, &activity, width)
                        .unwrap();
                    for f in 0..q {
                        assert!(actual[f * width..f * width + activity.len()]
                            .iter()
                            .all(|&x| x == 0.));
                    }
                }
            }
        });
    }

    #[test]
    fn compact_cells_bound_work_memory_and_tiny_diagonals() {
        with_design(&[0, 0, 1, 1], &[0, 1, 2, 2], 2, 3, |design| {
            for tiny in [1e-100, 1e-300, f64::from_bits(1)] {
                let weights = [1., tiny, 1., 1.];
                let full = build_v2(
                    design,
                    0,
                    BorrowedMapWeights::Provided(&weights),
                    1.,
                    3,
                    limits(),
                )
                .unwrap();
                let bytes = u128::from(full.peak.bytes().unwrap());
                let kernel = full.kernel.unwrap();
                assert_eq!(kernel.diagonal(), &[tiny, tiny, 2.]);
                assert!(kernel.retained_bytes() <= bytes);
                for work_budget in [0, 127, full.work_used / 2, full.work_used - 1] {
                    let stop = build_v2(
                        design,
                        0,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        3,
                        KernelLimits {
                            work_budget,
                            ..limits()
                        },
                    )
                    .unwrap();
                    assert!(stop.kernel.is_none());
                    assert_eq!(stop.refusal, Some(KernelRefusal::WorkBudget));
                    assert!(stop.work_used <= work_budget);
                }
                for (available, installed) in [(bytes - 1, false), (bytes, true)] {
                    let built = build_v2(
                        design,
                        0,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        3,
                        KernelLimits {
                            retained_live_bytes: 512,
                            memory: Some(FeSolverMemoryBudget {
                                execution_bytes: 128,
                                usable_bytes: 128 + 512 + available,
                            }),
                            ..limits()
                        },
                    )
                    .unwrap();
                    assert_eq!(built.kernel.is_some(), installed);
                }
            }
            assert_eq!(
                build_v2(
                    design,
                    0,
                    BorrowedMapWeights::Provided(&[1., 0., 1., 1.]),
                    1.,
                    3,
                    limits()
                )
                .err()
                .unwrap()
                .code,
                crate::error::ErrorCode::InvalidData
            );
        });
    }

    #[test]
    fn compact_gathers_run_parallel_and_repeat_exactly() {
        let left: Vec<_> = (0..40000).map(|i| (i / 4) as u32).collect();
        let right: Vec<_> = (0..40000).map(|i| ((i / 4 + i % 4) % 200) as u32).collect();
        with_design(&left, &right, 10000, 200, |design| {
            let mut reference = None;
            for workers in [1, 4, 4] {
                let full = build_v2(design, 0, BorrowedMapWeights::Unit, 1., 4, limits()).unwrap();
                let mut kernel = full.kernel.unwrap();
                assert!(kernel.retained_bytes() <= u128::from(full.peak.bytes().unwrap()));
                let input: Vec<_> = (0..800)
                    .map(|i| ((i * 13 % 53) as f64 - 26.) / 8.)
                    .collect();
                let mut output = vec![99.; 800];
                let mut scratch = vec![99.; 40000];
                crate::ThreadTeam::new(crate::ThreadConfig::from_requested(workers, true).unwrap())
                    .install(|| {
                        kernel
                            .apply_compact(&input, &mut output, &mut scratch, &[0., 1., 0., -1.], 4)
                            .unwrap()
                    })
                    .unwrap();
                if let Some(ref prior) = reference {
                    assert_eq!(&output, prior);
                } else {
                    reference = Some(output);
                }
            }
        });
    }
    #[test]
    fn compact_dominant_anchor_preserves_tiny_two_cell_conductance() {
        with_design(&[0, 0], &[0, 1], 1, 2, |design| {
            for tiny in [2.0_f64.powi(-54), 1e-100, 1e-300] {
                for weights in [[tiny, 1.], [1., tiny]] {
                    let mut kernel = build_v2(
                        design,
                        0,
                        BorrowedMapWeights::Provided(&weights),
                        1.,
                        2,
                        limits(),
                    )
                    .unwrap()
                    .kernel
                    .unwrap();
                    let mut output = [99.; 4];
                    kernel
                        .apply_compact(&[0., 1., 1., 0.], &mut output, &mut [0.; 2], &[1., 1.], 2)
                        .unwrap();
                    assert_eq!(output, [-tiny, tiny, tiny, -tiny]);
                    assert_eq!(kernel.diagonal(), &[tiny, tiny]);
                }
            }
        });
    }
    #[test]
    fn compact_extreme_weighted_differences_remain_finite() {
        with_design(&[0, 0, 1], &[0, 1, 2], 2, 3, |design| {
            let weights = [1e-320, 1e-320, 1.];
            let mut kernel = build_v2(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.,
                1,
                limits(),
            )
            .unwrap()
            .kernel
            .unwrap();
            let mut output = [99.; 3];
            kernel
                .apply_compact(&[1e308, -1e308, 1.], &mut output, &mut [0.; 2], &[1.], 1)
                .unwrap();
            let expected = weights[0] * 1e308;
            assert!((output[0] / expected - 1.).abs() < 1e-14);
            assert!((output[1] / expected + 1.).abs() < 1e-14);
            assert_eq!(output[2], 0.);
        });
        assert_eq!(
            std::mem::size_of::<CellSchur>(),
            5 * std::mem::size_of::<Vec<f64>>(),
            "incumbent owner layout remains unchanged"
        );
    }

    #[test]
    fn compact_builder_and_application_propagate_break_and_allocation() {
        const CHILD: &str = "FEREG_COMPACT_BREAK_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap()).env(CHILD, "1")
                .args(["fe::routing_kernel::tests::compact_builder_and_application_propagate_break_and_allocation", "--exact", "--test-threads=1"])
                .status().unwrap();
            assert!(status.success());
            return;
        }
        with_design(&[0, 0], &[0, 1], 1, 2, |design| {
            let mut kernel = build_v2(design, 0, BorrowedMapWeights::Unit, 1., 1, limits())
                .unwrap()
                .kernel
                .unwrap();
            let twin = engine(design, BorrowedMapWeights::Unit, limits())
                .kernel
                .unwrap();
            crate::interrupt::request_cancellation();
            let preset = super::super::RoutingPreset::ablation("cells_e").unwrap();
            assert_eq!(
                build_engine(design, 0, BorrowedMapWeights::Unit, 1., limits(), preset)
                    .err()
                    .unwrap()
                    .code,
                crate::error::ErrorCode::Interrupted
            );
            for error in [
                twin.apply_two_pass(&[0., 1.], &mut [0.; 2], &mut [0.], 1, 1)
                    .unwrap_err(),
                twin.subtract_left_means(&[0.], &mut [0.; 2], 1, 1)
                    .unwrap_err(),
                twin.reconstruct_left(&[0., 1.], &[0.], &[2.], &mut [0.], 1, 1)
                    .unwrap_err(),
            ] {
                assert_eq!(error.code, crate::error::ErrorCode::Interrupted);
            }
            let occurrence_error =
                build_occurrence(design, 0, BorrowedMapWeights::Unit, 1., limits())
                    .err()
                    .unwrap();
            assert_eq!(occurrence_error.code, crate::error::ErrorCode::Interrupted);
            let gated_error =
                build_occurrence_gated(design, 0, BorrowedMapWeights::Unit, 1., limits(), 1)
                    .err()
                    .unwrap();
            assert_eq!(gated_error.code, crate::error::ErrorCode::Interrupted);
            let membership_error =
                build_membership(design, 0, BorrowedMapWeights::Unit, 1., limits())
                    .err()
                    .unwrap();
            assert_eq!(membership_error.code, crate::error::ErrorCode::Interrupted);
            let build_error = build_v2(design, 0, BorrowedMapWeights::Unit, 1., 1, limits())
                .err()
                .unwrap();
            let apply_error = kernel
                .apply_compact(&[1., -1.], &mut [0.; 2], &mut [0.], &[1.], 1)
                .unwrap_err();
            crate::interrupt::reset_cancellation();
            assert_eq!(build_error.code, crate::error::ErrorCode::Interrupted);
            assert_eq!(apply_error.code, crate::error::ErrorCode::Interrupted);
            let mut work = Work {
                limit: u64::MAX,
                used: 0,
            };
            match work.capacity::<Cell>(usize::MAX).err().unwrap() {
                Stop::Fatal(error) => assert_eq!(error.code, crate::error::ErrorCode::Allocation),
                Stop::Refused(_) => panic!("allocation failure was swallowed"),
            }
        });
    }
    #[test]
    fn compact_unrepresentable_shift_is_recurrence_breakdown_not_fatal_error() {
        let workers = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let firms = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut weights = [1e-320; 11];
        weights[10] = 1.;
        with_design(&workers, &firms, 2, 11, |design| {
            let mut kernel = build_v2(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                1.,
                1,
                limits(),
            )
            .unwrap()
            .kernel
            .unwrap();
            let mut input = [-1e308; 11];
            input[0] = 1e308;
            assert!(!kernel
                .apply_compact(&input, &mut [0.; 11], &mut [0.; 2], &[1.], 1)
                .unwrap());
            input[0] = f64::INFINITY;
            assert_eq!(
                kernel
                    .apply_compact(&input, &mut [0.; 11], &mut [0.; 2], &[1.], 1)
                    .unwrap_err()
                    .code,
                crate::error::ErrorCode::InvalidData
            );
        });
    }
}
