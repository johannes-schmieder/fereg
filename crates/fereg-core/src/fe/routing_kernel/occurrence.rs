//! One membership pass, aggregating in source order without sorting cells.
//! Only this private builder changes cell order and completed-bank reductions.
use super::membership::{capacity, filled, finish};
use super::*;

pub(crate) fn build_occurrence(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    limits: KernelLimits,
) -> Result<OptionalSetup> {
    build(design, left, weights, scale, limits, None)
}

pub(crate) fn build_occurrence_gated(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    limits: KernelLimits,
    max_rhs: usize,
) -> Result<OptionalSetup> {
    build(design, left, weights, scale, limits, Some(max_rhs))
}

// The affine cost difference is nonnegative at every intermediate width if
// it is nonnegative at both endpoints. Ignore the old u64 cost cap here:
// overflow in that cap alone is not proof that a smaller width cannot pay.
pub(crate) fn cell_limit(
    n: usize,
    p: usize,
    q: usize,
    rhs: usize,
    weighted: bool,
) -> Option<usize> {
    if rhs == 0 {
        return None;
    }
    let endpoint = |r: u128| -> Option<u128> {
        let ten = r.checked_mul(10)?;
        let observations =
            (n as u128).checked_mul(ten.checked_add(if weighted { 8 } else { 4 })?)?;
        let fixed = (p as u128)
            .checked_mul(r.checked_mul(4)?.checked_add(8)?)?
            .checked_add((q as u128).checked_mul(r.checked_mul(6)?.checked_add(8)?)?)?
            .checked_add(128)?;
        Some(
            observations
                .checked_sub(fixed)
                .and_then(|v| v.checked_sub(1))
                .map_or(0, |v| v / (ten + 16)),
        )
    };
    usize::try_from(endpoint(1)?.max(endpoint(rhs as u128)?).min(n as u128)).ok()
}

fn build(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    limits: KernelLimits,
    max_rhs: Option<usize>,
) -> Result<OptionalSetup> {
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
    let mut work = Work {
        limit: limits.work_budget,
        used: 0,
    };
    let mut peak = SolverWorkspacePeak::ZERO;
    let attempt = (|| -> Attempt<CellSchur> {
        work.charge(128)?;
        let (n, p, q) = (
            design.nobs(),
            design.effects()[left].n_levels(),
            design.effects()[1 - left].n_levels(),
        );
        let limit = if let Some(rhs) = max_rhs {
            work.charge(128)?;
            cell_limit(
                n,
                p,
                q,
                rhs,
                matches!(weights, BorrowedMapWeights::Provided(_)),
            )
        } else {
            None
        };
        if let Some(limit) = limit.filter(|&limit| p.max(q) > limit) {
            return Err(Stop::Refused(KernelRefusal::Unprofitable {
                cells: p.max(q),
                limit,
            }));
        }
        // Marker pass, then compaction with markers dropped, then suffix and
        // final banks. Counts fit usize; their byte arithmetic is widened.
        let cells = n as u128 * std::mem::size_of::<Cell>() as u128;
        let markers = 2 * q as u128 * std::mem::size_of::<usize>() as u128;
        let banks = (p as u128 + 1) * std::mem::size_of::<usize>() as u128
            + (p as u128 + 2 * q as u128) * 8;
        let bytes = (cells + markers)
            .max(2 * cells)
            .max(cells + (n as u128 + 1) * 8)
            + banks
            + 4096;
        if !limits
            .retained_live_bytes
            .checked_add(bytes)
            .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)))
        {
            return Err(Stop::Refused(KernelRefusal::MemoryBudget));
        }
        peak = SolverWorkspacePeak::from_bytes(bytes)?;
        construct(
            design,
            left,
            weights,
            scale,
            &mut work,
            max_rhs.is_some(),
            limit,
        )
    })();
    let (kernel, refusal) = match attempt {
        Ok(kernel) => (Some(kernel), None),
        Err(Stop::Refused(reason)) => (None, Some(reason)),
        Err(Stop::Fatal(error)) => return Err(error),
    };
    Ok(OptionalSetup {
        kernel,
        refusal,
        work_used: work.used,
        peak,
    })
}

fn construct(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    work: &mut Work,
    gated: bool,
    limit: Option<usize>,
) -> Attempt<CellSchur> {
    let effect = &design.effects()[left];
    let firms = design.effects()[1 - left].codes();
    let (p, q) = (effect.n_levels(), design.effects()[1 - left].n_levels());
    let mut cells: Vec<Cell> = capacity(work, design.nobs())?;
    let mut marks = filled(work, q, usize::MAX)?;
    let mut slots = filled(work, q, 0usize)?;
    for worker in 0..p {
        work.charge(16)?;
        let members = effect.level_index().checked_members(worker)?;
        // Exactly the original 1024-work-unit (32-row) tiles. S2 additionally
        // charges eight units before each bounded count comparison.
        for rows in members.chunks(32) {
            work.charge(rows.len() as u16 * 32)?;
            if gated {
                work.charge(8)?;
            }
            for &row in rows {
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
            }
            if let Some(limit) = limit.filter(|&limit| cells.len() > limit) {
                return Err(Stop::Refused(KernelRefusal::Unprofitable {
                    cells: cells.len(),
                    limit,
                }));
            }
        }
    }
    drop(marks);
    drop(slots);
    let mut compact = capacity(work, cells.len())?;
    work.tiles(cells.len(), 4, |range| {
        compact.extend_from_slice(&cells[range]);
        Ok(())
    })?;
    drop(cells);
    finish(compact, p, q, work)
}
