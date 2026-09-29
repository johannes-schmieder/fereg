#[cfg(test)]
use super::family::{
    fill_poisson_working_state, means_from_eta, poisson_objective_from_eta, CompensatedSum,
    PoissonObjective,
};
#[cfg(test)]
use super::{PpmlConfig, PpmlStart};
#[cfg(test)]
use crate::data::DenseMatrix;
#[cfg(test)]
use crate::error::{ErrorCode, FeregError, Result};
#[cfg(test)]
use crate::fe::{absorb_fixed_effects, AbsorbConfig, FeDesign, SolveReport};
#[cfg(test)]
use crate::ols::{solve_rank_revealing, FiniteSolver, RegressorStatus};

#[cfg(test)]
const LEGACY_TEST_RANK_TOLERANCE: f64 = 1e-10;

#[cfg(test)]
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

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct AcceptedStep {
    pub eta: Vec<f64>,
    pub coefficients: Vec<f64>,
    pub mean: Vec<f64>,
    pub objective: PoissonObjective,
    pub step: f64,
    pub step_halvings: usize,
}

#[cfg(test)]
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
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML requires at least one observation",
        ));
    }
    if regressors.nrows() != outcome.len()
        || prior_weights.len() != outcome.len()
        || offset.len() != outcome.len()
        || fixed_effects.is_some_and(|design| design.nobs() != outcome.len())
        || clusters
            .iter()
            .any(|cluster| cluster.len() != outcome.len())
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML inputs have inconsistent observation counts",
        ));
    }
    if outcome
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML outcomes must be finite and nonnegative",
        ));
    }
    if outcome.iter().all(|value| *value == 0.0) {
        return Err(FeregError::new(
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
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML regressors and offsets must be finite and prior weights positive",
        ));
    }
    config.bounds.validate()?;
    let valid = valid_unit(LEGACY_TEST_RANK_TOLERANCE)
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
        Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML numerical configuration is invalid",
        ))
    }
}

#[cfg(test)]
pub(crate) fn validate_start(
    start: &PpmlStart,
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    config: PpmlConfig,
) -> Result<()> {
    let nrows = regressors.nrows();
    let ncols = regressors.ncols();
    if start.coefficients.len() != ncols || start.linear_predictor.len() != nrows {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "PPML warm start has inconsistent dimensions",
        ));
    }
    if start.coefficients.iter().any(|value| !value.is_finite()) {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML warm-start coefficients must be finite",
        ));
    }
    let mut mean = vec![0.0_f64; nrows];
    means_from_eta(&start.linear_predictor, config.bounds, &mut mean)?;

    let finite_predictor = multiply(regressors, &start.coefficients)?;
    let mut absorbed_predictor = Vec::with_capacity(nrows);
    let mut reference_scale = 1.0_f64;
    for row in 0..nrows {
        let remainder = start.linear_predictor[row] - offset[row] - finite_predictor[row];
        if !remainder.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("PPML warm-start decomposition is nonfinite at observation {row}"),
            ));
        }
        reference_scale = reference_scale
            .max(start.linear_predictor[row].abs())
            .max(offset[row].abs())
            .max(finite_predictor[row].abs())
            .max(remainder.abs());
        absorbed_predictor.push(remainder);
    }

    let has_fixed_effects = fixed_effects.is_some_and(|design| !design.effects().is_empty());
    let residual_infinity_norm = if has_fixed_effects {
        let mut residualized = DenseMatrix::from_columns(&[absorbed_predictor])?;
        let report = absorb_fixed_effects(
            fixed_effects.expect("fixed-effect presence checked"),
            prior_weights,
            &mut residualized,
            absorb_with_tolerance(config.absorb, config.inner_tolerance_final),
        )?;
        if !report.certified() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!(
                    "PPML warm-start fixed-effect decomposition did not certify: {:?}",
                    report.stop_reason
                ),
            ));
        }
        residualized
            .as_slice()
            .iter()
            .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
    } else {
        absorbed_predictor
            .iter()
            .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
    };
    let consistency_tolerance = config
        .tolerance
        .max(config.inner_tolerance_final)
        .max(256.0 * f64::EPSILON);
    if residual_infinity_norm <= consistency_tolerance * reference_scale {
        Ok(())
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            format!(
                "PPML warm start is inconsistent with offset, regressors, and fixed effects: relative decomposition residual {:.3e}",
                residual_infinity_norm / reference_scale
            ),
        ))
    }
}

#[cfg(test)]
pub(crate) fn initialize(
    outcome: &[f64],
    regressors: &DenseMatrix,
    prior_weights: &[f64],
    offset: &[f64],
    fixed_effects: Option<&FeDesign>,
    config: PpmlConfig,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut coefficients = vec![0.0_f64; regressors.ncols()];
    let has_fe_intercept = fixed_effects
        .is_some_and(|design| design.effects().iter().any(|effect| effect.has_intercept()));
    let constant_column = detect_constant_column(regressors);
    let can_shift = has_fe_intercept || constant_column.is_some();
    let shift = if can_shift {
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
        FeregError::new(
            error.code,
            format!(
                "PPML initialization is outside the finite domain: {}",
                error.message
            ),
        )
    })?;
    Ok((coefficients, eta))
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
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
    fill_poisson_working_state(
        outcome,
        mean,
        prior_weights,
        eta,
        offset,
        &mut working_weights,
        &mut working_response,
    )?;

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
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    format!(
                        "PPML fixed-effect solve did not certify: {:?}",
                        report.stop_reason
                    ),
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
        LEGACY_TEST_RANK_TOLERANCE,
        FiniteSolver::Auto,
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
#[cfg(test)]
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
        return Err(FeregError::new(
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
            // A proposed predictor can have representable means while its
            // objective still overflows.  Both conditions reject this trial and
            // let the caller's deterministic backtracking reduce the step.
            if let Ok(objective) = poisson_objective_from_eta(outcome, &eta, &mean, prior_weights) {
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
        }
        step *= 0.5;
    }
    Err(FeregError::new(
        ErrorCode::NonConvergence,
        format!(
            "PPML step-halving found no finite deviance-improving step after {} halvings",
            config.max_step_halvings
        ),
    ))
}

#[cfg(test)]
pub(crate) fn multiply(matrix: &DenseMatrix, coefficients: &[f64]) -> Result<Vec<f64>> {
    if matrix.ncols() != coefficients.len() {
        return Err(FeregError::new(
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

#[cfg(test)]
pub(crate) fn weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    if outcome.len() != weights.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "weighted PPML outcome inputs have inconsistent lengths",
        ));
    }
    let mut accumulator = CompensatedSum::default();
    for (row, (value, weight)) in outcome.iter().zip(weights).enumerate() {
        let term = value * weight;
        if !term.is_finite() {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                format!("weighted PPML outcome overflows at observation {row}"),
            ));
        }
        accumulator.add(term);
    }
    let sum = accumulator.total();
    if sum.is_finite() {
        Ok(sum)
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "weighted PPML outcome sum overflowed",
        ))
    }
}

#[cfg(test)]
fn detect_constant_column(regressors: &DenseMatrix) -> Option<(usize, f64)> {
    for column in 0..regressors.ncols() {
        let first = regressors[(0, column)];
        if first.abs() <= f64::EPSILON.sqrt() {
            continue;
        }
        let tolerance = 16.0 * f64::EPSILON * first.abs().max(1.0);
        if (1..regressors.nrows()).all(|row| (regressors[(row, column)] - first).abs() <= tolerance)
        {
            return Some((column, first));
        }
    }
    None
}

#[cfg(test)]
fn log_weighted_outcome_sum(outcome: &[f64], weights: &[f64]) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    let mut positive = false;
    for (value, weight) in outcome.iter().zip(weights) {
        if *value > 0.0 {
            maximum = maximum.max(value.ln() + weight.ln());
            positive = true;
        }
    }
    if !positive || !maximum.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "weighted PPML outcome sum is empty or nonfinite",
        ));
    }
    let mut scaled_sum = CompensatedSum::default();
    for (value, weight) in outcome.iter().zip(weights) {
        if *value > 0.0 {
            scaled_sum.add((value.ln() + weight.ln() - maximum).exp());
        }
    }
    finish_log_sum_exp(maximum, scaled_sum, "weighted PPML outcome sum")
}

#[cfg(test)]
fn log_weighted_offset_sum(offset: &[f64], weights: &[f64]) -> Result<f64> {
    let mut maximum = f64::NEG_INFINITY;
    for (value, weight) in offset.iter().zip(weights) {
        maximum = maximum.max(value + weight.ln());
    }
    if offset.is_empty() || !maximum.is_finite() {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "weighted PPML exposure sum is empty or nonfinite",
        ));
    }
    let mut scaled_sum = CompensatedSum::default();
    for (value, weight) in offset.iter().zip(weights) {
        scaled_sum.add((value + weight.ln() - maximum).exp());
    }
    finish_log_sum_exp(maximum, scaled_sum, "weighted PPML exposure sum")
}

#[cfg(test)]
fn finish_log_sum_exp(maximum: f64, scaled: CompensatedSum, label: &str) -> Result<f64> {
    let result = maximum + scaled.total().ln();
    if result.is_finite() {
        Ok(result)
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            format!("{label} overflowed"),
        ))
    }
}

#[cfg(test)]
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
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "PPML weighted regressor norm overflowed",
            ));
        }
    }
    Ok(norms)
}

#[cfg(test)]
fn weighted_vector_norm(values: &[f64], weights: &[f64]) -> Result<f64> {
    let mut square_sum = 0.0_f64;
    for (value, weight) in values.iter().zip(weights) {
        square_sum = (*weight * *value).mul_add(*value, square_sum);
    }
    let norm = square_sum.sqrt();
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(FeregError::new(
            ErrorCode::InvalidData,
            "PPML weighted vector norm overflowed",
        ))
    }
}

#[cfg(test)]
fn matrix_without_first_column(matrix: &DenseMatrix) -> Result<DenseMatrix> {
    if matrix.ncols() <= 1 {
        return DenseMatrix::zeros(matrix.nrows(), 0);
    }
    let columns = (1..matrix.ncols())
        .map(|column| matrix.column(column))
        .collect::<Result<Vec<_>>>()?;
    DenseMatrix::from_columns(&columns)
}

#[cfg(test)]
fn absorb_with_tolerance(mut config: AbsorbConfig, tolerance: f64) -> AbsorbConfig {
    config.map.tolerance = tolerance;
    config.lsmr.tolerance = tolerance;
    config
}

#[cfg(test)]
fn valid_unit(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;
    use crate::ppml::PoissonBounds;

    #[test]
    fn intercept_initialization_is_offset_aware_and_deterministic() {
        let outcome = [0.25_f64, 2.0, 8.0, 1.5];
        let weights = [3.0_f64, 0.5, 2.0, 4.0];
        let offset = [-300.0_f64, -5.0, 0.0, 300.0];
        let regressors =
            DenseMatrix::from_columns(&[vec![1.0; outcome.len()]]).expect("regressors");
        let first = initialize(
            &outcome,
            &regressors,
            &weights,
            &offset,
            None,
            PpmlConfig::default(),
        )
        .expect("first initialization");
        let second = initialize(
            &outcome,
            &regressors,
            &weights,
            &offset,
            None,
            PpmlConfig::default(),
        )
        .expect("second initialization");
        assert_eq!(first, second);

        let mut mean = vec![0.0_f64; outcome.len()];
        means_from_eta(&first.1, PoissonBounds::default(), &mut mean).expect("initial means");
        let outcome_total = weighted_outcome_sum(&outcome, &weights).expect("outcome total");
        let mean_total = weighted_outcome_sum(&mean, &weights).expect("mean total");
        assert!((outcome_total - mean_total).abs() <= 2e-13 * outcome_total.max(1.0));
        assert!(first.1.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn warm_start_rejects_an_inconsistent_finite_predictor() {
        let regressors = DenseMatrix::from_columns(&[vec![1.0; 4]]).expect("regressors");
        let start = PpmlStart {
            coefficients: vec![0.0],
            linear_predictor: vec![1.0; 4],
        };
        let error = validate_start(
            &start,
            &regressors,
            &[1.0; 4],
            &[0.0; 4],
            None,
            PpmlConfig::default(),
        )
        .expect_err("eta and beta must describe the same predictor");
        assert!(error.message.contains("warm start is inconsistent"));
    }

    #[test]
    fn warm_start_rejects_a_remainder_outside_the_fixed_effect_span() {
        let regressors = DenseMatrix::from_columns(&[vec![0.0; 4]]).expect("regressors");
        let effect = EffectTerm::new(vec![0, 0, 1, 1], 2, true, vec![]).expect("effect");
        let fixed_effects = FeDesign::new(4, vec![effect]).expect("design");
        let start = PpmlStart {
            coefficients: vec![0.0],
            linear_predictor: vec![0.0, 1.0, 0.0, 1.0],
        };
        let error = validate_start(
            &start,
            &regressors,
            &[1.0; 4],
            &[0.0; 4],
            Some(&fixed_effects),
            PpmlConfig::default(),
        )
        .expect_err("within-level variation is not an FE contribution");
        assert!(error.message.contains("warm start is inconsistent"));
    }
}
