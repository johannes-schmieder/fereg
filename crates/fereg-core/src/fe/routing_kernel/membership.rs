//! Original cell arithmetic, constructed from stable level memberships.
use super::*;

pub(crate) fn build_membership(
    design: &BorrowedInterceptFeDesign<'_>,
    left: usize,
    weights: BorrowedMapWeights<'_>,
    scale: f64,
    limits: KernelLimits,
) -> Result<OptionalSetup> {
    if design.effects().len() != 2 || left >= 2 || !scale.is_finite() || scale <= 0.0 {
        return Err(invalid_argument(
            "invalid membership cell dimensions or scale",
        ));
    }
    if let BorrowedMapWeights::Provided(values) = weights {
        if values.len() != design.nobs() {
            return Err(invalid_argument("membership cell weight length differs"));
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
        // Sorting: n cells plus at most n scratch cells. After dropping scratch,
        // fallible compaction temporarily overlaps n and C cells. Final banks
        // plus suffix use 24C+8 bytes. Include all dimension banks conservatively.
        let bytes = n as u128 * 32 + (p as u128 * 2 + q as u128 * 2 + 2) * 8 + 4096;
        if !limits
            .retained_live_bytes
            .checked_add(bytes)
            .is_some_and(|total| limits.memory.is_none_or(|budget| budget.fits(total)))
        {
            return Err(Stop::Refused(KernelRefusal::MemoryBudget));
        }
        peak = SolverWorkspacePeak::from_bytes(bytes)?;
        construct(design, left, weights, scale, &mut work)
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

pub(super) fn capacity<T>(work: &mut Work, count: usize) -> Attempt<Vec<T>> {
    let bank = work.capacity(count)?;
    if bank.capacity() > count {
        return Err(allocation("membership cell capacity exceeds admission").into());
    }
    Ok(bank)
}

pub(super) fn filled<T: Copy>(work: &mut Work, count: usize, value: T) -> Attempt<Vec<T>> {
    let mut bank = capacity(work, count)?;
    work.tiles(count, 4, |range| {
        bank.resize(range.end, value);
        Ok(())
    })?;
    Ok(bank)
}

// Stable firm-only bottom-up merge. Membership rows are in source order in
// both LevelIndex constructors. Equal-pair weights therefore sum in exactly
// the original global stable radix order, including heterogeneous weights.
fn sort(rows: &mut [Cell], scratch: &mut [Cell], work: &mut Work) -> Attempt<()> {
    let mut width = 1;
    while width < rows.len() {
        for first in (0..rows.len()).step_by(width.saturating_mul(2)) {
            let mid = first.saturating_add(width).min(rows.len());
            let end = mid.saturating_add(width).min(rows.len());
            let (mut i, mut j) = (first, mid);
            work.tiles(end - first, 32, |range| {
                for k in range {
                    let take_left = j == end || (i < mid && rows[i].firm <= rows[j].firm);
                    scratch[first + k] = if take_left {
                        let cell = rows[i];
                        i += 1;
                        cell
                    } else {
                        let cell = rows[j];
                        j += 1;
                        cell
                    };
                }
                Ok(())
            })?;
        }
        work.tiles(rows.len(), 4, |range| {
            rows[range.clone()].copy_from_slice(&scratch[range]);
            Ok(())
        })?;
        width = width.saturating_mul(2);
    }
    Ok(())
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
    let membership = effect.level_index().membership_offsets();
    let mut degree = 0;
    work.tiles(p, 4, |range| {
        for worker in range {
            degree = degree.max(membership[worker + 1] - membership[worker]);
        }
        Ok(())
    })?;
    let mut cells: Vec<Cell> = capacity(work, design.nobs())?;
    let mut scratch = filled(work, degree, Cell::default())?;
    for worker in 0..p {
        work.charge(16)?;
        let start = cells.len();
        let members = effect.level_index().checked_members(worker)?;
        work.tiles(members.len(), 16, |range| {
            for i in range {
                let row = members[i];
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
                cells.push(Cell {
                    worker: worker as u32,
                    firm: firms[row],
                    weight,
                });
            }
            Ok(())
        })?;
        sort(&mut cells[start..], &mut scratch, work)?;
        let mut write = start;
        work.tiles(cells.len() - start, 24, |range| {
            for i in range {
                let cell = cells[start + i];
                if write > start && cells[write - 1].firm == cell.firm {
                    cells[write - 1].weight += cell.weight;
                    if !cells[write - 1].weight.is_finite() {
                        return Err(Stop::Refused(KernelRefusal::Numerical));
                    }
                } else {
                    cells[write] = cell;
                    write += 1;
                }
            }
            Ok(())
        })?;
        cells.truncate(write);
    }
    drop(scratch);
    let mut compact = capacity(work, cells.len())?;
    work.tiles(cells.len(), 4, |range| {
        compact.extend_from_slice(&cells[range]);
        Ok(())
    })?;
    drop(cells);
    finish(compact, p, q, work)
}

// Preserve reduction order supplied by the builder; no canonicalization here.
pub(super) fn finish(cells: Vec<Cell>, p: usize, q: usize, work: &mut Work) -> Attempt<CellSchur> {
    let mut offsets = filled(work, p + 1, 0usize)?;
    let mut worker_weights = filled(work, p, 0.0)?;
    let mut firm_weights = filled(work, q, 0.0)?;
    let mut diagonal = filled(work, q, 0.0)?;
    work.tiles(cells.len(), 16, |range| {
        for i in range {
            let cell = cells[i];
            offsets[cell.worker as usize + 1] += 1;
            worker_weights[cell.worker as usize] += cell.weight;
            firm_weights[cell.firm as usize] += cell.weight;
        }
        Ok(())
    })?;
    work.tiles(p, 4, |range| {
        for i in range {
            offsets[i + 1] += offsets[i];
        }
        Ok(())
    })?;
    let mut suffix = filled(work, cells.len() + 1, 0.0)?;
    for worker in 0..p {
        work.charge(16)?;
        let (start, end) = (offsets[worker], offsets[worker + 1]);
        let total = worker_weights[worker];
        if !total.is_finite() {
            return Err(Stop::Refused(KernelRefusal::Numerical));
        }
        suffix[end] = 0.0;
        work.tiles(end - start, 8, |range| {
            for j in range {
                let i = end - 1 - j;
                suffix[i] = suffix[i + 1] + cells[i].weight;
            }
            Ok(())
        })?;
        let mut prefix = 0.0;
        work.tiles(end - start, 32, |range| {
            for j in range {
                let i = start + j;
                let cell = cells[i];
                let other = prefix + suffix[i + 1];
                let contribution = if cell.weight >= other {
                    (cell.weight / total) * other
                } else {
                    (other / total) * cell.weight
                };
                diagonal[cell.firm as usize] += contribution;
                prefix += cell.weight;
            }
            Ok(())
        })?;
    }
    work.tiles(q, 16, |range| {
        for firm in range {
            if !firm_weights[firm].is_finite()
                || !diagonal[firm].is_finite()
                || diagonal[firm] < 0.0
            {
                return Err(Stop::Refused(KernelRefusal::Numerical));
            }
            if diagonal[firm] == 0.0 {
                diagonal[firm] = firm_weights[firm];
            }
        }
        Ok(())
    })?;
    work.charge(32)?;
    Ok(CellSchur {
        cells,
        offsets,
        worker_weights,
        firm_weights,
        diagonal,
    })
}
