//! At a finite saturated optimum the weighted residual sum in each design cell
//! vanishes, including with unequal offsets or weights. Each cluster partition
//! that keeps whole cells then has zero group scores (also for the private
//! intercept). A dimension refining another declared partition cancels in CGM:
//! pair each subset containing the finer dimension with/without the coarser.
//! Thus split-cell dimensions may be removed if each refines a whole-cell one;
//! all intersections of the remaining whole-cell partitions also keep cells whole.

use super::modular_rank::{insert, residue};
use super::prepared_model::PreparedPpmlModelGeneration;
use crate::error::{allocation, Result};
use crate::memory::fallible_filled;
use crate::ols::prepared_fit_view::visit_prepared_column_range;
use crate::ols::PreparedOlsColumnKind;

const MAX_FIELD_OPERATIONS: u128 = 32 << 20;

#[derive(Clone, Copy)]
enum Coordinate {
    Indicator(usize),
    Numeric(usize),
}

impl Coordinate {
    fn set(self, key: &mut [u64], value: f64) {
        match self {
            Self::Indicator(bit) => key[bit / 64] |= u64::from(value == 1.) << (bit % 64),
            Self::Numeric(slot) => key[slot] = if value == 0. { 0 } else { value.to_bits() },
        }
    }

    fn field_value(self, key: &[u64]) -> u64 {
        match self {
            Self::Indicator(bit) => (key[bit / 64] >> (bit % 64)) & 1,
            Self::Numeric(slot) => residue(f64::from_bits(key[slot])),
        }
    }
}

pub(super) fn certified(
    generation: &PreparedPpmlModelGeneration,
    kept: &[usize],
    clusters: &[&[u32]],
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    if clusters.is_empty() || kept.is_empty() {
        return Ok(false);
    }
    let data = generation.fit_view();
    let columns = &data.model().columns;
    let topology = data.fe_topology();
    let levels = topology.registry().total_levels()?;
    let specs = data.effect_specs();
    let with_intercept = |source: usize| {
        specs
            .iter()
            .zip(topology.effect_to_topology())
            .any(|(spec, &partition)| source == partition && spec.intercept)
    };
    let intercept_levels: usize = topology
        .registry()
        .sources()
        .iter()
        .enumerate()
        .filter(|(idx, _)| with_intercept(*idx))
        .map(|(_, source)| source.n_levels())
        .sum();
    let rank_width = kept.len() + intercept_levels;
    let dimensions = data.clusters().dimensions();
    let rows = generation.outcome().len();
    // In a single dimension each cell must be wholly in one group. In the
    // union of dimensions, every same-cell pair must at least share a group.
    // An anchor has at most sum(N - G_d) possible neighbours.
    let neighbour_bound: u128 = 1 + dimensions
        .iter()
        .map(|d| (rows - d.groups()) as u128)
        .sum::<u128>();
    if (clusters.len() == 1 && dimensions[0].groups() > rank_width)
        || rank_width as u128 * neighbour_bound < rows as u128
    {
        return Ok(false);
    }
    let continuous = columns
        .iter()
        .filter(|column| matches!(column.kind, PreparedOlsColumnKind::Continuous { .. }))
        .count();
    let slopes: usize = specs.iter().map(|s| s.slopes.len()).sum();
    let bit_words = (columns.len() + levels).div_ceil(64);
    let stride = bit_words + continuous + slopes;
    let n = rows as u128;
    let k = rank_width as u128;
    let work = n
        * (columns.len() as u128 + slopes as u128 + topology.registry().sources().len() as u128)
        + n.saturating_mul(k).saturating_mul(k.min(n));
    if work > MAX_FIELD_OPERATIONS {
        return Ok(false);
    }
    // The scratch is retired before the already-admitted score reduction.
    let group_map = 4 * dimensions.iter().map(|d| d.groups()).max().unwrap() as u128;
    let bytes = n * (8 * stride as u128 + std::mem::size_of::<usize>() as u128)
        + k * std::mem::size_of::<Coordinate>() as u128
        + (k + 1) * std::mem::size_of::<usize>() as u128
        + group_map.max(8 * k * (k + 1));
    if bytes > super::vce::clustered_score_workspace_bound(rows, columns.len())? {
        return Ok(false);
    }

    if rows > rank_width && continuous > 0 && continuous_splits_cells(generation, rank_width, poll)?
    {
        return Ok(false);
    }
    let mut keys = fallible_filled(rows * stride, 0_u64, "unable to reserve PPML design cells")?;
    let mut witness = Vec::new();
    witness
        .try_reserve_exact(rank_width)
        .map_err(|_| allocation("unable to reserve PPML fitted cell coordinates"))?;
    let mut numeric = bit_words;
    for (j, column) in columns.iter().enumerate() {
        poll()?;
        let field = if matches!(column.kind, PreparedOlsColumnKind::Continuous { .. }) {
            numeric += 1;
            Coordinate::Numeric(numeric - 1)
        } else {
            Coordinate::Indicator(j)
        };
        visit_prepared_column_range(data, j, 0..rows, |row, value| {
            crate::interrupt::row_checkpoint(row)?;
            field.set(&mut keys[row * stride..(row + 1) * stride], value);
            Ok(())
        })?;
        if kept.contains(&j) {
            witness.push(field);
        }
    }
    let mut bit = columns.len();
    for (idx, source) in topology.registry().sources().iter().enumerate() {
        let intercept = with_intercept(idx);
        for level in 0..source.n_levels() {
            poll()?;
            let field = Coordinate::Indicator(bit);
            for (i, &row) in source.members(level)?.iter().enumerate() {
                crate::interrupt::row_checkpoint(i)?;
                field.set(&mut keys[row * stride..(row + 1) * stride], 1.);
            }
            if intercept {
                witness.push(field);
            }
            bit += 1;
        }
    }
    // Every original recipe and nuisance channel distinguishes cells. A
    // numerically omitted finite or local slope direction cannot supply a rank
    // witness. The full set of kept finite and categorical intercept directions
    // suffices if independent over these possibly finer cells.
    for slope in specs.iter().flat_map(|s| &s.slopes) {
        poll()?;
        let field = Coordinate::Numeric(numeric);
        for (row, &value) in data.numeric_source_values(slope.source)?.iter().enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            field.set(&mut keys[row * stride..(row + 1) * stride], value);
        }
        numeric += 1;
    }
    let mut groups = [0; 5];
    for (j, d) in dimensions.iter().enumerate() {
        groups[j] = d.groups();
    }
    saturated_cells(
        &keys,
        stride,
        &witness,
        generation.outcome(),
        clusters,
        &groups,
        poll,
    )
}

/// A single coordinate taking more distinct values than the fitted rank
/// rules out saturation before allocating the row keys.
fn continuous_splits_cells(
    generation: &PreparedPpmlModelGeneration,
    rank_width: usize,
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let data = generation.fit_view();
    let rows = generation.outcome().len();
    let mut sample = fallible_filled(rank_width + 1, 0_u64, "unable to reserve PPML cell probe")?;
    for column in &data.model().columns {
        if let PreparedOlsColumnKind::Continuous { source, .. } = column.kind {
            poll()?;
            let values = data.numeric_source_values(source)?;
            for (i, bit) in sample.iter_mut().enumerate() {
                let value = values[i * (rows - 1) / rank_width];
                *bit = if value == 0. { 0 } else { value.to_bits() };
            }
            sample.sort_unstable();
            if sample.windows(2).all(|pair| pair[0] != pair[1]) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn nested(fine: &[u32], groups: usize, coarse: &[u32]) -> Result<bool> {
    let mut owner = fallible_filled(
        groups,
        u32::MAX,
        "unable to reserve PPML cluster nesting map",
    )?;
    for (row, (&group, &parent)) in fine.iter().zip(coarse).enumerate() {
        crate::interrupt::row_checkpoint(row)?;
        let slot = &mut owner[group as usize];
        if *slot != u32::MAX && *slot != parent {
            return Ok(false);
        }
        *slot = parent;
    }
    Ok(true)
}

fn saturated_cells(
    keys: &[u64],
    stride: usize,
    witness: &[Coordinate],
    outcome: &[f64],
    clusters: &[&[u32]],
    groups: &[usize; 5],
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let key = |row: usize| &keys[row * stride..(row + 1) * stride];
    let mut order = fallible_filled(
        outcome.len(),
        0usize,
        "unable to reserve PPML design-cell order",
    )?;
    for (id, row) in order.iter_mut().enumerate() {
        *row = id;
    }
    order.sort_unstable_by(|&a, &b| key(a).cmp(key(b)));
    let mut whole = [true; 5];
    let mut starts = Vec::new();
    starts
        .try_reserve_exact(witness.len() + 1)
        .map_err(|_| allocation("unable to reserve PPML design-cell boundaries"))?;
    let mut start = 0;
    while start < order.len() {
        let representative = order[start];
        let mut end = start;
        let mut positive = false;
        while end < order.len() && key(order[end]) == key(representative) {
            crate::interrupt::row_checkpoint(end)?;
            for (j, codes) in clusters.iter().enumerate() {
                whole[j] &= codes[order[end]] == codes[representative];
            }
            positive |= outcome[order[end]] > 0.;
            end += 1;
        }
        if !positive || starts.len() == witness.len() {
            return Ok(false);
        }
        starts.push(start);
        start = end;
    }
    starts.push(order.len());
    for (fine, &codes) in clusters.iter().enumerate().filter(|(d, _)| !whole[*d]) {
        let mut cancels = false;
        for (coarse, &parent) in clusters.iter().enumerate() {
            if whole[coarse]
                && groups[coarse] <= groups[fine]
                && nested(codes, groups[fine], parent)?
            {
                cancels = true;
                break;
            }
        }
        if !cancels {
            if !annihilates_cell_contrasts(&order, &starts, clusters, poll)? {
                return Ok(false);
            }
            break;
        }
    }
    let width = witness.len();
    let mut basis = fallible_filled(
        width * width,
        0u64,
        "unable to reserve PPML saturated rank basis",
    )?;
    let mut row = fallible_filled(width, 0u64, "unable to reserve PPML saturated rank row")?;
    for &start in &starts[..starts.len() - 1] {
        let id = order[start];
        poll()?;
        for (dst, field) in row.iter_mut().zip(witness) {
            *dst = field.field_value(key(id));
        }
        // A modular rank deficit leaves the ordinary estimator in charge.
        if !insert(&mut basis, &mut row) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Inclusion–exclusion weights the pair (i,j) once if any dimension puts it
/// in the same group. The residual vector within each saturated cell sums to
/// zero. Its contrast basis consists of e_i - e_anchor, so the following
/// integer differences certify the entire quadratic form, not one outcome.
fn annihilates_cell_contrasts(
    order: &[usize],
    starts: &[usize],
    clusters: &[&[u32]],
    poll: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let contrasts = (order.len() + 1 - starts.len()) as u128;
    if 4 * contrasts * contrasts * clusters.len() as u128 > MAX_FIELD_OPERATIONS {
        return Ok(false);
    }
    let shares = |i: usize, j: usize| i8::from(clusters.iter().any(|codes| codes[i] == codes[j]));
    for (a, left) in starts.windows(2).enumerate() {
        poll()?;
        let left_anchor = order[left[0]];
        for right in starts[a..].windows(2) {
            let right_anchor = order[right[0]];
            let corners = shares(left_anchor, right_anchor);
            for (step, &i) in order[left[0] + 1..left[1]].iter().enumerate() {
                crate::interrupt::row_checkpoint(step)?;
                let fixed = corners - shares(i, right_anchor);
                for &j in &order[right[0] + 1..right[1]] {
                    if fixed + shares(i, j) - shares(left_anchor, j) != 0 {
                        return Ok(false);
                    }
                }
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "saturated_cluster_tests.rs"]
mod tests;
