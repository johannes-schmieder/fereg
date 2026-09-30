//! Fixed-order score reductions and an independent influence-coordinate replay.
use super::{invalid, numerical};
use crate::{
    data::DenseMatrix,
    error::{reserved, Result},
    execution::{self, Poll},
    threads::ThreadTeam,
};
use rayon::prelude::*;

pub(super) fn meat<F>(
    n: usize,
    p: usize,
    score: F,
    team: &ThreadTeam,
    poll: Poll<'_>,
) -> Result<DenseMatrix>
where
    F: Fn(usize, usize) -> f64 + Sync,
{
    execution::check(poll)?;
    let mut output = DenseMatrix::zeros(p, p)?;
    if p == 0 {
        return Ok(output);
    }
    team.install(|| {
        output
            .as_mut_slice()
            .par_chunks_mut(p)
            .enumerate()
            .try_for_each(|(left, row)| -> Result<()> {
                execution::check(poll)?;
                for (right, value) in row.iter_mut().enumerate().skip(left) {
                    *value = execution::blocked_sum(n, |i| score(i, left) * score(i, right), poll)?;
                }
                Ok(())
            })
    })?;
    for left in 0..p {
        execution::check(poll)?;
        for right in 0..left {
            execution::check_chunk(poll, right)?;
            output[(left, right)] = output[(right, left)];
        }
    }
    finite(&output, poll)?;
    Ok(output)
}
pub(super) fn sandwich(
    bread: &DenseMatrix,
    meat: &DenseMatrix,
    poll: Poll<'_>,
) -> Result<DenseMatrix> {
    execution::check(poll)?;
    let p = bread.nrows();
    let mut left = DenseMatrix::zeros(p, p)?;
    for i in 0..p {
        execution::check(poll)?;
        for j in 0..p {
            execution::check_chunk(poll, j)?;
            left[(i, j)] = execution::sum((0..p).map(|k| bread[(i, k)] * meat[(k, j)]), poll)?;
        }
    }
    let mut output = DenseMatrix::zeros(p, p)?;
    for i in 0..p {
        execution::check(poll)?;
        for j in 0..p {
            execution::check_chunk(poll, j)?;
            output[(i, j)] = execution::sum((0..p).map(|k| left[(i, k)] * bread[(j, k)]), poll)?;
        }
    }
    finite(&output, poll)?;
    Ok(output)
}
/// B s is formed afresh, then summed in reverse observation/group order. This
/// verifies the full sandwich without reusing the raw-meat products.
pub(super) fn influence_meat<F>(
    n: usize,
    bread: &DenseMatrix,
    score: F,
    team: &ThreadTeam,
    poll: Poll<'_>,
) -> Result<DenseMatrix>
where
    F: Fn(usize, usize) -> Result<f64> + Sync,
{
    execution::check(poll)?;
    let p = bread.nrows();
    let mut partials = reserved(n.div_ceil(1024))?;
    partials.resize_with(n.div_ceil(1024), || None);
    team.install(|| {
        partials
            .par_iter_mut()
            .enumerate()
            .for_each(|(block, slot)| {
                *slot = Some(influence_block(n, block, bread, &score, poll));
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
            poll,
        )?;
    }
    finish(&mut output, &correction, poll)?;
    Ok(output)
}
fn influence_block<F>(
    n: usize,
    block: usize,
    bread: &DenseMatrix,
    score: &F,
    poll: Poll<'_>,
) -> Result<DenseMatrix>
where
    F: Fn(usize, usize) -> Result<f64>,
{
    execution::check(poll)?;
    let p = bread.nrows();
    let mut influence = reserved(p)?;
    influence.resize(p, 0.);
    let mut result = DenseMatrix::zeros(p, p)?;
    let mut correction = DenseMatrix::zeros(p, p)?;
    // Count coordinate work across rows so wide matrices also poll within a
    // fixed observation block. This counter never changes an accumulation.
    let mut visited = 0_usize;
    for position in block * 1024..((block + 1) * 1024).min(n) {
        execution::check_chunk(poll, position)?;
        let row = n - 1 - position;
        for j in 0..p {
            execution::check_chunk(poll, visited)?;
            visited = visited.wrapping_add(1);
            influence[j] = execution::try_sum(
                (0..p)
                    .rev()
                    .map(|k| score(row, k).map(|s| bread[(j, k)] * s)),
                poll,
            )?;
        }
        for i in 0..p {
            for j in i..p {
                execution::check_chunk(poll, visited)?;
                visited = visited.wrapping_add(1);
                compensated_add(
                    influence[i] * influence[j],
                    &mut result[(i, j)],
                    &mut correction[(i, j)],
                );
            }
        }
    }
    finish(&mut result, &correction, poll)?;
    for i in 0..p {
        execution::check_chunk(poll, i)?;
        for j in 0..i {
            execution::check_chunk(poll, j)?;
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
    poll: Poll<'_>,
) -> Result<()> {
    execution::check(poll)?;
    if sum.nrows() != value.nrows()
        || sum.ncols() != value.ncols()
        || sum.nrows() != correction.nrows()
        || sum.ncols() != correction.ncols()
    {
        return Err(invalid("covariance accumulator dimensions disagree"));
    }
    for (position, ((a, c), b)) in sum
        .as_mut_slice()
        .iter_mut()
        .zip(correction.as_mut_slice())
        .zip(value.as_slice())
        .enumerate()
    {
        execution::check_chunk(poll, position)?;
        compensated_add(sign * b, a, c);
    }
    finite(sum, poll)?;
    finite(correction, poll)
}
pub(super) fn finish(
    sum: &mut DenseMatrix,
    correction: &DenseMatrix,
    poll: Poll<'_>,
) -> Result<()> {
    execution::check(poll)?;
    for (position, (a, c)) in sum
        .as_mut_slice()
        .iter_mut()
        .zip(correction.as_slice())
        .enumerate()
    {
        execution::check_chunk(poll, position)?;
        *a += c;
    }
    finite(sum, poll)
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
pub(super) fn finite(matrix: &DenseMatrix, poll: Poll<'_>) -> Result<()> {
    execution::check(poll)?;
    for (position, value) in matrix.as_slice().iter().enumerate() {
        execution::check_chunk(poll, position)?;
        if !value.is_finite() {
            return Err(numerical("covariance reduction is not representable"));
        }
    }
    Ok(())
}
pub(super) fn difference(a: &DenseMatrix, b: &DenseMatrix, poll: Poll<'_>) -> Result<f64> {
    execution::check(poll)?;
    let mut error = 0_f64;
    for i in 0..a.nrows() {
        execution::check(poll)?;
        let discrepancy =
            execution::sum((0..a.ncols()).map(|j| (a[(i, j)] - b[(i, j)]).abs()), poll)?;
        let scale = execution::sum(
            (0..a.ncols()).map(|j| a[(i, j)].abs().max(b[(i, j)].abs())),
            poll,
        )?;
        error = error.max(discrepancy / scale.max(f64::MIN_POSITIVE));
    }
    Ok(error)
}
