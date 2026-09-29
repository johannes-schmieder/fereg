//! Fixed-order score reductions and an independent influence-coordinate replay.
use super::{invalid, numerical};
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    threads::{blocked_sum, compensated_sum, ThreadTeam},
};
use rayon::prelude::*;

pub(super) fn meat<F>(n: usize, p: usize, score: F, team: &ThreadTeam) -> Result<DenseMatrix>
where
    F: Fn(usize, usize) -> f64 + Sync,
{
    let mut output = DenseMatrix::zeros(p, p)?;
    if p == 0 {
        return Ok(output);
    }
    team.install(|| {
        output
            .as_mut_slice()
            .par_chunks_mut(p)
            .enumerate()
            .for_each(|(left, row)| {
                for (right, value) in row.iter_mut().enumerate().skip(left) {
                    *value = blocked_sum(n, |i| score(i, left) * score(i, right));
                }
            })
    });
    for left in 0..p {
        for right in 0..left {
            output[(left, right)] = output[(right, left)];
        }
    }
    finite(&output)?;
    Ok(output)
}
pub(super) fn sandwich(bread: &DenseMatrix, meat: &DenseMatrix) -> Result<DenseMatrix> {
    let p = bread.nrows();
    let mut left = DenseMatrix::zeros(p, p)?;
    for i in 0..p {
        for j in 0..p {
            left[(i, j)] = compensated_sum((0..p).map(|k| bread[(i, k)] * meat[(k, j)]));
        }
    }
    let mut output = DenseMatrix::zeros(p, p)?;
    for i in 0..p {
        for j in 0..p {
            output[(i, j)] = compensated_sum((0..p).map(|k| left[(i, k)] * bread[(j, k)]));
        }
    }
    finite(&output)?;
    Ok(output)
}
/// B s is formed afresh, then summed in reverse observation/group order. This
/// verifies the full sandwich without reusing the raw-meat products.
pub(super) fn influence_meat<F>(
    n: usize,
    bread: &DenseMatrix,
    score: F,
    team: &ThreadTeam,
) -> Result<DenseMatrix>
where
    F: Fn(usize, usize) -> f64 + Sync,
{
    let p = bread.nrows();
    let mut partials = reserved(n.div_ceil(1024))?;
    partials.resize_with(n.div_ceil(1024), || None);
    team.install(|| {
        partials
            .par_iter_mut()
            .enumerate()
            .for_each(|(block, slot)| {
                *slot = Some(influence_block(n, block, bread, &score));
            })
    });
    let mut output = DenseMatrix::zeros(p, p)?;
    let mut correction = DenseMatrix::zeros(p, p)?;
    for part in partials {
        add(
            &mut output,
            &mut correction,
            &part.expect("visited score block")?,
            1.,
        )?;
    }
    finish(&mut output, &correction)?;
    Ok(output)
}
fn influence_block<F>(n: usize, block: usize, bread: &DenseMatrix, score: &F) -> Result<DenseMatrix>
where
    F: Fn(usize, usize) -> f64,
{
    let p = bread.nrows();
    let mut influence = reserved(p)?;
    influence.resize(p, 0.);
    let mut result = DenseMatrix::zeros(p, p)?;
    let mut correction = DenseMatrix::zeros(p, p)?;
    for position in block * 1024..((block + 1) * 1024).min(n) {
        let row = n - 1 - position;
        for j in 0..p {
            influence[j] = compensated_sum((0..p).rev().map(|k| bread[(j, k)] * score(row, k)));
        }
        for i in 0..p {
            for j in i..p {
                compensated_add(
                    influence[i] * influence[j],
                    &mut result[(i, j)],
                    &mut correction[(i, j)],
                );
            }
        }
    }
    finish(&mut result, &correction)?;
    for i in 0..p {
        for j in 0..i {
            result[(i, j)] = result[(j, i)];
        }
    }
    Ok(result)
}
pub(super) fn add(
    sum: &mut DenseMatrix,
    correction: &mut DenseMatrix,
    value: &DenseMatrix,
    sign: f64,
) -> Result<()> {
    if sum.nrows() != value.nrows()
        || sum.ncols() != value.ncols()
        || sum.nrows() != correction.nrows()
        || sum.ncols() != correction.ncols()
    {
        return Err(invalid("covariance accumulator dimensions disagree"));
    }
    for ((a, c), b) in sum
        .as_mut_slice()
        .iter_mut()
        .zip(correction.as_mut_slice())
        .zip(value.as_slice())
    {
        compensated_add(sign * b, a, c);
    }
    finite(sum)?;
    finite(correction)
}
pub(super) fn finish(sum: &mut DenseMatrix, correction: &DenseMatrix) -> Result<()> {
    for (a, c) in sum.as_mut_slice().iter_mut().zip(correction.as_slice()) {
        *a += c;
    }
    finite(sum)
}
fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) {
    let next = *sum + value;
    *correction += if sum.abs() >= value.abs() {
        (*sum - next) + value
    } else {
        (value - next) + *sum
    };
    *sum = next;
}
pub(super) fn finite(matrix: &DenseMatrix) -> Result<()> {
    if matrix.as_slice().iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(numerical("covariance reduction is not representable"))
    }
}
pub(super) fn difference(a: &DenseMatrix, b: &DenseMatrix) -> f64 {
    (0..a.nrows())
        .map(|i| {
            let discrepancy =
                compensated_sum((0..a.ncols()).map(|j| (a[(i, j)] - b[(i, j)]).abs()));
            let scale =
                compensated_sum((0..a.ncols()).map(|j| a[(i, j)].abs().max(b[(i, j)].abs())));
            discrepancy / scale.max(f64::MIN_POSITIVE)
        })
        .fold(0., f64::max)
}
