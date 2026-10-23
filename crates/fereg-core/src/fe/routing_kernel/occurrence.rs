//! Source-order membership aggregation: first occurrence fixes each cell's slot.
use super::*;

pub(crate) fn build_occurrence(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    limits: KernelLimits,
) -> Result<OptionalSetup> {
    crate::interrupt::checkpoint()?;
    if design.effects().len() != 2 || left >= 2 || !scale.is_finite() || scale <= 0.0 {
        return Err(invalid_argument(
            "invalid occurrence cell dimensions or scale",
        ));
    }
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != design.nobs() {
            return Err(invalid_argument("occurrence cell weight length differs"));
        }
    }
    let (n, p, q) = (
        design.nobs() as u128,
        design.effects()[left].n_levels() as u128,
        design.effects()[1 - left].n_levels() as u128,
    );
    // Markers die before compaction; the full and compact cell banks coexist.
    // Finishing then owns the compact bank, suffix, and every dimension bank.
    // All arithmetic is widened before multiplying addressable dimensions.
    let cells = n * std::mem::size_of::<Cell>() as u128;
    let markers = 2 * q * std::mem::size_of::<usize>() as u128;
    let banks = (p + 1) * std::mem::size_of::<usize>() as u128 + (p + 2 * q) * 8;
    let bytes = (cells + markers).max(2 * cells).max(cells + (n + 1) * 8) + banks + 4096;
    if !limits
        .retained_live_bytes
        .checked_add(bytes)
        .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)))
    {
        return Ok(OptionalSetup {
            kernel: None,
            peak: SolverWorkspacePeak::ZERO,
            refusal: Some(KernelRefusal::MemoryBudget),
        });
    }
    let (kernel, refusal) = match construct(design, left, weights, scale, &mut Work) {
        Ok(kernel) => (Some(kernel), None),
        Err(Stop::Refused(reason)) => (None, Some(reason)),
        Err(Stop::Fatal(error)) => return Err(error),
    };
    if kernel.is_some() && CELL_RECORD_ACTIVITY.get() {
        CELL_BUILD_COUNT.set(CELL_BUILD_COUNT.get() + 1);
    }
    // Catch a sticky cancellation requested after the final completed tile.
    crate::interrupt::checkpoint()?;
    Ok(OptionalSetup {
        kernel,
        refusal,
        peak: SolverWorkspacePeak::from_bytes(bytes)?,
    })
}

fn construct(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    work: &mut Work,
) -> Attempt<CellSchur> {
    let effect = &design.effects()[left];
    let firms = design.effects()[1 - left].codes();
    let (p, q) = (effect.n_levels(), design.effects()[1 - left].n_levels());
    let mut cells: Vec<Cell> = work.capacity(design.nobs())?;
    let mut marks = work.filled(q, usize::MAX)?;
    let mut slots = work.filled(q, 0usize)?;
    for worker in 0..p {
        work.charge(16)?;
        let members = effect.level_index().checked_members(worker)?;
        for &row in members {
            work.charge(32)?;
            let firm = firms[row] as usize;
            let weight = match weights {
                BorrowedMapWeights::Unit => 1.0,
                BorrowedMapWeights::Provided(values) => values[row] / scale,
            };
            if !weight.is_finite() || weight <= 0.0 {
                return Err(invalid_data(
                    "cell kernel normalized weight must be finite and positive",
                )
                .into());
            }
            if marks[firm] != worker {
                marks[firm] = worker;
                slots[firm] = cells.len();
                cells.push(Cell {
                    worker: worker as u32,
                    firm: firm as u32,
                    weight,
                });
            } else {
                let total = &mut cells[slots[firm]].weight;
                *total += weight;
                if !total.is_finite() {
                    return Err(Stop::Refused(KernelRefusal::Numerical));
                }
            }
            completed_work(1, 32);
        }
    }
    drop(marks);
    drop(slots);
    let mut compact = work.capacity(cells.len())?;
    for chunk in cells.chunks(256) {
        work.charge((chunk.len() * 4) as u16)?;
        compact.extend_from_slice(chunk);
    }
    drop(cells);
    finish(compact, p, q, work)
}
