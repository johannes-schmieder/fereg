//! Harvested from fereg-testkit/src/explicit_ols.rs at legacy-v0.5.0-beta.
//! Independent dense oracle; no production numerical implementation is called.

use super::*;

use std::collections::BTreeMap;
pub(super) fn reported_intercept_hc1(
    problem: &ExplicitOlsProblem,
    kept_regressors: &[usize],
    residualized_regressors: &[Vec<f64>],
    residuals: &[f64],
    validated: &ValidatedProblem,
    finite_bread: &OracleMatrix,
    hc1_scale: f64,
) -> Result<OracleMatrix, OracleError> {
    let nobs = problem.outcome.len();
    let rank = kept_regressors.len();
    let dimension = rank + 1;
    let weight_sum = validated.objective_weights.iter().sum::<f64>();
    if !weight_sum.is_finite() || weight_sum <= 0.0 {
        return Err(OracleError::NumericalFailure(
            "reported-intercept objective-weight sum is invalid".to_owned(),
        ));
    }
    let means = kept_regressors
        .iter()
        .map(|&column| {
            problem.regressors[column]
                .iter()
                .zip(&validated.objective_weights)
                .map(|(value, weight)| value * weight)
                .sum::<f64>()
                / weight_sum
        })
        .collect::<Vec<_>>();
    let mut bread = OracleMatrix::zeros(dimension, dimension);
    for row in 0..rank {
        for column in 0..rank {
            bread.data[row * dimension + column] = finite_bread[(row, column)];
        }
    }
    for row in 0..rank {
        let cross = (0..rank)
            .map(|column| -finite_bread[(row, column)] * means[column])
            .sum::<f64>();
        bread.data[row * dimension + rank] = cross;
        bread.data[rank * dimension + row] = cross;
    }
    let mut corner = 1.0 / weight_sum;
    for row in 0..rank {
        for column in 0..rank {
            corner += means[row] * finite_bread[(row, column)] * means[column];
        }
    }
    bread.data[rank * dimension + rank] = corner;

    let mut scores = vec![vec![0.0_f64; nobs]; dimension];
    for column in 0..rank {
        for row in 0..nobs {
            scores[column][row] = residualized_regressors[column][row] + means[column];
        }
    }
    scores[rank].fill(1.0);
    let meat = observation_meat(&scores, residuals, &validated.hc0_multipliers);
    let covariance = scale_matrix(&sandwich(&bread, &meat), hc1_scale);
    if covariance.as_slice().iter().all(|value| value.is_finite()) {
        Ok(covariance)
    } else {
        Err(OracleError::NumericalFailure(
            "reported-intercept HC1 covariance is not finite".to_owned(),
        ))
    }
}

pub(super) fn reported_intercept_clustered(
    problem: &ExplicitOlsProblem,
    kept_regressors: &[usize],
    residualized_regressors: &[Vec<f64>],
    residuals: &[f64],
    validated: &ValidatedProblem,
    finite_bread: &OracleMatrix,
    model_rank: usize,
) -> Result<OracleMatrix, OracleError> {
    let nobs = problem.outcome.len();
    let rank = kept_regressors.len();
    let dimension = rank + 1;
    let weight_sum = validated.objective_weights.iter().sum::<f64>();
    if !weight_sum.is_finite() || weight_sum <= 0.0 {
        return Err(OracleError::NumericalFailure(
            "reported-intercept objective-weight sum is invalid".to_owned(),
        ));
    }
    let means = kept_regressors
        .iter()
        .map(|&column| {
            problem.regressors[column]
                .iter()
                .zip(&validated.objective_weights)
                .map(|(value, weight)| value * weight)
                .sum::<f64>()
                / weight_sum
        })
        .collect::<Vec<_>>();
    let mut bread = OracleMatrix::zeros(dimension, dimension);
    for row in 0..rank {
        for column in 0..rank {
            bread.data[row * dimension + column] = finite_bread[(row, column)];
        }
    }
    for row in 0..rank {
        let cross = (0..rank)
            .map(|column| -finite_bread[(row, column)] * means[column])
            .sum::<f64>();
        bread.data[row * dimension + rank] = cross;
        bread.data[rank * dimension + row] = cross;
    }
    let mut corner = 1.0 / weight_sum;
    for row in 0..rank {
        for column in 0..rank {
            corner += means[row] * finite_bread[(row, column)] * means[column];
        }
    }
    bread.data[rank * dimension + rank] = corner;

    let mut scores = vec![vec![0.0_f64; nobs]; dimension];
    for column in 0..rank {
        for row in 0..nobs {
            scores[column][row] = residualized_regressors[column][row] + means[column];
        }
    }
    scores[rank].fill(1.0);
    generic_clustered_covariance(
        &bread,
        &scores,
        residuals,
        &validated.objective_weights,
        &problem.clusters,
        validated.effective_observations,
        model_rank,
    )
}

pub(super) fn reorder_matrix(
    matrix: &OracleMatrix,
    old_order: &[usize],
    new_order: &[usize],
) -> OracleMatrix {
    let positions = new_order
        .iter()
        .map(|wanted| {
            old_order
                .iter()
                .position(|candidate| candidate == wanted)
                .expect("new matrix order is a permutation of the old order")
        })
        .collect::<Vec<_>>();
    let mut reordered = OracleMatrix::zeros(matrix.nrows, matrix.ncols);
    for row in 0..matrix.nrows {
        for column in 0..matrix.ncols {
            reordered.data[row * matrix.ncols + column] =
                matrix[(positions[row], positions[column])];
        }
    }
    reordered
}

pub(super) fn cross_product(columns: &[Vec<f64>], weights: &[f64]) -> OracleMatrix {
    let mut result = OracleMatrix::zeros(columns.len(), columns.len());
    for row in 0..columns.len() {
        for column in 0..=row {
            let value = columns[row]
                .iter()
                .zip(&columns[column])
                .zip(weights)
                .fold(0.0, |sum, ((left, right), weight)| {
                    (weight * left).mul_add(*right, sum)
                });
            result.data[row * columns.len() + column] = value;
            result.data[column * columns.len() + row] = value;
        }
    }
    result
}

pub(super) fn observation_meat(
    columns: &[Vec<f64>],
    residuals: &[f64],
    multipliers: &[f64],
) -> OracleMatrix {
    let mut meat = OracleMatrix::zeros(columns.len(), columns.len());
    for row in 0..columns.len() {
        for column in 0..=row {
            let value = residuals.iter().zip(multipliers).enumerate().fold(
                0.0,
                |sum, (observation, (residual, multiplier))| {
                    let product = columns[row][observation]
                        * columns[column][observation]
                        * residual
                        * residual;
                    multiplier.mul_add(product, sum)
                },
            );
            meat.data[row * columns.len() + column] = value;
            meat.data[column * columns.len() + row] = value;
        }
    }
    meat
}

pub(super) fn sandwich(bread: &OracleMatrix, meat: &OracleMatrix) -> OracleMatrix {
    let size = bread.nrows;
    let mut intermediate = OracleMatrix::zeros(size, size);
    for row in 0..size {
        for column in 0..size {
            intermediate.data[row * size + column] = (0..size).fold(0.0, |sum, inner| {
                bread[(row, inner)].mul_add(meat[(inner, column)], sum)
            });
        }
    }
    let mut result = OracleMatrix::zeros(size, size);
    for row in 0..size {
        for column in 0..size {
            result.data[row * size + column] = (0..size).fold(0.0, |sum, inner| {
                intermediate[(row, inner)].mul_add(bread[(inner, column)], sum)
            });
        }
    }
    symmetrize(&mut result);
    result
}

pub(super) fn scale_matrix(matrix: &OracleMatrix, scale: f64) -> OracleMatrix {
    OracleMatrix {
        nrows: matrix.nrows,
        ncols: matrix.ncols,
        data: matrix.data.iter().map(|value| value * scale).collect(),
    }
}

pub(super) fn symmetrize(matrix: &mut OracleMatrix) {
    for row in 0..matrix.nrows {
        for column in 0..row {
            let average = 0.5 * (matrix[(row, column)] + matrix[(column, row)]);
            matrix.data[row * matrix.ncols + column] = average;
            matrix.data[column * matrix.ncols + row] = average;
        }
    }
}

pub(super) fn clustered_component_uncorrected(
    bread: &OracleMatrix,
    columns: &[Vec<f64>],
    residuals: &[f64],
    objective_weights: &[f64],
    clusters: &[&[u64]],
) -> Result<(OracleMatrix, usize), OracleError> {
    if clusters.is_empty()
        || clusters
            .iter()
            .any(|cluster| cluster.len() != residuals.len())
    {
        return Err(OracleError::InvalidInput(
            "cluster subset is empty or not observation-aligned".to_owned(),
        ));
    }
    let mut group_scores: BTreeMap<Vec<u64>, Vec<f64>> = BTreeMap::new();
    for observation in 0..residuals.len() {
        let key = clusters
            .iter()
            .map(|cluster| cluster[observation])
            .collect::<Vec<_>>();
        let scores = group_scores
            .entry(key)
            .or_insert_with(|| vec![0.0; columns.len()]);
        for (column, score) in columns.iter().zip(scores) {
            *score = (objective_weights[observation] * column[observation])
                .mul_add(residuals[observation], *score);
        }
    }
    let group_count = group_scores.len();
    if group_count <= 1 {
        return Err(OracleError::InsufficientDegreesOfFreedom(
            "clustered VCE requires at least two clusters in every inclusion-exclusion component"
                .to_owned(),
        ));
    }
    let mut meat = OracleMatrix::zeros(columns.len(), columns.len());
    for scores in group_scores.values() {
        for row in 0..columns.len() {
            for column in 0..=row {
                let value = scores[row] * scores[column];
                meat.data[row * columns.len() + column] += value;
                if row != column {
                    meat.data[column * columns.len() + row] += value;
                }
            }
        }
    }
    Ok((sandwich(bread, &meat), group_count))
}

pub(super) fn generic_clustered_covariance(
    bread: &OracleMatrix,
    columns: &[Vec<f64>],
    residuals: &[f64],
    objective_weights: &[f64],
    clusters: &[Vec<u64>],
    effective_observations: usize,
    model_rank: usize,
) -> Result<OracleMatrix, OracleError> {
    if clusters.is_empty() || clusters.len() > 10 {
        return Err(OracleError::InvalidInput(
            "generic clustered covariance requires one to ten dimensions".to_owned(),
        ));
    }
    let counts = clusters
        .iter()
        .map(|cluster| {
            cluster
                .iter()
                .copied()
                .map(|value| (value, ()))
                .collect::<BTreeMap<_, _>>()
                .len()
        })
        .collect::<Vec<_>>();
    let minimum_groups = *counts.iter().min().ok_or_else(|| {
        OracleError::InvalidInput("cluster count table is unexpectedly empty".to_owned())
    })?;
    if minimum_groups < 2 {
        return Err(OracleError::InsufficientDegreesOfFreedom(
            "clustered VCE requires at least two groups per declared dimension".to_owned(),
        ));
    }
    let mut combined = OracleMatrix::zeros(columns.len(), columns.len());
    let subset_count = (1_usize << clusters.len()) - 1;
    for mask in 1..=subset_count {
        let selected = clusters
            .iter()
            .enumerate()
            .filter_map(|(dimension, cluster)| {
                (mask & (1_usize << dimension) != 0).then_some(cluster.as_slice())
            })
            .collect::<Vec<_>>();
        let (component, _) = clustered_component_uncorrected(
            bread,
            columns,
            residuals,
            objective_weights,
            &selected,
        )?;
        let sign = if mask.count_ones() % 2 == 1 {
            1.0
        } else {
            -1.0
        };
        for (target, value) in combined.data.iter_mut().zip(component.data) {
            *target += sign * value;
        }
    }
    let correction = cluster_correction(effective_observations, model_rank, minimum_groups)?;
    let mut covariance = scale_matrix(&combined, correction);
    symmetrize(&mut covariance);
    Ok(covariance)
}

pub(super) fn cluster_correction(
    effective_observations: usize,
    model_rank: usize,
    minimum_groups: usize,
) -> Result<f64, OracleError> {
    if minimum_groups <= 1 || effective_observations <= model_rank {
        return Err(OracleError::InsufficientDegreesOfFreedom(
            "clustered CR1 correction has no positive denominator".to_owned(),
        ));
    }
    Ok(
        minimum_groups as f64 / (minimum_groups - 1) as f64 * (effective_observations - 1) as f64
            / (effective_observations - model_rank) as f64,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn certify_result(
    problem: &ExplicitOlsProblem,
    dummy_columns: &[Vec<f64>],
    residualized_regressors: &[Vec<f64>],
    kept_regressors: &[usize],
    weights: &[f64],
    fitted_values: &[f64],
    residuals: &[f64],
    bread: &OracleMatrix,
) -> OracleCertificate {
    let outcome_norm = stable_norm(
        problem
            .outcome
            .iter()
            .zip(weights)
            .map(|(value, weight)| value * weight.sqrt()),
    );
    let residual_norm = stable_norm(
        residuals
            .iter()
            .zip(weights)
            .map(|(value, weight)| value * weight.sqrt()),
    );
    let response_scale = outcome_norm.max(residual_norm).max(f64::MIN_POSITIVE);
    let mut normalized_normal_equation_infinity = 0.0_f64;
    for column in dummy_columns.iter().chain(residualized_regressors) {
        let column_norm = stable_norm(
            column
                .iter()
                .zip(weights)
                .map(|(value, weight)| value * weight.sqrt()),
        );
        if column_norm == 0.0 {
            continue;
        }
        let score = column
            .iter()
            .zip(residuals)
            .zip(weights)
            .fold(0.0, |sum, ((value, residual), weight)| {
                (weight * value).mul_add(*residual, sum)
            });
        normalized_normal_equation_infinity =
            normalized_normal_equation_infinity.max(score.abs() / (column_norm * response_scale));
    }

    let kept_columns = kept_regressors
        .iter()
        .map(|&column| {
            let position = kept_regressors
                .iter()
                .position(|candidate| *candidate == column)
                .expect("kept regressor is present");
            residualized_regressors[position].clone()
        })
        .collect::<Vec<_>>();
    let cross = cross_product(&kept_columns, weights);
    let mut bread_inverse_infinity = 0.0_f64;
    for row in 0..bread.nrows {
        let row_error = (0..bread.ncols)
            .map(|column| {
                let value = (0..bread.ncols).fold(0.0, |sum, inner| {
                    bread[(row, inner)].mul_add(cross[(inner, column)], sum)
                });
                (value - f64::from(row == column)).abs()
            })
            .sum::<f64>();
        bread_inverse_infinity = bread_inverse_infinity.max(row_error);
    }
    let response_absolute_scale = problem
        .outcome
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
        .max(1.0);
    let reconstruction_infinity = problem
        .outcome
        .iter()
        .zip(fitted_values)
        .zip(residuals)
        .map(|((outcome, fitted), residual)| (outcome - fitted - residual).abs())
        .fold(0.0_f64, f64::max)
        / response_absolute_scale;
    OracleCertificate {
        normalized_normal_equation_infinity,
        bread_inverse_infinity,
        reconstruction_infinity,
    }
}

pub(super) fn validate_certificate(
    certificate: OracleCertificate,
    tolerance: f64,
) -> Result<(), OracleError> {
    let values = [
        certificate.normalized_normal_equation_infinity,
        certificate.bread_inverse_infinity,
        certificate.reconstruction_infinity,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(OracleError::NumericalFailure(
            "a final certificate is not finite".to_owned(),
        ));
    }
    if certificate.normalized_normal_equation_infinity > tolerance {
        return Err(OracleError::NumericalFailure(format!(
            "normalized normal-equation error {} exceeds {tolerance}",
            certificate.normalized_normal_equation_infinity
        )));
    }
    if certificate.bread_inverse_infinity > tolerance {
        return Err(OracleError::NumericalFailure(format!(
            "bread inverse error {} exceeds {tolerance}",
            certificate.bread_inverse_infinity
        )));
    }
    if certificate.reconstruction_infinity > tolerance {
        return Err(OracleError::NumericalFailure(format!(
            "reconstruction error {} exceeds {tolerance}",
            certificate.reconstruction_infinity
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_finite_outputs(
    coefficients: &[Option<f64>],
    weighted_rss: f64,
    finite_fitted_values: &[f64],
    absorbed_fitted_values: &[f64],
    fitted_values: &[f64],
    residuals: &[f64],
    bread: &OracleMatrix,
    iid_vce: &OracleMatrix,
    hc1_vce: &OracleMatrix,
    one_way_cluster_vces: &[OracleMatrix],
    clustered_vce: Option<&OracleMatrix>,
    reported_clustered_vce: Option<&OracleMatrix>,
) -> Result<(), OracleError> {
    let finite = coefficients.iter().flatten().all(|value| value.is_finite())
        && weighted_rss.is_finite()
        && finite_fitted_values.iter().all(|value| value.is_finite())
        && absorbed_fitted_values.iter().all(|value| value.is_finite())
        && fitted_values.iter().all(|value| value.is_finite())
        && residuals.iter().all(|value| value.is_finite())
        && bread.as_slice().iter().all(|value| value.is_finite())
        && iid_vce.as_slice().iter().all(|value| value.is_finite())
        && hc1_vce.as_slice().iter().all(|value| value.is_finite())
        && one_way_cluster_vces
            .iter()
            .flat_map(OracleMatrix::as_slice)
            .all(|value| value.is_finite())
        && clustered_vce
            .into_iter()
            .flat_map(OracleMatrix::as_slice)
            .all(|value| value.is_finite())
        && reported_clustered_vce
            .into_iter()
            .flat_map(OracleMatrix::as_slice)
            .all(|value| value.is_finite());
    if !finite {
        return Err(OracleError::NumericalFailure(
            "a coefficient, RSS, fitted component, residual, bread, or VCE is not finite"
                .to_owned(),
        ));
    }
    Ok(())
}
