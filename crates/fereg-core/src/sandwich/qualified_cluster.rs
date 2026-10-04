//! Qualified clustered subset routes shared by OLS and PPML.
//!
//! OLS and PPML retain their distinct certificate and allocation policies.
//! Dense-table, ordered-Rayon, and scratch-serial routes share compensated
//! group scores; estimator adapters supply the row design, scalar multiplier,
//! final-sample cluster codes, and independent bread policy.

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
    /// PPML retains independently constructed production/certificate scores.
    PairedIndependent(FixedBlockReductionPolicy, Option<&'a [f64]>),
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
        if let QualifiedClusterCertificateRoute::PairedIndependent(blocks, bread) = certificate {
            super::influence::validate_bread(bread, dimension)?;
            if dense.prefers_sequential_scan() {
                if let Some(plan) = paired_subset_plan(dimensions, mask, nobs, dimension) {
                    return aggregate_paired_scan(
                        dense,
                        multipliers,
                        dimensions,
                        mask,
                        plan,
                        bread,
                        reservation,
                    );
                }
            }
            let reduced = super::reduce_one_cluster_subset(
                dense,
                multipliers,
                dimensions,
                mask,
                blocks,
                bread,
                reservation,
            )?;
            return Ok(QualifiedClusterSubsetResult {
                production: reduced.production,
                certificate: reduced.certificate,
                groups: reduced.groups,
            });
        }
        let threads = rayon::current_num_threads();
        let prefer_ordered =
            threads > 1 && dimension >= 8 || threads >= 8 && nobs >= 16_384 && dimension >= 6;
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
        QualifiedClusterCertificateRoute::PairedIndependent(_, _) => {
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

// The complete scan workspace replaces the two simultaneous radix row orders.
// Its bound also implies the old route admits radix counts, rather than using
// the sparse-label single-order fallback. No bank coexists with a row ordering.
fn paired_subset_plan(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    dimension: usize,
) -> Option<DenseSubsetPlan> {
    let plan = dense_subset_plan(dimensions, mask, nobs, dimension.checked_mul(2)?, 16).ok()??;
    let bytes = paired_workspace_bytes(plan.group_slots, dimension)?;
    (bytes <= nobs.checked_mul(2 * std::mem::size_of::<usize>())?).then_some(plan)
}

fn paired_workspace_bytes(groups: usize, dimension: usize) -> Option<usize> {
    groups
        .checked_mul(dimension.checked_mul(32)?.checked_add(1)?)?
        .checked_add(dimension.checked_mul(dimension)?.checked_mul(24)?)?
        .checked_add(dimension.checked_mul(8)?)?
        .checked_add(std::mem::size_of::<DenseSubsetPlan>())?
        .checked_add(6 * std::mem::size_of::<Vec<f64>>())
}

fn aggregate_paired_scan<D, M, R>(
    design: D,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    plan: DenseSubsetPlan,
    influence_bread: Option<&[f64]>,
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult>
where
    D: SandwichDesign,
    M: SandwichScoreMultiplier,
    R: SandwichReservation,
{
    let dimension = design.dimension();
    let slots = plan
        .group_slots
        .checked_mul(dimension)
        .ok_or_else(|| allocation("paired cluster score slots overflow"))?;
    let bank_elements = slots
        .checked_mul(4)
        .ok_or_else(|| allocation("paired cluster score bank overflows"))?;
    let mut bank = reserved_filled(
        reservation,
        bank_elements,
        0.0_f64,
        "unable to reserve paired cluster score bank",
    )?;
    let mut seen = reserved_filled(
        reservation,
        plan.group_slots,
        0_u8,
        "unable to reserve paired cluster presence",
    )?;
    let actual_bytes = paired_workspace_bytes(plan.group_slots, dimension)
        .ok_or_else(|| allocation("paired cluster workspace overflows"))?
        as u128
        + (bank.capacity() - bank_elements) as u128 * 8
        + (seen.capacity() - plan.group_slots) as u128;
    if actual_bytes > design.nrows() as u128 * 2 * std::mem::size_of::<usize>() as u128 {
        return Err(allocation("paired cluster bank exceeds displaced ordering"));
    }
    let mut row_values = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve paired cluster row",
    )?;
    let (production, tail) = bank.split_at_mut(slots);
    let (production_correction, tail) = tail.split_at_mut(slots);
    let (certificate, certificate_correction) = tail.split_at_mut(slots);
    design.visit_range(0..design.nrows(), &mut row_values, |row, values| {
        let group = dense_group(dimensions, mask, plan, row)?;
        seen[group] = 1;
        let pair = multipliers.score_pair(row)?;
        for (coordinate, value) in values.iter().copied().enumerate() {
            let position = group * dimension + coordinate;
            compensated_add(
                pair.production * value,
                &mut production[position],
                &mut production_correction[position],
            )?;
            compensated_add(
                pair.certificate * value,
                &mut certificate[position],
                &mut certificate_correction[position],
            )?;
        }
        Ok(())
    })?;
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("paired cluster meat dimensions overflow"))?;
    let mut production_meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve paired cluster production meat",
    )?;
    let mut certificate_meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve paired cluster certificate meat",
    )?;
    let mut correction = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve paired cluster meat correction",
    )?;
    aggregate_paired_groups::<false>(
        production,
        production_correction,
        &seen,
        &mut row_values,
        &mut production_meat,
        &mut correction,
        influence_bread,
    )?;
    super::finalize_symmetric_triangle(&mut production_meat, correction, dimension)?;
    let mut correction = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve paired cluster certificate correction",
    )?;
    // Independent multipliers, storage and reverse traversal; this pass never
    // reads production sums or its meat.
    aggregate_paired_groups::<true>(
        certificate,
        certificate_correction,
        &seen,
        &mut row_values,
        &mut certificate_meat,
        &mut correction,
        influence_bread,
    )?;
    super::finalize_symmetric_triangle(&mut certificate_meat, correction, dimension)?;
    Ok(QualifiedClusterSubsetResult {
        production: production_meat,
        certificate: certificate_meat,
        groups: seen.iter().filter(|&&present| present != 0).count(),
    })
}

#[allow(clippy::needless_range_loop)]
fn aggregate_paired_groups<const REVERSE: bool>(
    scores: &mut [f64],
    score_corrections: &mut [f64],
    seen: &[u8],
    workspace: &mut [f64],
    meat: &mut [f64],
    corrections: &mut [f64],
    influence_bread: Option<&[f64]>,
) -> Result<()> {
    let dimension = workspace.len();
    for position in 0..seen.len() {
        let group = if REVERSE {
            seen.len() - 1 - position
        } else {
            position
        };
        if seen[group] == 0 {
            continue;
        }
        let offset = group * dimension;
        if let Some(bread) = influence_bread {
            super::influence::transform_group(
                &mut scores[offset..offset + dimension],
                &mut score_corrections[offset..offset + dimension],
                workspace,
                bread,
                REVERSE,
            )?;
        }
        for left in 0..dimension {
            let a = scores[offset + left] + score_corrections[offset + left];
            for right in left..dimension {
                let b = scores[offset + right] + score_corrections[offset + right];
                let slot = left * dimension + right;
                compensated_add(a * b, &mut meat[slot], &mut corrections[slot])?;
            }
        }
    }
    Ok(())
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

struct DenseGroupScores {
    values: Vec<f64>,
    corrections: Vec<f64>,
    seen: Vec<u8>,
}

fn accumulate_dense_groups<S, M, R>(
    design: &S,
    multipliers: M,
    dimensions: &[&[u32]],
    mask: usize,
    plan: DenseSubsetPlan,
    row_values: &mut [f64],
    reservation: &mut R,
) -> Result<DenseGroupScores>
where
    S: SandwichDesign + ?Sized,
    M: SandwichScoreMultiplier,
    R: SandwichReservation,
{
    let nobs = design.nrows();
    let dimension = design.dimension();
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
    for row in 0..nobs {
        let group = dense_group(dimensions, mask, plan, row)?;
        seen[group] = 1;
        design.fill_row(row, row_values)?;
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
    Ok(DenseGroupScores {
        values: group_scores,
        corrections: group_corrections,
        seen,
    })
}

#[derive(Clone, Copy)]
struct ClusterSubsetRange {
    group_start: usize,
    groups: usize,
}

/// Action-local group scores; all subsets share one original-row design pass.
/// The complete bank and range metadata replace one N-row ordering allocation.
pub(crate) struct QualifiedClusterScoreBatch {
    dimension: usize,
    ranges: Vec<ClusterSubsetRange>,
    scores: DenseGroupScores,
}

impl QualifiedClusterScoreBatch {
    pub(crate) fn reduce_subset<R: SandwichReservation>(
        &self,
        mask: usize,
        bread: &[f64],
        reservation: &mut R,
    ) -> Result<QualifiedClusterSubsetResult> {
        let range = mask
            .checked_sub(1)
            .and_then(|index| self.ranges.get(index))
            .ok_or_else(|| invalid_argument("batched cluster subset is out of range"))?;
        let group_end = range.group_start + range.groups;
        let start = range.group_start * self.dimension;
        let end = group_end * self.dimension;
        finish_serial_dense_groups(
            &self.scores.values[start..end],
            &self.scores.corrections[start..end],
            &self.scores.seen[range.group_start..group_end],
            self.dimension,
            bread,
            reservation,
        )
    }
}

fn subset_group_slots(radices: &[usize], mask: usize) -> Option<usize> {
    radices
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .try_fold(1usize, |product, (_, radix)| product.checked_mul(*radix))
}

pub(crate) fn prepare_cluster_score_batch<S, M, R>(
    design: &S,
    multipliers: M,
    dimensions: &[&[u32]],
    maximum_dimensions: usize,
    reservation: &mut R,
) -> Result<Option<QualifiedClusterScoreBatch>>
where
    S: SandwichDesign + ?Sized,
    M: SandwichScoreMultiplier,
    R: SandwichReservation,
{
    let nobs = design.nrows();
    let dimension = design.dimension();
    if dimensions.len() < 2 || dimension == 0 {
        return Ok(None);
    }
    if nobs == 0
        || multipliers.nrows() != nobs
        || dimensions.len() > maximum_dimensions
        || dimensions.len() > 16
        || dimensions.iter().any(|codes| codes.len() != nobs)
    {
        return Err(invalid_argument(
            "batched cluster scores are not sample aligned",
        ));
    }
    let mut radices = [0usize; 16];
    for (target, codes) in radices.iter_mut().zip(dimensions) {
        *target = codes
            .iter()
            .copied()
            .max()
            .and_then(|code| usize::try_from(code).ok())
            .and_then(|code| code.checked_add(1))
            .ok_or_else(|| invalid_data("batched cluster dimension has no groups"))?;
    }
    let subset_count = (1usize << dimensions.len()) - 1;
    let budget = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("batched cluster ordering budget overflows"))?;
    let metadata_bytes = subset_count
        .checked_mul(std::mem::size_of::<ClusterSubsetRange>())
        .ok_or_else(|| allocation("batched cluster range bytes overflow"))?;
    let row_codes_bytes = (subset_count + 1)
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("batched cluster row-code bytes overflow"))?;
    let metadata_bytes = metadata_bytes
        .checked_add(row_codes_bytes)
        .ok_or_else(|| allocation("batched cluster metadata bytes overflow"))?;
    let Some(bytes_per_group) = dimension
        .checked_mul(2 * std::mem::size_of::<f64>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<u8>()))
    else {
        return Ok(None);
    };
    let mut total_groups = 0usize;
    for mask in 1..=subset_count {
        let Some(groups) = subset_group_slots(&radices[..dimensions.len()], mask) else {
            return Ok(None);
        };
        let Some(total) = total_groups.checked_add(groups) else {
            return Ok(None);
        };
        let Some(bytes) = total
            .checked_mul(bytes_per_group)
            .and_then(|bytes| bytes.checked_add(metadata_bytes))
        else {
            return Ok(None);
        };
        if bytes > budget {
            return Ok(None);
        }
        total_groups = total;
    }
    // All shape arithmetic and admission precede bank reservation. A declined
    // batch leaves the existing per-subset ordering/table budget available.
    let mut ranges = reserved_empty(
        reservation,
        subset_count,
        "unable to reserve batched cluster ranges",
    )?;
    let mut group_start = 0usize;
    for mask in 1..=subset_count {
        let groups = subset_group_slots(&radices[..dimensions.len()], mask)
            .ok_or_else(|| allocation("admitted batched cluster range overflows"))?;
        push_reserved(
            &mut ranges,
            ClusterSubsetRange {
                group_start,
                groups,
            },
            "batched cluster range capacity exhausted",
        )?;
        group_start += groups;
    }
    let score_slots = total_groups
        .checked_mul(dimension)
        .ok_or_else(|| allocation("batched cluster score slots overflow"))?;
    let mut scores = DenseGroupScores {
        values: reserved_filled(
            reservation,
            score_slots,
            0.0,
            "unable to reserve batched cluster scores",
        )?,
        corrections: reserved_filled(
            reservation,
            score_slots,
            0.0,
            "unable to reserve batched cluster corrections",
        )?,
        seen: reserved_filled(
            reservation,
            total_groups,
            0u8,
            "unable to reserve batched cluster presence",
        )?,
    };
    let mut row_values = reserved_filled(
        reservation,
        dimension,
        0.0,
        "unable to reserve batched cluster score row",
    )?;
    let mut row_codes = reserved_filled(
        reservation,
        subset_count + 1,
        0usize,
        "unable to reserve batched cluster row codes",
    )?;
    for row in 0..nobs {
        design.fill_row(row, &mut row_values)?;
        let scale = multipliers.score_pair(row)?.production;
        for value in &mut row_values {
            *value *= scale;
        }
        for (index, range) in ranges.iter().enumerate() {
            let mask = index + 1;
            let last = mask.ilog2() as usize;
            let prefix = mask ^ (1usize << last);
            // Prefix masks precede this mask. Appending the last declared code
            // gives the same lexicographic mixed radix, with admitted bounds.
            row_codes[mask] = row_codes[prefix] * radices[last] + dimensions[last][row] as usize;
            let group = range.group_start + row_codes[mask];
            scores.seen[group] = 1;
            let start = group * dimension;
            for (coordinate, value) in row_values.iter().copied().enumerate() {
                compensated_add(
                    value,
                    &mut scores.values[start + coordinate],
                    &mut scores.corrections[start + coordinate],
                )?;
            }
        }
    }
    Ok(Some(QualifiedClusterScoreBatch {
        dimension,
        ranges,
        scores,
    }))
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
    let dimension = design.dimension();
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("qualified clustered dense subset meat overflows"))?;
    let mut row_values = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve qualified clustered dense score row",
    )?;
    let scores = accumulate_dense_groups(
        &design,
        multipliers,
        dimensions,
        mask,
        plan,
        &mut row_values,
        reservation,
    )?;
    finish_dense_groups(
        &scores.values,
        &scores.corrections,
        &scores.seen,
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
        *target = super::extend_cluster_boundary(&order, chunk * nobs / chunks, |left, right| {
            compare_rows(dimensions, mask, left, right) == Ordering::Equal
        });
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
    // The group table replaces, rather than overlaps, the existing N-row
    // ordering budget. An unrepresentable Cartesian product keeps tuple sorting.
    let ordering_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("qualified clustered ordering bytes overflow"))?;
    let dense_plan = match dense_subset_plan(dimensions, mask, nobs, dimension, maximum_dimensions)
    {
        Ok(plan) => plan.filter(|plan| {
            dimension
                .checked_mul(16)
                .and_then(|v| v.checked_add(1))
                .and_then(|stride| plan.group_slots.checked_mul(stride))
                .is_some_and(|bytes| bytes <= ordering_bytes)
        }),
        Err(error) if error.code == crate::error::ErrorCode::Allocation => None,
        Err(error) => return Err(error),
    };
    if let Some(plan) = dense_plan {
        let mut row_values = reserved_filled(
            reservation,
            dimension,
            0.0,
            "unable to reserve qualified clustered score row",
        )?;
        let scores = accumulate_dense_groups(
            design,
            multipliers,
            dimensions,
            mask,
            plan,
            &mut row_values,
            reservation,
        )?;
        return finish_serial_dense_groups(
            &scores.values,
            &scores.corrections,
            &scores.seen,
            dimension,
            bread,
            reservation,
        );
    }
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

fn finish_serial_dense_groups<R: SandwichReservation>(
    values: &[f64],
    corrections: &[f64],
    seen: &[u8],
    dimension: usize,
    bread: &[f64],
    reservation: &mut R,
) -> Result<QualifiedClusterSubsetResult> {
    let square = dimension
        .checked_mul(dimension)
        .ok_or_else(|| allocation("qualified clustered serial meat overflows"))?;
    if bread.len() != square
        || values.len() != corrections.len()
        || seen.len().checked_mul(dimension) != Some(values.len())
    {
        return Err(invalid_argument(
            "qualified clustered group-score dimensions disagree",
        ));
    }
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
    for (group, present) in seen.iter().enumerate() {
        if *present == 0 {
            continue;
        }
        let start = group * dimension;
        let end = start + dimension;
        finish_serial_group(
            bread,
            &values[start..end],
            &corrections[start..end],
            &mut raw_score,
            &mut influence_score,
            &mut raw,
            &mut raw_corrections,
            &mut influence,
            &mut influence_corrections,
        )?;
        groups += 1;
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

#[cfg(test)]
mod scratch_table_tests {
    use super::*;
    use std::cell::RefCell;

    struct Design {
        nrows: usize,
        reads: RefCell<Vec<usize>>,
    }
    impl SandwichDesign for Design {
        fn nrows(&self) -> usize {
            self.nrows
        }
        fn dimension(&self) -> usize {
            2
        }
        fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
            self.reads.borrow_mut().push(row);
            output.copy_from_slice(&[(row % 7) as f64, (row % 3) as f64 * 0.25]);
            Ok(())
        }
    }
    #[derive(Clone, Copy)]
    struct Multipliers(usize);
    impl SandwichScoreMultiplier for Multipliers {
        fn nrows(&self) -> usize {
            self.0
        }
        fn score_pair(&self, row: usize) -> Result<super::super::SandwichScorePair> {
            let value = 1.0 + (row % 5) as f64;
            Ok(super::super::SandwichScorePair {
                production: value,
                certificate: value,
            })
        }
    }
    #[derive(Default)]
    struct Reservation(Vec<(&'static str, usize)>);
    impl SandwichReservation for Reservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            label: &'static str,
        ) -> Result<()> {
            self.0.push((label, elements * std::mem::size_of::<T>()));
            target
                .try_reserve_exact(elements)
                .map_err(|_| allocation(label))
        }
    }

    #[test]
    fn narrow_threaded_cluster_meat_matches_exact_weighted_group_oracle() {
        #[derive(Clone, Copy)]
        struct NarrowDesign(usize, usize);
        impl SandwichDesign for NarrowDesign {
            fn nrows(&self) -> usize {
                self.0
            }
            fn dimension(&self) -> usize {
                self.1
            }
            fn fill_row(&self, row: usize, output: &mut [f64]) -> Result<()> {
                for (column, value) in output.iter_mut().enumerate() {
                    *value = ((row * (2 * column + 1) % 19) as i64 - 9) as f64 / 8.0;
                }
                Ok(())
            }
        }
        #[derive(Clone, Copy)]
        struct WeightedScores(usize, usize);
        impl SandwichScoreMultiplier for WeightedScores {
            fn nrows(&self) -> usize {
                self.0
            }
            fn score_pair(&self, row: usize) -> Result<super::super::SandwichScorePair> {
                let weight = match self.1 {
                    0 => 1.0,
                    1 => (1 + row % 4) as f64 / 4.0,
                    2 => {
                        if row % 17 == 0 {
                            8.0
                        } else {
                            0.125
                        }
                    }
                    _ => (1 + row % 5) as f64,
                };
                let value = weight * ((row % 11) as i64 - 5) as f64;
                Ok(super::super::SandwichScorePair {
                    production: value,
                    certificate: value,
                })
            }
        }
        // Dyadic scores keep every group sum and outer product exact in f64.
        // The independent map oracle uses integer arithmetic in units of 1/64,
        // including noncontiguous labels, unbalanced groups and intersections.
        let nrows = 32_771;
        let first: Vec<u32> = (0..nrows)
            .map(|r| if r % 7 == 0 { 17 } else { (r % 3) as u32 })
            .collect();
        let second: Vec<u32> = (0..nrows).map(|r| ((r / 13) % 41) as u32 * 2).collect();
        let dimensions: [&[u32]; 2] = [&first, &second];
        let policy = QualifiedClusterRoutePolicy {
            maximum_dimensions: 10,
            reduction_maximum_blocks: 64,
            partial_budget_bytes: 64 << 20,
            ordered_rows_per_block: 16_384,
            dense_groups_per_block: 1_024,
        };
        for dimension in [6, 7] {
            for weight_kind in 0..4 {
                let design = NarrowDesign(nrows, dimension);
                let multipliers = WeightedScores(nrows, weight_kind);
                for mask in 1..4 {
                    let mut groups = std::collections::BTreeMap::<(u32, u32), Vec<i64>>::new();
                    for row in 0..nrows {
                        let key = (
                            if mask & 1 != 0 { first[row] } else { 0 },
                            if mask & 2 != 0 { second[row] } else { 0 },
                        );
                        let group = groups.entry(key).or_insert_with(|| vec![0; dimension]);
                        let weight_eighths = match weight_kind {
                            0 => 8,
                            1 => 2 * (1 + row % 4) as i64,
                            2 => {
                                if row % 17 == 0 {
                                    64
                                } else {
                                    1
                                }
                            }
                            _ => 8 * (1 + row % 5) as i64,
                        };
                        for (column, value) in group.iter_mut().enumerate() {
                            *value += weight_eighths
                                * ((row % 11) as i64 - 5)
                                * ((row * (2 * column + 1) % 19) as i64 - 9);
                        }
                    }
                    let mut exact = vec![0i64; dimension * dimension];
                    for group in groups.values() {
                        for i in 0..dimension {
                            for j in 0..dimension {
                                exact[i * dimension + j] += group[i] * group[j];
                            }
                        }
                    }
                    let expected: Vec<f64> = exact.iter().map(|&v| v as f64 / 4096.0).collect();
                    for threads in [1, 8, 16] {
                        let team = rayon::ThreadPoolBuilder::new()
                            .num_threads(threads)
                            .build()
                            .unwrap();
                        let mut reservation = Reservation::default();
                        let actual = team
                            .install(|| {
                                reduce_qualified_cluster_subset(
                                    &design,
                                    Some(design),
                                    multipliers,
                                    &dimensions,
                                    mask,
                                    QualifiedClusterCertificateRoute::OlsIndependentRaw,
                                    policy,
                                    &mut reservation,
                                )
                            })
                            .unwrap();
                        assert_eq!(actual.groups, groups.len());
                        assert_eq!(actual.production, expected);
                        assert_eq!(actual.certificate, expected);
                        assert_eq!(
                            reservation.0.iter().any(|(label, _)| *label
                                == "unable to reserve qualified clustered ordered partials"),
                            threads > 1
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn batched_scores_read_once_and_charge_metadata_at_the_boundary() {
        let bread = [1.0, 0.25, 0.25, 2.0];
        // Eleven group slots * 33 bytes + three ranges + four row codes = 443.
        for nrows in [55, 56] {
            let design = Design {
                nrows,
                reads: RefCell::new(Vec::new()),
            };
            let first: Vec<u32> = (0..nrows).map(|row| (row % 3) as u32).collect();
            let second: Vec<u32> = (0..nrows).map(|row| (row % 2) as u32).collect();
            let dimensions: [&[u32]; 2] = [&first, &second];
            let mut reservation = Reservation::default();
            let batch = prepare_cluster_score_batch(
                &design,
                Multipliers(nrows),
                &dimensions,
                10,
                &mut reservation,
            )
            .expect("batch planning");
            if nrows == 55 {
                assert!(batch.is_none());
                assert!(reservation.0.is_empty());
                assert!(design.reads.borrow().is_empty());
                continue;
            }
            let batch = batch.expect("443 bytes fit 448-byte order budget");
            assert_eq!(
                reservation
                    .0
                    .iter()
                    .filter(|(label, _)| *label != "unable to reserve batched cluster score row")
                    .map(|(_, bytes)| *bytes)
                    .sum::<usize>(),
                443
            );
            assert_eq!(*design.reads.borrow(), (0..nrows).collect::<Vec<_>>());
            for mask in [1, 2, 3] {
                let oracle_design = Design {
                    nrows,
                    reads: RefCell::new(Vec::new()),
                };
                let expected = aggregate_streaming_serial(
                    &oracle_design,
                    Multipliers(nrows),
                    &dimensions,
                    mask,
                    &bread,
                    10,
                    &mut Reservation::default(),
                )
                .expect("separate subset oracle");
                let actual = batch
                    .reduce_subset(mask, &bread, &mut reservation)
                    .expect("bank subset");
                assert_eq!(actual, expected);
            }
            assert_eq!(design.reads.borrow().len(), nrows);
            assert!(batch.reduce_subset(0, &bread, &mut reservation).is_err());
            assert!(batch.reduce_subset(4, &bread, &mut reservation).is_err());
            assert!(batch.reduce_subset(1, &[1.0], &mut reservation).is_err());
        }
    }

    #[test]
    fn five_way_batch_preserves_every_lexicographic_subset_and_sparse_decline() {
        let nrows = 2048;
        let design = Design {
            nrows,
            reads: RefCell::new(Vec::new()),
        };
        let columns: Vec<Vec<u32>> = (0..5)
            .map(|index| {
                (0..nrows)
                    .map(|row| ((row / (2 * index + 1)) % 2) as u32)
                    .collect()
            })
            .collect();
        let dimensions: Vec<&[u32]> = columns.iter().map(Vec::as_slice).collect();
        let mut reservation = Reservation::default();
        let batch = prepare_cluster_score_batch(
            &design,
            Multipliers(nrows),
            &dimensions,
            10,
            &mut reservation,
        )
        .expect("five-way planning")
        .expect("combined table fits");
        let bread = [1.0, 0.25, 0.25, 2.0];
        for mask in 1..32 {
            let expected = aggregate_streaming_serial(
                &design,
                Multipliers(nrows),
                &dimensions,
                mask,
                &bread,
                10,
                &mut Reservation::default(),
            )
            .expect("separate subset");
            assert_eq!(
                batch
                    .reduce_subset(mask, &bread, &mut reservation)
                    .expect("combined subset"),
                expected
            );
        }
        design.reads.borrow_mut().clear();
        let sparse = vec![u32::MAX; nrows];
        let dimensions: [&[u32]; 4] = [&sparse; 4];
        let mut reservation = Reservation::default();
        assert!(prepare_cluster_score_batch(
            &design,
            Multipliers(nrows),
            &dimensions,
            10,
            &mut reservation
        )
        .expect("sparse batch declines")
        .is_none());
        assert!(reservation.0.is_empty());
        assert!(design.reads.borrow().is_empty());
    }

    #[test]
    fn every_batch_reservation_failure_and_undersize_fails_before_row_reads() {
        struct SelectiveReservation {
            calls: usize,
            fail_at: usize,
            undersize: bool,
        }
        impl SandwichReservation for SelectiveReservation {
            fn reserve<T>(
                &mut self,
                target: &mut Vec<T>,
                elements: usize,
                label: &'static str,
            ) -> Result<()> {
                let fails = self.calls == self.fail_at;
                self.calls += 1;
                if fails && !self.undersize {
                    return Err(allocation(label));
                }
                target
                    .try_reserve_exact(if fails {
                        elements.saturating_sub(1)
                    } else {
                        elements
                    })
                    .map_err(|_| allocation(label))
            }
        }
        let nrows = 64;
        let first: Vec<u32> = (0..nrows).map(|row| (row % 3) as u32).collect();
        let second: Vec<u32> = (0..nrows).map(|row| (row % 2) as u32).collect();
        let dimensions: [&[u32]; 2] = [&first, &second];
        let design = Design {
            nrows,
            reads: RefCell::new(Vec::new()),
        };
        let mut count = Reservation::default();
        assert!(prepare_cluster_score_batch(
            &design,
            Multipliers(nrows),
            &dimensions,
            10,
            &mut count
        )
        .expect("count reservations")
        .is_some());
        for fail_at in 0..count.0.len() {
            for undersize in [false, true] {
                design.reads.borrow_mut().clear();
                let mut reservation = SelectiveReservation {
                    calls: 0,
                    fail_at,
                    undersize,
                };
                assert!(prepare_cluster_score_batch(
                    &design,
                    Multipliers(nrows),
                    &dimensions,
                    10,
                    &mut reservation
                )
                .is_err());
                assert!(design.reads.borrow().is_empty());
            }
        }
    }

    #[test]
    fn sequential_scratch_table_respects_ordering_budget_and_sparse_fallback() {
        // Six group slots require exactly 198 bytes for two compensated scores
        // and presence flags. The old row order permits 192 or 200 bytes.
        for (nrows, sparse) in [(24, false), (25, false), (25, true)] {
            let design = Design {
                nrows,
                reads: RefCell::new(Vec::new()),
            };
            let first: Vec<u32> = (0..nrows)
                .map(|row| {
                    if sparse {
                        if row % 2 == 0 {
                            0
                        } else {
                            u32::MAX
                        }
                    } else {
                        (row % 3) as u32
                    }
                })
                .collect();
            let second: Vec<u32> = (0..nrows)
                .map(|row| {
                    if sparse {
                        if row % 3 == 0 {
                            u32::MAX
                        } else {
                            0
                        }
                    } else {
                        (row % 2) as u32
                    }
                })
                .collect();
            let mut reservation = Reservation::default();
            let result = aggregate_streaming_serial(
                &design,
                Multipliers(nrows),
                &[&first, &second],
                3,
                &[1.0, 0.0, 0.0, 1.0],
                10,
                &mut reservation,
            )
            .expect("bounded or fallback grouping");
            assert_eq!(result.groups, if sparse { 4 } else { 6 });
            let table_bytes: usize = reservation
                .0
                .iter()
                .filter(|(label, _)| {
                    [
                        "unable to reserve qualified clustered dense group scores",
                        "unable to reserve qualified clustered dense group corrections",
                        "unable to reserve qualified clustered dense group presence",
                    ]
                    .contains(label)
                })
                .map(|(_, bytes)| *bytes)
                .sum();
            let ordered = reservation.0.iter().any(|(label, _)| {
                *label == "unable to reserve qualified clustered subset ordering"
            });
            if nrows == 25 && !sparse {
                assert_eq!(table_bytes, 198);
                assert!(table_bytes <= nrows * std::mem::size_of::<usize>());
                assert!(!ordered);
                assert_eq!(*design.reads.borrow(), (0..nrows).collect::<Vec<_>>());
            } else {
                assert_eq!(table_bytes, 0);
                assert!(ordered);
                let mut expected: Vec<_> = (0..nrows).collect();
                expected.sort_unstable_by_key(|&row| (first[row], second[row], row));
                assert_eq!(*design.reads.borrow(), expected);
            }
        }
    }
}

#[cfg(test)]
mod paired_scan_tests {
    use super::super::{SandwichScorePair, SystemSandwichReservation};
    use super::*;

    #[derive(Clone, Copy)]
    struct Scan {
        rows: usize,
        width: usize,
    }
    impl SandwichDesign for Scan {
        fn nrows(&self) -> usize {
            self.rows
        }
        fn dimension(&self) -> usize {
            self.width
        }
        fn prefers_sequential_scan(&self) -> bool {
            true
        }
        fn fill_row(&self, _: usize, _: &mut [f64]) -> Result<()> {
            panic!("admitted paired scan must retain its range reader")
        }
        fn visit_range(
            &self,
            rows: std::ops::Range<usize>,
            out: &mut [f64],
            mut visit: impl FnMut(usize, &mut [f64]) -> Result<()>,
        ) -> Result<()> {
            for row in rows {
                for (j, v) in out.iter_mut().enumerate() {
                    *v = ((row * (2 * j + 1) % 19) as i64 - 9) as f64 / 8.;
                }
                visit(row, out)?;
            }
            Ok(())
        }
    }
    #[derive(Clone, Copy)]
    struct Scores {
        rows: usize,
        mode: usize,
        poison: bool,
    }
    impl SandwichScoreMultiplier for Scores {
        fn nrows(&self) -> usize {
            self.rows
        }
        fn score_pair(&self, row: usize) -> Result<SandwichScorePair> {
            let weight = match self.mode {
                0 => 1.,
                1 => (1 + row % 4) as f64 / 4.,
                2 => {
                    if row % 17 == 0 {
                        8.
                    } else {
                        0.125
                    }
                }
                _ => (1 + row % 5) as f64,
            };
            Ok(SandwichScorePair {
                production: weight * ((row % 11) as i64 - 5) as f64,
                certificate: if self.poison && row == 19 {
                    f64::NAN
                } else {
                    weight * ((row % 7) as i64 - 3) as f64
                },
            })
        }
    }
    fn policy() -> QualifiedClusterRoutePolicy {
        QualifiedClusterRoutePolicy {
            maximum_dimensions: 5,
            reduction_maximum_blocks: 64,
            partial_budget_bytes: 32 << 20,
            ordered_rows_per_block: 16384,
            dense_groups_per_block: 1024,
        }
    }
    fn certificate() -> QualifiedClusterCertificateRoute<'static> {
        QualifiedClusterCertificateRoute::PairedIndependent(
            FixedBlockReductionPolicy {
                rows_per_block: 16384,
                maximum_blocks: 64,
                partial_budget_bytes: 32 << 20,
            },
            None,
        )
    }

    #[test]
    fn paired_scan_matches_separate_integer_group_oracles_and_teams() {
        let rows = 4097;
        let a: Vec<u32> = (0..rows)
            .map(|r| if r % 7 == 0 { 4 } else { (r % 2) as u32 })
            .collect();
        let b: Vec<u32> = (0..rows).map(|r| ((r / 13) % 5) as u32).collect();
        let dimensions = [&a[..], &b[..]];
        for width in [1, 6, 21] {
            for mode in 0..4 {
                for mask in 1..4 {
                    let mut groups =
                        std::collections::BTreeMap::<(u32, u32), (Vec<i64>, Vec<i64>)>::new();
                    for row in 0..rows {
                        let pair = groups
                            .entry((
                                if mask & 1 != 0 { a[row] } else { 0 },
                                if mask & 2 != 0 { b[row] } else { 0 },
                            ))
                            .or_insert_with(|| (vec![0; width], vec![0; width]));
                        let w = match mode {
                            0 => 8,
                            1 => 2 * (1 + row % 4) as i64,
                            2 => {
                                if row % 17 == 0 {
                                    64
                                } else {
                                    1
                                }
                            }
                            _ => 8 * (1 + row % 5) as i64,
                        };
                        for j in 0..width {
                            let x = (row * (2 * j + 1) % 19) as i64 - 9;
                            pair.0[j] += x * w * ((row % 11) as i64 - 5);
                            pair.1[j] += x * w * ((row % 7) as i64 - 3);
                        }
                    }
                    let mut expected = [vec![0i64; width * width], vec![0i64; width * width]];
                    for (p, c) in groups.values() {
                        for (arm, scores) in [p, c].iter().enumerate() {
                            for i in 0..width {
                                for j in 0..width {
                                    expected[arm][i * width + j] += scores[i] * scores[j];
                                }
                            }
                        }
                    }
                    for threads in [1, 4] {
                        let pool = rayon::ThreadPoolBuilder::new()
                            .num_threads(threads)
                            .build()
                            .unwrap();
                        let design = Scan { rows, width };
                        let result = pool
                            .install(|| {
                                reduce_qualified_cluster_subset(
                                    &design,
                                    Some(design),
                                    Scores {
                                        rows,
                                        mode,
                                        poison: false,
                                    },
                                    &dimensions,
                                    mask,
                                    certificate(),
                                    policy(),
                                    &mut SystemSandwichReservation,
                                )
                            })
                            .unwrap();
                        assert_eq!(result.groups, groups.len());
                        for (actual, expected) in [&result.production, &result.certificate]
                            .iter()
                            .zip(&expected)
                        {
                            assert_eq!(
                                **actual,
                                expected
                                    .iter()
                                    .map(|&v| v as f64 / 4096.)
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn paired_scan_budget_refuses_sparse_products_and_checks_complete_bank() {
        let dense = [0, 1, 2, 3];
        let sparse = [0, u32::MAX, 1, 2];
        let bytes = paired_workspace_bytes(4, 6).unwrap();
        let minimum_rows = bytes.div_ceil(2 * std::mem::size_of::<usize>());
        assert!(paired_subset_plan(&[&dense], 1, minimum_rows, 6).is_some());
        assert!(paired_subset_plan(&[&dense], 1, minimum_rows - 1, 6).is_none());
        assert!(paired_subset_plan(&[&sparse, &sparse, &sparse], 7, 4096, 6).is_none());
        assert!(paired_subset_plan(&[&dense], 1, usize::MAX, usize::MAX).is_none());
    }

    #[test]
    fn paired_scan_allocation_and_independent_certificate_fail_closed() {
        struct Fail(usize);
        impl SandwichReservation for Fail {
            fn reserve<T>(&mut self, v: &mut Vec<T>, n: usize, label: &'static str) -> Result<()> {
                if self.0 == 0 {
                    return Err(allocation(label));
                }
                self.0 -= 1;
                v.try_reserve_exact(n).map_err(|_| allocation(label))
            }
        }
        let rows = 4096;
        let codes = vec![0; rows];
        let design = Scan { rows, width: 2 };
        for fail in 0..7 {
            let result = reduce_qualified_cluster_subset(
                &design,
                Some(design),
                Scores {
                    rows,
                    mode: 0,
                    poison: false,
                },
                &[&codes],
                1,
                certificate(),
                policy(),
                &mut Fail(fail),
            );
            assert_eq!(
                result.unwrap_err().code,
                crate::error::ErrorCode::Allocation
            );
        }
        let result = reduce_qualified_cluster_subset(
            &design,
            Some(design),
            Scores {
                rows,
                mode: 0,
                poison: true,
            },
            &[&codes],
            1,
            certificate(),
            policy(),
            &mut SystemSandwichReservation,
        );
        assert!(result.is_err());
    }
}
