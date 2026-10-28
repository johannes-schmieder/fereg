//! Exact pass fusion of consecutive effect projections in a MAP sweep.
//!
//! One scaled effect projection walks the matrix three times: the column
//! scales of each level (the largest member magnitude), the scaled
//! compensated sums in member order, and the subtraction of each row's level
//! mean. A maximum is exact in any order, so the next effect's scales are
//! taken in the row-streaming pass that subtracts the current effect's means,
//! with one table per lane merged by maximum. The sums keep their
//! member-order Neumaier recurrence on the same scales, the weight sums keep
//! their member order, and every element is subtracted once with the same
//! operands: the sweep is bitwise the unfused sweep at every worker count.
use super::*;

/// Lane tables of next-effect scales and the current effect's level means.
pub(super) struct FusedSweepScratch {
    lanes: usize,
    columns: usize,
    scales: Vec<f64>,
    means: Vec<f64>,
}

crate::fault::fault_plan! {
    // Test-only comparison arm: always run the unfused sweep.
    static UNFUSED: bool = false;
}

/// One lane's next-effect scale table must stay cache resident: the
/// row-streaming pass raises entries of a random level on every row. A larger
/// table keeps the unfused sweep (identical bits).
const LANE_TABLE_MAX_BYTES: usize = 256 * 1024;

/// Scratch bytes of the fused sweep for this design and matrix shape, or
/// `None` when the sweep keeps its unfused passes.
pub(super) fn planned_bytes(
    design: &BorrowedInterceptFeDesign<'_>,
    rows: usize,
    columns: usize,
) -> Option<u128> {
    if UNFUSED.get() || columns == 0 || columns > PARALLEL_GROUP_MAX_RHS {
        return None;
    }
    let table = max_levels(design)?.checked_mul(columns)?;
    if table.checked_mul(8)? > LANE_TABLE_MAX_BYTES {
        return None;
    }
    let lanes = sweep_lanes(rows, columns);
    Some(table as u128 * (lanes as u128 + 1) * 8)
}

/// The admitted pilot bytes including the fused tables, or `None` when the
/// tables are not planned or do not fit: the unfused sweep then runs, with
/// identical bits.
pub(super) fn admit(
    design: &BorrowedInterceptFeDesign<'_>,
    original: &DenseMatrix,
    bytes: u128,
    budget: Option<FeSolverMemoryBudget>,
) -> Option<u128> {
    planned_bytes(design, original.nrows(), original.ncols())
        .and_then(|extra| bytes.checked_add(extra))
        .filter(|&total| budget.is_none_or(|limit| limit.fits(total)))
}

/// Reserve the admitted tables; an allocation refusal propagates as the
/// pilot's other reservations do.
pub(super) fn reserve(
    design: &BorrowedInterceptFeDesign<'_>,
    original: &DenseMatrix,
    admitted: bool,
) -> Result<Option<FusedSweepScratch>> {
    admitted
        .then(|| FusedSweepScratch::new(design, original.nrows(), original.ncols()))
        .transpose()
}

/// One symmetric pilot sweep: fused when tables were reserved, otherwise the
/// unfused sweep on the MAP workspace.
pub(super) fn symmetric_sweep(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    fused: Option<&mut FusedSweepScratch>,
    coefficients: Option<&mut FeCoefficientBatch>,
) -> Result<()> {
    match fused {
        Some(scratch) => sweep(design, weights, values, scratch, coefficients, true),
        None => borrowed_residual_sweep_recording(
            design,
            weights,
            values,
            workspace,
            coefficients,
            true,
        ),
    }
}

fn max_levels(design: &BorrowedInterceptFeDesign<'_>) -> Option<usize> {
    design
        .effects()
        .iter()
        .map(crate::fe::BorrowedInterceptEffect::n_levels)
        .max()
}

impl FusedSweepScratch {
    pub(super) fn new(
        design: &BorrowedInterceptFeDesign<'_>,
        rows: usize,
        columns: usize,
    ) -> Result<Self> {
        let lanes = sweep_lanes(rows, columns);
        let table = max_levels(design)
            .unwrap_or(0)
            .checked_mul(columns)
            .ok_or_else(|| allocation("fused MAP scratch dimensions overflow"))?;
        let scales = fallible_zeros(
            table
                .checked_mul(lanes)
                .ok_or_else(|| allocation("fused MAP scratch dimensions overflow"))?,
        )?;
        let means = fallible_zeros(table)?;
        Ok(Self {
            lanes,
            columns,
            scales,
            means,
        })
    }

    pub(super) fn capacity_bytes(&self) -> u128 {
        (self.scales.capacity() as u128 + self.means.capacity() as u128) * 8
    }
}

fn fallible_zeros(elements: usize) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(elements)
        .map_err(|_| allocation("unable to reserve fused MAP scratch"))?;
    values.resize(elements, 0.0);
    Ok(values)
}

/// One sweep (forward, then backward when `symmetric`) with fused passes;
/// bitwise [`borrowed_residual_sweep_recording`] on unit or validated weights.
/// The scratch must have been built for this design and matrix shape.
pub(super) fn sweep<R: InterceptCoefficientRecorder>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    scratch: &mut FusedSweepScratch,
    mut coefficients: Option<&mut R>,
    symmetric: bool,
) -> Result<()> {
    let effects = design.effects();
    let count = effects.len();
    if count == 0 {
        return Ok(());
    }
    if values.ncols() != scratch.columns
        || scratch.lanes != sweep_lanes(values.nrows(), values.ncols())
        || scratch.means.len() < max_levels(design).unwrap_or(0) * scratch.columns
    {
        return Err(invalid_argument(
            "fused MAP scratch does not match the sweep",
        ));
    }
    for effect in effects {
        if effect.codes().len() != values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP effect and matrix observation counts differ",
            ));
        }
    }
    let mut order = Vec::with_capacity(2 * count);
    order.extend(0..count);
    if symmetric && count > 1 {
        order.extend((0..count - 1).rev());
    }
    stream(values, scratch, None, Some(&effects[order[0]]))?;
    for (step, &effect_index) in order.iter().enumerate() {
        let effect = &effects[effect_index];
        merge_scales(scratch, effect.n_levels());
        level_means(effect, weights, values, scratch)?;
        if let Some(batch) = coefficients.as_deref_mut() {
            let levels = effect.n_levels();
            record_parallel_group_means(
                batch,
                effect_index,
                &scratch.means[..levels * scratch.columns],
                levels,
                scratch.columns,
            )?;
        }
        let next = order.get(step + 1).map(|&index| &effects[index]);
        stream(values, scratch, Some(effect), next)?;
    }
    Ok(())
}

/// Lane maxima merged into lane zero; maxima are exact in any order.
fn merge_scales(scratch: &mut FusedSweepScratch, levels: usize) {
    let table = levels * scratch.columns;
    let stride = scratch.scales.len() / scratch.lanes;
    let (first, rest) = scratch.scales.split_at_mut(stride);
    for lane in rest.chunks(stride) {
        for (scale, &other) in first[..table].iter_mut().zip(&lane[..table]) {
            *scale = scale.max(other);
        }
    }
}

/// Per-level means from merged scales: the weight sum and the scaled sums in
/// member order, exactly as the scaled two-pass group means.
fn level_means(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    scratch: &mut FusedSweepScratch,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = scratch.columns;
    let rows = values.nrows();
    let data = values.as_slice();
    let scales = &scratch.scales[..levels * columns];
    let hardware = crate::kernels::installed();
    let second_pass = hardware.map_or(
        crate::kernels::scaled_group_sums as crate::kernels::ScaledGroupSums,
        |kernels| kernels.scaled_group_sums,
    );
    let plan = crate::parallel::StagePlan::current(rows as u128 * columns as u128, levels);
    plan.groups_mut(
        &mut scratch.means[..levels * columns],
        columns,
        effect.level_index().membership_offsets(),
        |first_level, means| {
            let mut sums = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
            let mut corrections = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
            for (local, level_means) in means.chunks_mut(columns).enumerate() {
                let level = first_level + local;
                let members = effect.level_index().checked_members(level)?;
                if members.is_empty() {
                    return Err(invalid_argument(
                        "borrowed MAP effect contains an empty final level",
                    ));
                }
                let mut weight_sum = 0.0_f64;
                let mut weight_correction = 0.0_f64;
                for &observation in members {
                    if observation >= rows {
                        return Err(invalid_argument(
                            "borrowed MAP membership exceeds the observation axis",
                        ));
                    }
                    neumaier_add(
                        weights
                            .values
                            .map_or(1.0, |values| values[observation] / weights.raw_scale),
                        &mut weight_sum,
                        &mut weight_correction,
                    );
                }
                compensated_finite(weight_sum, weight_correction)?;
                let total_weight = weight_sum + weight_correction;
                if !total_weight.is_finite() || total_weight <= 0.0 {
                    return Err(invalid_data(
                        "borrowed MAP group weight sum is not finite and positive",
                    ));
                }
                let level_scales = &scales[level * columns..(level + 1) * columns];
                let sums = &mut sums[..columns];
                let corrections = &mut corrections[..columns];
                sums.fill(0.0);
                corrections.fill(0.0);
                second_pass(
                    weights.values,
                    weights.raw_scale,
                    data,
                    members,
                    level_scales,
                    sums,
                    corrections,
                );
                for (&sum, &correction) in sums.iter().zip(corrections.iter()) {
                    compensated_finite(sum, correction)?;
                }
                for (((mean, &scale), &sum), &correction) in level_means
                    .iter_mut()
                    .zip(level_scales)
                    .zip(sums.iter())
                    .zip(corrections.iter())
                {
                    let value = if scale == 0.0 {
                        0.0
                    } else {
                        scale * ((sum + correction) / total_weight)
                    };
                    if !value.is_finite() {
                        return Err(invalid_data("borrowed MAP group mean is not finite"));
                    }
                    *mean = value;
                }
            }
            Ok(())
        },
    )
}

/// One row-streaming pass: subtract `current`'s level means (when given) and
/// raise `next`'s lane scale tables (when given) to each result's magnitude.
fn stream(
    values: &mut DenseMatrix,
    scratch: &mut FusedSweepScratch,
    current: Option<&crate::fe::BorrowedInterceptEffect<'_>>,
    next: Option<&crate::fe::BorrowedInterceptEffect<'_>>,
) -> Result<()> {
    let columns = scratch.columns;
    let rows = values.nrows();
    if rows == 0 || columns == 0 {
        return Ok(());
    }
    let lanes = scratch.lanes;
    let stride = scratch.scales.len() / lanes;
    if let Some(next) = next {
        let table = next.n_levels() * columns;
        for lane in scratch.scales.chunks_mut(stride) {
            lane[..table].fill(0.0);
        }
    }
    let means = &scratch.means;
    let current = current.map(|effect| (effect.codes(), effect.n_levels()));
    let next = next.map(|effect| (effect.codes(), effect.n_levels()));
    let block = rows.div_ceil(lanes);
    let row_work = rows as u128 * columns as u128;
    crate::parallel::stage_try_for_each(
        values
            .as_mut_slice()
            .par_chunks_mut(block * columns)
            .zip(scratch.scales.par_chunks_mut(stride))
            .enumerate(),
        row_work,
        |(lane, (block_values, table))| {
            let first = lane * block;
            for (offset, row) in block_values.chunks_exact_mut(columns).enumerate() {
                let observation = first + offset;
                if let Some((codes, levels)) = current {
                    let level = codes[observation] as usize;
                    if level >= levels {
                        return Err(invalid_argument(
                            "borrowed MAP code exceeds the finalized level count",
                        ));
                    }
                    let level_means = &means[level * columns..(level + 1) * columns];
                    for (value, &mean) in row.iter_mut().zip(level_means) {
                        let output = *value - mean;
                        if !output.is_finite() {
                            return Err(invalid_data(
                                "borrowed MAP projection produced a nonfinite residual",
                            ));
                        }
                        *value = output;
                    }
                }
                if let Some((codes, levels)) = next {
                    let level = codes[observation] as usize;
                    if level >= levels {
                        return Err(invalid_argument(
                            "borrowed MAP code exceeds the finalized level count",
                        ));
                    }
                    let scales = &mut table[level * columns..(level + 1) * columns];
                    for (scale, value) in scales.iter_mut().zip(row.iter()) {
                        *scale = scale.max(value.abs());
                    }
                }
            }
            Ok(())
        },
    )
}

/// The lanes of the row-streaming pass over a `rows x columns` matrix.
pub(super) fn sweep_lanes(rows: usize, columns: usize) -> usize {
    crate::parallel::StagePlan::current(rows as u128 * columns as u128, rows).lanes
}

#[cfg(test)]
mod tests;
