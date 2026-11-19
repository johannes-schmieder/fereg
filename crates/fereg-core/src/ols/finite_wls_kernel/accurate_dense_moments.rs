//! Recompute retained finite moments after an inference certificate rejects the
//! ordinary fit. Each Gram entry and RHS has a 32-row FMA leaf and a
//! compensated sum of its leaves. Column partitions own disjoint entries,
//! so partitioning across workers does not change any reduction order.
use super::{
    checked_square, moment_lane_work, normalized_weight, scaled_column, FiniteDenseRows,
    FixedBlockMomentSource, ValidatedMomentSource,
};
use crate::error::{allocation, invalid_argument, invalid_data, Result};
use crate::linalg::neumaier_add;
use crate::ols::prepared_scratch::{PreparedOlsScratch, PreparedOlsScratchReader};
use rayon::prelude::*;

const LEAF_ROWS: usize = 32;

#[derive(Clone, Copy)]
enum RetainedRows<'a> {
    Dense(FiniteDenseRows<'a>),
    Scratch(&'a PreparedOlsScratch),
}

enum PartitionRows<'a, 'reader> {
    Dense(FiniteDenseRows<'a>),
    Scratch(&'reader mut PreparedOlsScratchReader<'a>),
}

impl PartitionRows<'_, '_> {
    fn standardized_row(
        &mut self,
        observation: usize,
        row: &mut [f64],
        first: usize,
        k: usize,
        selected: Option<&[usize]>,
        scales: Option<&[f64]>,
    ) -> Result<()> {
        match self {
            Self::Dense(values) => {
                let source = values.row(observation);
                for column in first..k {
                    let index = selected.map_or(column, |selected| selected[column]);
                    row[column] = scaled_column(source[index], scales, column)?;
                }
            }
            Self::Scratch(reader) => {
                reader.read_linear(observation * row.len(), row)?;
                // Selected indices increase, so forward selection leaves every
                // later source in this private decoded row intact.
                for column in first..k {
                    let index = selected.map_or(column, |selected| selected[column]);
                    row[column] = scaled_column(row[index], scales, column)?;
                }
            }
        }
        Ok(())
    }
}

fn row_width(moments: ValidatedMomentSource<'_>) -> Result<usize> {
    match moments.source {
        FixedBlockMomentSource::Dense { .. } => Ok(moments.source.dimension()),
        FixedBlockMomentSource::Scratch { values, .. } => Ok(values.columns()),
        _ => Err(invalid_argument("accurate moments require retained rows")),
    }
}

/// Per partition one row and a leaf and correction for every output entry.
/// Scratch decodes a complete stored row. Workspace is independent of N.
pub(in crate::ols) fn workspace_elements(moments: ValidatedMomentSource<'_>) -> Result<usize> {
    let k = moments.source.dimension();
    let rows = row_width(moments)?
        .checked_mul(moments.layout.partitions)
        .ok_or_else(|| allocation("accurate moment row workspace overflows"))?;
    k.checked_add(1)
        .and_then(|width| width.checked_mul(2))
        .and_then(|width| width.checked_mul(k))
        .and_then(|entries| entries.checked_add(rows))
        .ok_or_else(|| allocation("accurate moment workspace overflows"))
}

pub(in crate::ols) fn fill(
    moments: ValidatedMomentSource<'_>,
    scratch: &mut [f64],
    rhs: &mut [f64],
    gram: &mut [f64],
) -> Result<()> {
    let values = match moments.source {
        FixedBlockMomentSource::Dense { values, .. } => RetainedRows::Dense(values),
        FixedBlockMomentSource::Scratch { values, .. } => RetainedRows::Scratch(values),
        _ => return Err(invalid_argument("accurate moments require retained rows")),
    };
    let (FixedBlockMomentSource::Dense {
        selected_columns,
        response,
        weights,
        solver_weight_divisor: divisor,
        column_scales,
        ..
    }
    | FixedBlockMomentSource::Scratch {
        selected_columns,
        response,
        weights,
        solver_weight_divisor: divisor,
        column_scales,
        ..
    }) = moments.source
    else {
        return Err(invalid_argument("accurate moments require retained rows"));
    };
    let rows = response.len();
    let k = moments.source.dimension();
    let elements = workspace_elements(moments)?;
    if rhs.len() != k
        || gram.len() != checked_square(k, "accurate Gram overflows")?
        || scratch.len() < elements
    {
        return Err(invalid_argument("accurate moment bank dimensions disagree"));
    }
    let width = moments.layout.columns_per_partition;
    let row_width = row_width(moments)?;
    let stride = 2 * (k + 1) * width + row_width;
    let hardware = crate::kernels::installed_upper_rank_one();
    rhs.fill(0.);
    gram.fill(0.);
    crate::parallel::stage_try_for_each(
        rhs.par_chunks_mut(width)
            .zip(gram.par_chunks_mut(k * width))
            .zip(scratch[..elements].par_chunks_mut(stride))
            .enumerate(),
        moment_lane_work(rows, k),
        |(partition, ((rhs, gram), scratch))| -> Result<()> {
            let first = partition * width;
            let mut page;
            let mut reader = match values {
                RetainedRows::Dense(values) => PartitionRows::Dense(values),
                RetainedRows::Scratch(values) => {
                    page = values.shared_reader();
                    PartitionRows::Scratch(&mut page)
                }
            };
            let (row, scratch) = scratch.split_at_mut(row_width);
            let (leaf_rhs, scratch) = scratch.split_at_mut(rhs.len());
            let (leaf_gram, scratch) = scratch.split_at_mut(gram.len());
            let (rhs_correction, gram_correction) = scratch.split_at_mut(rhs.len());
            rhs_correction.fill(0.);
            gram_correction.fill(0.);
            for start in (0..rows).step_by(LEAF_ROWS) {
                crate::interrupt::row_checkpoint(start)?;
                leaf_rhs.fill(0.);
                leaf_gram.fill(0.);
                for observation in start..start.saturating_add(LEAF_ROWS).min(rows) {
                    let weight = normalized_weight(weights, divisor, observation)?;
                    reader.standardized_row(
                        observation,
                        row,
                        first,
                        k,
                        selected_columns,
                        column_scales,
                    )?;
                    crate::kernels::upper_rank_one(
                        hardware,
                        weight,
                        &row[..k],
                        response[observation],
                        first,
                        leaf_rhs,
                        leaf_gram,
                    );
                }
                merge_leaf(leaf_rhs, rhs, rhs_correction);
                merge_leaf(leaf_gram, gram, gram_correction);
            }
            for (sum, &correction) in rhs
                .iter_mut()
                .chain(gram)
                .zip(rhs_correction.iter().chain(gram_correction.iter()))
            {
                *sum += correction;
                if !sum.is_finite() {
                    return Err(invalid_data("accurate moments are not finite"));
                }
            }
            Ok(())
        },
    )?;
    for left in 0..k {
        for right in left + 1..k {
            gram[right * k + left] = gram[left * k + right];
        }
    }
    Ok(())
}

fn merge_leaf(leaf: &[f64], sums: &mut [f64], corrections: &mut [f64]) {
    for ((&part, sum), correction) in leaf.iter().zip(sums).zip(corrections) {
        neumaier_add(part, sum, correction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::BorrowedMapWeights;
    use crate::ols::finite_wls_kernel::{validate_moment_source, FiniteDenseRows};

    #[test]
    fn weighted_selected_gram_and_rhs_recover_small_leaves_for_narrow_and_wide_fits() {
        // First observation contributes 2^54. Its leaf's remaining observations
        // are zero; all subsequent ones contribute ±1 or ±2. A single full-row
        // sum loses each of these additions. Every final answer is exact in f64.
        let rows = LEAF_ROWS + 2048;
        for k in [2, 67] {
            let stored = 2 * k;
            let selected: Vec<_> = (0..k).map(|j| 2 * j).collect();
            let scales: Vec<_> = (0..k).map(|j| if j % 3 == 0 { 0.5 } else { 2. }).collect();
            let sign = |j: usize| if j % 2 == 0 { 1. } else { -1. };
            let mut values = vec![123.45; stored * rows];
            let mut response = vec![0.; rows];
            let mut weights = vec![2.; rows];
            for (i, row) in values.chunks_exact_mut(stored).enumerate() {
                let x = if i == 0 {
                    2_f64.powi(27)
                } else if i < LEAF_ROWS {
                    0.
                } else {
                    1.
                };
                weights[i] = if i >= LEAF_ROWS && i % 2 == 0 { 4. } else { 2. };
                response[i] = -x;
                for j in 0..k {
                    row[selected[j]] = x * scales[j] * sign(j);
                }
            }
            let exact = 2_f64.powi(54) + 3072.;
            let source = FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::packed(&values, stored),
                rows,
                stored_columns: stored,
                selected_columns: Some(&selected),
                response: (&response).into(),
                weights: BorrowedMapWeights::Provided(&weights),
                solver_weight_divisor: 2.,
                column_scales: Some(&scales),
            };
            let mut file = PreparedOlsScratch::create_row_major(rows, stored).unwrap();
            file.write_linear(0, &values).unwrap();
            let scratch_source = FixedBlockMomentSource::Scratch {
                values: &file,
                selected_columns: Some(&selected),
                response: (&response).into(),
                weights: BorrowedMapWeights::Provided(&weights),
                solver_weight_divisor: 2.,
                column_scales: Some(&scales),
            };
            for source in [source, scratch_source] {
                for workers in [1, 3] {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(workers)
                        .build()
                        .unwrap()
                        .install(|| {
                            let validated = validate_moment_source(source).unwrap();
                            let mut scratch =
                                vec![f64::NAN; workspace_elements(validated).unwrap()];
                            let mut rhs = vec![f64::NAN; k];
                            let mut gram = vec![f64::NAN; k * k];
                            fill(validated, &mut scratch, &mut rhs, &mut gram).unwrap();
                            for i in 0..k {
                                assert_eq!(rhs[i], -sign(i) * exact);
                                for j in 0..k {
                                    assert_eq!(gram[i * k + j], sign(i) * sign(j) * exact);
                                }
                            }
                        });
                }
            }
        }
    }
}
