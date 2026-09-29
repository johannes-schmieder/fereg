//! Harvested from fereg-testkit/src/explicit_ols.rs at legacy-v0.5.0-beta.
//! Independent dense oracle; no production numerical implementation is called.

use super::*;

struct Projection {
    validated: ValidatedProblem,
    nobs: usize,
    dummy_columns: Vec<Vec<f64>>,
    explicit_dummy_columns: usize,
    rank_tolerance: f64,
    dummy_qr: PivotedQr,
    residualized_weighted_outcome: Vec<f64>,
    regressor_reference_scales: Vec<f64>,
    residualized_weighted_regressors: Vec<Vec<f64>>,
}

struct LinearFit {
    validated: ValidatedProblem,
    dummy_columns: Vec<Vec<f64>>,
    explicit_dummy_columns: usize,
    explicit_dummy_rank: usize,
    coefficients: Vec<Option<f64>>,
    kept_regressors: Vec<usize>,
    dropped_regressors: Vec<usize>,
    finite_fitted_values: Vec<f64>,
    absorbed_fitted_values: Vec<f64>,
    fitted_values: Vec<f64>,
    residuals: Vec<f64>,
    finite_regressor_rank: usize,
    model_rank: usize,
    residual_degrees_of_freedom: usize,
    normalized_weighted_rss: f64,
    weighted_rss: f64,
    normalized_bread: OracleMatrix,
    bread: OracleMatrix,
    residualized_regressors: Vec<Vec<f64>>,
}

struct Inference {
    iid_vce: OracleMatrix,
    hc1_vce: OracleMatrix,
    normalized_reported_intercept_hc1_vce: Option<OracleMatrix>,
    normalized_reported_intercept_clustered_vce: Option<OracleMatrix>,
    one_way_cluster_vces: Vec<OracleMatrix>,
    cluster_counts: Vec<usize>,
    clustered_vce: Option<OracleMatrix>,
}

pub fn fit_explicit_ols(problem: &ExplicitOlsProblem) -> Result<ExplicitOlsResult, OracleError> {
    fit_explicit_ols_with_config(problem, OracleConfig::default())
}

pub fn fit_explicit_ols_with_config(
    problem: &ExplicitOlsProblem,
    config: OracleConfig,
) -> Result<ExplicitOlsResult, OracleError> {
    let fit = fit_linear(problem, config)?;
    let inference = fit_inference(problem, &fit)?;
    let certificate = certify_result(
        problem,
        &fit.dummy_columns,
        &fit.residualized_regressors,
        &fit.kept_regressors,
        &fit.validated.objective_weights,
        &fit.fitted_values,
        &fit.residuals,
        &fit.normalized_bread,
    );
    validate_certificate(certificate, config.certificate_tolerance)?;
    validate_finite_outputs(
        &fit.coefficients,
        fit.weighted_rss,
        &fit.finite_fitted_values,
        &fit.absorbed_fitted_values,
        &fit.fitted_values,
        &fit.residuals,
        &fit.bread,
        &inference.iid_vce,
        &inference.hc1_vce,
        &inference.one_way_cluster_vces,
        inference.clustered_vce.as_ref(),
        inference
            .normalized_reported_intercept_clustered_vce
            .as_ref(),
    )?;

    Ok(ExplicitOlsResult {
        coefficients: fit.coefficients,
        kept_regressors: fit.kept_regressors,
        dropped_regressors: fit.dropped_regressors,
        explicit_dummy_columns: fit.explicit_dummy_columns,
        explicit_dummy_rank: fit.explicit_dummy_rank,
        finite_regressor_rank: fit.finite_regressor_rank,
        model_rank: fit.model_rank,
        effective_observations: fit.validated.effective_observations,
        residual_degrees_of_freedom: fit.residual_degrees_of_freedom,
        finite_fitted_values: fit.finite_fitted_values,
        absorbed_fitted_values: fit.absorbed_fitted_values,
        fitted_values: fit.fitted_values,
        residuals: fit.residuals,
        weighted_rss: fit.weighted_rss,
        bread: fit.bread,
        iid_vce: inference.iid_vce,
        hc1_vce: inference.hc1_vce,
        normalized_reported_intercept_hc1_vce: inference.normalized_reported_intercept_hc1_vce,
        normalized_reported_intercept_clustered_vce: inference
            .normalized_reported_intercept_clustered_vce,
        one_way_cluster_vces: inference.one_way_cluster_vces,
        clustered_vce: inference.clustered_vce,
        cluster_counts: inference.cluster_counts,
        certificate,
    })
}

fn project(problem: &ExplicitOlsProblem, config: OracleConfig) -> Result<Projection, OracleError> {
    let validated = validate_problem(problem, config)?;
    let nobs = problem.outcome.len();
    let dummy_columns = materialize_dummy_columns(problem);
    let explicit_dummy_columns = dummy_columns.len();
    let maximum_columns = explicit_dummy_columns.max(problem.regressors.len()).max(1);
    let rank_tolerance =
        config.rank_tolerance_multiplier * f64::EPSILON * nobs.max(maximum_columns).max(1) as f64;

    let weighted_dummies = weight_columns(&dummy_columns, &validated.sqrt_weights);
    let dummy_qr = PivotedQr::factor(weighted_dummies, nobs, rank_tolerance)?;

    let weighted_outcome = weight_column(&problem.outcome, &validated.sqrt_weights);
    let residualized_weighted_outcome = dummy_qr.residualize(&weighted_outcome);
    let weighted_regressors = problem
        .regressors
        .iter()
        .map(|column| weight_column(column, &validated.sqrt_weights))
        .collect::<Vec<_>>();
    let regressor_reference_scales = weighted_regressors
        .iter()
        .map(|column| stable_norm(column.iter().copied()))
        .collect::<Vec<_>>();
    let residualized_weighted_regressors = weighted_regressors
        .iter()
        .map(|column| dummy_qr.residualize(column))
        .collect::<Vec<_>>();

    Ok(Projection {
        validated,
        nobs,
        dummy_columns,
        explicit_dummy_columns,
        rank_tolerance,
        dummy_qr,
        residualized_weighted_outcome,
        regressor_reference_scales,
        residualized_weighted_regressors,
    })
}

fn fit_linear(
    problem: &ExplicitOlsProblem,
    config: OracleConfig,
) -> Result<LinearFit, OracleError> {
    let Projection {
        validated,
        nobs,
        dummy_columns,
        explicit_dummy_columns,
        rank_tolerance,
        dummy_qr,
        residualized_weighted_outcome,
        regressor_reference_scales,
        residualized_weighted_regressors,
    } = project(problem, config)?;
    let finite_qr = PivotedQr::factor_with_scales(
        residualized_weighted_regressors.clone(),
        nobs,
        rank_tolerance,
        regressor_reference_scales,
    )?;
    let pivot_coefficients = finite_qr.solve(&residualized_weighted_outcome)?;
    let mut coefficients = vec![None; problem.regressors.len()];
    for (pivot_position, &original_column) in finite_qr.kept_original_indices().iter().enumerate() {
        coefficients[original_column] = Some(pivot_coefficients[pivot_position]);
    }
    let mut kept_regressors = finite_qr.kept_original_indices().to_vec();
    kept_regressors.sort_unstable();
    let dropped_regressors = (0..problem.regressors.len())
        .filter(|column| coefficients[*column].is_none())
        .collect::<Vec<_>>();

    let finite_fitted_values = (0..nobs)
        .map(|row| {
            problem
                .regressors
                .iter()
                .enumerate()
                .filter_map(|(column, values)| {
                    coefficients[column].map(|coefficient| coefficient * values[row])
                })
                .sum::<f64>()
        })
        .collect::<Vec<_>>();

    let weighted_regression_residual = (0..nobs)
        .map(|row| {
            finite_qr.kept_original_indices().iter().enumerate().fold(
                residualized_weighted_outcome[row],
                |value, (position, &column)| {
                    (-pivot_coefficients[position])
                        .mul_add(residualized_weighted_regressors[column][row], value)
                },
            )
        })
        .collect::<Vec<_>>();
    let residuals = weighted_regression_residual
        .iter()
        .zip(&validated.sqrt_weights)
        .map(|(residual, sqrt_weight)| residual / sqrt_weight)
        .collect::<Vec<_>>();
    let fitted_values = problem
        .outcome
        .iter()
        .zip(&residuals)
        .map(|(outcome, residual)| outcome - residual)
        .collect::<Vec<_>>();
    let absorbed_fitted_values = fitted_values
        .iter()
        .zip(&finite_fitted_values)
        .map(|(fitted, finite)| fitted - finite)
        .collect::<Vec<_>>();

    let finite_regressor_rank = finite_qr.rank();
    let model_rank = dummy_qr
        .rank()
        .checked_add(finite_regressor_rank)
        .ok_or_else(|| OracleError::InvalidInput("model rank overflowed usize".to_owned()))?;
    if validated.effective_observations <= model_rank {
        return Err(OracleError::InsufficientDegreesOfFreedom(format!(
            "effective N={} is not greater than model rank={model_rank}",
            validated.effective_observations
        )));
    }
    let residual_degrees_of_freedom = validated.effective_observations - model_rank;
    let normalized_weighted_rss = weighted_regression_residual
        .iter()
        .map(|residual| residual * residual)
        .sum::<f64>();
    let weighted_rss = normalized_weighted_rss * validated.raw_weight_scale;
    if normalized_weighted_rss > 0.0 && weighted_rss == 0.0 {
        return Err(OracleError::NumericalFailure(
            "raw-scale weighted RSS underflowed".to_owned(),
        ));
    }

    let normalized_bread_in_pivot_order = finite_qr.bread()?;
    let normalized_bread = reorder_matrix(
        &normalized_bread_in_pivot_order,
        finite_qr.kept_original_indices(),
        &kept_regressors,
    );
    let bread = scale_matrix(&normalized_bread, validated.raw_weight_scale.recip());
    let residualized_regressors = kept_regressors
        .iter()
        .map(|&column| {
            residualized_weighted_regressors[column]
                .iter()
                .zip(&validated.sqrt_weights)
                .map(|(value, sqrt_weight)| value / sqrt_weight)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let explicit_dummy_rank = dummy_qr.rank();
    Ok(LinearFit {
        validated,
        dummy_columns,
        explicit_dummy_columns,
        explicit_dummy_rank,
        coefficients,
        kept_regressors,
        dropped_regressors,
        finite_fitted_values,
        absorbed_fitted_values,
        fitted_values,
        residuals,
        finite_regressor_rank,
        model_rank,
        residual_degrees_of_freedom,
        normalized_weighted_rss,
        weighted_rss,
        normalized_bread,
        bread,
        residualized_regressors,
    })
}

fn fit_inference(problem: &ExplicitOlsProblem, fit: &LinearFit) -> Result<Inference, OracleError> {
    let iid_scale = fit.normalized_weighted_rss / fit.residual_degrees_of_freedom as f64;
    let iid_vce = scale_matrix(&fit.normalized_bread, iid_scale);
    let hc0_meat = observation_meat(
        &fit.residualized_regressors,
        &fit.residuals,
        &fit.validated.hc0_multipliers,
    );
    let hc1_scale =
        fit.validated.effective_observations as f64 / fit.residual_degrees_of_freedom as f64;
    let hc1_vce = scale_matrix(&sandwich(&fit.normalized_bread, &hc0_meat), hc1_scale);
    let normalized_reported_intercept_hc1_vce =
        if problem.effects.iter().any(DummyEffect::has_intercept) {
            Some(reported_intercept_hc1(
                problem,
                &fit.kept_regressors,
                &fit.residualized_regressors,
                &fit.residuals,
                &fit.validated,
                &fit.normalized_bread,
                hc1_scale,
            )?)
        } else {
            None
        };
    let normalized_reported_intercept_clustered_vce =
        if !problem.clusters.is_empty() && problem.effects.iter().any(DummyEffect::has_intercept) {
            Some(reported_intercept_clustered(
                problem,
                &fit.kept_regressors,
                &fit.residualized_regressors,
                &fit.residuals,
                &fit.validated,
                &fit.normalized_bread,
                fit.model_rank,
            )?)
        } else {
            None
        };

    let mut one_way_cluster_vces = Vec::with_capacity(problem.clusters.len());
    let mut cluster_counts = Vec::with_capacity(problem.clusters.len());
    for cluster in &problem.clusters {
        let (uncorrected, count) = clustered_component_uncorrected(
            &fit.normalized_bread,
            &fit.residualized_regressors,
            &fit.residuals,
            &fit.validated.objective_weights,
            &[cluster.as_slice()],
        )?;
        let correction =
            cluster_correction(fit.validated.effective_observations, fit.model_rank, count)?;
        one_way_cluster_vces.push(scale_matrix(&uncorrected, correction));
        cluster_counts.push(count);
    }
    let clustered_vce = if problem.clusters.is_empty() {
        None
    } else {
        let mut combined =
            OracleMatrix::zeros(fit.finite_regressor_rank, fit.finite_regressor_rank);
        let subset_count = (1_usize << problem.clusters.len()) - 1;
        for mask in 1..=subset_count {
            let selected = problem
                .clusters
                .iter()
                .enumerate()
                .filter_map(|(dimension, cluster)| {
                    (mask & (1_usize << dimension) != 0).then_some(cluster.as_slice())
                })
                .collect::<Vec<_>>();
            let (component, _) = clustered_component_uncorrected(
                &fit.normalized_bread,
                &fit.residualized_regressors,
                &fit.residuals,
                &fit.validated.objective_weights,
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
        let minimum_groups = *cluster_counts.iter().min().ok_or_else(|| {
            OracleError::InvalidInput("cluster count table is unexpectedly empty".to_owned())
        })?;
        let correction = cluster_correction(
            fit.validated.effective_observations,
            fit.model_rank,
            minimum_groups,
        )?;
        let mut covariance = scale_matrix(&combined, correction);
        symmetrize(&mut covariance);
        Some(covariance)
    };

    Ok(Inference {
        iid_vce,
        hc1_vce,
        normalized_reported_intercept_hc1_vce,
        normalized_reported_intercept_clustered_vce,
        one_way_cluster_vces,
        cluster_counts,
        clustered_vce,
    })
}
