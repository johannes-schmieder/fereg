//! Independent small-model explicit-dummy OLS/WLS oracle.
//!
//! The implementation deliberately does not call the production OLS, VCE, or
//! degrees-of-freedom code. It materializes every nuisance dummy and slope
//! column, applies weighted Frisch--Waugh--Lovell projections with a scaled
//! column-pivoted Householder QR, and fails closed when its numerical
//! certificates do not pass.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Index;

/// One explicitly materialized absorbed effect.
#[derive(Debug, Clone, PartialEq)]
pub struct DummyEffect {
    codes: Vec<u32>,
    n_levels: usize,
    intercept: bool,
    slopes: Vec<Vec<f64>>,
}

impl DummyEffect {
    /// Construct an absorbed term. Each level gets an intercept column when
    /// `intercept` is true and one column per supplied slope.
    pub fn new(
        codes: Vec<u32>,
        n_levels: usize,
        intercept: bool,
        slopes: Vec<Vec<f64>>,
    ) -> Result<Self, OracleError> {
        if n_levels == 0 {
            return Err(OracleError::InvalidInput(
                "an absorbed effect must declare at least one level".to_owned(),
            ));
        }
        if !intercept && slopes.is_empty() {
            return Err(OracleError::InvalidInput(
                "an absorbed effect must contain an intercept or a slope".to_owned(),
            ));
        }
        if codes.iter().any(|&code| code as usize >= n_levels) {
            return Err(OracleError::InvalidInput(
                "an absorbed-effect code exceeds its declared level count".to_owned(),
            ));
        }
        if slopes.iter().any(|slope| slope.len() != codes.len()) {
            return Err(OracleError::InvalidInput(
                "absorbed slope and code lengths differ".to_owned(),
            ));
        }
        if slopes.iter().flatten().any(|value| !value.is_finite()) {
            return Err(OracleError::InvalidInput(
                "absorbed slopes must be finite".to_owned(),
            ));
        }
        Ok(Self {
            codes,
            n_levels,
            intercept,
            slopes,
        })
    }

    #[must_use]
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    #[must_use]
    pub const fn n_levels(&self) -> usize {
        self.n_levels
    }

    #[must_use]
    pub const fn has_intercept(&self) -> bool {
        self.intercept
    }

    #[must_use]
    pub fn slopes(&self) -> &[Vec<f64>] {
        &self.slopes
    }
}

/// Weight semantics used by the oracle.
#[derive(Debug, Clone, PartialEq)]
pub enum OracleWeights {
    /// Ordinary least squares.
    Unit,
    /// Positive precision weights for the objective `sum(w_i e_i^2)`.
    Wls(Vec<f64>),
    /// Positive integer row multiplicities. HC1 treats the repetitions as
    /// separate observations, while cluster scores aggregate all repetitions.
    Frequency(Vec<u32>),
}

/// Complete input to the independent oracle. Regressors are column-major and
/// no intercept is added implicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitOlsProblem {
    pub outcome: Vec<f64>,
    pub regressors: Vec<Vec<f64>>,
    pub effects: Vec<DummyEffect>,
    pub weights: OracleWeights,
    /// Zero, one, or two cluster dimensions.
    pub clusters: Vec<Vec<u64>>,
}

/// Numerical policy for the small-model oracle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleConfig {
    /// Multiplier on `eps * max(n, p)` after every design column is scaled to
    /// unit two-norm.
    pub rank_tolerance_multiplier: f64,
    /// Maximum normalized normal-equation or inverse certificate.
    pub certificate_tolerance: f64,
}

impl Default for OracleConfig {
    fn default() -> Self {
        Self {
            rank_tolerance_multiplier: 128.0,
            certificate_tolerance: 5e-10,
        }
    }
}

/// Dense row-major matrix returned for identified regressors only. The
/// corresponding original regressor indices are in `kept_regressors`.
#[derive(Debug, Clone, PartialEq)]
pub struct OracleMatrix {
    nrows: usize,
    ncols: usize,
    data: Vec<f64>,
}

impl OracleMatrix {
    fn zeros(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            data: vec![0.0; nrows * ncols],
        }
    }

    #[must_use]
    pub const fn nrows(&self) -> usize {
        self.nrows
    }

    #[must_use]
    pub const fn ncols(&self) -> usize {
        self.ncols
    }

    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}

impl Index<(usize, usize)> for OracleMatrix {
    type Output = f64;

    fn index(&self, (row, column): (usize, usize)) -> &Self::Output {
        &self.data[row * self.ncols + column]
    }
}

/// Independent numerical certificates checked before a result is returned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleCertificate {
    pub normalized_normal_equation_infinity: f64,
    pub bread_inverse_infinity: f64,
    pub reconstruction_infinity: f64,
}

/// Explicit-dummy OLS/WLS result. Coefficients are `None` for finite
/// regressors that are collinear with the dummies or earlier identified
/// columns. Fitted values and residuals remain identified.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitOlsResult {
    pub coefficients: Vec<Option<f64>>,
    pub kept_regressors: Vec<usize>,
    pub dropped_regressors: Vec<usize>,
    pub explicit_dummy_columns: usize,
    pub explicit_dummy_rank: usize,
    pub finite_regressor_rank: usize,
    pub model_rank: usize,
    pub effective_observations: usize,
    pub residual_degrees_of_freedom: usize,
    pub finite_fitted_values: Vec<f64>,
    pub absorbed_fitted_values: Vec<f64>,
    pub fitted_values: Vec<f64>,
    pub residuals: Vec<f64>,
    pub weighted_rss: f64,
    pub bread: OracleMatrix,
    pub iid_vce: OracleMatrix,
    pub hc1_vce: OracleMatrix,
    /// CR1 VCE for each individual cluster dimension.
    pub one_way_cluster_vces: Vec<OracleMatrix>,
    /// For one cluster this equals its one-way VCE. For two clusters it is the
    /// inclusion--exclusion CR1 VCE.
    pub clustered_vce: Option<OracleMatrix>,
    pub cluster_counts: Vec<usize>,
    pub certificate: OracleCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleError {
    InvalidInput(String),
    InsufficientDegreesOfFreedom(String),
    NumericalFailure(String),
}

impl Display for OracleError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid oracle input: {message}"),
            Self::InsufficientDegreesOfFreedom(message) => {
                write!(
                    formatter,
                    "insufficient oracle degrees of freedom: {message}"
                )
            }
            Self::NumericalFailure(message) => {
                write!(formatter, "oracle numerical certificate failed: {message}")
            }
        }
    }
}

impl Error for OracleError {}

/// Fit with the hardened default numerical policy.
pub fn fit_explicit_ols(problem: &ExplicitOlsProblem) -> Result<ExplicitOlsResult, OracleError> {
    fit_explicit_ols_with_config(problem, OracleConfig::default())
}

/// Fit with an explicit numerical policy. This entry point is intended for
/// sensitivity tests; production estimators must not call the test oracle.
pub fn fit_explicit_ols_with_config(
    problem: &ExplicitOlsProblem,
    config: OracleConfig,
) -> Result<ExplicitOlsResult, OracleError> {
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
    let residualized_weighted_regressors = problem
        .regressors
        .iter()
        .map(|column| dummy_qr.residualize(&weight_column(column, &validated.sqrt_weights)))
        .collect::<Vec<_>>();

    let finite_qr = PivotedQr::factor(
        residualized_weighted_regressors.clone(),
        nobs,
        rank_tolerance,
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
    let weighted_rss = weighted_regression_residual
        .iter()
        .map(|residual| residual * residual)
        .sum::<f64>();

    let bread_in_pivot_order = finite_qr.bread()?;
    let bread = reorder_matrix(
        &bread_in_pivot_order,
        finite_qr.kept_original_indices(),
        &kept_regressors,
    );
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

    let iid_scale = weighted_rss / residual_degrees_of_freedom as f64;
    let iid_vce = scale_matrix(&bread, iid_scale);
    let hc0_meat = observation_meat(
        &residualized_regressors,
        &residuals,
        &validated.hc0_multipliers,
    );
    let hc1_scale = validated.effective_observations as f64 / residual_degrees_of_freedom as f64;
    let hc1_vce = scale_matrix(&sandwich(&bread, &hc0_meat), hc1_scale);

    let mut one_way_cluster_vces = Vec::with_capacity(problem.clusters.len());
    let mut cluster_counts = Vec::with_capacity(problem.clusters.len());
    for cluster in &problem.clusters {
        let (vce, count) = clustered_component(
            &bread,
            &residualized_regressors,
            &residuals,
            &validated.objective_weights,
            cluster.iter().copied().map(|value| (value, 0_u64)),
            validated.effective_observations,
            model_rank,
        )?;
        one_way_cluster_vces.push(vce);
        cluster_counts.push(count);
    }
    let clustered_vce = match problem.clusters.as_slice() {
        [] => None,
        [_] => Some(one_way_cluster_vces[0].clone()),
        [first, second] => {
            let (intersection, _) = clustered_component(
                &bread,
                &residualized_regressors,
                &residuals,
                &validated.objective_weights,
                first.iter().copied().zip(second.iter().copied()),
                validated.effective_observations,
                model_rank,
            )?;
            Some(add_subtract_matrices(
                &one_way_cluster_vces[0],
                &one_way_cluster_vces[1],
                &intersection,
            ))
        }
        _ => unreachable!("validation limits cluster dimensions to two"),
    };

    let certificate = certify_result(
        problem,
        &dummy_columns,
        &residualized_regressors,
        &kept_regressors,
        &validated.objective_weights,
        &fitted_values,
        &residuals,
        &bread,
    );
    validate_certificate(certificate, config.certificate_tolerance)?;
    validate_finite_outputs(
        &coefficients,
        &fitted_values,
        &residuals,
        &bread,
        &iid_vce,
        &hc1_vce,
        &one_way_cluster_vces,
        clustered_vce.as_ref(),
    )?;

    Ok(ExplicitOlsResult {
        coefficients,
        kept_regressors,
        dropped_regressors,
        explicit_dummy_columns,
        explicit_dummy_rank: dummy_qr.rank(),
        finite_regressor_rank,
        model_rank,
        effective_observations: validated.effective_observations,
        residual_degrees_of_freedom,
        finite_fitted_values,
        absorbed_fitted_values,
        fitted_values,
        residuals,
        weighted_rss,
        bread,
        iid_vce,
        hc1_vce,
        one_way_cluster_vces,
        clustered_vce,
        cluster_counts,
        certificate,
    })
}

struct ValidatedProblem {
    objective_weights: Vec<f64>,
    sqrt_weights: Vec<f64>,
    hc0_multipliers: Vec<f64>,
    effective_observations: usize,
}

fn validate_problem(
    problem: &ExplicitOlsProblem,
    config: OracleConfig,
) -> Result<ValidatedProblem, OracleError> {
    let nobs = problem.outcome.len();
    if nobs == 0 {
        return Err(OracleError::InvalidInput(
            "the outcome must contain at least one observation".to_owned(),
        ));
    }
    if !config.rank_tolerance_multiplier.is_finite()
        || config.rank_tolerance_multiplier <= 0.0
        || !config.certificate_tolerance.is_finite()
        || config.certificate_tolerance <= 0.0
    {
        return Err(OracleError::InvalidInput(
            "numerical tolerances must be finite and positive".to_owned(),
        ));
    }
    if problem.outcome.iter().any(|value| !value.is_finite()) {
        return Err(OracleError::InvalidInput(
            "outcomes must be finite".to_owned(),
        ));
    }
    if problem.regressors.iter().any(|column| column.len() != nobs) {
        return Err(OracleError::InvalidInput(
            "regressor and outcome lengths differ".to_owned(),
        ));
    }
    if problem
        .regressors
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(OracleError::InvalidInput(
            "regressors must be finite".to_owned(),
        ));
    }
    for effect in &problem.effects {
        if effect.codes.len() != nobs || effect.slopes.iter().any(|slope| slope.len() != nobs) {
            return Err(OracleError::InvalidInput(
                "absorbed-effect and outcome lengths differ".to_owned(),
            ));
        }
        if effect
            .codes
            .iter()
            .any(|&code| code as usize >= effect.n_levels)
        {
            return Err(OracleError::InvalidInput(
                "an absorbed-effect code exceeds its declared level count".to_owned(),
            ));
        }
        if effect
            .slopes
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(OracleError::InvalidInput(
                "absorbed slopes must be finite".to_owned(),
            ));
        }
    }
    if problem.clusters.len() > 2 {
        return Err(OracleError::InvalidInput(
            "the small-model oracle supports at most two cluster dimensions".to_owned(),
        ));
    }
    if problem.clusters.iter().any(|cluster| cluster.len() != nobs) {
        return Err(OracleError::InvalidInput(
            "cluster and outcome lengths differ".to_owned(),
        ));
    }

    let (objective_weights, hc0_multipliers, effective_observations) = match &problem.weights {
        OracleWeights::Unit => (vec![1.0; nobs], vec![1.0; nobs], nobs),
        OracleWeights::Wls(weights) => {
            validate_positive_weights(weights, nobs)?;
            (
                weights.clone(),
                weights.iter().map(|weight| weight * weight).collect(),
                nobs,
            )
        }
        OracleWeights::Frequency(frequencies) => {
            if frequencies.len() != nobs || frequencies.contains(&0) {
                return Err(OracleError::InvalidInput(
                    "frequency weights must be positive and match the outcome length".to_owned(),
                ));
            }
            let effective_observations = frequencies.iter().try_fold(0_usize, |sum, &frequency| {
                sum.checked_add(frequency as usize)
            });
            let effective_observations = effective_observations.ok_or_else(|| {
                OracleError::InvalidInput(
                    "frequency-weight observation count overflowed".to_owned(),
                )
            })?;
            let weights = frequencies
                .iter()
                .map(|&frequency| f64::from(frequency))
                .collect::<Vec<_>>();
            (weights.clone(), weights, effective_observations)
        }
    };
    if objective_weights.iter().any(|weight| !weight.is_finite())
        || hc0_multipliers
            .iter()
            .any(|multiplier| !multiplier.is_finite())
    {
        return Err(OracleError::InvalidInput(
            "weight transformation overflowed".to_owned(),
        ));
    }
    let sqrt_weights = objective_weights
        .iter()
        .map(|weight| weight.sqrt())
        .collect();
    Ok(ValidatedProblem {
        objective_weights,
        sqrt_weights,
        hc0_multipliers,
        effective_observations,
    })
}

fn validate_positive_weights(weights: &[f64], nobs: usize) -> Result<(), OracleError> {
    if weights.len() != nobs
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(OracleError::InvalidInput(
            "WLS weights must be finite, positive, and match the outcome length".to_owned(),
        ));
    }
    Ok(())
}

fn materialize_dummy_columns(problem: &ExplicitOlsProblem) -> Vec<Vec<f64>> {
    let nobs = problem.outcome.len();
    let capacity = problem
        .effects
        .iter()
        .map(|effect| effect.n_levels * (usize::from(effect.intercept) + effect.slopes.len()))
        .sum();
    let mut columns = Vec::with_capacity(capacity);
    for effect in &problem.effects {
        if effect.intercept {
            for level in 0..effect.n_levels {
                columns.push(
                    effect
                        .codes
                        .iter()
                        .map(|&code| f64::from((code as usize == level) as u8))
                        .collect(),
                );
            }
        }
        for slope in &effect.slopes {
            for level in 0..effect.n_levels {
                columns.push(
                    (0..nobs)
                        .map(|row| {
                            if effect.codes[row] as usize == level {
                                slope[row]
                            } else {
                                0.0
                            }
                        })
                        .collect(),
                );
            }
        }
    }
    columns
}

fn weight_columns(columns: &[Vec<f64>], sqrt_weights: &[f64]) -> Vec<Vec<f64>> {
    columns
        .iter()
        .map(|column| weight_column(column, sqrt_weights))
        .collect()
}

fn weight_column(column: &[f64], sqrt_weights: &[f64]) -> Vec<f64> {
    column
        .iter()
        .zip(sqrt_weights)
        .map(|(value, sqrt_weight)| value * sqrt_weight)
        .collect()
}

#[derive(Debug, Clone)]
struct Reflector {
    start: usize,
    direction: Vec<f64>,
}

#[derive(Debug, Clone)]
struct PivotedQr {
    columns: Vec<Vec<f64>>,
    permutation: Vec<usize>,
    scales: Vec<f64>,
    reflectors: Vec<Reflector>,
    rank: usize,
    nrows: usize,
}

impl PivotedQr {
    fn factor(
        mut columns: Vec<Vec<f64>>,
        nrows: usize,
        tolerance: f64,
    ) -> Result<Self, OracleError> {
        if columns.iter().any(|column| column.len() != nrows) {
            return Err(OracleError::InvalidInput(
                "QR columns have inconsistent lengths".to_owned(),
            ));
        }
        let mut scales = columns
            .iter()
            .map(|column| stable_norm(column.iter().copied()))
            .collect::<Vec<_>>();
        if scales.iter().any(|scale| !scale.is_finite()) {
            return Err(OracleError::NumericalFailure(
                "a design-column norm is not finite".to_owned(),
            ));
        }
        for (column, &scale) in columns.iter_mut().zip(&scales) {
            if scale != 0.0 {
                for value in column {
                    *value /= scale;
                }
            }
        }
        let mut permutation = (0..columns.len()).collect::<Vec<_>>();
        let mut reflectors = Vec::new();
        let mut rank = 0_usize;

        for step in 0..nrows.min(columns.len()) {
            let mut pivot = step;
            let mut pivot_norm = -1.0_f64;
            for (column, values) in columns.iter().enumerate().skip(step) {
                let norm = stable_norm(values[step..].iter().copied());
                if norm > pivot_norm {
                    pivot = column;
                    pivot_norm = norm;
                }
            }
            if pivot_norm <= tolerance {
                break;
            }
            columns.swap(step, pivot);
            permutation.swap(step, pivot);
            scales.swap(step, pivot);

            let alpha = if columns[step][step].is_sign_negative() {
                pivot_norm
            } else {
                -pivot_norm
            };
            let mut direction = columns[step][step..].to_vec();
            direction[0] -= alpha;
            let direction_norm = stable_norm(direction.iter().copied());
            if direction_norm <= tolerance || !direction_norm.is_finite() {
                return Err(OracleError::NumericalFailure(
                    "Householder direction is numerically unresolved".to_owned(),
                ));
            }
            for value in &mut direction {
                *value /= direction_norm;
            }
            for column in &mut columns[step..] {
                apply_reflector(&direction, &mut column[step..]);
            }
            reflectors.push(Reflector {
                start: step,
                direction,
            });
            rank += 1;
        }

        Ok(Self {
            columns,
            permutation,
            scales,
            reflectors,
            rank,
            nrows,
        })
    }

    const fn rank(&self) -> usize {
        self.rank
    }

    fn kept_original_indices(&self) -> &[usize] {
        &self.permutation[..self.rank]
    }

    fn transform(&self, values: &[f64]) -> Vec<f64> {
        let mut transformed = values.to_vec();
        for reflector in &self.reflectors {
            apply_reflector(&reflector.direction, &mut transformed[reflector.start..]);
        }
        transformed
    }

    fn inverse_transform(&self, values: &[f64]) -> Vec<f64> {
        let mut transformed = values.to_vec();
        for reflector in self.reflectors.iter().rev() {
            apply_reflector(&reflector.direction, &mut transformed[reflector.start..]);
        }
        transformed
    }

    fn residualize(&self, values: &[f64]) -> Vec<f64> {
        debug_assert_eq!(values.len(), self.nrows);
        let mut transformed = self.transform(values);
        transformed[..self.rank].fill(0.0);
        self.inverse_transform(&transformed)
    }

    fn solve(&self, right_hand_side: &[f64]) -> Result<Vec<f64>, OracleError> {
        if right_hand_side.len() != self.nrows {
            return Err(OracleError::InvalidInput(
                "QR right-hand-side length differs from the design".to_owned(),
            ));
        }
        let transformed = self.transform(right_hand_side);
        let mut scaled_solution = vec![0.0; self.rank];
        for row in (0..self.rank).rev() {
            let mut value = transformed[row];
            for (column, coefficient) in scaled_solution
                .iter()
                .enumerate()
                .take(self.rank)
                .skip(row + 1)
            {
                value = (-self.columns[column][row]).mul_add(*coefficient, value);
            }
            let diagonal = self.columns[row][row];
            if diagonal == 0.0 || !diagonal.is_finite() {
                return Err(OracleError::NumericalFailure(
                    "identified QR block is singular".to_owned(),
                ));
            }
            scaled_solution[row] = value / diagonal;
        }
        Ok(scaled_solution
            .iter()
            .zip(&self.scales)
            .map(|(coefficient, scale)| coefficient / scale)
            .collect())
    }

    fn bread(&self) -> Result<OracleMatrix, OracleError> {
        let mut inverse_r = OracleMatrix::zeros(self.rank, self.rank);
        for right_hand_side in 0..self.rank {
            for row in (0..self.rank).rev() {
                let mut value = f64::from(row == right_hand_side);
                for column in row + 1..self.rank {
                    value = (-self.columns[column][row])
                        .mul_add(inverse_r.data[column * self.rank + right_hand_side], value);
                }
                let diagonal = self.columns[row][row];
                if diagonal == 0.0 || !diagonal.is_finite() {
                    return Err(OracleError::NumericalFailure(
                        "identified QR block has no finite inverse".to_owned(),
                    ));
                }
                inverse_r.data[row * self.rank + right_hand_side] = value / diagonal;
            }
        }
        let mut bread = OracleMatrix::zeros(self.rank, self.rank);
        for row in 0..self.rank {
            for column in 0..self.rank {
                let value = (0..self.rank).fold(0.0, |sum, inner| {
                    inverse_r.data[row * self.rank + inner]
                        .mul_add(inverse_r.data[column * self.rank + inner], sum)
                });
                bread.data[row * self.rank + column] =
                    value / (self.scales[row] * self.scales[column]);
            }
        }
        Ok(bread)
    }
}

fn apply_reflector(direction: &[f64], values: &mut [f64]) {
    let inner = direction
        .iter()
        .zip(values.iter())
        .fold(0.0, |sum, (&left, &right)| left.mul_add(right, sum));
    for (&component, value) in direction.iter().zip(values) {
        *value = (-2.0 * inner).mul_add(component, *value);
    }
}

fn stable_norm(values: impl Iterator<Item = f64>) -> f64 {
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            scaled_square_sum = 1.0 + scaled_square_sum * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            scaled_square_sum += ratio * ratio;
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * scaled_square_sum.sqrt()
    }
}

fn reorder_matrix(matrix: &OracleMatrix, old_order: &[usize], new_order: &[usize]) -> OracleMatrix {
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

fn cross_product(columns: &[Vec<f64>], weights: &[f64]) -> OracleMatrix {
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

fn observation_meat(columns: &[Vec<f64>], residuals: &[f64], multipliers: &[f64]) -> OracleMatrix {
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

fn sandwich(bread: &OracleMatrix, meat: &OracleMatrix) -> OracleMatrix {
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

fn scale_matrix(matrix: &OracleMatrix, scale: f64) -> OracleMatrix {
    OracleMatrix {
        nrows: matrix.nrows,
        ncols: matrix.ncols,
        data: matrix.data.iter().map(|value| value * scale).collect(),
    }
}

fn add_subtract_matrices(
    first: &OracleMatrix,
    second: &OracleMatrix,
    subtract: &OracleMatrix,
) -> OracleMatrix {
    let mut result = OracleMatrix {
        nrows: first.nrows,
        ncols: first.ncols,
        data: first
            .data
            .iter()
            .zip(&second.data)
            .zip(&subtract.data)
            .map(|((left, right), minus)| left + right - minus)
            .collect(),
    };
    symmetrize(&mut result);
    result
}

fn symmetrize(matrix: &mut OracleMatrix) {
    for row in 0..matrix.nrows {
        for column in 0..row {
            let average = 0.5 * (matrix[(row, column)] + matrix[(column, row)]);
            matrix.data[row * matrix.ncols + column] = average;
            matrix.data[column * matrix.ncols + row] = average;
        }
    }
}

fn clustered_component(
    bread: &OracleMatrix,
    columns: &[Vec<f64>],
    residuals: &[f64],
    objective_weights: &[f64],
    keys: impl Iterator<Item = (u64, u64)>,
    effective_observations: usize,
    model_rank: usize,
) -> Result<(OracleMatrix, usize), OracleError> {
    let mut group_scores: BTreeMap<(u64, u64), Vec<f64>> = BTreeMap::new();
    for (observation, key) in keys.enumerate() {
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
    let correction = group_count as f64 / (group_count - 1) as f64
        * (effective_observations - 1) as f64
        / (effective_observations - model_rank) as f64;
    Ok((
        scale_matrix(&sandwich(bread, &meat), correction),
        group_count,
    ))
}

#[allow(clippy::too_many_arguments)]
fn certify_result(
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

fn validate_certificate(certificate: OracleCertificate, tolerance: f64) -> Result<(), OracleError> {
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
fn validate_finite_outputs(
    coefficients: &[Option<f64>],
    fitted_values: &[f64],
    residuals: &[f64],
    bread: &OracleMatrix,
    iid_vce: &OracleMatrix,
    hc1_vce: &OracleMatrix,
    one_way_cluster_vces: &[OracleMatrix],
    clustered_vce: Option<&OracleMatrix>,
) -> Result<(), OracleError> {
    let finite = coefficients.iter().flatten().all(|value| value.is_finite())
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
            .all(|value| value.is_finite());
    if !finite {
        return Err(OracleError::NumericalFailure(
            "a coefficient, fitted value, residual, bread, or VCE is not finite".to_owned(),
        ));
    }
    Ok(())
}
