//! Independent, current-weight two-FE kernel experiments.
//!
//! Scope: cell regrouping and Schur-diagonal arithmetic may change low bits.
//! Sample, original factor slots, rank and omission decisions are untouched.
//! The caller retains the original observation operator for certification.

use crate::error::{allocation, invalid_argument, invalid_data, FeregError, Result};
use crate::memory::{FeSolverMemoryBudget, SolverWorkspacePeak};

use super::{BorrowedInterceptFeDesign, BorrowedMapWeights};

mod occurrence;
pub(crate) use occurrence::build_occurrence;

crate::fault::fault_plan! {
    static CELL_ALLOCATION_FAILURE: bool = false;
    static CELL_CANCEL_PHASE: u8 = 0;
    static CELL_COMPLETED_WORK: usize = 0;
    static CELL_RECORD_ACTIVITY: bool = false;
    static CELL_BUILD_COUNT: usize = 0;
    static CELL_APPLY_COUNT: usize = 0;
}

// Test-only sticky cancellation is triggered after real constructor rows or
// operator lanes, never by an estimated poll ordinal. Plans compile away.
fn completed_work(phase: u8, units: usize) {
    if CELL_CANCEL_PHASE.get() == phase {
        CELL_COMPLETED_WORK.set(CELL_COMPLETED_WORK.get() + units);
        crate::interrupt::request_cancellation();
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct KernelLimits {
    pub retained_live_bytes: u128,
    pub memory: Option<FeSolverMemoryBudget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KernelRefusal {
    MemoryBudget,
    Numerical,
}

pub(crate) struct OptionalSetup {
    pub kernel: Option<CellSchur>,
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
    /// True Schur diagonal, with exactly-zero entries replaced by D_f solely
    /// for preconditioning. This is never a numerical-rank classification.
    pub fn diagonal(&self) -> &[f64] {
        &self.diagonal
    }

    pub(crate) fn cell_count(&self) -> usize {
        self.cells.len()
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
            for (tile, chunk) in work.chunks_mut(256).enumerate() {
                if tile != 0 {
                    crate::interrupt::checkpoint()?;
                }
                chunk.fill(0.0);
                completed_work(2, chunk.len());
            }
            for cell in &self.cells[self.offsets[worker]..self.offsets[worker + 1]] {
                crate::interrupt::checkpoint()?;
                let source =
                    &input[cell.firm as usize * stride..cell.firm as usize * stride + rhs_count];
                for (tile, (target, source)) in
                    work.chunks_mut(256).zip(source.chunks(256)).enumerate()
                {
                    if tile != 0 {
                        crate::interrupt::checkpoint()?;
                    }
                    for (sum, &value) in target.iter_mut().zip(source) {
                        *sum += cell.weight * value;
                    }
                    completed_work(3, 2 * source.len());
                    if CELL_RECORD_ACTIVITY.get() {
                        CELL_APPLY_COUNT.set(CELL_APPLY_COUNT.get() + 1);
                    }
                }
            }
            if total > 0.0 {
                for chunk in work.chunks_mut(256) {
                    crate::interrupt::checkpoint()?;
                    for value in chunk.iter_mut() {
                        *value /= total;
                    }
                    completed_work(2, chunk.len());
                }
            }
        }
        for (firm, &total) in self.firm_weights.iter().enumerate() {
            crate::interrupt::checkpoint()?;
            for start in (0..rhs_count).step_by(256) {
                if start != 0 {
                    crate::interrupt::checkpoint()?;
                }
                let end = (start + 256).min(rhs_count);
                for rhs in start..end {
                    output[firm * stride + rhs] = total * input[firm * stride + rhs];
                }
                completed_work(2, 2 * (end - start));
            }
        }
        for cell in &self.cells {
            crate::interrupt::checkpoint()?;
            for start in (0..rhs_count).step_by(256) {
                if start != 0 {
                    crate::interrupt::checkpoint()?;
                }
                let end = (start + 256).min(rhs_count);
                for rhs in start..end {
                    output[cell.firm as usize * stride + rhs] -=
                        cell.weight * left_scratch[cell.worker as usize * stride + rhs];
                }
                completed_work(2, 2 * (end - start));
            }
        }
        for firm in 0..self.firm_weights.len() {
            crate::interrupt::checkpoint()?;
            for start in (0..rhs_count).step_by(256) {
                if start != 0 {
                    crate::interrupt::checkpoint()?;
                }
                let end = (start + 256).min(rhs_count);
                for rhs in start..end {
                    if !output[firm * stride + rhs].is_finite() {
                        return Err(invalid_data("cell Schur product is nonfinite"));
                    }
                }
                completed_work(2, end - start);
            }
        }
        crate::interrupt::checkpoint()
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

/// Cancellation-only construction steps: each bounded operation of at most
/// 1024 scalar units polls the host flag before it runs. There is no allowance.
struct Work;
impl Work {
    fn charge(&mut self, units: u16) -> Attempt<()> {
        crate::interrupt::checkpoint()?;
        if units == 0 || units > 1024 {
            return Err(invalid_argument("invalid cell-kernel work chunk").into());
        }
        Ok(())
    }
    fn capacity<T>(&mut self, count: usize) -> Attempt<Vec<T>> {
        self.charge(16)?;
        if CELL_ALLOCATION_FAILURE.replace(false) {
            return Err(allocation("injected cell kernel allocation failure").into());
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| allocation("cell kernel allocation failed"))?;
        if result.capacity() > count {
            return Err(allocation("cell kernel capacity exceeds admission").into());
        }
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

/// Use the incumbent's validated normalization scale without recomputing it.
/// Positive row weights are divided exactly once, as in ValidatedWeights.
/// Memory refusal discards private partial storage; allocator failure and
/// cancellation are fatal.
pub(crate) fn build(
    design: &BorrowedInterceptFeDesign<'_>,
    left_effect: usize,
    weights: BorrowedMapWeights<'_>,
    weight_scale: f64,
    limits: KernelLimits,
) -> Result<OptionalSetup> {
    let mut work = Work;
    match work.charge(128) {
        Ok(()) => {}
        Err(Stop::Refused(reason)) => {
            return Ok(OptionalSetup {
                kernel: None,
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
    finish(cells, p, q, work)
}

// The caller's cell order is numerical authority; do not sort here.
fn finish(cells: Vec<Cell>, p: usize, q: usize, work: &mut Work) -> Attempt<CellSchur> {
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
            retained_live_bytes: 0,
            memory: None,
        }
    }

    #[test]
    fn repeated_weighted_cells_match_independent_rectangle_conductance() {
        let left = [0, 0, 0, 1, 1, 1, 2, 2];
        let right = [0, 0, 1, 0, 1, 1, 2, 2];
        let weights = [2.0, 1.0, 3.0, 4.0, 5.0, 2.0, 7.0, 1.0];
        with_design(&left, &right, 3, 3, |design| {
            let setup = build(
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
            let second = build(
                design,
                0,
                BorrowedMapWeights::Provided(&weights),
                7.0,
                limits(),
            )
            .unwrap();
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

    #[test]
    fn cell_operator_matches_independent_dense_observation_elimination() {
        let left = [2, 0, 1, 0, 2, 1, 0, 1, 2, 0, 2, 1];
        let right = [0, 1, 3, 0, 2, 1, 1, 3, 3, 2, 0, 0];
        let weights = [1.0, 2.0, 4.0, 3.0, 1.0, 5.0, 2.0, 1.0, 7.0, 3.0, 2.0, 4.0];
        with_design(&left, &right, 3, 4, |design| {
            for side in [0, 1] {
                let kernel = build(
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
                                (weights[i] / 7.0) * (weights[j] / 7.0) / dw[workers[i] as usize];
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

    #[test]
    fn diagonal_retains_tiny_positive_complements_without_subtraction() {
        with_design(&[0, 0], &[0, 1], 1, 2, |design| {
            for tiny in [1e-100, 1e-300, f64::from_bits(1)] {
                let weights = [1.0, tiny];
                let kernel = build(
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

    #[test]
    fn setup_memory_refusal_preserves_inputs() {
        let left = [0, 0, 1, 1];
        let right = [0, 1, 0, 1];
        with_design(&left, &right, 2, 2, |design| {
            let full = build(design, 0, BorrowedMapWeights::Unit, 1.0, limits()).unwrap();
            assert!(full.kernel.is_some() && full.refusal.is_none());
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
}

#[cfg(test)]
#[path = "routing_kernel/occurrence_tests.rs"]
mod occurrence_tests;

#[cfg(test)]
pub(crate) fn recorded_activity<T>(action: impl FnOnce() -> T) -> (T, usize, usize) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            CELL_RECORD_ACTIVITY.set(false);
        }
    }
    assert!(!CELL_RECORD_ACTIVITY.replace(true));
    let _guard = Guard;
    CELL_BUILD_COUNT.set(0);
    CELL_APPLY_COUNT.set(0);
    let result = action();
    (result, CELL_BUILD_COUNT.get(), CELL_APPLY_COUNT.get())
}
