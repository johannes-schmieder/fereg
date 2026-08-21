#![allow(clippy::needless_range_loop)]

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::factorize::interaction_codes;
use crate::ols::{ClusterSubsetReport, VceKind, VceReport};

pub(crate) struct PpmlVceInput<'a> {
    pub regressors: &'a DenseMatrix,
    pub score_residuals: &'a [f64],
    pub prior_weights: &'a [f64],
    pub bread: &'a [f64],
    pub kept: &'a [usize],
    pub residual_dof: usize,
    pub clusters: &'a [Vec<u32>],
    pub kind: VceKind,
    pub small_sample: bool,
}

pub(crate) fn compute_ppml_vce(input: PpmlVceInput<'_>) -> Result<VceReport> {
    validate(&input)?;
    let full = input.regressors.ncols();
    let rank = input.kept.len();
    let identified_bread = select_square(input.bread, full, input.kept);
    let (mut identified, cluster_subsets) = match input.kind {
        VceKind::Unadjusted => (identified_bread, Vec::new()),
        VceKind::Robust => {
            let mut meat = score_meat(
                input.regressors,
                input.score_residuals,
                input.prior_weights,
                input.kept,
            );
            if input.small_sample {
                let correction = input.regressors.nrows() as f64 / input.residual_dof as f64;
                meat.iter_mut().for_each(|value| *value *= correction);
            }
            (sandwich(&identified_bread, &meat, rank), Vec::new())
        }
        VceKind::Clustered => clustered_covariance(&input, &identified_bread)?,
    };
    symmetrize(&mut identified, rank);

    let mut covariance = vec![0.0_f64; full * full];
    for (identified_row, &row) in input.kept.iter().enumerate() {
        for (identified_column, &column) in input.kept.iter().enumerate() {
            covariance[row * full + column] = identified[identified_row * rank + identified_column];
        }
    }
    let kept = input.kept.iter().copied().collect::<std::collections::BTreeSet<_>>();
    let standard_errors = (0..full)
        .map(|column| {
            if !kept.contains(&column) {
                return f64::NAN;
            }
            let variance = covariance[column * full + column];
            if variance.is_finite() && variance >= 0.0 { variance.sqrt() } else { f64::NAN }
        })
        .collect();
    Ok(VceReport { kind: input.kind, covariance, standard_errors, cluster_subsets })
}

fn validate(input: &PpmlVceInput<'_>) -> Result<()> {
    let nrows = input.regressors.nrows();
    let ncols = input.regressors.ncols();
    if input.score_residuals.len() != nrows
        || input.prior_weights.len() != nrows
        || input.bread.len() != ncols.saturating_mul(ncols)
        || input.residual_dof == 0
        || input.kept.iter().any(|column| *column >= ncols)
        || input.clusters.iter().any(|cluster| cluster.len() != nrows)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "PPML VCE inputs have inconsistent dimensions",
        ));
    }
    if input.kind == VceKind::Clustered && input.clusters.is_empty() {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "clustered PPML VCE requires at least one cluster dimension",
        ));
    }
    if input.clusters.len() > 4 {
        return Err(StFixestError::new(
            ErrorCode::Unsupported,
            "PPML VCE supports at most four cluster dimensions",
        ));
    }
    if input.score_residuals.iter().any(|value| !value.is_finite())
        || input.prior_weights.iter().any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "PPML VCE residuals and prior weights must be finite and weights positive",
        ));
    }
    Ok(())
}

fn score_meat(
    regressors: &DenseMatrix,
    score_residuals: &[f64],
    prior_weights: &[f64],
    kept: &[usize],
) -> Vec<f64> {
    let rank = kept.len();
    let mut meat = vec![0.0_f64; rank * rank];
    let mut score = vec![0.0_f64; rank];
    for row in 0..regressors.nrows() {
        let weighted_residual = prior_weights[row] * score_residuals[row];
        for (position, &column) in kept.iter().enumerate() {
            score[position] = regressors[(row, column)] * weighted_residual;
        }
        add_outer(&mut meat, &score, 1.0);
    }
    meat
}

fn clustered_covariance(
    input: &PpmlVceInput<'_>,
    bread: &[f64],
) -> Result<(Vec<f64>, Vec<ClusterSubsetReport>)> {
    let rank = input.kept.len();
    let dimensions = input.clusters.len();
    let mut combined = vec![0.0_f64; rank * rank];
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
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                format!("PPML cluster subset {subset:?} contains fewer than two groups"),
            ));
        }
        let mut group_scores = vec![0.0_f64; groups * rank];
        for row in 0..input.regressors.nrows() {
            let group = encoded.codes[row] as usize;
            let weighted_residual = input.prior_weights[row] * input.score_residuals[row];
            for (position, &column) in input.kept.iter().enumerate() {
                group_scores[group * rank + position] +=
                    input.regressors[(row, column)] * weighted_residual;
            }
        }
        let mut meat = vec![0.0_f64; rank * rank];
        for group in 0..groups {
            add_outer(&mut meat, &group_scores[group * rank..(group + 1) * rank], 1.0);
        }
        let sign = if subset.len() % 2 == 1 { 1_i8 } else { -1_i8 };
        let correction = if input.small_sample {
            groups as f64 / (groups - 1) as f64
                * (input.regressors.nrows().saturating_sub(1) as f64 / input.residual_dof as f64)
        } else {
            1.0
        };
        for (target, value) in combined.iter_mut().zip(meat) {
            *target += f64::from(sign) * correction * value;
        }
        reports.push(ClusterSubsetReport { dimensions: subset, groups, sign });
    }
    Ok((sandwich(bread, &combined, rank), reports))
}

fn add_outer(target: &mut [f64], vector: &[f64], scale: f64) {
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
    for (out_row, &row) in indices.iter().enumerate() {
        for (out_column, &column) in indices.iter().enumerate() {
            selected[out_row * indices.len() + out_column] = matrix[row * dimension + column];
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
                value = bread[row * dimension + inner]
                    .mul_add(meat[inner * dimension + column], value);
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

fn symmetrize(matrix: &mut [f64], dimension: usize) {
    for row in 0..dimension {
        for column in 0..row {
            let value = 0.5 * (matrix[row * dimension + column] + matrix[column * dimension + row]);
            matrix[row * dimension + column] = value;
            matrix[column * dimension + row] = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_based_covariance_equals_bread() {
        let x = DenseMatrix::from_columns(&[vec![1.0; 4], vec![0.0, 1.0, 2.0, 3.0]])
            .expect("matrix");
        let bread = vec![0.7, -0.2, -0.2, 0.1];
        let report = compute_ppml_vce(PpmlVceInput {
            regressors: &x,
            score_residuals: &[0.0; 4],
            prior_weights: &[1.0; 4],
            bread: &bread,
            kept: &[0, 1],
            residual_dof: 2,
            clusters: &[],
            kind: VceKind::Unadjusted,
            small_sample: false,
        })
        .expect("VCE");
        assert_eq!(report.covariance, bread);
    }

    #[test]
    fn two_way_clusters_use_inclusion_exclusion() {
        let x = DenseMatrix::from_columns(&[
            vec![1.0; 8],
            vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0],
        ])
        .expect("matrix");
        let report = compute_ppml_vce(PpmlVceInput {
            regressors: &x,
            score_residuals: &[-1.0, 1.0, -0.5, 0.5, 0.75, -0.75, 1.25, -1.25],
            prior_weights: &[1.0; 8],
            bread: &[0.2, -0.05, -0.05, 0.03],
            kept: &[0, 1],
            residual_dof: 6,
            clusters: &[vec![0, 0, 1, 1, 2, 2, 3, 3], vec![0, 1, 0, 1, 0, 1, 0, 1]],
            kind: VceKind::Clustered,
            small_sample: false,
        })
        .expect("VCE");
        assert_eq!(report.cluster_subsets.len(), 3);
        assert!(report.covariance.iter().all(|value| value.is_finite()));
    }
}
