//! Exact parallel forms of serial sections of the PPML joint recurrence.
//!
//! Each section splits independent per-element or per-column work into fixed
//! chunks. Every element and every column keeps its serial operation sequence
//! (the same operations on the same operands in the same order), so results
//! are bitwise identical to the serial form at every team width; only the
//! scheduling changes. Each parallel chunk polls cancellation before it
//! starts. Sections use no scratch beyond their callers' banks.
use crate::error::Result;
use crate::fe::LevelIndex;
use rayon::prelude::*;

/// Element operations below which a section stays on the calling thread.
pub(crate) const MIN_PARALLEL_WORK: usize = 1 << 16;
/// Output elements per parallel task.
const CHUNK: usize = 1 << 12;

fn parallel(work: usize) -> bool {
    #[cfg(test)]
    if let Some(forced) = tests::forced() {
        return forced;
    }
    work >= MIN_PARALLEL_WORK && rayon::current_num_threads() > 1
}

/// `output += partial` for each partial in order: the merge of helper
/// products after a shared single-RHS application.
pub(crate) fn add_partials<T: Sync>(
    output: &mut [f64],
    partials: &[T],
    partial: impl Fn(&T) -> &[f64] + Sync,
) -> Result<()> {
    if !parallel(output.len().saturating_mul(partials.len())) {
        for item in partials {
            for (value, addend) in output.iter_mut().zip(partial(item)) {
                *value += addend;
            }
        }
        return Ok(());
    }
    output
        .par_chunks_mut(CHUNK)
        .enumerate()
        .try_for_each(|(chunk, values)| {
            crate::interrupt::checkpoint()?;
            let start = chunk * CHUNK;
            for item in partials {
                let addends = partial(item).get(start..).unwrap_or(&[]);
                for (value, addend) in values.iter_mut().zip(addends) {
                    *value += addend;
                }
            }
            Ok(())
        })
}

/// The finite cross block of one joint product. With `p` retained and `k`
/// finite coordinates, `cross` holds the `k` retained cross columns and
/// `hessian` the finite Gram rows. Each finite output is
/// `cross_j . d_p + hessian_j . d_k`, and each retained output adds
/// `cross_j * d_{p+j}` for `j` in order.
pub(crate) fn cross_block(
    cross: &[f64],
    hessian: &[f64],
    direction: &[f64],
    product: &mut [f64],
    p: usize,
    k: usize,
) -> Result<()> {
    let hardware = crate::kernels::installed();
    let dot =
        |left: &[f64], right: &[f64]| crate::kernels::dispatch!(hardware, ordered_dot(left, right));
    let (retained, finite) = product[..p + k].split_at_mut(p);
    if !parallel(p.saturating_mul(k)) {
        for j in 0..k {
            let column = &cross[j * p..(j + 1) * p];
            finite[j] = dot(column, &direction[..p])
                + dot(&hessian[j * k..(j + 1) * k], &direction[p..p + k]);
            crate::kernels::dispatch!(hardware, scaled_add(column, direction[p + j], retained));
        }
        return Ok(());
    }
    // The finite outputs read only the direction, never the retained product.
    finite
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(j, value)| {
            crate::interrupt::checkpoint()?;
            *value = dot(&cross[j * p..(j + 1) * p], &direction[..p])
                + dot(&hessian[j * k..(j + 1) * k], &direction[p..p + k]);
            Ok(())
        })?;
    retained
        .par_chunks_mut(CHUNK)
        .enumerate()
        .try_for_each(|(chunk, values)| {
            crate::interrupt::checkpoint()?;
            let start = chunk * CHUNK;
            for j in 0..k {
                let column = &cross[j * p + start..j * p + start + values.len()];
                crate::kernels::dispatch!(hardware, scaled_add(column, direction[p + j], values));
            }
            Ok(())
        })
}

/// `roots[level] = sqrt(sum of weights[row])` over each level's rows in row
/// order, as a serial scatter over `codes` accumulates them.
pub(crate) fn level_root_masses(
    roots: &mut [f64],
    codes: &[u32],
    index: &LevelIndex,
    weights: &[f64],
) -> Result<()> {
    if !parallel(weights.len()) {
        roots.fill(0.0);
        for (&code, &weight) in codes.iter().zip(weights) {
            roots[code as usize] += weight;
        }
        for root in roots.iter_mut() {
            *root = root.sqrt();
        }
        return Ok(());
    }
    roots
        .par_chunks_mut(CHUNK)
        .enumerate()
        .try_for_each(|(chunk, values)| {
            crate::interrupt::checkpoint()?;
            for (offset, root) in values.iter_mut().enumerate() {
                let mut sum = 0.0;
                for &row in index.members(chunk * CHUNK + offset) {
                    sum += weights[row];
                }
                *root = sum.sqrt();
            }
            Ok(())
        })
}

#[cfg(test)]
pub(crate) mod tests;
