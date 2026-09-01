//! Qualified clustered subset routes shared by OLS and PPML.
//!
//! OLS is the floating-point and allocation compatibility authority.  The
//! dense-table, ordered-Rayon, and scratch-serial routes below are mechanical
//! extractions of its accepted implementation; estimator adapters supply only
//! a row design, scalar score multiplier, cluster codes, and bread policy.

use rayon::prelude::*;
use std::cmp::Ordering;

use super::{
    allocation, compensated_add, invalid_argument, invalid_data, push_reserved, reserved_empty,
    reserved_filled, FixedBlockReductionPolicy, SandwichDesign, SandwichReservation,
    SandwichScoreMultiplier,
};
use crate::error::Result;

#[derive(Debug, Clone, Copy)]
pub(crate) enum QualifiedClusterCertificateRoute<'a> {
    /// Existing PPML paired-score route. Its exact five-vector workspace and
    /// merge order remain the accepted implementation.
    PairedIndependent(FixedBlockReductionPolicy),
    /// OLS scratch certification transforms every completed group by bread.
    OlsPerGroupBread(&'a [f64]),
    /// Dense OLS retains a reverse-order raw meat and applies bread once after
    /// inclusion--exclusion.
    OlsIndependentRaw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QualifiedClusterRoutePolicy {
    pub maximum_dimensions: usize,
    pub reduction_maximum_blocks: usize,
    pub partial_budget_bytes: usize,
    pub ordered_rows_per_block: usize,
    pub dense_groups_per_block: usize,
}

#[derive(Debug, PartialEq)]
pub(crate) struct QualifiedClusterSubsetResult {
    pub production: Vec<f64>,
    pub certificate: Vec<f64>,
    pub groups: usize,
}

#[derive(Debug, Clone, Copy)]
struct DenseSubsetPlan {
    radices: [usize; 16],
    group_slots: usize,
}

/// One estimator-neutral clustered subset reduction.  `dense` enables the
/// exact dense-table/ordered-Rayon routes; `streaming` is used only by the
/// serial scratch route and therefore need not be `Sync`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reduce_qualified_cluster_subset<D, S, M, R>(
    streaming: &S,
    dense: Option<D>,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    certificate: QualifiedClusterCertificateRoute<'_>,
    policy: QualifiedClusterRoutePolicy,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult>
where
    D: SandwichDesign + Copy + Sync,
    S: SandwichDesign + ?Sized,
    M: SandwichScoreMultiplier + Copy + Sync,
    R: SandwichReservation,
{
    let nobs = streaming.nrows();
    let dimension = streaming.dimension();
    if nobs == 0
        || multipliers.nrows() != nobs
        || dimensions.is_empty()
        || dimensions.len() > policy.maximum_dimensions
        || mask == 0
        || mask >= (1_usize << dimensions.len())
        || dimensions.iter().any(|codes| codes.len() != nobs)
    {
        return Err(invalid_argument(
            "qualified clustered score and cluster dimensions are not aligned",
        ));
    }
    if let Some(dense) = dense {
        if dense.nrows() != nobs || dense.dimension() != dimension {
            return Err(invalid_argument(
                "qualified clustered dense and streaming designs disagree",
            ));
        }
        if let QualifiedClusterCertificateRoute::PairedIndependent(blocks) = certificate {
            let reduced = super::reduce_one_cluster_subset(
                dense,
                multipliers,
                dimensions,
                mask,
                blocks,
                reservation,
            )?;
            return Ok(QualifiedClusterSubsetResult {
                production: reduced.production,
                certificate: reduced.certificate,
                groups: reduced.groups,
            });
        }
        let prefer_ordered = rayon::current_num_threads() > 1 && dimension >= 8;
        if !prefer_ordered {
            if let Some(plan) =
                dense_subset_plan(dimensions, mask, nobs, dimension, policy.maximum_dimensions)?
            {
                return aggregate_dense_table(
                    dense,
                    multipliers,
                    dimensions,
                    mask,
                    plan,
                    policy,
                    reservation,
                );
            }
        }
        return aggregate_ordered_dense(dense, multipliers, dimensions, mask, policy, reservation);
    }
    let bread = match certificate {
        QualifiedClusterCertificateRoute::OlsPerGroupBread(bread) => bread,
        QualifiedClusterCertificateRoute::OlsIndependentRaw => {
            return Err(invalid_argument(
                "qualified clustered streaming route requires per-group bread",
            ))
        }
        QualifiedClusterCertificateRoute::PairedIndependent(_) => {
            return Err(invalid_argument(
                "qualified clustered paired route requires a parallel-safe design",
            ))
        }
    };
    aggregate_streaming_serial(
        streaming,
        multipliers,
        dimensions,
        mask,
        bread,
        policy.maximum_dimensions,
        reservation,
    )
}

fn dense_subset_plan(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    score_dimension: usize,
    maximum_dimensions: usize,
) -> Result<Option<DenseSubsetPlan>> {
    let mut radices = [0_usize; 16];
    let mut group_slots = 1_usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| invalid_data("qualified clustered dense subset has no groups"))?;
        radices[dimension] = radix;
        group_slots = group_slots
            .checked_mul(radix)
            .ok_or_else(|| allocation("qualified clustered dense group product overflows"))?;
    }
    if dimensions.len() > maximum_dimensions || dimensions.len() > radices.len() {
        return Err(invalid_argument(
            "qualified clustered dense subset has too many dimensions",
        ));
    }
    let score_slots = group_slots
        .checked_mul(score_dimension)
        .ok_or_else(|| allocation("qualified clustered dense score table overflows"))?;
    let score_bytes = score_slots
        .checked_mul(std::mem::size_of::<f64>())
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| allocation("qualified clustered dense score bytes overflow"))?;
    let dense_bytes = score_bytes
        .checked_add(group_slots)
        .ok_or_else(|| allocation("qualified clustered dense scratch bytes overflow"))?;
    let ordering_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered ordering bytes overflow"))?;
    let budget = ordering_bytes.saturating_mul(4).max(64 << 20);
    Ok((dense_bytes <= budget).then_some(DenseSubsetPlan {
        radices,
        group_slots,
    }))
}

fn dense_group(
    dimensions: &[&[u32]],
    mask: usize,
    plan: DenseSubsetPlan,
    row: usize,
) -> Result<usize> {
    let mut group = 0_usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let code = usize::try_from(codes[row])
            .map_err(|_| invalid_data("qualified clustered dense code exceeds usize"))?;
        let radix = plan.radices[dimension];
        if radix == 0 || code >= radix {
            return Err(invalid_data(
                "qualified clustered dense code exceeds its certified radix",
            ));
        }
        group = group
            .checked_mul(radix)
            .and_then(|value| value.checked_add(code))
            .ok_or_else(|| allocation("qualified clustered dense group index overflows"))?;
    }
    Ok(group)
}

fn aggregate_dense_table<D, M, R>(
    design: D,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    plan: DenseSubsetPlan,
    policy: QualifiedClusterRoutePolicy,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult>
where
    D: SandwichDesign + Copy + Sync,
    M: SandwichScoreMultiplier + Copy + Sync,
    R: SandwichReservation,
{
    let nobs = design.nrows();
    let dimension = design.dimension();
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("qualified clustered dense subset meat overflows"))?;
    let score_slots = plan
        .group_slots
        .checked_mul(dimension)
        .ok_or_else(|| allocation("qualified clustered dense score slots overflow"))?;
    let mut group_scores = reserved_filled(
        reservation,
        score_slots,
        0.0_f64,
        "unable to reserve qualified clustered dense group scores",
    )?;
    let mut group_corrections = reserved_filled(
        reservation,
        score_slots,
        0.0_f64,
        "unable to reserve qualified clustered dense group corrections",
    )?;
    let mut seen = reserved_filled(
        reservation,
        plan.group_slots,
        0_u8,
        "unable to reserve qualified clustered dense group presence",
    )?;
    let mut row_values = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve qualified clustered dense score row",
    )?;
    for row in 0..nobs {
        let group = dense_group(dimensions, mask, plan, row)?;
        seen[group] = 1;
        design.fill_row(row, &mut row_values)?;
        let scale = multipliers.score_pair(row)?.production;
        let offset = group * dimension;
        for (coordinate, value) in row_values.iter().copied().enumerate() {
            let position = offset + coordinate;
            compensated_add(
                scale * value,
                &mut group_scores[position],
                &mut group_corrections[position],
            )?;
        }
    }
    finish_dense_groups(
        &group_scores,
        &group_corrections,
        &seen,
        plan.group_slots,
        dimension,
        square,
        policy,
        reservation,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_dense_groups<R: SandwichReservation>(
    group_scores: &[f64],
    group_corrections: &[f64],
    seen: &[u8],
    group_slots: usize,
    dimension: usize,
    square: usize,
    policy: QualifiedClusterRoutePolicy,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult> {
    let workspace = dimension
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| allocation("qualified clustered dense workspace overflows"))?;
    let stride = square
        .checked_mul(2)
        .and_then(|value| value.checked_add(workspace))
        .ok_or_else(|| allocation("qualified clustered dense partial stride overflows"))?;
    let chunks = reduction_chunks(group_slots, policy.dense_groups_per_block, stride, policy)?;
    let mut partials = reserved_filled(
        reservation,
        chunks * stride,
        0.0_f64,
        "unable to reserve qualified clustered dense partials",
    )?;
    partials
        .par_chunks_mut(stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (raw, tail) = partial.split_at_mut(square);
            let (independent, workspace) = tail.split_at_mut(square);
            let (raw_score, workspace) = workspace.split_at_mut(dimension);
            let (independent_score, count) = workspace.split_at_mut(dimension);
            let per = group_slots / chunks;
            let remainder = group_slots % chunks;
            let start = chunk * per + chunk.min(remainder);
            let end = start + per + usize::from(chunk < remainder);
            for (group, present) in seen.iter().enumerate().take(end).skip(start) {
                if *present == 0 {
                    continue;
                }
                let offset = group * dimension;
                for coordinate in 0..dimension {
                    raw_score[coordinate] =
                        group_scores[offset + coordinate] + group_corrections[offset + coordinate];
                    independent_score[coordinate] =
                        group_corrections[offset + coordinate] + group_scores[offset + coordinate];
                }
                accumulate_outer_pair(raw_score, independent_score, raw, independent)?;
                count[0] += 1.0;
            }
            Ok(())
        })?;
    merge_partials(&partials, stride, dimension, square, reservation)
}

fn aggregate_ordered_dense<D, M, R>(
    design: D,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    policy: QualifiedClusterRoutePolicy,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult>
where
    D: SandwichDesign + Copy + Sync,
    M: SandwichScoreMultiplier + Copy + Sync,
    R: SandwichReservation,
{
    let nobs = design.nrows();
    let dimension = design.dimension();
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("qualified clustered ordered meat overflows"))?;
    let order = build_radix_order(
        dimensions,
        mask,
        nobs,
        policy.maximum_dimensions,
        reservation,
    )?;
    let workspace = dimension
        .checked_mul(4)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| allocation("qualified clustered ordered workspace overflows"))?;
    let stride = square
        .checked_mul(2)
        .and_then(|v| v.checked_add(workspace))
        .ok_or_else(|| allocation("qualified clustered ordered stride overflows"))?;
    let chunks = reduction_chunks(nobs, policy.ordered_rows_per_block, stride, policy)?;
    let mut boundaries = reserved_filled(
        reservation,
        chunks + 1,
        0usize,
        "unable to reserve qualified clustered ordered boundaries",
    )?;
    boundaries[chunks] = nobs;
    for (chunk, target) in boundaries.iter_mut().enumerate().take(chunks).skip(1) {
        let mut boundary = chunk * nobs / chunks;
        while boundary < nobs
            && compare_rows(dimensions, mask, order[boundary - 1], order[boundary])
                == Ordering::Equal
        {
            boundary += 1;
        }
        *target = boundary;
    }
    for chunk in 1..chunks {
        boundaries[chunk] = boundaries[chunk].max(boundaries[chunk - 1]);
    }
    let mut partials = reserved_filled(
        reservation,
        chunks
            .checked_mul(stride)
            .ok_or_else(|| allocation("qualified clustered ordered partials overflow"))?,
        0.0,
        "unable to reserve qualified clustered ordered partials",
    )?;
    partials
        .par_chunks_mut(stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (raw, tail) = partial.split_at_mut(square);
            let (independent, workspace) = tail.split_at_mut(square);
            let (group, workspace) = workspace.split_at_mut(dimension);
            let (corrections, workspace) = workspace.split_at_mut(dimension);
            let (raw_score, workspace) = workspace.split_at_mut(dimension);
            let (independent_score, count) = workspace.split_at_mut(dimension);
            let mut previous = None;
            for row in order[boundaries[chunk]..boundaries[chunk + 1]]
                .iter()
                .copied()
            {
                if previous.is_some_and(|prior| {
                    compare_rows(dimensions, mask, prior, row) != Ordering::Equal
                }) {
                    accumulate_uncompensated(
                        group,
                        corrections,
                        raw_score,
                        independent_score,
                        raw,
                        independent,
                    )?;
                    group.fill(0.0);
                    corrections.fill(0.0);
                    count[0] += 1.0;
                }
                let scale = multipliers.score_pair(row)?.production;
                design.fill_row(row, raw_score)?;
                for coordinate in 0..dimension {
                    compensated_add(
                        scale * raw_score[coordinate],
                        &mut group[coordinate],
                        &mut corrections[coordinate],
                    )?;
                }
                previous = Some(row);
            }
            if previous.is_some() {
                accumulate_uncompensated(
                    group,
                    corrections,
                    raw_score,
                    independent_score,
                    raw,
                    independent,
                )?;
                count[0] += 1.0;
            }
            if partial.iter().all(|v| v.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "qualified clustered ordered partial is nonfinite",
                ))
            }
        })?;
    merge_partials(&partials, stride, dimension, square, reservation)
}

fn aggregate_streaming_serial<S, M, R>(
    design: &S,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    bread: &[f64],
    maximum_dimensions: usize,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult>
where
    S: SandwichDesign + ?Sized,
    M: SandwichScoreMultiplier + Copy,
    R: SandwichReservation,
{
    let nobs = design.nrows();
    let dimension = design.dimension();
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("qualified clustered serial meat overflows"))?;
    if bread.len() != square {
        return Err(invalid_argument(
            "qualified clustered serial bread dimensions disagree",
        ));
    }
    if dimensions.len() > maximum_dimensions {
        return Err(invalid_argument(
            "qualified clustered serial route has too many dimensions",
        ));
    }
    // The accepted scratch OLS path reserves exactly one N-element tuple
    // order; radix scratch is exclusive to dense score storage.
    let mut order = reserved_empty(
        reservation,
        nobs,
        "unable to reserve qualified clustered subset ordering",
    )?;
    for row in 0..nobs {
        push_reserved(
            &mut order,
            row,
            "qualified clustered subset ordering capacity exhausted",
        )?;
    }
    order.sort_unstable_by(|left, right| {
        compare_rows(dimensions, mask, *left, *right).then_with(|| left.cmp(right))
    });
    let mut group = reserved_filled(
        reservation,
        dimension,
        0.0,
        "unable to reserve qualified clustered group score",
    )?;
    let mut group_corrections = reserved_filled(
        reservation,
        dimension,
        0.0,
        "unable to reserve qualified clustered group corrections",
    )?;
    let mut raw_score = reserved_filled(
        reservation,
        dimension,
        0.0,
        "unable to reserve qualified clustered raw score",
    )?;
    let mut influence_score = reserved_filled(
        reservation,
        dimension,
        0.0,
        "unable to reserve qualified clustered influence score",
    )?;
    let mut row_values = reserved_filled(
        reservation,
        dimension,
        0.0,
        "unable to reserve qualified clustered score row",
    )?;
    let mut raw = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered raw meat",
    )?;
    let mut raw_corrections = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered raw corrections",
    )?;
    let mut influence = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered influence meat",
    )?;
    let mut influence_corrections = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered influence corrections",
    )?;
    let mut groups = 0usize;
    let mut previous = None;
    for row in order {
        if previous
            .is_some_and(|prior| compare_rows(dimensions, mask, prior, row) != Ordering::Equal)
        {
            finish_serial_group(
                bread,
                &group,
                &group_corrections,
                &mut raw_score,
                &mut influence_score,
                &mut raw,
                &mut raw_corrections,
                &mut influence,
                &mut influence_corrections,
            )?;
            group.fill(0.0);
            group_corrections.fill(0.0);
            groups = groups
                .checked_add(1)
                .ok_or_else(|| allocation("qualified clustered group count overflows"))?;
        }
        let scale = multipliers.score_pair(row)?.production;
        design.fill_row(row, &mut row_values)?;
        for coordinate in 0..dimension {
            compensated_add(
                scale * row_values[coordinate],
                &mut group[coordinate],
                &mut group_corrections[coordinate],
            )?;
        }
        previous = Some(row);
    }
    if previous.is_some() {
        finish_serial_group(
            bread,
            &group,
            &group_corrections,
            &mut raw_score,
            &mut influence_score,
            &mut raw,
            &mut raw_corrections,
            &mut influence,
            &mut influence_corrections,
        )?;
        groups = groups
            .checked_add(1)
            .ok_or_else(|| allocation("qualified clustered group count overflows"))?;
    }
    finalize(&mut raw, raw_corrections, dimension)?;
    finalize(&mut influence, influence_corrections, dimension)?;
    Ok(QualifiedClusterSubsetResult {
        production: raw,
        certificate: influence,
        groups,
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_serial_group(
    bread: &[f64],
    score: &[f64],
    corrections: &[f64],
    raw_score: &mut [f64],
    influence_score: &mut [f64],
    raw: &mut [f64],
    raw_corrections: &mut [f64],
    influence: &mut [f64],
    influence_corrections: &mut [f64],
) -> Result<()> {
    let dimension = score.len();
    for coordinate in 0..dimension {
        raw_score[coordinate] = score[coordinate] + corrections[coordinate];
    }
    for (row, target) in influence_score.iter_mut().enumerate() {
        let mut value = 0.0;
        let mut correction = 0.0;
        for column in 0..dimension {
            compensated_add(
                bread[row * dimension + column] * (score[column] + corrections[column]),
                &mut value,
                &mut correction,
            )?;
        }
        *target = value + correction;
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            compensated_add(
                raw_score[left] * raw_score[right],
                &mut raw[position],
                &mut raw_corrections[position],
            )?;
            compensated_add(
                influence_score[left] * influence_score[right],
                &mut influence[position],
                &mut influence_corrections[position],
            )?;
        }
    }
    Ok(())
}

fn accumulate_uncompensated(
    score: &[f64],
    corrections: &[f64],
    raw_score: &mut [f64],
    independent_score: &mut [f64],
    raw: &mut [f64],
    independent: &mut [f64],
) -> Result<()> {
    let dimension = score.len();
    if corrections.len() != dimension
        || raw_score.len() != dimension
        || independent_score.len() != dimension
        || raw.len() != dimension * dimension
        || independent.len() != raw.len()
    {
        return Err(invalid_argument(
            "qualified clustered group dimensions disagree",
        ));
    }
    for coordinate in 0..dimension {
        raw_score[coordinate] = score[coordinate] + corrections[coordinate];
        independent_score[coordinate] = corrections[coordinate] + score[coordinate];
    }
    for left in 0..dimension {
        for right in left..dimension {
            raw[left * dimension + right] += raw_score[left] * raw_score[right];
            independent[left * dimension + right] +=
                independent_score[right] * independent_score[left];
        }
    }
    Ok(())
}

fn accumulate_outer_pair(
    raw_score: &[f64],
    independent_score: &[f64],
    raw: &mut [f64],
    independent: &mut [f64],
) -> Result<()> {
    let dimension = raw_score.len();
    if independent_score.len() != dimension
        || raw.len() != dimension * dimension
        || independent.len() != raw.len()
    {
        return Err(invalid_argument(
            "qualified clustered outer-product dimensions disagree",
        ));
    }
    for left in 0..dimension {
        for right in left..dimension {
            raw[left * dimension + right] += raw_score[left] * raw_score[right];
            independent[left * dimension + right] +=
                independent_score[right] * independent_score[left];
        }
    }
    Ok(())
}

fn merge_partials<R: SandwichReservation>(
    partials: &[f64],
    stride: usize,
    dimension: usize,
    square: usize,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult> {
    let mut raw = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered meat",
    )?;
    let mut raw_corr = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered meat corrections",
    )?;
    let mut independent = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered certificate",
    )?;
    let mut independent_corr = reserved_filled(
        reservation,
        square,
        0.0,
        "unable to reserve qualified clustered certificate corrections",
    )?;
    let mut groups = 0usize;
    for partial in partials.chunks_exact(stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let pos = left * dimension + right;
                compensated_add(partial[pos], &mut raw[pos], &mut raw_corr[pos])?;
            }
        }
        let count = partial[stride - 1];
        if count.fract() != 0.0 || count < 0.0 || count > usize::MAX as f64 {
            return Err(invalid_data("qualified clustered group count is invalid"));
        }
        groups = groups
            .checked_add(count as usize)
            .ok_or_else(|| allocation("qualified clustered group count overflows"))?;
    }
    for partial in partials.rchunks_exact(stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let pos = left * dimension + right;
                compensated_add(
                    partial[square + pos],
                    &mut independent[pos],
                    &mut independent_corr[pos],
                )?;
            }
        }
    }
    finalize(&mut raw, raw_corr, dimension)?;
    finalize(&mut independent, independent_corr, dimension)?;
    Ok(QualifiedClusterSubsetResult {
        production: raw,
        certificate: independent,
        groups,
    })
}

fn finalize(meat: &mut [f64], corrections: Vec<f64>, dimension: usize) -> Result<()> {
    for left in 0..dimension {
        for right in left..dimension {
            let pos = left * dimension + right;
            meat[pos] += corrections[pos];
            if !meat[pos].is_finite() {
                return Err(invalid_data("qualified clustered meat is nonfinite"));
            }
            meat[right * dimension + left] = meat[pos];
        }
    }
    Ok(())
}

fn reduction_chunks(
    work_items: usize,
    minimum_items: usize,
    stride: usize,
    policy: QualifiedClusterRoutePolicy,
) -> Result<usize> {
    if work_items == 0 || minimum_items == 0 || stride == 0 {
        return Err(invalid_argument(
            "qualified clustered reduction dimensions are empty",
        ));
    }
    let bytes = stride
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("qualified clustered partial bytes overflow"))?;
    let memory = (policy.partial_budget_bytes / bytes.max(1)).max(1);
    Ok(work_items
        .div_ceil(minimum_items)
        .clamp(1, policy.reduction_maximum_blocks)
        .min(memory))
}

fn compare_rows(dimensions: &[&[u32]], mask: usize, left: usize, right: usize) -> Ordering {
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let ordering = codes[left].cmp(&codes[right]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn build_radix_order<R: SandwichReservation>(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    maximum_dimensions: usize,
    reservation: &mut R,
) -> Result<Vec<usize>> {
    let mut radices = [0usize; 16];
    let mut maximum = 0usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|v| usize::try_from(v).ok())
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| invalid_data("qualified clustered radix has no groups"))?;
        radices[dimension] = radix;
        maximum = maximum.max(radix);
    }
    let scratch_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered radix bytes overflow"))?;
    let count_bytes = maximum
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered radix count bytes overflow"))?;
    if dimensions.len() > maximum_dimensions
        || count_bytes > scratch_bytes.saturating_mul(2).max(1 << 20)
    {
        let mut order = reserved_empty(
            reservation,
            nobs,
            "unable to reserve qualified clustered ordering",
        )?;
        for row in 0..nobs {
            push_reserved(
                &mut order,
                row,
                "qualified clustered ordering capacity exhausted",
            )?;
        }
        order.sort_unstable_by(|left, right| {
            compare_rows(dimensions, mask, *left, *right).then_with(|| left.cmp(right))
        });
        return Ok(order);
    }
    let mut order = reserved_filled(
        reservation,
        nobs,
        0usize,
        "unable to reserve qualified clustered radix order",
    )?;
    for (row, target) in order.iter_mut().enumerate() {
        *target = row;
    }
    let mut scratch = reserved_filled(
        reservation,
        nobs,
        0usize,
        "unable to reserve qualified clustered radix scratch",
    )?;
    let mut counts = reserved_filled(
        reservation,
        maximum,
        0usize,
        "unable to reserve qualified clustered radix counts",
    )?;
    for dimension in (0..dimensions.len()).rev() {
        if mask & (1 << dimension) == 0 {
            continue;
        }
        let active = &mut counts[..radices[dimension]];
        active.fill(0);
        for row in order.iter().copied() {
            let key = dimensions[dimension][row] as usize;
            active[key] += 1;
        }
        let mut offset = 0;
        for count in active.iter_mut() {
            let frequency = *count;
            *count = offset;
            offset += frequency;
        }
        for row in order.iter().copied() {
            let key = dimensions[dimension][row] as usize;
            let position = active[key];
            scratch[position] = row;
            active[key] = position + 1;
        }
        std::mem::swap(&mut order, &mut scratch);
    }
    Ok(order)
}
