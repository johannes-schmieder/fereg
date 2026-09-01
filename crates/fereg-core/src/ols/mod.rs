//! Certified Frisch-Waugh-Lovell OLS core.

mod dof;
mod finite_wls_kernel;
mod fit;
mod linear_session;
mod prepared;
mod prepared_categorical_clusters;
mod prepared_clustered;
mod prepared_clustered_view;
mod prepared_clusters;
mod prepared_dof;
mod prepared_fit;
mod prepared_inference_view;
mod prepared_rhs;
mod prepared_robust;
mod prepared_scores;
mod prepared_scratch;
mod prepared_unadjusted;
mod rank;
mod vce;

pub use dof::{compute_dof, DofConfig, DofMethod, DofReport};
pub use fit::FitStatistics;
#[cfg(test)]
pub(crate) use linear_session::{
    begin_frozen_qr_fallback_observation, finish_frozen_qr_fallback_observation,
    inject_frozen_linear_failure, FrozenLinearFailureInjection,
};
#[allow(unused_imports)]
pub(crate) use linear_session::{
    classify_frozen_linear_fallback, CertifiedFrozenLinearOutput, CertifiedLinearSession,
    CertifiedLinearSessionOutput, CertifiedLinearSessionProducts, CertifiedLinearWithinDesign,
    FrozenLinearBasis, FrozenLinearBasisSlot, FrozenLinearFallbackKind,
    FrozenLinearSolveCertificate, LinearReconstructionCertificate, LinearSessionGenerations,
    LinearSessionPhaseAllocationObservation, LinearSessionReuseReport, LinearSessionSolveKind,
    LinearSessionWorkspaceCapacities, LinearWithinDesignCertificate, LinearWithinMemoryReport,
};
#[doc(hidden)]
pub use linear_session::{
    LinearSessionAllocationPhase, LinearSessionPhaseAllocationEvent,
    LinearSessionPhaseAllocatorProbe,
};
pub(crate) use prepared::prepare_ols_data_with_cluster_sources;
pub use prepared::{
    prepare_ols_data, prepare_ols_data_owned, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedCategoricalSource, PreparedOlsCategoricalComponent,
    PreparedOlsCoefficient, PreparedOlsCoefficientStatus, PreparedOlsColumn, PreparedOlsColumnKind,
    PreparedOlsData, PreparedOlsEffect, PreparedOlsFeSingletonPolicy, PreparedOlsFeTopology,
    PreparedOlsFeTopologySource, PreparedOlsModel, PreparedOlsSingletons,
};
pub(crate) use prepared_categorical_clusters::prepare_ppml_categorical_ols_clustered_data;
pub use prepared_categorical_clusters::{
    prepare_categorical_ols_clustered_data, PreparedCategoricalOlsClusteredData,
};
pub use prepared_clustered::{
    fit_prepared_categorical_ols_clustered_map, fit_prepared_ols_clustered_map,
    CertifiedPreparedCategoricalOlsClustered, CertifiedPreparedOlsClustered,
    PreparedOlsClusterSubsetCertificate, PreparedOlsClusteredCertificate,
    PreparedOlsClusteredConfig, PreparedOlsClusteredCovariance, PreparedOlsClusteredDofReport,
    PreparedOlsClusteredPsdReport, PreparedOlsClusteredPsdState,
};
pub use prepared_clusters::{
    prepare_ols_clustered_data, OlsClusterTermV1, OlsClusteredRequestV1, PreparedOlsClusterDesign,
    PreparedOlsClusterDimension, PreparedOlsClusteredData,
};
pub use prepared_dof::{
    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffect,
    PreparedOlsAbsorbedDofEffectIdentity, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsAbsorbedDofReport,
};
pub use prepared_fit::{
    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_coefficients_map,
    CertifiedPreparedOlsFit, PreparedOlsBread, PreparedOlsBreadCertificate,
    PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind, PreparedOlsBreadWeightScale,
    PreparedOlsExecutionConfig, PreparedOlsExecutionMode, PreparedOlsExecutionReport,
    PreparedOlsFitConfig, PreparedOlsFitStatus, PreparedOlsFittedCoefficient,
    PreparedOlsRankCertificate, PreparedOlsScoreCertificate,
};
#[allow(unused_imports)]
pub(crate) use prepared_fit::{
    fit_prepared_ols_coefficients_view_map_with_frame,
    fit_prepared_weighted_linear_projection_once, CertifiedPreparedLinearProjection,
    LinearSolveFrame, PreparedLinearProjectionReconstructionCertificate,
};
pub(crate) use prepared_rhs::build_prepared_ols_rhs_with_response;
pub use prepared_rhs::{absorb_prepared_ols_rhs_map, build_prepared_ols_rhs, PreparedOlsRhs};
pub use prepared_robust::{
    fit_prepared_categorical_ols_robust_map, fit_prepared_ols_robust_map,
    CertifiedPreparedCategoricalOlsRobust, CertifiedPreparedOlsRobust,
    PreparedOlsRobustCertificate, PreparedOlsRobustConfig, PreparedOlsRobustCovariance,
};
pub use prepared_unadjusted::{
    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_unadjusted_map,
    CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsUnadjusted,
    PreparedOlsFitStatistics, PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport,
    PreparedOlsUnadjustedCertificate, PreparedOlsUnadjustedConfig, PreparedOlsUnadjustedCovariance,
};
pub use rank::{solve_rank_revealing, FiniteSolver, RankSolve};
pub use vce::{ClusterSubsetReport, VceKind, VceReport};

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{absorb_fixed_effects, AbsorbConfig, FeDesign, SolveReport};
use dof::compute_dof as calculate_dof;
use fit::compute_fit_statistics;
use rank::solve_rank_revealing as solve_finite;
use vce::{compute_vce, VceInput};

/// Status of one canonical finite-dimensional model column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressorStatus {
    Kept,
    BaseLevel,
    SyntaxOmitted,
    CollinearWithFixedEffects,
    CollinearAfterResidualization,
    NumericallyRankDeficient,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OlsConfig {
    pub absorb: AbsorbConfig,
    pub finite_solver: FiniteSolver,
    pub rank_tolerance: f64,
    pub fixed_effect_collinearity_tolerance: f64,
    pub vce: VceKind,
    pub small_sample_correction: bool,
    pub dof: DofConfig,
}

impl Default for OlsConfig {
    fn default() -> Self {
        Self {
            absorb: AbsorbConfig::default(),
            finite_solver: FiniteSolver::Auto,
            rank_tolerance: 1e-10,
            fixed_effect_collinearity_tolerance: 1e-10,
            vce: VceKind::Unadjusted,
            small_sample_correction: true,
            dof: DofConfig::default(),
        }
    }
}

/// Complete Stata-independent result of the OLS scientific core.
#[derive(Debug, Clone, PartialEq)]
pub struct OlsResult {
    pub coefficients: Vec<f64>,
    pub covariance: Vec<f64>,
    pub standard_errors: Vec<f64>,
    pub statuses: Vec<RegressorStatus>,
    pub kept: Vec<usize>,
    /// Rank diagnostics on the post-FE candidate-column coordinate system.
    pub finite_solve: RankSolve,
    pub absorb_report: Option<SolveReport>,
    pub dof: DofReport,
    pub fit: FitStatistics,
    pub vce: VceReport,
    pub residuals: Vec<f64>,
    pub fitted: Vec<f64>,
    pub within_outcome: Vec<f64>,
    pub within_fitted: Vec<f64>,
}

/// Fit weighted OLS with optional high-dimensional fixed effects.
///
/// `weights` are effective least-squares weights. Weight-type-specific Stata
/// semantics are validated and converted before entering this scientific core.
pub fn fit_ols(
    outcome: &[f64],
    regressors: &DenseMatrix,
    weights: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    config: OlsConfig,
) -> Result<OlsResult> {
    validate_inputs(
        outcome,
        regressors,
        weights,
        fixed_effects,
        clusters,
        config,
    )?;
    let nrows = regressors.nrows();
    let ncols = regressors.ncols();
    let original_norms = weighted_regressor_norms(regressors, weights);

    let mut joint_columns = Vec::with_capacity(ncols + 1);
    joint_columns.push(outcome.to_vec());
    for column in 0..ncols {
        joint_columns.push(regressors.column(column)?);
    }
    let mut transformed = DenseMatrix::from_columns(&joint_columns)?;
    let absorb_report = if let Some(design) = fixed_effects {
        if design.effects().is_empty() {
            None
        } else {
            let report = absorb_fixed_effects(design, weights, &mut transformed, config.absorb)?;
            if !report.certified() {
                return Err(FeregError::new(
                    ErrorCode::InvalidData,
                    format!(
                        "fixed-effect absorption did not certify: {:?}",
                        report.stop_reason
                    ),
                ));
            }
            Some(report)
        }
    } else {
        None
    };

    let within_outcome = transformed.column(0)?;
    let mut statuses = vec![RegressorStatus::Kept; ncols];
    let mut candidate_indices = Vec::new();
    let mut candidate_columns = Vec::new();
    for column in 0..ncols {
        let transformed_column = transformed.column(column + 1)?;
        let transformed_norm = weighted_vector_norm(&transformed_column, weights);
        let scale = original_norms[column].max(1.0);
        if transformed_norm <= config.fixed_effect_collinearity_tolerance * scale {
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
    let finite_solve = solve_finite(
        &candidate_matrix,
        &within_outcome,
        weights,
        config.rank_tolerance,
        config.finite_solver,
    )?;

    let mut coefficients = vec![0.0_f64; ncols];
    let mut full_bread = vec![0.0_f64; ncols * ncols];
    let mut kept = Vec::with_capacity(finite_solve.rank);
    for &candidate in &finite_solve.kept {
        let original = candidate_indices[candidate];
        statuses[original] = RegressorStatus::Kept;
        coefficients[original] = finite_solve.coefficients[candidate];
        kept.push(original);
    }
    for &candidate in &finite_solve.dropped {
        statuses[candidate_indices[candidate]] = RegressorStatus::NumericallyRankDeficient;
    }
    for (candidate_row, &original_row) in candidate_indices.iter().enumerate() {
        for (candidate_column, &original_column) in candidate_indices.iter().enumerate() {
            full_bread[original_row * ncols + original_column] =
                finite_solve.bread[candidate_row * candidate_indices.len() + candidate_column];
        }
    }

    let mut residuals = within_outcome.clone();
    let mut within_fitted = vec![0.0_f64; nrows];
    for row in 0..nrows {
        let mut fitted_value = 0.0_f64;
        for &column in &kept {
            fitted_value =
                transformed[(row, column + 1)].mul_add(coefficients[column], fitted_value);
        }
        within_fitted[row] = fitted_value;
        residuals[row] -= fitted_value;
    }
    let fitted = outcome
        .iter()
        .zip(&residuals)
        .map(|(value, residual)| value - residual)
        .collect::<Vec<_>>();
    let dof = calculate_dof(nrows, finite_solve.rank, fixed_effects, config.dof)?;
    let fit = compute_fit_statistics(outcome, &within_outcome, &residuals, weights, dof.residual);
    let transformed_regressors = matrix_without_first_column(&transformed)?;
    let vce = compute_vce(VceInput {
        regressors: &transformed_regressors,
        residuals: &residuals,
        weights,
        bread: &full_bread,
        kept: &kept,
        residual_dof: dof.residual,
        clusters,
        kind: config.vce,
        small_sample: config.small_sample_correction,
    })?;

    Ok(OlsResult {
        coefficients,
        covariance: vce.covariance.clone(),
        standard_errors: vce.standard_errors.clone(),
        statuses,
        kept,
        finite_solve,
        absorb_report,
        dof,
        fit,
        vce,
        residuals,
        fitted,
        within_outcome,
        within_fitted,
    })
}

fn validate_inputs(
    outcome: &[f64],
    regressors: &DenseMatrix,
    weights: &[f64],
    fixed_effects: Option<&FeDesign>,
    clusters: &[Vec<u32>],
    config: OlsConfig,
) -> Result<()> {
    if outcome.is_empty() {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "OLS requires at least one observation",
        ));
    }
    if regressors.nrows() != outcome.len() || weights.len() != outcome.len() {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "OLS inputs have inconsistent observation counts",
        ));
    }
    if let Some(design) = fixed_effects {
        if design.nobs() != outcome.len() {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "fixed-effect design has a different observation count",
            ));
        }
    }
    if clusters
        .iter()
        .any(|cluster| cluster.len() != outcome.len())
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "cluster columns have inconsistent observation counts",
        ));
    }
    if outcome.iter().any(|value| !value.is_finite())
        || regressors.as_slice().iter().any(|value| !value.is_finite())
        || weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return Err(FeregError::new(
            ErrorCode::InvalidData,
            "OLS values and weights must be finite and weights positive",
        ));
    }
    if !config.rank_tolerance.is_finite()
        || config.rank_tolerance <= 0.0
        || config.rank_tolerance >= 1.0
        || !config.fixed_effect_collinearity_tolerance.is_finite()
        || config.fixed_effect_collinearity_tolerance <= 0.0
        || config.fixed_effect_collinearity_tolerance >= 1.0
    {
        return Err(FeregError::new(
            ErrorCode::InvalidArgument,
            "OLS numerical tolerances must lie strictly between zero and one",
        ));
    }
    Ok(())
}

fn weighted_regressor_norms(regressors: &DenseMatrix, weights: &[f64]) -> Vec<f64> {
    (0..regressors.ncols())
        .map(|column| {
            let mut square_sum = 0.0_f64;
            for row in 0..regressors.nrows() {
                let value = regressors[(row, column)];
                square_sum = (weights[row] * value).mul_add(value, square_sum);
            }
            square_sum.sqrt()
        })
        .collect()
}

fn weighted_vector_norm(values: &[f64], weights: &[f64]) -> f64 {
    values
        .iter()
        .zip(weights)
        .fold(0.0_f64, |sum, (value, weight)| {
            (*weight * *value).mul_add(*value, sum)
        })
        .sqrt()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{EffectTerm, FeSolverKind};

    #[test]
    fn fits_weighted_line_without_fixed_effects() {
        let x = DenseMatrix::from_columns(&[vec![1.0; 6], vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]])
            .expect("matrix");
        let y = vec![1.0, 3.0, 5.0, 7.0, 9.0, 11.0];
        let result = fit_ols(
            &y,
            &x,
            &[1.0, 2.0, 1.0, 3.0, 1.0, 2.0],
            None,
            &[],
            OlsConfig::default(),
        )
        .expect("fit");
        assert!((result.coefficients[0] - 1.0).abs() < 1e-9);
        assert!((result.coefficients[1] - 2.0).abs() < 1e-9);
        assert!(result.residuals.iter().all(|value| value.abs() < 1e-9));
    }

    #[test]
    fn fwl_recovers_slope_with_two_way_fixed_effects() {
        let worker_codes = vec![0, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let firm_codes = vec![0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0];
        let x_values = vec![0.0, 1.0, 1.5, 0.5, 2.0, 3.0, 2.5, 4.0, 1.2, 2.2, 3.2, 4.2];
        let worker_effect = [1.0, -0.5, 2.0, 0.25];
        let firm_effect = [0.75, -1.25];
        let y = x_values
            .iter()
            .enumerate()
            .map(|(row, value)| {
                2.0 * value
                    + worker_effect[worker_codes[row] as usize]
                    + firm_effect[firm_codes[row] as usize]
            })
            .collect::<Vec<_>>();
        let x = DenseMatrix::from_columns(&[vec![1.0; y.len()], x_values]).expect("matrix");
        let worker = EffectTerm::new(worker_codes, 4, true, vec![]).expect("worker");
        let firm = EffectTerm::new(firm_codes, 2, true, vec![]).expect("firm");
        let design = FeDesign::new(y.len(), vec![worker, firm]).expect("design");
        let result = fit_ols(
            &y,
            &x,
            &vec![1.0; y.len()],
            Some(&design),
            &[],
            OlsConfig::default(),
        )
        .expect("fit");
        assert_eq!(
            result.statuses[0],
            RegressorStatus::CollinearWithFixedEffects
        );
        assert!((result.coefficients[1] - 2.0).abs() < 1e-7);
        assert!(result.residuals.iter().all(|value| value.abs() < 1e-7));
        assert!(result
            .absorb_report
            .as_ref()
            .is_some_and(SolveReport::certified));
    }

    #[test]
    fn map_and_lsmr_agree_on_identified_slope() {
        let first =
            EffectTerm::new(vec![0, 0, 1, 1, 2, 2, 0, 1, 2], 3, true, vec![]).expect("first");
        let second =
            EffectTerm::new(vec![0, 1, 0, 1, 0, 1, 1, 1, 0], 2, true, vec![]).expect("second");
        let design = FeDesign::new(9, vec![first, second]).expect("design");
        let x_values = vec![0.2, 1.0, 1.5, 0.3, 2.2, 2.8, 0.7, 1.9, 2.5];
        let y = x_values
            .iter()
            .enumerate()
            .map(|(row, value)| 1.5 * value + (row % 3) as f64 - (row % 2) as f64)
            .collect::<Vec<_>>();
        let x = DenseMatrix::from_columns(&[x_values]).expect("matrix");
        let mut map_config = OlsConfig::default();
        map_config.absorb.solver = FeSolverKind::Map;
        let mut lsmr_config = OlsConfig::default();
        lsmr_config.absorb.solver = FeSolverKind::Lsmr;
        let map = fit_ols(&y, &x, &[1.0; 9], Some(&design), &[], map_config).expect("map");
        let lsmr = fit_ols(&y, &x, &[1.0; 9], Some(&design), &[], lsmr_config).expect("lsmr");
        assert!((map.coefficients[0] - lsmr.coefficients[0]).abs() < 1e-7);
        for (left, right) in map.fitted.iter().zip(lsmr.fitted) {
            assert!((*left - right).abs() < 1e-7);
        }
    }

    #[test]
    fn exact_duplicate_is_reported_as_rank_deficient() {
        let x = DenseMatrix::from_columns(&[
            vec![1.0; 6],
            vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0],
        ])
        .expect("matrix");
        let y = vec![1.0, 3.0, 5.0, 7.0, 9.0, 11.0];
        let result = fit_ols(&y, &x, &[1.0; 6], None, &[], OlsConfig::default()).expect("fit");
        assert_eq!(result.kept.len(), 2);
        assert_eq!(
            result
                .statuses
                .iter()
                .filter(|status| **status == RegressorStatus::NumericallyRankDeficient)
                .count(),
            1
        );
    }
}
