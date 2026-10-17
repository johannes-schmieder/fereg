//! Private S cell engine: stable firm transpose and grouped contractions.
//! The original observation operator remains the independent certificate.
use super::membership::{capacity, filled};
use super::*;
use crate::parallel::StagePlan;

pub(crate) fn build_engine(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    limits: KernelLimits,
    preset: super::super::RoutingPreset,
) -> Result<OptionalSetup> {
    let mut built = build_occurrence(design, left, weights, scale, limits)?;
    let Some(kernel) = built.kernel.as_mut() else {
        return Ok(built);
    };
    // Keep the next S pricing decision available even if this optional stage
    // exhausts its allowance. Sunk work is retained across subsequent batches.
    let mut work = Work {
        limit: limits
            .work_budget
            .saturating_sub(built.work_used)
            .saturating_sub(128),
        used: 0,
    };
    let mut peak = SolverWorkspacePeak::ZERO;
    let attempt = (|| -> Attempt<()> {
        work.charge(128)?;
        let (p, q, c) = (
            kernel.worker_weights.len(),
            kernel.firm_weights.len(),
            kernel.cells.len(),
        );
        let count = c
            .checked_mul(2)
            .ok_or(Stop::Refused(KernelRefusal::MemoryBudget))?;
        let offsets_len = p
            .checked_add(q)
            .and_then(|v| v.checked_add(2))
            .ok_or(Stop::Refused(KernelRefusal::MemoryBudget))?;
        // Old owner stays alive until publication. New banks and cursor all
        // coexist; capacities are checked by the fallible allocation helpers.
        let bytes = kernel.retained_bytes()
            + count as u128 * 16
            + offsets_len as u128 * std::mem::size_of::<usize>() as u128
            + q as u128 * std::mem::size_of::<usize>() as u128
            + 4096;
        if !limits
            .retained_live_bytes
            .checked_add(bytes)
            .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)))
        {
            return Err(Stop::Refused(KernelRefusal::MemoryBudget));
        }
        peak = SolverWorkspacePeak::from_bytes(bytes)?;
        let mut cells = filled(&mut work, count, Cell::default())?;
        let mut offsets = filled(&mut work, offsets_len, 0usize)?;
        let mut cursor: Vec<usize> = capacity(&mut work, q)?;
        work.tiles(p + 1, 4, |range| {
            offsets[range.clone()].copy_from_slice(&kernel.offsets[range]);
            Ok(())
        })?;
        let firm = &mut offsets[p + 1..];
        work.tiles(c, 12, |range| {
            for i in range {
                let cell = kernel.cells[i];
                cells[i] = cell;
                firm[cell.firm as usize + 1] += 1;
            }
            Ok(())
        })?;
        work.tiles(q, 8, |range| {
            for f in range {
                cursor.push(firm[f]);
                firm[f + 1] += firm[f];
            }
            Ok(())
        })?;
        work.tiles(c, 16, |range| {
            for i in range {
                let cell = cells[i];
                let slot = &mut cursor[cell.firm as usize];
                cells[c + *slot] = cell;
                *slot += 1;
            }
            Ok(())
        })?;
        // No partially built bank can escape on optional refusal or Break.
        work.charge(16)?;
        kernel.cells = cells;
        kernel.offsets = offsets;
        Ok(())
    })();
    match attempt {
        Ok(()) => {}
        Err(Stop::Refused(reason)) => built.refusal = Some(reason),
        Err(Stop::Fatal(error)) => return Err(error),
    }
    built.work_used += work.used;
    built.peak = built.peak.max(peak);
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=auto_cells_twin|preset={preset}|installed={}|refusal={:?}|work={}|peak_bytes={}",
            u8::from(kernel.two_pass()), built.refusal, work.used, peak.bytes().unwrap_or(0)));
    }
    Ok(built)
}

impl CellSchur {
    pub(crate) fn two_pass(&self) -> bool {
        let p = self.worker_weights.len();
        self.offsets.len() == p + self.firm_weights.len() + 2
            && self.offsets[p].checked_mul(2) == Some(self.cells.len())
    }

    fn dimensions(&self, right: usize, left: usize, rhs: usize, stride: usize) -> Result<()> {
        crate::interrupt::checkpoint()?;
        if !self.two_pass()
            || stride == 0
            || rhs > stride
            || self.firm_weights.len().checked_mul(stride) != Some(right)
            || self.worker_weights.len().checked_mul(stride) != Some(left)
        {
            return Err(invalid_argument(
                "two-pass cell workspace dimensions differ",
            ));
        }
        Ok(())
    }

    /// Same multiply/add sequence as S for every worker and active coordinate.
    /// No inactive-lane packing or anchoring changes the recurrence arithmetic.
    fn worker_sums(
        &self,
        input: &[f64],
        scratch: &mut [f64],
        rhs: usize,
        stride: usize,
        finish: impl Fn(usize, usize, &mut [f64]) + Sync,
    ) -> Result<()> {
        self.dimensions(input.len(), scratch.len(), rhs, stride)?;
        let offsets = &self.offsets[..self.worker_weights.len() + 1];
        StagePlan::current(
            self.cell_count() as u128 * rhs as u128 * 4,
            self.worker_weights.len(),
        )
        .groups_mut(scratch, stride, offsets, |first, bank| {
            for (local, row) in bank.chunks_exact_mut(stride).enumerate() {
                crate::interrupt::checkpoint()?;
                let worker = first + local;
                let cells = &self.cells[offsets[worker]..offsets[worker + 1]];
                for (block, output) in row[..rhs].chunks_mut(64).enumerate() {
                    crate::interrupt::checkpoint()?;
                    let start = block * 64;
                    output.fill(0.0);
                    for tile in cells.chunks((1024 / (4 * output.len())).max(1)) {
                        crate::interrupt::checkpoint()?;
                        for cell in tile {
                            let source =
                                &input[cell.firm as usize * stride + start..][..output.len()];
                            for (sum, &value) in output.iter_mut().zip(source) {
                                *sum += cell.weight * value;
                            }
                        }
                    }
                    finish(worker, start, output);
                    if output.iter().any(|v| !v.is_finite()) {
                        return Err(invalid_data(
                            "two-pass cell worker contraction is nonfinite",
                        ));
                    }
                }
            }
            Ok(())
        })
    }

    fn firm_subtract(
        &self,
        left: &[f64],
        output: &mut [f64],
        rhs: usize,
        stride: usize,
        input: Option<&[f64]>,
    ) -> Result<()> {
        self.dimensions(output.len(), left.len(), rhs, stride)?;
        if input.is_some_and(|input| input.len() != output.len()) {
            return Err(invalid_argument("two-pass cell input dimensions differ"));
        }
        let offsets = &self.offsets[self.worker_weights.len() + 1..];
        let cells = &self.cells[self.cell_count()..];
        StagePlan::current(
            self.cell_count() as u128 * rhs as u128 * 4,
            self.firm_weights.len(),
        )
        .groups_mut(output, stride, offsets, |first, bank| {
            for (local, row) in bank.chunks_exact_mut(stride).enumerate() {
                crate::interrupt::checkpoint()?;
                let firm = first + local;
                for (block, output) in row[..rhs].chunks_mut(64).enumerate() {
                    crate::interrupt::checkpoint()?;
                    let start = block * 64;
                    if let Some(input) = input {
                        for (lane, value) in output.iter_mut().enumerate() {
                            *value = self.firm_weights[firm] * input[firm * stride + start + lane];
                        }
                    }
                    for tile in cells[offsets[firm]..offsets[firm + 1]]
                        .chunks((1024 / (4 * output.len())).max(1))
                    {
                        crate::interrupt::checkpoint()?;
                        for cell in tile {
                            let source =
                                &left[cell.worker as usize * stride + start..][..output.len()];
                            // Stable transpose preserves S's per-firm subtraction order.
                            for (out, &value) in output.iter_mut().zip(source) {
                                *out -= cell.weight * value;
                            }
                        }
                    }
                    if output.iter().any(|v| !v.is_finite()) {
                        return Err(invalid_data("two-pass cell firm contraction is nonfinite"));
                    }
                }
            }
            Ok(())
        })
    }

    pub(crate) fn apply_two_pass(
        &self,
        input: &[f64],
        output: &mut [f64],
        scratch: &mut [f64],
        rhs: usize,
        stride: usize,
    ) -> Result<()> {
        self.worker_sums(input, scratch, rhs, stride, |worker, _, values| {
            let total = self.worker_weights[worker];
            if total > 0.0 {
                for value in values {
                    *value /= total;
                }
            }
        })?;
        self.firm_subtract(scratch, output, rhs, stride, Some(input))
    }

    pub(crate) fn subtract_left_means(
        &self,
        left: &[f64],
        output: &mut [f64],
        rhs: usize,
        stride: usize,
    ) -> Result<()> {
        self.firm_subtract(left, output, rhs, stride, None)
    }

    pub(crate) fn reconstruct_left(
        &self,
        solution: &[f64],
        left_rhs: &[f64],
        totals: &[f64],
        scratch: &mut [f64],
        rhs: usize,
        stride: usize,
    ) -> Result<()> {
        if left_rhs.len() != scratch.len() || totals.len() != self.worker_weights.len() {
            return Err(invalid_argument(
                "two-pass cell reconstruction dimensions differ",
            ));
        }
        self.worker_sums(solution, scratch, rhs, stride, |worker, start, values| {
            for (lane, value) in values.iter_mut().enumerate() {
                *value = left_rhs[worker * stride + start + lane] - *value / totals[worker];
            }
        })
    }
}
