//! Independent score Gram recheck, with short leaves and compensated merges.
use super::*;

const LEAF_ROWS: usize = 128;

#[derive(Clone, Copy)]
enum DenseScoreRows<'a> {
    Packed(&'a DenseMatrix),
    Selected(&'a PreparedOlsSelectedScoreStorage<'a>),
}

pub(super) fn build<R: PreparedInferenceReservation>(
    storage: &PreparedOlsScoreStorage<'_>,
    nrows: usize,
    rank: usize,
    weights: ReferenceWeightView<'_>,
    reservation: &mut R,
) -> Result<Option<Vec<f64>>> {
    let storage = match storage {
        PreparedOlsScoreStorage::Dense(within) => DenseScoreRows::Packed(within),
        PreparedOlsScoreStorage::Selected(strided) => DenseScoreRows::Selected(strided),
        PreparedOlsScoreStorage::Scratch(_) => return Ok(None),
    };
    let square = rank
        .checked_mul(rank)
        .ok_or_else(|| allocation("accurate score Gram dimensions overflow"))?;
    if rank == 0 {
        return Ok(Some(Vec::new()));
    }
    let per_partition = square
        .checked_mul(3)
        .ok_or_else(|| allocation("accurate score Gram buffer length overflows"))?;
    let max_partitions =
        SCORE_CERTIFICATE_PARTIAL_BUDGET_BYTES / std::mem::size_of::<f64>() / per_partition;
    if max_partitions == 0 {
        return Ok(None);
    }
    let blocks = nrows
        .div_ceil(SCORE_CERTIFICATE_REDUCTION_ROWS)
        .clamp(1, SCORE_CERTIFICATE_MAX_BLOCKS);
    // The frame already provides two square reducer banks per ordinary block.
    // Fit the three recheck banks in that reserve by using fewer partitions.
    // For one partition, the extra square comes from the sequential covariance
    // workspace, which is not live during score validation.
    let partitions = (2 * blocks / 3).max(1).min(max_partitions);
    let mut partials = reserved_filled(
        reservation,
        partitions * per_partition,
        0.,
        "unable to reserve accurate score Gram partials",
    )?;
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(per_partition).enumerate(),
        nrows as u128 * square as u128,
        |(partition, partial)| {
            let count = nrows / partitions;
            let remainder = nrows % partitions;
            let start = partition * count + partition.min(remainder);
            let end = start + count + usize::from(partition < remainder);
            let (sums, rest) = partial.split_at_mut(square);
            let (correction, leaf) = rest.split_at_mut(square);
            for (block, first) in (start..end).step_by(LEAF_ROWS).enumerate() {
                crate::interrupt::row_checkpoint(block * LEAF_ROWS)?;
                let last = (first + LEAF_ROWS).min(end);
                leaf.fill(0.);
                fill_leaf(storage, first..last, rank, weights, leaf)?;
                merge_upper(rank, leaf, sums, correction)?;
            }
            for (sum, correction) in sums.iter_mut().zip(correction) {
                *sum += *correction;
            }
            Ok(())
        },
    )?;
    let mut gram = reserved_filled(
        reservation,
        square,
        0.,
        "unable to reserve accurate score Gram",
    )?;
    let mut correction = reserved_filled(
        reservation,
        square,
        0.,
        "unable to reserve accurate score Gram correction",
    )?;
    for partial in partials.chunks_exact(per_partition) {
        merge_upper(rank, &partial[..square], &mut gram, &mut correction)?;
    }
    for row in 0..rank {
        for col in row..rank {
            let i = row * rank + col;
            gram[i] += correction[i];
            if !gram[i].is_finite() {
                return Err(invalid_data("accurate score Gram is not finite"));
            }
            gram[col * rank + row] = gram[i];
        }
    }
    Ok(Some(gram))
}

fn merge_upper(rank: usize, from: &[f64], to: &mut [f64], correction: &mut [f64]) -> Result<()> {
    for row in 0..rank {
        for i in row * rank + row..(row + 1) * rank {
            compensated_add(from[i], &mut to[i], &mut correction[i])?;
        }
    }
    Ok(())
}

fn fill_leaf(
    storage: DenseScoreRows<'_>,
    rows: std::ops::Range<usize>,
    rank: usize,
    weights: ReferenceWeightView<'_>,
    leaf: &mut [f64],
) -> Result<()> {
    match storage {
        DenseScoreRows::Selected(strided) => accumulate_selected_score_gram_rows(
            &strided.within.as_slice()
                [rows.start * strided.within.ncols()..rows.end * strided.within.ncols()],
            strided.within.ncols(),
            &strided.generated_columns,
            strided.weights,
            rows.start,
            leaf,
        ),
        DenseScoreRows::Packed(within) => {
            for i in rows {
                let row = &within.as_slice()[i * rank..(i + 1) * rank];
                let w = weights.value(i)?;
                for (left, &value) in row.iter().enumerate() {
                    let weighted = w * value;
                    for (entry, &right) in leaf[left * rank + left..(left + 1) * rank]
                        .iter_mut()
                        .zip(&row[left..])
                    {
                        *entry += weighted * right;
                    }
                }
            }
            Ok(())
        }
    }
}
