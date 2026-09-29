#![allow(clippy::needless_range_loop)]

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::factorize::interaction_codes;

/// Variance estimator requested for an OLS fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VceKind {
    Unadjusted,
    Robust,
    Clustered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSubsetReport {
    /// Zero-based cluster dimensions in this inclusion-exclusion subset.
    pub dimensions: Vec<usize>,
    pub groups: usize,
    pub sign: i8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VceReport {
    pub kind: VceKind,
    pub covariance: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub cluster_subsets: Vec<ClusterSubsetReport>,
}

pub(crate) struct VceInput<'a> {
    pub regressors: &'a DenseMatrix,
    pub residuals: &'a [f64],
    pub weights: &'a [f64],
    pub bread: &'a [f64],
    pub kept: &'a [usize],
    pub residual_dof: usize,
    pub clusters: &'a [Vec<u32>],
    pub kind: VceKind,
    pub small_sample: bool,
}

pub(crate) fn compute_vce(input: VceInput<'_>) -> Result<VceReport> {
    validate(&input)?;
    let full_dimension = input.regressors.ncols();
    let rank = input.kept.len();
    let bread = select_square(input.bread, full_dimension, input.kept);
    let (identified_covariance, cluster_subsets) = match input.kind {
        VceKind::Unadjusted => {
            let rss = weighted_rss(input.residuals, input.weights);
            let scale = rss / input.residual_dof as f64;
            (
                bread.iter().map(|value| scale * value).collect(),
                Vec::new(),
            )
        }
        VceKind::Robust => {
            let mut meat = score_meat(input.regressors, input.residuals, input.weights, input.kept);
            if input.small_sample {
                let correction = input.regressors.nrows() as f64 / input.residual_dof as f64;
                for value in &mut meat {
                    *value *= correction;
                }
            }
            (sandwich(&bread, &meat, rank), Vec::new())
        }
        VceKind::Clustered => clustered_covariance(&input, &bread)?,
    };

    let mut covariance = vec![0.0_f64; full_dimension * full_dimension];
    for (identified_row, &row) in input.kept.iter().enumerate() {
        for (identified_column, &column) in input.kept.iter().enumerate() {
            covariance[row * full_dimension + column] =
                identified_covariance[identified_row * rank + identified_column];
        }
    }
    let kept_lookup = input
        .kept
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let standard_errors = (0..full_dimension)
        .map(|index| {
            if kept_lookup.contains(&index) {
                let variance = covariance[index * full_dimension + index];
                if variance >= 0.0 {
                    variance.sqrt()
                } else {
                    f64::NAN
                }
            } else {
                f64::NAN
            }
        })
        .collect();
    Ok(VceReport {
        kind: input.kind,
        covariance,
        standard_errors,
        cluster_subsets,
    })
}

fn validate(input: &VceInput<'_>) -> Result<()> {
    let nrows = input.regressors.nrows();
    let ncols = input.regressors.ncols();
    if input.residuals.len() != nrows
        || input.weights.len() != nrows
        || input.bread.len() != ncols.saturating_mul(ncols)
        || input.residual_dof == 0
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "VCE inputs have inconsistent dimensions",
        ));
    }
    if input.kept.iter().any(|index| *index >= ncols) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "VCE kept-column index is out of bounds",
        ));
    }
    if input.clusters.iter().any(|column| column.len() != nrows) {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "cluster columns have inconsistent observation counts",
        ));
    }
    if input.kind == VceKind::Clustered {
        if input.clusters.is_empty() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "clustered VCE requires at least one cluster dimension",
            ));
        }
        if input.clusters.len() > 4 {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                "clustered VCE currently supports at most four dimensions",
            ));
        }
    }
    Ok(())
}

fn weighted_rss(residuals: &[f64], weights: &[f64]) -> f64 {
    residuals
        .iter()
        .zip(weights)
        .fold(0.0_f64, |sum, (residual, weight)| {
            (*weight * *residual).mul_add(*residual, sum)
        })
}

fn score_meat(
    regressors: &DenseMatrix,
    residuals: &[f64],
    weights: &[f64],
    kept: &[usize],
) -> Vec<f64> {
    let rank = kept.len();
    let mut meat = vec![0.0_f64; rank * rank];
    let mut score = vec![0.0_f64; rank];
    for row in 0..regressors.nrows() {
        for (position, &column) in kept.iter().enumerate() {
            score[position] = weights[row] * regressors[(row, column)] * residuals[row];
        }
        add_outer_product(&mut meat, &score, 1.0);
    }
    meat
}

fn clustered_covariance(
    input: &VceInput<'_>,
    bread: &[f64],
) -> Result<(Vec<f64>, Vec<ClusterSubsetReport>)> {
    let rank = input.kept.len();
    let dimensions = input.clusters.len();
    let mut combined_meat = vec![0.0_f64; rank * rank];
    let mut reports = Vec::with_capacity((1_usize << dimensions) - 1);
    for mask in 1_usize..(1_usize << dimensions) {
        let subset = (0..dimensions)
            .filter(|dimension| mask & (1_usize << dimension) != 0)
            .collect::<Vec<_>>();
        let sources = subset
            .iter()
            .map(|dimension| input.clusters[*dimension].as_slice())
            .collect::<Vec<_>>();
        let encoded = interaction_codes(&sources)?;
        let groups = encoded.n_levels();
        if groups <= 1 {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("cluster subset {subset:?} contains fewer than two groups"),
            ));
        }
        let mut group_scores = vec![0.0_f64; groups * rank];
        for row in 0..input.regressors.nrows() {
            let group = encoded.codes[row] as usize;
            for (position, &column) in input.kept.iter().enumerate() {
                group_scores[group * rank + position] +=
                    input.weights[row] * input.regressors[(row, column)] * input.residuals[row];
            }
        }
        let mut meat = vec![0.0_f64; rank * rank];
        for group in 0..groups {
            let score = &group_scores[group * rank..(group + 1) * rank];
            add_outer_product(&mut meat, score, 1.0);
        }
        let sign = if subset.len() % 2 == 1 { 1_i8 } else { -1_i8 };
        let correction = if input.small_sample {
            groups as f64 / (groups - 1) as f64
                * (input.regressors.nrows().saturating_sub(1) as f64 / input.residual_dof as f64)
        } else {
            1.0
        };
        for (combined, value) in combined_meat.iter_mut().zip(meat) {
            *combined += f64::from(sign) * correction * value;
        }
        reports.push(ClusterSubsetReport {
            dimensions: subset,
            groups,
            sign,
        });
    }
    Ok((sandwich(bread, &combined_meat, rank), reports))
}

fn add_outer_product(target: &mut [f64], vector: &[f64], scale: f64) {
    let dimension = vector.len();
    for row in 0..dimension {
        for column in 0..dimension {
            target[row * dimension + column] =
                (scale * vector[row]).mul_add(vector[column], target[row * dimension + column]);
        }
    }
}

fn select_square(matrix: &[f64], dimension: usize, indices: &[usize]) -> Vec<f64> {
    let mut selected = vec![0.0_f64; indices.len() * indices.len()];
    for (row_position, &row) in indices.iter().enumerate() {
        for (column_position, &column) in indices.iter().enumerate() {
            selected[row_position * indices.len() + column_position] =
                matrix[row * dimension + column];
        }
    }
    selected
}

fn sandwich(bread: &[f64], meat: &[f64], dimension: usize) -> Vec<f64> {
    let mut intermediate = vec![0.0_f64; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            let mut value = 0.0_f64;
            for inner in 0..dimension {
                value =
                    bread[row * dimension + inner].mul_add(meat[inner * dimension + column], value);
            }
            intermediate[row * dimension + column] = value;
        }
    }
    let mut covariance = vec![0.0_f64; dimension * dimension];
    for row in 0..dimension {
        for column in 0..dimension {
            let mut value = 0.0_f64;
            for inner in 0..dimension {
                value = intermediate[row * dimension + inner]
                    .mul_add(bread[column * dimension + inner], value);
            }
            covariance[row * dimension + column] = value;
        }
    }
    covariance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_way_cluster_uses_inclusion_exclusion() {
        let regressors = DenseMatrix::from_columns(&[
            vec![1.0; 8],
            vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0],
        ])
        .expect("matrix");
        let residuals = vec![-1.0, 1.0, -0.5, 0.5, 0.75, -0.75, 1.25, -1.25];
        let bread = vec![0.2, -0.05, -0.05, 0.03];
        let first = vec![0, 0, 1, 1, 2, 2, 3, 3];
        let second = vec![0, 1, 0, 1, 0, 1, 0, 1];
        let report = compute_vce(VceInput {
            regressors: &regressors,
            residuals: &residuals,
            weights: &[1.0; 8],
            bread: &bread,
            kept: &[0, 1],
            residual_dof: 6,
            clusters: &[first, second],
            kind: VceKind::Clustered,
            small_sample: false,
        })
        .expect("VCE");
        assert_eq!(report.cluster_subsets.len(), 3);
        assert_eq!(report.cluster_subsets[0].sign, 1);
        assert_eq!(report.cluster_subsets[1].sign, 1);
        assert_eq!(report.cluster_subsets[2].sign, -1);
        assert!(report.covariance.iter().all(|value| value.is_finite()));
    }
}
