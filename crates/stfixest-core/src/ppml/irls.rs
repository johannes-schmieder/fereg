use super::family::{means_from_eta, pairwise_sum, poisson_objective, PoissonObjective};
use super::{PpmlConfig, PpmlInitialization, PpmlStart};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, Result, StFixestError};
use crate::fe::{absorb_fixed_effects, AbsorbConfig, FeDesign, SolveReport};
use crate::ols::{solve_rank_revealing, RegressorStatus};

#[derive(Debug)]
pub(crate) struct ProfileStep {
    pub candidate_eta: Vec<f64>,
    pub coefficients: Vec<f64>,
    pub statuses: Vec<RegressorStatus>,
    pub kept: Vec<usize>,
    pub bread: Vec<f64>,
    pub transformed_regressors: DenseMatrix,
    pub absorb_report: Option<SolveReport>,
}

#[derive(Debug)]
pub(crate) struct AcceptedStep {
    pub eta: Vec<f64>,
    pub coefficients: Vec<f64>,
    pub mean: Vec<f64>,
    pub objective: PoissonObjective,
    pub step: f64,
    pub step_halvings: usize,
}

pub(crate) fn validate_inputs(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    config: PpmlConfig,
) -> Result<()> {
    if outcome.is_empty() {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "PPML requires at least one observation",
        ));
    }
    if regressors.nrows() != outcome.len()
        || prior_weights.len() != outcome.len()
        || offset.len() != outcome.len()
        || fixed_effects.is_some_and(|design| design.nobs() != outcome.len())
        || clusters.iter().any(|cluster| cluster.len() != outcome.len())
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "PPML inputs have inconsistent observation counts",
        ));
    }
    if outcome.iter().any(|value| !value.is_finite() || *value < 0.0) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "PPML outcomes must be finite and nonnegative",
        ));
    }
    if outcome.iter().all(|value| *value == 0.0) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "the all-zero outcome has no finite PPML intercept solution",
        ));
    }
    if regressors.as_slice().iter().any(|value| !value.is_finite())
        || offset.iter().any(|value| !value.is_finite())
        || prior_weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "PPML regressors and offsets must be finite and prior weights positive",
        ));
    }
    config.bounds.validate()?;
    let valid = valid_unit(config.rank_tolerance)
        && valid_unit(config.fixed_effect_collinearity_tolerance)
        && valid_unit(config.tolerance)
        && valid_unit(config.score_tolerance)
        && valid_unit(config.inner_tolerance_initial)
        && valid_unit(config.inner_tolerance_final)
        && config.inner_tolerance_initial >= config.inner_tolerance_final
        && config.max_iterations > 0
        && config.minimum_step.is_finite()
        && config.minimum_step > 0.0
        && config.minimum_step <= 1.0
        && config.objective_slack.is_finite()
        && config.objective_slack >= 0.0;
    if valid {
        Ok(())
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "PPML numerical configuration is invalid",
        ))
    }
}

pub(crate) fn validate_start(
    start: &PpmlStart,
    nrows: usize,
    ncols: usize,
    config: PpmlConfig,
) -> Result<()> {
    if start.coefficients.len() != ncols || start.linear_predictor.len() != nrows {
        return Err(StFixestError::new(
            ErrorCode::InvalidArgument,
            "PPML warm start has inconsistent dimensions",
        ));
    }
    if start.coefficients.iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            "PPML warm-start coefficients must be finite",
        ));
    }
    let mut mean = vec![0.0_f64; nrows];
    means_from_eta(&start.linear_predictor, config.bounds, &mut mean)
}

pub(crate) fn initialize(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    config: PpmlConfig,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut coefficients = vec![0.0_f64; regressors.ncols()];
    let has_fe_intercept = fixed_effects.is_some_and(|design| {
        design.effects().iter().any(|effect| effect.has_intercept())
    });
    let constant_column = detect_constant_column(regressors);
    let can_shift = has_fe_intercept || constant_column.is_some();
    let shift = if config.initialization == PpmlInitialization::Intercept && can_shift {
        log_weighted_outcome_sum(outcome, prior_weights)?
            - log_weighted_offset_sum(offset, prior_weights)?
    } else {
        0.0
    };
    if !has_fe_intercept {
        if let Some((column, value)) = constant_column {
            coefficients[column] = shift / value;
        }
    }
    let eta = offset.iter().map(|value| value + shift).collect::<Vec<_>>();
    let mut mean = vec![0.0_f64; eta.len()];
    means_from_eta(&eta, config.bounds, &mut mean).map_err(|error| {
        StFixestError::new(
            error.code,
            format!("PPML initialization is outside the finite domain: {}", error.message),
        )
    })?;
    Ok((coefficients, eta))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn profile_irls_step(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    eta: &[f64],
    mean: &[f64],
    config: PpmlConfig,
    inner_tolerance: f64,
) -> Result<ProfileStep> {
    let nrows = outcome.len();
    let ncols = regressors.ncols();
    let mut working_weights = vec![0.0_f64; nrows];
    let mut working_response = vec![0.0_f64; nrows];
    for row in 0..nrows {
        working_weights[row] = prior_weights[row] * mean[row];
        working_response[row] =
            eta[row] - offset[row] + (outcome[row] - mean[row]) / mean[row];
        if !working_weights[row].is_finite()
            || working_weights[row] <= 0.0
            || !working_response[row].is_finite()
        {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                format!("invalid PPML working state at observation {row}"),
            ));
        }
    }

    let original_norms = weighted_regressor_norms(regressors, &working_weights)?;
    let mut columns = Vec::with_capacity(ncols + 1);
    columns.push(working_response.clone());
    for column in 0..ncols {
        columns.push(regressors.column(column)?);
    }
    let mut transformed = DenseMatrix::from_columns(&columns)?;
    let absorb_report = if let Some(design) = fixed_effects {
        if design.effects().is_empty() {
            None
        } else {
            let report = absorb_fixed_effects(
                design,
                &working_weights,
                &mut transformed,
                absorb_with_tolerance(config.absorb, inner_tolerance),
            )?;
            if !report.certified() {
                return Err(StFixestError::new(
                    ErrorCode::InvalidData,
                    format!("PPML fixed-effect solve did not certify: {:?}", report.stop_reason),
                ));
            }
            Some(report)
        }
    } else {
        None
    };

    let within_response = transformed.column(0)?;
    let mut statuses = vec![RegressorStatus::Kept; ncols];
    let mut candidate_indices = Vec::new();
    let mut candidate_columns = Vec::new();
    for column in 0..ncols {
        let transformed_column = transformed.column(column + 1)?;
        let transformed_norm = weighted_vector_norm(&transformed_column, &working_weights)?;
        if transformed_norm
            <= config.fixed_effect_collinearity_tolerance * original_norms[column].max(1.0)
        {
            statuses[column] = if fixed_effects.is_some() {
                RegressorStatus::CollinearWithFixedEffects
            } else {
                RegressorStatus::CollinearAfterResidualization
            };
        } else {
            candidate_indices.push(column);
            candidate_columns.push(transformed_column);
        }
    }
    let candidate_matrix = if candidate_columns.is_empty() {
        DenseMatrix::zeros(nrows, 0)?
    } else {
        DenseMatrix::from_columns(&candidate_columns)?
    };
    let finite = solve_rank_revealing(
        &candidate_matrix,
        &within_response,
        &working_weights,
        config.rank_tolerance,
        config.finite_solver,
    )?;

    let mut coefficients = vec![0.0_f64; ncols];
    let mut kept = Vec::with_capacity(finite.rank);
    for &candidate in &finite.kept {
        let original = candidate_indices[candidate];
        coefficients[original] = finite.coefficients[candidate];
        kept.push(original);
    }
    for &candidate in &finite.dropped {
        statuses[candidate_indices[candidate]] = RegressorStatus::NumericallyRankDeficient;
    }
    let mut bread = vec![0.0_f64; ncols * ncols];
    for (candidate_row, &original_row) in candidate_indices.iter().enumerate() {
        for (candidate_column, &original_column) in candidate_indices.iter().enumerate() {
            bread[original_row * ncols + original_column] =
                finite.bread[candidate_row * candidate_indices.len() + candidate_column];
        }
    }

    let mut residual = within_response;
    for row in 0..nrows {
        let mut fitted = 0.0_f64;
        for &column in &kept {
            fitted = transformed[(row, column + 1)].mul_add(coefficients[column], fitted);
        }
        residual[row] -= fitted;
    }
    let candidate_eta = (0..nrows)
        .map(|row| offset[row] + working_response[row] - residual[row])
        .collect();
    Ok(ProfileStep {
        candidate_eta,
        coefficients,
        statuses,
        kept,
        bread,
        transformed_regressors: matrix_without_first_column(&transformed)?,
        absorb_report,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn accept_step(
    current_eta: &[f64],
    current_coefficients: &[f64],
    candidate_eta: &[f64],
    candidate_coefficients: &[f64],
    outcome: &[f64],
    prior_weights: &[f64],
    current_objective: PoissonObjective,
    config: PpmlConfig,
) -> Result<AcceptedStep> {
    if current_eta.len() != candidate_eta.len()
        || current_coefficients.len() != candidate_coefficients.len()
    {
        return Err(StFixestError::new(
            ErrorCode::Internal,
            "PPML line-search states have inconsistent dimensions",
        ));
    }
    let allowed = current_objective.deviance
        + config.objective_slack * current_objective.deviance.abs().max(1.0);
    let mut step = 1.0_f64;
    for step_halvings in 0..=config.max_step_halvings {
        if step < config.minimum_step {
            break;
        }
        let eta = current_eta
            .iter()
            .zip(candidate_eta)
            .map(|(old, candidate)| old + step * (candidate - old))
            .collect::<Vec<_>>();
        let mut mean = vec![0.0_f64; eta.len()];
        if means_from_eta(&eta, config.bounds, &mut mean).is_ok() {
            let objective = poisson_objective(outcome, &mean, prior_weights)?;
            if objective.deviance <= allowed {
                let coefficients = current_coefficients
                    .iter()
                    .zip(candidate_coefficients)
                    .map(|(old, candidate)| old + step * (candidate - old))
                    .collect();
                return Ok(AcceptedStep {
                    eta,
                    coefficients,
                    mean,
                    objective,
                    step,
                    step_halvings,
                });
            }
        }
        step *= 0.5;
    }
    Err(StFixestError::new(
        ErrorCode::InvalidData,
        format!(
            "PPML step-halving found no finite deviance-improving step after {} halvings",
            config.max_step_halvings
        ),
    ))
}

pub(crate) fn scheduled_inner_tolerance(config: PpmlConfig, iteration: usize) -> f64 {
    const STAGES: usize = 8;
    if iteration >= STAGES {
        return config.inner_tolerance_final;
    }
    let fraction = iteration.saturating_sub(1) as f64 / (STAGES - 1) as f64;
    ((1.0 - fraction) * config.inner_tolerance_initial.ln()
        + fraction * config.inner_tolerance_final.ln())
    .exp()
}

pub(crate) fn multiply(matrix: &DenseMatrix, coefficients: &[f64]) -> Result<Vec<f64>> {
    if matrix.ncols() != coefficients.len() {
        return Err(StFixestError::new(
            ErrorCode::Internal,
            "PPML coefficient and regressor dimensions differ",
        ));
    }
    let mut output = vec![0.0_f64; matrix.nrows()];
    for row in 0..matrix.nrows() {
        for column in 0..matrix.ncols() {
            output[row] = matrix[(row, column)].mul_add(coefficients[column], output[row]);
        }
    }
    Ok(output)
}

pub(crate) fn weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    let terms = outcome
        .iter()
        .zip(weights)
        .map(|(value, weight)| value * weight)
        .collect::<Vec<_>>();
    let sum = pairwise_sum(&terms);
    if sum.is_finite() {
        Ok(sum)
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidData,
            "weighted PPML outcome sum overflowed",
        ))
    }
}

fn detect_constant_column(regressors: &DenseMatrix) -> Option<(usize, f64)> {
    for column in 0..regressors.ncols() {
        let first = regressors[(0, column)];
        if first.abs() <= f64::EPSILON.sqrt() {
            continue;
        }
        let tolerance = 16.0 * f64::EPSILON * first.abs().max(1.0);
        if (1..regressors.nrows())
            .all(|row| (regressors[(row, column)] - first).abs() <= tolerance)
        {
            return Some((column, first));
        }
    }
    None
}

fn log_weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    let terms = outcome
        .iter()
        .zip(weights)
        .filter_map(|(value, weight)| {
            (*value > 0.0).then(|| value.ln() + weight.ln())
        })
        .collect::<Vec<_>>();
    log_sum_exp(&terms, "weighted PPML outcome sum")
}

fn log_weighted_offset_sum(offset: &[f64], weights: &[f64]) -> Result<f64> {
    let terms = offset
        .iter()
        .zip(weights)
        .map(|(value, weight)| value + weight.ln())
        .collect::<Vec<_>>();
    log_sum_exp(&terms, "weighted PPML exposure sum")
}

fn log_sum_exp(terms: &[f64], label: &str) -> Result<f64> {
    if terms.is_empty() || terms.iter().any(|value| !value.is_finite()) {
        return Err(StFixestError::new(
            ErrorCode::InvalidData,
            format!("{label} is empty or nonfinite"),
        ));
    }
    let maximum = terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let scaled = terms.iter().map(|value| (value - maximum).exp()).collect::<Vec<_>>();
    let result = maximum + pairwise_sum(&scaled).ln();
    if result.is_finite() {
        Ok(result)
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidData,
            format!("{label} overflowed"),
        ))
    }
}

fn weighted_regressor_norms(regressors: &DenseMatrix, weights: &[f64]) -> Result<Vec<f64>> {
    let mut norms = vec![0.0_f64; regressors.ncols()];
    for row in 0..regressors.nrows() {
        for column in 0..regressors.ncols() {
            let value = regressors[(row, column)];
            norms[column] = (weights[row] * value).mul_add(value, norms[column]);
        }
    }
    for norm in &mut norms {
        *norm = norm.sqrt();
        if !norm.is_finite() {
            return Err(StFixestError::new(
                ErrorCode::InvalidData,
                "PPML weighted regressor norm overflowed",
            ));
        }
    }
    Ok(norms)
}

fn weighted_vector_norm(values: &[f64], weights: &[f64]) -> Result<f64> {
    let mut square_sum = 0.0_f64;
    for (value, weight) in values.iter().zip(weights) {
        square_sum = (*weight * *value).mul_add(*value, square_sum);
    }
    let norm = square_sum.sqrt();
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(StFixestError::new(
            ErrorCode::InvalidData,
            "PPML weighted vector norm overflowed",
        ))
    }
}

fn matrix_without_first_column(matrix: &DenseMatrix) -> Result<DenseMatrix> {
    if matrix.ncols() <= 1 {
        return DenseMatrix::zeros(matrix.nrows(), 0);
    }
    let columns = (1..matrix.ncols())
        .map(|column| matrix.column(column))
        .collect::<Result<Vec<_>>>()?;
    DenseMatrix::from_columns(&columns)
}

fn absorb_with_tolerance(mut config: AbsorbConfig, tolerance: f64) -> AbsorbConfig {
    config.map.tolerance = tolerance;
    config.lsmr.tolerance = tolerance;
    config
}

fn valid_unit(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}
