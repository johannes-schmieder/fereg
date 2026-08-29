use std::cmp::Ordering;

use crate::error::{ErrorCode, FeregError, Result};
use crate::parallel::ThreadTeam;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::topology_dof::CategoricalTopologyDofReport;
use rayon::prelude::*;

use super::prepared_clustered_view::PreparedOlsClusteredView;
#[cfg(test)]
use super::prepared_fit::fit_prepared_ols_coefficients_view_map;
use super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference;
use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView};
use super::prepared_scores::{
    build_prepared_ols_score_design, reserved_filled, CertifiedPreparedOlsScoreDesign,
    PreparedInferenceReservation, SystemPreparedInferenceReservation,
};
use super::prepared_unadjusted::{
    build_reference_weight_report_view, build_residual_dof_view, build_statistics, checked_square,
    compensated_add, exact_integer_as_f64, relative_difference, validate_sum_of_squares,
    weighted_mean, weighted_sum_of_squares, ReferenceWeightView,
};
use super::{
    CertifiedPreparedOlsFit, PreparedCategoricalOlsClusteredData, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind,
    PreparedOlsClusteredData, PreparedOlsColumnKind, PreparedOlsFitConfig,
    PreparedOlsFitStatistics, PreparedOlsFitStatus, PreparedOlsReferenceWeightReport,
    PreparedOlsResidualDofReport,
};

const CLUSTER_REDUCTION_MAX_BLOCKS: usize = 64;
const CLUSTER_PARTIAL_BUDGET_BYTES: usize = 32 << 20;

/// Numerical and structural policy for the core-only clustered prepared OLS
/// checkpoint. Cluster nesting is independent of the mobility policy so
/// `dof(none)` can be represented without silently disabling nesting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsClusteredConfig {
    pub fit: PreparedOlsFitConfig,
    pub absorbed_dof: PreparedOlsAbsorbedDofPolicy,
    pub account_cluster_nesting: bool,
    pub certificate_tolerance: f64,
}

impl Default for PreparedOlsClusteredConfig {
    fn default() -> Self {
        Self {
            fit: PreparedOlsFitConfig::default(),
            absorbed_dof: PreparedOlsAbsorbedDofPolicy::default(),
            account_cluster_nesting: true,
            certificate_tolerance: 5e-8,
        }
    }
}

/// The three deliberately distinct residual-DoF quantities required by the
/// pinned clustered reference contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsClusteredDofReport {
    pub finite_rank: usize,
    pub finite_model_dof: usize,
    pub absorbed_rank: usize,
    pub nested_absorbed_categories: usize,
    pub unclustered_residual_dof: usize,
    pub reported_residual_dof: usize,
    pub fit_residual_dof: usize,
    pub correction_denominator_dof: usize,
    pub minimum_cluster_count: usize,
    pub has_structural_intercept: bool,
    pub has_normalized_absorbed_intercept: bool,
    pub absorbed_rank_is_approximate: bool,
}

/// One nonempty inclusion--exclusion subset in deterministic order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedOlsClusterSubsetCertificate {
    pub mask: u16,
    pub dimensions: usize,
    pub sign: i8,
    pub groups: usize,
}

/// Cameron--Gelbach--Miller repair diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedOlsClusteredPsdState {
    /// Q=1: no CGM transform is permitted and final PSD is strict.
    OneWayStrict,
    /// Q>1 raw full covariance was already PSD; no transform was applied.
    MultiwayRawPsd,
    /// Q>1 full covariance was clipped and remains the final covariance.
    MultiwayFullClipped,
    /// Q>1 full covariance was clipped, then the independently repaired
    /// original raw slope block was restored around a reported constant.
    MultiwayFullClippedSlopeRestored,
}

/// Cameron--Gelbach--Miller repair diagnostics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsClusteredPsdReport {
    pub state: PreparedOlsClusteredPsdState,
    pub full_adjusted: bool,
    pub slope_block_restored: bool,
    pub slope_block_adjusted: bool,
    pub full_clipped_eigenvalues: usize,
    pub slope_clipped_eigenvalues: usize,
    pub minimum_raw_full_eigenvalue: f64,
    pub minimum_pre_restore_full_eigenvalue: f64,
    pub minimum_raw_slope_eigenvalue: Option<f64>,
    pub minimum_restored_slope_eigenvalue: Option<f64>,
    /// Diagnostic-only in the exact `MultiwayFullClippedSlopeRestored` state.
    pub minimum_final_full_eigenvalue: f64,
    pub pre_restore_full_rank: usize,
    pub restored_slope_rank: Option<usize>,
    pub strict_final_full_psd: bool,
    pub transform_reconstruction_relative_error: f64,
    pub eigensystem_relative_error: f64,
    pub orthogonality_relative_error: f64,
}

/// Cluster covariance on the canonical reported bread coordinate.
#[derive(Debug, PartialEq)]
pub struct PreparedOlsClusteredCovariance {
    correction: f64,
    raw_values: Vec<f64>,
    values: Vec<f64>,
    standard_errors: Vec<f64>,
    psd: PreparedOlsClusteredPsdReport,
}

impl PreparedOlsClusteredCovariance {
    #[must_use]
    pub const fn correction(&self) -> f64 {
        self.correction
    }

    /// Corrected inclusion--exclusion sandwich before any multiway CGM repair.
    #[must_use]
    pub fn raw_values(&self) -> &[f64] {
        &self.raw_values
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    #[must_use]
    pub fn standard_errors(&self) -> &[f64] {
        &self.standard_errors
    }

    #[must_use]
    pub fn dimension(&self) -> usize {
        self.standard_errors.len()
    }

    #[must_use]
    pub const fn psd(&self) -> PreparedOlsClusteredPsdReport {
        self.psd
    }
}

/// Independent composition and numerical certificate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedOlsClusteredCertificate {
    pub reference_weight_sum_relative_error: f64,
    pub score_design_left_inverse_relative_infinity_norm: f64,
    pub score_design_right_inverse_relative_infinity_norm: f64,
    pub intercept_coordinate_relative_infinity_norm: f64,
    pub correction_relative_error: f64,
    pub independent_influence_relative_infinity_norm: f64,
    pub corrected_covariance_relative_infinity_norm: f64,
    pub covariance_symmetry_relative_infinity_norm: f64,
    pub standard_error_relative_infinity_norm: f64,
    pub pre_restore_full_psd_relative_error: f64,
    pub restored_slope_psd_relative_error: f64,
    pub final_structure_relative_error: f64,
    pub wald_solve_relative_infinity_norm: f64,
    pub statistic_formula_relative_infinity_norm: f64,
    pub passed: bool,
}

/// Same-call fit, cluster-aware absorbed DoF, multiway covariance, statistics,
/// and independent certificates. No observation-scale inference matrix or
/// subset ordering survives in this returned value.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedOlsClustered {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsClusteredDofReport,
    cluster_counts: Vec<usize>,
    subsets: Vec<PreparedOlsClusterSubsetCertificate>,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsClusteredCovariance,
    certificate: PreparedOlsClusteredCertificate,
}

impl CertifiedPreparedOlsClustered {
    #[must_use]
    pub const fn fit(&self) -> &CertifiedPreparedOlsFit {
        &self.fit
    }

    #[must_use]
    pub const fn absorbed_dof(&self) -> &PreparedOlsAbsorbedDofReport {
        &self.absorbed_dof
    }

    #[must_use]
    pub const fn weights(&self) -> PreparedOlsReferenceWeightReport {
        self.weights
    }

    #[must_use]
    pub const fn dof(&self) -> PreparedOlsClusteredDofReport {
        self.dof
    }

    #[must_use]
    pub fn cluster_counts(&self) -> &[usize] {
        &self.cluster_counts
    }

    #[must_use]
    pub fn subsets(&self) -> &[PreparedOlsClusterSubsetCertificate] {
        &self.subsets
    }

    #[must_use]
    pub const fn statistics(&self) -> PreparedOlsFitStatistics {
        self.statistics
    }

    #[must_use]
    pub const fn covariance(&self) -> &PreparedOlsClusteredCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsClusteredCertificate {
        self.certificate
    }
}

/// Categorical-interaction same-call fit, cluster-aware absorbed DoF,
/// multiway covariance, statistics, and independent certificates.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedCategoricalOlsClustered {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsClusteredDofReport,
    cluster_counts: Vec<usize>,
    subsets: Vec<PreparedOlsClusterSubsetCertificate>,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsClusteredCovariance,
    certificate: PreparedOlsClusteredCertificate,
}

impl CertifiedPreparedCategoricalOlsClustered {
    #[must_use]
    pub const fn fit(&self) -> &CertifiedPreparedOlsFit {
        &self.fit
    }

    #[must_use]
    pub const fn absorbed_dof(&self) -> &CategoricalTopologyDofReport {
        &self.absorbed_dof
    }

    #[must_use]
    pub const fn weights(&self) -> PreparedOlsReferenceWeightReport {
        self.weights
    }

    #[must_use]
    pub const fn dof(&self) -> PreparedOlsClusteredDofReport {
        self.dof
    }

    #[must_use]
    pub fn cluster_counts(&self) -> &[usize] {
        &self.cluster_counts
    }

    #[must_use]
    pub fn subsets(&self) -> &[PreparedOlsClusterSubsetCertificate] {
        &self.subsets
    }

    #[must_use]
    pub const fn statistics(&self) -> PreparedOlsFitStatistics {
        self.statistics
    }

    #[must_use]
    pub const fn covariance(&self) -> &PreparedOlsClusteredCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsClusteredCertificate {
        self.certificate
    }
}

#[derive(Debug, PartialEq)]
struct PreparedOlsClusteredComponents<A> {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: A,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsClusteredDofReport,
    cluster_counts: Vec<usize>,
    subsets: Vec<PreparedOlsClusterSubsetCertificate>,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsClusteredCovariance,
    certificate: PreparedOlsClusteredCertificate,
}

#[derive(Debug)]
struct RawClusterCovariance {
    dimension: usize,
    uncorrected: Vec<f64>,
    independent_uncorrected: Vec<f64>,
    corrected: Vec<f64>,
    correction: f64,
    subsets: Vec<PreparedOlsClusterSubsetCertificate>,
    independent_subsets: Vec<PreparedOlsClusterSubsetCertificate>,
}

#[derive(Debug)]
struct SubsetMeats {
    raw: Vec<f64>,
    influence: Vec<f64>,
    groups: usize,
}

#[derive(Debug, Clone, Copy)]
struct DenseSubsetPlan {
    radices: [usize; 10],
    group_slots: usize,
}

#[derive(Debug)]
struct SymmetricEigensystem {
    values: Vec<f64>,
    vectors: Vec<f64>,
    reconstruction_error: f64,
    orthogonality_error: f64,
}

#[derive(Debug, Clone, Copy)]
struct WaldCalculation {
    statistic: Option<f64>,
    solve_relative_infinity_norm: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct ClusteredPerturbations {
    score_design: bool,
    residual_dof: bool,
    subset_sign: bool,
    subset_groups: bool,
    uncorrected_covariance: bool,
    corrected_covariance: bool,
    correction: bool,
    final_covariance: bool,
    covariance_symmetry: bool,
    standard_error: bool,
    psd_state: bool,
    rss: bool,
    f_statistic: bool,
}

/// Fit and certify one-/multiway clustered covariance through the prepared
/// strict-MAP path. This entry point owns the complete scientific composition.
pub fn fit_prepared_ols_clustered_map(
    prepared: &PreparedOlsClusteredData,
    config: PreparedOlsClusteredConfig,
) -> Result<CertifiedPreparedOlsClustered> {
    ThreadTeam::new(config.fit.execution.threads).install(|| {
        let mut reservation = SystemPreparedInferenceReservation;
        fit_with_reservation(prepared, config, &mut reservation)
    })?
}

/// Fit and certify categorical-interaction OLS/WLS with one-/multiway
/// clustered covariance through the same numerical pipeline.
pub fn fit_prepared_categorical_ols_clustered_map(
    prepared: &PreparedCategoricalOlsClusteredData,
    config: PreparedOlsClusteredConfig,
) -> Result<CertifiedPreparedCategoricalOlsClustered> {
    let components = ThreadTeam::new(config.fit.execution.threads).install(|| {
        let mut reservation = SystemPreparedInferenceReservation;
        fit_clustered_view_with_reservation(prepared, config, &mut reservation)
    })??;
    Ok(CertifiedPreparedCategoricalOlsClustered {
        fit: components.fit,
        absorbed_dof: components.absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        cluster_counts: components.cluster_counts,
        subsets: components.subsets,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn fit_with_reservation<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsClusteredData,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
) -> Result<CertifiedPreparedOlsClustered> {
    let components = fit_clustered_view_with_reservation(prepared, config, reservation)?;
    Ok(CertifiedPreparedOlsClustered {
        fit: components.fit,
        absorbed_dof: components.absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        cluster_counts: components.cluster_counts,
        subsets: components.subsets,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn fit_clustered_view_with_reservation<
    P: PreparedOlsClusteredView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &P,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
) -> Result<PreparedOlsClusteredComponents<<P::Ols as PreparedOlsInferenceView>::AbsorbedDofReport>>
{
    validate_config(prepared, config)?;
    let fit =
        fit_prepared_ols_coefficients_view_map_for_inference(prepared.clustered_ols(), config.fit)?;
    compose_clustered_view(
        prepared,
        fit,
        config,
        reservation,
        ClusteredPerturbations::default(),
    )
}

#[cfg(test)]
fn compose<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsClusteredData,
    fit: CertifiedPreparedOlsFit,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
) -> Result<CertifiedPreparedOlsClustered> {
    let components = compose_clustered_view(prepared, fit, config, reservation, perturbations)?;
    Ok(CertifiedPreparedOlsClustered {
        fit: components.fit,
        absorbed_dof: components.absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        cluster_counts: components.cluster_counts,
        subsets: components.subsets,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn compose_clustered_view<P: PreparedOlsClusteredView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    mut fit: CertifiedPreparedOlsFit,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
) -> Result<PreparedOlsClusteredComponents<<P::Ols as PreparedOlsInferenceView>::AbsorbedDofReport>>
{
    let ols = prepared.clustered_ols();
    let dimensions = borrow_cluster_dimensions(prepared, reservation)?;
    let nesting_dimensions = if config.account_cluster_nesting {
        dimensions.as_slice()
    } else {
        &[]
    };
    let absorbed_dof =
        ols.inference_compute_clustered_absorbed_dof(config.absorbed_dof, nesting_dimensions)?;
    let absorbed_summary =
        ols.inference_clustered_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;

    let (weights, weight_view, weight_error) =
        build_reference_weight_report_view(ols, &fit, config.certificate_tolerance, true)?;
    let base_dof = build_residual_dof_view(ols, &fit, absorbed_summary, weights)?;
    let expected_dof = build_clustered_dof(prepared, weights, base_dof, absorbed_summary)?;
    let mut dof = expected_dof;
    if perturbations.residual_dof {
        dof.reported_residual_dof = dof
            .reported_residual_dof
            .checked_add(1)
            .ok_or_else(|| allocation("injected prepared clustered residual DoF overflows"))?;
    }
    if dof != expected_dof {
        return Err(invalid_data(
            "prepared clustered residual-DoF identity did not pass",
        ));
    }

    let outcome_mean = weighted_mean(&ols.fit_sample().outcome, weight_view)?;
    let center = if dof.has_structural_intercept {
        outcome_mean
    } else {
        0.0
    };
    let tss = weighted_sum_of_squares(&ols.fit_sample().outcome, weight_view, center)?;
    let within_tss = if ols.inference_has_explicit_absorbed_effects() {
        fit.initial_absorbed_outcome_sum_of_squares()
    } else {
        tss
    };
    let rss = weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)?;
    validate_sum_of_squares(rss, tss, within_tss)?;
    let fit_statistics_dof = PreparedOlsResidualDofReport {
        finite_rank: dof.finite_rank,
        finite_model_dof: dof.finite_model_dof,
        absorbed_rank: dof.absorbed_rank,
        model_rank: dof
            .finite_rank
            .checked_add(dof.absorbed_rank)
            .ok_or_else(|| allocation("prepared clustered fit model rank overflows"))?,
        residual_dof: dof.fit_residual_dof,
        has_structural_intercept: dof.has_structural_intercept,
        has_normalized_absorbed_intercept: dof.has_normalized_absorbed_intercept,
        absorbed_rank_is_approximate: dof.absorbed_rank_is_approximate,
    };
    let mut statistics = build_statistics(rss, tss, within_tss, weights, fit_statistics_dof)?;
    if perturbations.rss {
        statistics.rss += 0.25;
    }

    let mut scores = build_prepared_ols_score_design(
        ols,
        &mut fit,
        weight_view,
        config.fit.absorb,
        config.certificate_tolerance,
        reservation,
    )?;
    if perturbations.score_design {
        scores.certificate.passed = false;
    }
    let mut raw = build_raw_covariance(
        &fit,
        &scores,
        weight_view,
        &dimensions,
        dof,
        weights,
        reservation,
    )?;
    if perturbations.subset_sign && !raw.subsets.is_empty() {
        raw.subsets[0].sign = -raw.subsets[0].sign;
    }
    if perturbations.subset_groups && !raw.subsets.is_empty() {
        raw.subsets[0].groups = raw.subsets[0]
            .groups
            .checked_add(1)
            .ok_or_else(|| allocation("injected prepared clustered group count overflows"))?;
    }
    if perturbations.uncorrected_covariance && !raw.uncorrected.is_empty() {
        raw.uncorrected[0] += 0.25;
    }
    if perturbations.corrected_covariance && !raw.corrected.is_empty() {
        raw.corrected[0] += 0.25;
    }
    if perturbations.correction {
        raw.correction += 0.25;
    }
    let independent_error = certify_with_group_influences(&raw)?;
    let corrected_covariance_error = certify_corrected_covariance(&raw)?;
    let (values, psd) = apply_psd_policy(
        prepared,
        &fit,
        &raw.corrected,
        config.certificate_tolerance,
        reservation,
    )?;
    let standard_errors = build_standard_errors(&values, fit.bread().dimension(), reservation)?;
    let mut covariance = PreparedOlsClusteredCovariance {
        correction: raw.correction,
        raw_values: raw.corrected,
        values,
        standard_errors,
        psd,
    };
    if perturbations.final_covariance && !covariance.values.is_empty() {
        covariance.values[0] += 0.25;
    }
    if perturbations.covariance_symmetry && covariance.dimension() >= 2 {
        covariance.values[1] += 0.25;
    }
    if perturbations.standard_error && !covariance.standard_errors.is_empty() {
        covariance.standard_errors[0] += 0.25;
    }
    if perturbations.psd_state {
        covariance.psd.state = match covariance.psd.state {
            PreparedOlsClusteredPsdState::OneWayStrict => {
                PreparedOlsClusteredPsdState::MultiwayRawPsd
            }
            _ => PreparedOlsClusteredPsdState::OneWayStrict,
        };
    }
    let wald = build_wald(
        ols,
        &fit,
        &covariance,
        dof.finite_model_dof,
        config.certificate_tolerance,
        reservation,
    )?;
    statistics.f_statistic = wald.statistic;
    if perturbations.f_statistic {
        statistics.f_statistic = Some(statistics.f_statistic.unwrap_or(0.0) + 0.25);
    }

    let expected_correction = cluster_correction(weights, dof)?;
    let correction_error = relative_difference(
        covariance.correction,
        expected_correction,
        expected_correction.abs().max(1.0),
    )?;
    let symmetry_error = symmetry_error(covariance.values(), covariance.dimension())?;
    let standard_error_error = standard_error_error(&covariance)?;
    let (pre_restore_full_psd_error, restored_slope_psd_error, final_structure_error) =
        certify_psd_state(
            prepared,
            &fit,
            &covariance,
            config.certificate_tolerance,
            reservation,
        )?;
    let statistic_error = certify_statistics(
        statistics,
        rss,
        tss,
        within_tss,
        weights,
        fit_statistics_dof,
        wald,
    )?;
    let score_certificate = scores.certificate();
    let passed = weight_error <= config.certificate_tolerance
        && score_certificate.passed
        && correction_error <= config.certificate_tolerance
        && independent_error <= config.certificate_tolerance
        && corrected_covariance_error <= config.certificate_tolerance
        && symmetry_error <= config.certificate_tolerance
        && standard_error_error <= config.certificate_tolerance
        && pre_restore_full_psd_error <= config.certificate_tolerance
        && restored_slope_psd_error <= config.certificate_tolerance
        && final_structure_error <= config.certificate_tolerance
        && covariance.psd.transform_reconstruction_relative_error <= config.certificate_tolerance
        && covariance.psd.eigensystem_relative_error <= config.certificate_tolerance
        && covariance.psd.orthogonality_relative_error <= config.certificate_tolerance
        && wald.solve_relative_infinity_norm <= config.certificate_tolerance
        && statistic_error <= config.certificate_tolerance;
    let certificate = PreparedOlsClusteredCertificate {
        reference_weight_sum_relative_error: weight_error,
        score_design_left_inverse_relative_infinity_norm: score_certificate
            .left_inverse_relative_infinity_norm,
        score_design_right_inverse_relative_infinity_norm: score_certificate
            .right_inverse_relative_infinity_norm,
        intercept_coordinate_relative_infinity_norm: score_certificate
            .intercept_coordinate_relative_infinity_norm,
        correction_relative_error: correction_error,
        independent_influence_relative_infinity_norm: independent_error,
        corrected_covariance_relative_infinity_norm: corrected_covariance_error,
        covariance_symmetry_relative_infinity_norm: symmetry_error,
        standard_error_relative_infinity_norm: standard_error_error,
        pre_restore_full_psd_relative_error: pre_restore_full_psd_error,
        restored_slope_psd_relative_error: restored_slope_psd_error,
        final_structure_relative_error: final_structure_error,
        wald_solve_relative_infinity_norm: wald.solve_relative_infinity_norm,
        statistic_formula_relative_infinity_norm: statistic_error,
        passed,
    };
    if !certificate.passed {
        return Err(invalid_data(
            "prepared clustered covariance or statistic certificate did not pass",
        ));
    }

    let mut cluster_counts = reserved_empty(
        reservation,
        prepared.clustered_design().len(),
        "unable to reserve prepared clustered final group counts",
    )?;
    for dimension in prepared.clustered_design().dimensions() {
        push_reserved(
            &mut cluster_counts,
            dimension.groups(),
            "prepared clustered final group-count capacity exhausted",
        )?;
    }
    Ok(PreparedOlsClusteredComponents {
        fit,
        absorbed_dof,
        weights,
        dof,
        cluster_counts,
        subsets: raw.subsets,
        statistics,
        covariance,
        certificate,
    })
}

fn validate_config<P: PreparedOlsClusteredView + ?Sized>(
    prepared: &P,
    config: PreparedOlsClusteredConfig,
) -> Result<()> {
    if !(1..=10).contains(&prepared.clustered_design().len()) {
        return Err(invalid_argument(
            "prepared clustered OLS requires one to ten cluster dimensions",
        ));
    }
    if !config.certificate_tolerance.is_finite()
        || config.certificate_tolerance <= 0.0
        || config.certificate_tolerance >= 1.0
    {
        return Err(invalid_argument(
            "prepared clustered certificate tolerance must be finite and in (0,1)",
        ));
    }
    Ok(())
}

fn borrow_cluster_dimensions<
    'a,
    P: PreparedOlsClusteredView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &'a P,
    reservation: &mut R,
) -> Result<Vec<&'a [u32]>> {
    let mut dimensions = reserved_empty(
        reservation,
        prepared.clustered_design().len(),
        "unable to reserve prepared clustered code views",
    )?;
    for index in 0..prepared.clustered_design().len() {
        push_reserved(
            &mut dimensions,
            prepared.clustered_codes(index)?,
            "prepared clustered code-view capacity exhausted",
        )?;
    }
    Ok(dimensions)
}

fn build_clustered_dof<P: PreparedOlsClusteredView + ?Sized>(
    prepared: &P,
    weights: PreparedOlsReferenceWeightReport,
    base: PreparedOlsResidualDofReport,
    absorbed: PreparedOlsAbsorbedDofSummary,
) -> Result<PreparedOlsClusteredDofReport> {
    let minimum_cluster_count = prepared
        .clustered_design()
        .dimensions()
        .iter()
        .map(|dimension| dimension.groups())
        .min()
        .ok_or_else(|| invalid_argument("prepared clustered design contains no dimensions"))?;
    if minimum_cluster_count < 2 {
        return Err(invalid_data(
            "prepared clustered inference requires at least two groups per dimension",
        ));
    }
    let unclustered_residual_dof = weights
        .effective_observations
        .checked_sub(absorbed.absorbed_rank)
        .and_then(|value| value.checked_sub(base.finite_model_dof))
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid_data("prepared clustered unclustered residual DoF is not positive")
        })?;
    if unclustered_residual_dof != base.residual_dof {
        return Err(invalid_data(
            "prepared clustered base residual-DoF identity did not pass",
        ));
    }
    let fit_residual_dof = unclustered_residual_dof
        .checked_sub(absorbed.nested_categories)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid_data("prepared clustered fit-statistic DoF is not positive"))?;
    let reported_residual_dof = unclustered_residual_dof.min(minimum_cluster_count - 1);
    let nested_adjustment = usize::from(absorbed.nested_categories > 0);
    let correction_denominator_dof = weights
        .effective_observations
        .checked_sub(nested_adjustment)
        .and_then(|value| value.checked_sub(base.finite_model_dof))
        .and_then(|value| value.checked_sub(absorbed.absorbed_rank))
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid_data("prepared clustered small-sample correction denominator is not positive")
        })?;
    Ok(PreparedOlsClusteredDofReport {
        finite_rank: base.finite_rank,
        finite_model_dof: base.finite_model_dof,
        absorbed_rank: absorbed.absorbed_rank,
        nested_absorbed_categories: absorbed.nested_categories,
        unclustered_residual_dof,
        reported_residual_dof,
        fit_residual_dof,
        correction_denominator_dof,
        minimum_cluster_count,
        has_structural_intercept: base.has_structural_intercept,
        has_normalized_absorbed_intercept: base.has_normalized_absorbed_intercept,
        absorbed_rank_is_approximate: absorbed.is_approximate,
    })
}

fn cluster_correction(
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsClusteredDofReport,
) -> Result<f64> {
    let n = exact_integer_as_f64(weights.effective_observations as u128)?;
    let denominator = exact_integer_as_f64(dof.correction_denominator_dof as u128)?;
    let groups = exact_integer_as_f64(dof.minimum_cluster_count as u128)?;
    let correction = ((n - 1.0) / denominator) * (groups / (groups - 1.0));
    if correction.is_finite() && correction > 0.0 {
        Ok(correction)
    } else {
        Err(invalid_data(
            "prepared clustered small-sample correction is invalid",
        ))
    }
}

fn build_raw_covariance<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign,
    weights: ReferenceWeightView<'_>,
    dimensions: &[&[u32]],
    dof: PreparedOlsClusteredDofReport,
    weight_report: PreparedOlsReferenceWeightReport,
    reservation: &mut R,
) -> Result<RawClusterCovariance> {
    let p = fit.bread().dimension();
    let square = checked_square(p, "prepared clustered meat dimensions overflow usize")?;
    let subset_count =
        1_usize
            .checked_shl(u32::try_from(dimensions.len()).map_err(|_| {
                allocation("prepared clustered dimension count exceeds shift width")
            })?)
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| allocation("prepared clustered subset count overflows"))?;
    let mut combined_meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered inclusion-exclusion meat",
    )?;
    let mut combined_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered inclusion-exclusion corrections",
    )?;
    let mut combined_influence = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered independent influence covariance",
    )?;
    let mut combined_influence_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered independent influence corrections",
    )?;
    let mut subsets = reserved_empty(
        reservation,
        subset_count,
        "unable to reserve prepared clustered subset certificates",
    )?;
    let mut independent_subsets = reserved_empty(
        reservation,
        subset_count,
        "unable to reserve prepared clustered independent subset certificates",
    )?;
    for cardinality in 1..=dimensions.len() {
        for mask in 1_usize..=subset_count {
            if mask.count_ones() as usize != cardinality {
                continue;
            }
            let sign = if cardinality % 2 == 1 { 1_i8 } else { -1_i8 };
            let meats = subset_score_meats(fit, scores, weights, dimensions, mask, reservation)?;
            for position in 0..square {
                compensated_add(
                    f64::from(sign) * meats.raw[position],
                    &mut combined_meat[position],
                    &mut combined_corrections[position],
                )?;
                compensated_add(
                    f64::from(sign) * meats.influence[position],
                    &mut combined_influence[position],
                    &mut combined_influence_corrections[position],
                )?;
            }
            let report = PreparedOlsClusterSubsetCertificate {
                mask: u16::try_from(mask)
                    .map_err(|_| allocation("prepared clustered subset mask exceeds u16"))?,
                dimensions: cardinality,
                sign,
                groups: meats.groups,
            };
            push_reserved(
                &mut subsets,
                report,
                "prepared clustered subset-certificate capacity exhausted",
            )?;
            push_reserved(
                &mut independent_subsets,
                report,
                "prepared clustered independent subset-certificate capacity exhausted",
            )?;
        }
    }
    for (value, correction) in combined_meat.iter_mut().zip(combined_corrections) {
        *value += correction;
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared clustered inclusion-exclusion meat is not finite",
            ));
        }
    }
    for (value, correction) in combined_influence
        .iter_mut()
        .zip(combined_influence_corrections)
    {
        *value += correction;
        if !value.is_finite() {
            return Err(invalid_data(
                "prepared clustered independent influence covariance is not finite",
            ));
        }
    }
    symmetrize(&mut combined_influence, p)?;
    let uncorrected = sandwich(fit.bread().values(), &combined_meat, p, reservation)?;
    let correction = cluster_correction(weight_report, dof)?;
    let mut corrected = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered corrected covariance",
    )?;
    for (target, source) in corrected.iter_mut().zip(&uncorrected) {
        *target = correction * *source;
        if !target.is_finite() {
            return Err(invalid_data(
                "prepared clustered corrected covariance is not finite",
            ));
        }
    }
    symmetrize(&mut corrected, p)?;
    Ok(RawClusterCovariance {
        dimension: p,
        uncorrected,
        independent_uncorrected: combined_influence,
        corrected,
        correction,
        subsets,
        independent_subsets,
    })
}

fn subset_score_meats<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign,
    weights: ReferenceWeightView<'_>,
    dimensions: &[&[u32]],
    mask: usize,
    reservation: &mut R,
) -> Result<SubsetMeats> {
    // Compact direct tables avoid ordering overhead in serial and for very
    // narrow score vectors. Once the score axis is wide enough to dominate,
    // deterministic radix ordering exposes whole cluster groups to the Rayon
    // pool and avoids the serial observation-by-coordinate accumulation.
    let prefer_ordered_parallel = rayon::current_num_threads() > 1
        && scores
            .dense_view()
            .is_some_and(|dense| dense.reported_dimension() >= 8);
    if scores.dense_view().is_some() && !prefer_ordered_parallel {
        if let Some(plan) =
            dense_subset_plan(dimensions, mask, scores.nrows(), fit.bread().dimension())?
        {
            return aggregate_subset_dense(
                fit,
                scores,
                weights,
                dimensions,
                mask,
                plan,
                reservation,
            );
        }
    }
    aggregate_subset_sorted(fit, scores, weights, dimensions, mask, reservation)
}

fn dense_subset_plan(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    score_dimension: usize,
) -> Result<Option<DenseSubsetPlan>> {
    if dimensions.is_empty()
        || dimensions.len() > 10
        || mask == 0
        || mask >= (1_usize << dimensions.len())
    {
        return Err(invalid_argument(
            "prepared clustered dense subset plan is malformed",
        ));
    }
    let mut radices = [0_usize; 10];
    let mut group_slots = 1_usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if codes.len() != nobs {
            return Err(invalid_argument(
                "prepared clustered dense subset codes are not sample-aligned",
            ));
        }
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| invalid_data("prepared clustered dense subset has no groups"))?;
        radices[dimension] = radix;
        group_slots = group_slots
            .checked_mul(radix)
            .ok_or_else(|| allocation("prepared clustered dense group product overflows"))?;
    }
    let score_slots = group_slots
        .checked_mul(score_dimension)
        .ok_or_else(|| allocation("prepared clustered dense score table overflows"))?;
    let score_bytes = score_slots
        .checked_mul(std::mem::size_of::<f64>())
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| allocation("prepared clustered dense score bytes overflow"))?;
    let dense_bytes = score_bytes
        .checked_add(group_slots)
        .ok_or_else(|| allocation("prepared clustered dense scratch bytes overflow"))?;
    let ordering_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("prepared clustered ordering bytes overflow"))?;
    let fast_dense_budget = ordering_bytes
        .checked_mul(4)
        .unwrap_or(usize::MAX)
        .max(64 << 20);
    if dense_bytes > fast_dense_budget {
        return Ok(None);
    }
    Ok(Some(DenseSubsetPlan {
        radices,
        group_slots,
    }))
}

fn dense_subset_group(
    dimensions: &[&[u32]],
    mask: usize,
    plan: DenseSubsetPlan,
    row: usize,
) -> Result<usize> {
    let mut group = 0_usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let code = usize::try_from(codes[row])
            .map_err(|_| invalid_data("prepared clustered dense code exceeds usize"))?;
        let radix = plan.radices[dimension];
        if radix == 0 || code >= radix {
            return Err(invalid_data(
                "prepared clustered dense code exceeds its certified radix",
            ));
        }
        group = group
            .checked_mul(radix)
            .and_then(|value| value.checked_add(code))
            .ok_or_else(|| allocation("prepared clustered dense group index overflows"))?;
    }
    if group >= plan.group_slots {
        return Err(invalid_data(
            "prepared clustered dense group index exceeds its table",
        ));
    }
    Ok(group)
}

fn aggregate_subset_dense<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign,
    weights: ReferenceWeightView<'_>,
    dimensions: &[&[u32]],
    mask: usize,
    plan: DenseSubsetPlan,
    reservation: &mut R,
) -> Result<SubsetMeats> {
    let nobs = scores.nrows();
    let p = fit.bread().dimension();
    if fit.residuals().len() != nobs || dimensions.iter().any(|codes| codes.len() != nobs) {
        return Err(invalid_argument(
            "prepared clustered dense scores and dimensions are not aligned",
        ));
    }
    let square = checked_square(
        p,
        "prepared clustered dense subset meat dimensions overflow usize",
    )?;
    let score_slots = plan
        .group_slots
        .checked_mul(p)
        .ok_or_else(|| allocation("prepared clustered dense score slots overflow"))?;
    let mut group_scores = reserved_filled(
        reservation,
        score_slots,
        0.0_f64,
        "unable to reserve prepared clustered dense group scores",
    )?;
    let mut group_score_corrections = reserved_filled(
        reservation,
        score_slots,
        0.0_f64,
        "unable to reserve prepared clustered dense group score corrections",
    )?;
    let mut seen = reserved_filled(
        reservation,
        plan.group_slots,
        0_u8,
        "unable to reserve prepared clustered dense group presence",
    )?;
    let mut reported_score = reserved_filled(
        reservation,
        p,
        0.0_f64,
        "unable to reserve prepared clustered dense reported score row",
    )?;
    for row in 0..nobs {
        let group = dense_subset_group(dimensions, mask, plan, row)?;
        seen[group] = 1;
        let scale = weights.value(row)? * fit.residuals()[row];
        if !scale.is_finite() {
            return Err(invalid_data(
                "prepared clustered dense observation score scale is not finite",
            ));
        }
        scores.fill_reported_row(row, &mut reported_score)?;
        let offset = group
            .checked_mul(p)
            .ok_or_else(|| allocation("prepared clustered dense score offset overflows"))?;
        for (coordinate, score_value) in reported_score.iter().copied().enumerate() {
            let position = offset + coordinate;
            compensated_add(
                scale * score_value,
                &mut group_scores[position],
                &mut group_score_corrections[position],
            )?;
        }
    }
    finish_dense_groups_parallel(
        fit,
        &group_scores,
        &group_score_corrections,
        &seen,
        plan.group_slots,
        p,
        square,
        reservation,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_dense_groups_parallel<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    group_scores: &[f64],
    group_score_corrections: &[f64],
    seen: &[u8],
    group_slots: usize,
    dimension: usize,
    square: usize,
    reservation: &mut R,
) -> Result<SubsetMeats> {
    if group_scores.len() != group_slots * dimension
        || group_score_corrections.len() != group_scores.len()
        || seen.len() != group_slots
        || square != dimension * dimension
    {
        return Err(invalid_argument(
            "prepared clustered dense group-finalization dimensions are inconsistent",
        ));
    }
    let workspace = dimension
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| allocation("prepared clustered dense group workspace overflows"))?;
    let partial_stride = square
        .checked_mul(2)
        .and_then(|value| value.checked_add(workspace))
        .ok_or_else(|| allocation("prepared clustered dense group partial stride overflows"))?;
    let chunks = cluster_reduction_chunks(group_slots, 1_024, partial_stride)?;
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("prepared clustered dense group partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared clustered dense group partials",
    )?;
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (raw_meat, remainder) = partial.split_at_mut(square);
            let (influence_meat, workspace) = remainder.split_at_mut(square);
            let (raw_score, workspace) = workspace.split_at_mut(dimension);
            let (influence, count) = workspace.split_at_mut(dimension);
            let groups_per_chunk = group_slots / chunks;
            let remainder = group_slots % chunks;
            let start = chunk * groups_per_chunk + chunk.min(remainder);
            let end = start + groups_per_chunk + usize::from(chunk < remainder);
            for (group, present) in seen.iter().enumerate().take(end).skip(start) {
                if *present == 0 {
                    continue;
                }
                let offset = group * dimension;
                for coordinate in 0..dimension {
                    raw_score[coordinate] = group_scores[offset + coordinate]
                        + group_score_corrections[offset + coordinate];
                }
                for (left, influence_value) in influence.iter_mut().enumerate() {
                    let mut value = 0.0_f64;
                    let mut correction = 0.0_f64;
                    for (right, raw_value) in raw_score.iter().copied().enumerate() {
                        compensated_add(
                            fit.bread().values()[left * dimension + right] * raw_value,
                            &mut value,
                            &mut correction,
                        )?;
                    }
                    *influence_value = value + correction;
                }
                for left in 0..dimension {
                    let raw_left = raw_score[left];
                    let influence_left = influence[left];
                    for right in left..dimension {
                        raw_meat[left * dimension + right] += raw_left * raw_score[right];
                        influence_meat[left * dimension + right] +=
                            influence_left * influence[right];
                    }
                }
                count[0] += 1.0;
            }
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared clustered dense group partial contains a nonfinite value",
                ))
            }
        })?;
    let mut raw = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered dense subset meat",
    )?;
    let mut raw_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered dense subset meat corrections",
    )?;
    let mut influence = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered dense subset influence covariance",
    )?;
    let mut influence_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered dense subset influence corrections",
    )?;
    let mut groups = 0_usize;
    for partial in partials.chunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[position],
                    &mut raw[position],
                    &mut raw_corrections[position],
                )?;
                compensated_add(
                    partial[square + position],
                    &mut influence[position],
                    &mut influence_corrections[position],
                )?;
            }
        }
        let count = partial[2 * square + 2 * dimension];
        if count.fract() != 0.0 || count < 0.0 || count > usize::MAX as f64 {
            return Err(invalid_data(
                "prepared clustered dense group partial count is invalid",
            ));
        }
        groups = groups
            .checked_add(count as usize)
            .ok_or_else(|| allocation("prepared clustered dense group count overflows"))?;
    }
    finalize_subset_meat(&mut raw, raw_corrections, dimension)?;
    finalize_subset_meat(&mut influence, influence_corrections, dimension)?;
    Ok(SubsetMeats {
        raw,
        influence,
        groups,
    })
}

/// Generic bounded-memory subset aggregation. One sort and score scan produces
/// both the raw-score meat and the algebraically independent bread-transformed
/// influence covariance.
fn aggregate_subset_sorted<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: &CertifiedPreparedOlsScoreDesign,
    weights: ReferenceWeightView<'_>,
    dimensions: &[&[u32]],
    mask: usize,
    reservation: &mut R,
) -> Result<SubsetMeats> {
    let nobs = scores.nrows();
    let p = fit.bread().dimension();
    if fit.residuals().len() != nobs || dimensions.iter().any(|codes| codes.len() != nobs) {
        return Err(invalid_argument(
            "prepared clustered score and cluster dimensions are not aligned",
        ));
    }
    let square = checked_square(
        p,
        "prepared clustered subset meat dimensions overflow usize",
    )?;
    let mut order = if scores.dense_view().is_some() {
        build_subset_radix_order(dimensions, mask, nobs, reservation)?
    } else {
        None
    }
    .unwrap_or_else(Vec::new);
    if order.is_empty() {
        order = reserved_empty(
            reservation,
            nobs,
            "unable to reserve prepared clustered subset ordering",
        )?;
        for row in 0..nobs {
            push_reserved(
                &mut order,
                row,
                "prepared clustered subset ordering capacity exhausted",
            )?;
        }
        order.sort_unstable_by(|left, right| {
            compare_cluster_rows(dimensions, mask, *left, *right).then_with(|| left.cmp(right))
        });
    }
    if let Some(dense) = scores.dense_view() {
        return aggregate_ordered_dense_parallel(
            fit,
            dense,
            weights,
            dimensions,
            mask,
            &order,
            reservation,
        );
    }
    let mut group_score = reserved_filled(
        reservation,
        p,
        0.0_f64,
        "unable to reserve prepared clustered group score",
    )?;
    let mut group_score_correction = reserved_filled(
        reservation,
        p,
        0.0_f64,
        "unable to reserve prepared clustered group score corrections",
    )?;
    let mut raw_transformed = reserved_filled(
        reservation,
        p,
        0.0_f64,
        "unable to reserve prepared clustered raw group score",
    )?;
    let mut influence_transformed = reserved_filled(
        reservation,
        p,
        0.0_f64,
        "unable to reserve prepared clustered group influence",
    )?;
    let mut reported_score = reserved_filled(
        reservation,
        p,
        0.0_f64,
        "unable to reserve prepared clustered reported score row",
    )?;
    let mut raw_meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered subset meat",
    )?;
    let mut raw_meat_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered subset meat corrections",
    )?;
    let mut influence_meat = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered subset influence covariance",
    )?;
    let mut influence_meat_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered subset influence corrections",
    )?;
    let mut groups = 0_usize;
    let mut previous = None;
    for row in order {
        if previous.is_some_and(|prior| {
            compare_cluster_rows(dimensions, mask, prior, row) != Ordering::Equal
        }) {
            finish_group_pair(
                fit,
                &group_score,
                &group_score_correction,
                &mut raw_transformed,
                &mut influence_transformed,
                &mut raw_meat,
                &mut raw_meat_corrections,
                &mut influence_meat,
                &mut influence_meat_corrections,
            )?;
            group_score.fill(0.0);
            group_score_correction.fill(0.0);
            groups = groups
                .checked_add(1)
                .ok_or_else(|| allocation("prepared clustered subset group count overflows"))?;
        }
        let scale = weights.value(row)? * fit.residuals()[row];
        if !scale.is_finite() {
            return Err(invalid_data(
                "prepared clustered observation score scale is not finite",
            ));
        }
        scores.fill_reported_row(row, &mut reported_score)?;
        for coordinate in 0..p {
            compensated_add(
                scale * reported_score[coordinate],
                &mut group_score[coordinate],
                &mut group_score_correction[coordinate],
            )?;
        }
        previous = Some(row);
    }
    if previous.is_some() {
        finish_group_pair(
            fit,
            &group_score,
            &group_score_correction,
            &mut raw_transformed,
            &mut influence_transformed,
            &mut raw_meat,
            &mut raw_meat_corrections,
            &mut influence_meat,
            &mut influence_meat_corrections,
        )?;
        groups = groups
            .checked_add(1)
            .ok_or_else(|| allocation("prepared clustered subset group count overflows"))?;
    }
    finalize_subset_meat(&mut raw_meat, raw_meat_corrections, p)?;
    finalize_subset_meat(&mut influence_meat, influence_meat_corrections, p)?;
    Ok(SubsetMeats {
        raw: raw_meat,
        influence: influence_meat,
        groups,
    })
}

fn aggregate_ordered_dense_parallel<R: PreparedInferenceReservation>(
    fit: &CertifiedPreparedOlsFit,
    scores: super::prepared_scores::PreparedOlsDenseScoreView<'_>,
    weights: ReferenceWeightView<'_>,
    dimensions: &[&[u32]],
    mask: usize,
    order: &[usize],
    reservation: &mut R,
) -> Result<SubsetMeats> {
    let nobs = scores.nrows();
    let dimension = scores.reported_dimension();
    let square = checked_square(
        dimension,
        "prepared clustered ordered dense meat dimensions overflow usize",
    )?;
    if order.len() != nobs
        || fit.residuals().len() != nobs
        || fit.bread().dimension() != dimension
        || dimensions.iter().any(|codes| codes.len() != nobs)
    {
        return Err(invalid_argument(
            "prepared clustered ordered dense dimensions are inconsistent",
        ));
    }
    let workspace = dimension
        .checked_mul(4)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| allocation("prepared clustered ordered dense workspace overflows"))?;
    let partial_stride = square
        .checked_mul(2)
        .and_then(|value| value.checked_add(workspace))
        .ok_or_else(|| allocation("prepared clustered ordered dense partial stride overflows"))?;
    let chunks = cluster_reduction_chunks(nobs, 16_384, partial_stride)?;
    let mut boundaries = reserved_filled(
        reservation,
        chunks + 1,
        0_usize,
        "unable to reserve prepared clustered ordered dense boundaries",
    )?;
    boundaries[chunks] = nobs;
    for (chunk, target) in boundaries.iter_mut().enumerate().take(chunks).skip(1) {
        let mut boundary = chunk * nobs / chunks;
        while boundary < nobs
            && compare_cluster_rows(dimensions, mask, order[boundary - 1], order[boundary])
                == Ordering::Equal
        {
            boundary += 1;
        }
        *target = boundary;
    }
    for chunk in 1..chunks {
        boundaries[chunk] = boundaries[chunk].max(boundaries[chunk - 1]);
    }
    let partial_elements = chunks
        .checked_mul(partial_stride)
        .ok_or_else(|| allocation("prepared clustered ordered dense partials overflow usize"))?;
    let mut partials = reserved_filled(
        reservation,
        partial_elements,
        0.0_f64,
        "unable to reserve prepared clustered ordered dense partials",
    )?;
    partials
        .par_chunks_mut(partial_stride)
        .enumerate()
        .try_for_each(|(chunk, partial)| -> Result<()> {
            let (raw_meat, remainder) = partial.split_at_mut(square);
            let (influence_meat, workspace) = remainder.split_at_mut(square);
            let (group_score, workspace) = workspace.split_at_mut(dimension);
            let (group_corrections, workspace) = workspace.split_at_mut(dimension);
            let (raw_score, workspace) = workspace.split_at_mut(dimension);
            let (influence, count) = workspace.split_at_mut(dimension);
            let start = boundaries[chunk];
            let end = boundaries[chunk + 1];
            let mut previous = None;
            for row in order[start..end].iter().copied() {
                if previous.is_some_and(|prior| {
                    compare_cluster_rows(dimensions, mask, prior, row) != Ordering::Equal
                }) {
                    finish_group_pair_uncompensated_meat(
                        fit,
                        group_score,
                        group_corrections,
                        raw_score,
                        influence,
                        raw_meat,
                        influence_meat,
                    )?;
                    group_score.fill(0.0);
                    group_corrections.fill(0.0);
                    count[0] += 1.0;
                }
                let scale = weights.value(row)? * fit.residuals()[row];
                if !scale.is_finite() {
                    return Err(invalid_data(
                        "prepared clustered ordered dense score scale is not finite",
                    ));
                }
                scores.fill_reported_row(row, raw_score)?;
                for coordinate in 0..dimension {
                    compensated_add(
                        scale * raw_score[coordinate],
                        &mut group_score[coordinate],
                        &mut group_corrections[coordinate],
                    )?;
                }
                previous = Some(row);
            }
            if previous.is_some() {
                finish_group_pair_uncompensated_meat(
                    fit,
                    group_score,
                    group_corrections,
                    raw_score,
                    influence,
                    raw_meat,
                    influence_meat,
                )?;
                count[0] += 1.0;
            }
            if partial.iter().all(|value| value.is_finite()) {
                Ok(())
            } else {
                Err(invalid_data(
                    "prepared clustered ordered dense partial contains a nonfinite value",
                ))
            }
        })?;
    merge_ordered_dense_partials(&partials, partial_stride, dimension, square, reservation)
}

#[allow(clippy::too_many_arguments)]
fn finish_group_pair_uncompensated_meat(
    fit: &CertifiedPreparedOlsFit,
    score: &[f64],
    score_corrections: &[f64],
    raw_score: &mut [f64],
    influence: &mut [f64],
    raw_meat: &mut [f64],
    influence_meat: &mut [f64],
) -> Result<()> {
    let dimension = score.len();
    if score_corrections.len() != dimension
        || raw_score.len() != dimension
        || influence.len() != dimension
        || raw_meat.len() != dimension * dimension
        || influence_meat.len() != raw_meat.len()
    {
        return Err(invalid_argument(
            "prepared clustered dense group pair dimensions are inconsistent",
        ));
    }
    for coordinate in 0..dimension {
        raw_score[coordinate] = score[coordinate] + score_corrections[coordinate];
    }
    for (left, influence_value) in influence.iter_mut().enumerate() {
        let mut value = 0.0_f64;
        let mut correction = 0.0_f64;
        for (right, raw_value) in raw_score.iter().copied().enumerate() {
            compensated_add(
                fit.bread().values()[left * dimension + right] * raw_value,
                &mut value,
                &mut correction,
            )?;
        }
        *influence_value = value + correction;
    }
    for left in 0..dimension {
        let raw_left = raw_score[left];
        let influence_left = influence[left];
        for right in left..dimension {
            raw_meat[left * dimension + right] += raw_left * raw_score[right];
            influence_meat[left * dimension + right] += influence_left * influence[right];
        }
    }
    Ok(())
}

fn merge_ordered_dense_partials<R: PreparedInferenceReservation>(
    partials: &[f64],
    partial_stride: usize,
    dimension: usize,
    square: usize,
    reservation: &mut R,
) -> Result<SubsetMeats> {
    if partial_stride == 0 || partials.len() % partial_stride != 0 {
        return Err(invalid_argument(
            "prepared clustered ordered dense partial dimensions are inconsistent",
        ));
    }
    let mut raw = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered ordered dense meat",
    )?;
    let mut raw_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered ordered dense meat corrections",
    )?;
    let mut influence = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered ordered dense influence covariance",
    )?;
    let mut influence_corrections = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered ordered dense influence corrections",
    )?;
    let mut groups = 0_usize;
    for partial in partials.chunks_exact(partial_stride) {
        for left in 0..dimension {
            for right in left..dimension {
                let position = left * dimension + right;
                compensated_add(
                    partial[position],
                    &mut raw[position],
                    &mut raw_corrections[position],
                )?;
                compensated_add(
                    partial[square + position],
                    &mut influence[position],
                    &mut influence_corrections[position],
                )?;
            }
        }
        let count = partial[2 * square + 4 * dimension];
        if count.fract() != 0.0 || count < 0.0 || count > usize::MAX as f64 {
            return Err(invalid_data(
                "prepared clustered ordered dense group count is invalid",
            ));
        }
        groups = groups
            .checked_add(count as usize)
            .ok_or_else(|| allocation("prepared clustered ordered dense group count overflows"))?;
    }
    finalize_subset_meat(&mut raw, raw_corrections, dimension)?;
    finalize_subset_meat(&mut influence, influence_corrections, dimension)?;
    Ok(SubsetMeats {
        raw,
        influence,
        groups,
    })
}

fn build_subset_radix_order<R: PreparedInferenceReservation>(
    dimensions: &[&[u32]],
    mask: usize,
    nobs: usize,
    reservation: &mut R,
) -> Result<Option<Vec<usize>>> {
    if nobs == 0
        || dimensions.is_empty()
        || dimensions.len() > 10
        || mask == 0
        || mask >= (1_usize << dimensions.len())
        || dimensions.iter().any(|codes| codes.len() != nobs)
    {
        return Err(invalid_argument(
            "prepared clustered radix ordering dimensions are inconsistent",
        ));
    }
    let mut radices = [0_usize; 10];
    let mut maximum_radix = 0_usize;
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let radix = codes
            .iter()
            .copied()
            .max()
            .and_then(|value| usize::try_from(value).ok())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| invalid_data("prepared clustered radix dimension has no groups"))?;
        radices[dimension] = radix;
        maximum_radix = maximum_radix.max(radix);
    }
    let scratch_bytes = nobs
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("prepared clustered radix scratch bytes overflow"))?;
    let count_bytes = maximum_radix
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(|| allocation("prepared clustered radix count bytes overflow"))?;
    if count_bytes > scratch_bytes.saturating_mul(2).max(1 << 20) {
        return Ok(None);
    }
    let mut order = reserved_filled(
        reservation,
        nobs,
        0_usize,
        "unable to reserve prepared clustered radix ordering",
    )?;
    for (row, target) in order.iter_mut().enumerate() {
        *target = row;
    }
    let mut scratch = reserved_filled(
        reservation,
        nobs,
        0_usize,
        "unable to reserve prepared clustered radix scratch ordering",
    )?;
    let mut counts = reserved_filled(
        reservation,
        maximum_radix,
        0_usize,
        "unable to reserve prepared clustered radix counts",
    )?;
    for dimension in (0..dimensions.len()).rev() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let radix = radices[dimension];
        let active_counts = &mut counts[..radix];
        active_counts.fill(0);
        for row in order.iter().copied() {
            let key = usize::try_from(dimensions[dimension][row])
                .map_err(|_| invalid_data("prepared clustered radix code exceeds usize"))?;
            active_counts[key] = active_counts[key]
                .checked_add(1)
                .ok_or_else(|| allocation("prepared clustered radix count overflows"))?;
        }
        let mut offset = 0_usize;
        for count in active_counts.iter_mut() {
            let frequency = *count;
            *count = offset;
            offset = offset
                .checked_add(frequency)
                .ok_or_else(|| allocation("prepared clustered radix offset overflows"))?;
        }
        if offset != nobs {
            return Err(invalid_data(
                "prepared clustered radix counts do not cover the sample",
            ));
        }
        for row in order.iter().copied() {
            let key = usize::try_from(dimensions[dimension][row])
                .map_err(|_| invalid_data("prepared clustered radix code exceeds usize"))?;
            let position = active_counts[key];
            scratch[position] = row;
            active_counts[key] = position
                .checked_add(1)
                .ok_or_else(|| allocation("prepared clustered radix cursor overflows"))?;
        }
        std::mem::swap(&mut order, &mut scratch);
    }
    Ok(Some(order))
}

#[allow(clippy::too_many_arguments)]
fn finish_group_pair(
    fit: &CertifiedPreparedOlsFit,
    score: &[f64],
    score_corrections: &[f64],
    raw_transformed: &mut [f64],
    influence_transformed: &mut [f64],
    raw_meat: &mut [f64],
    raw_meat_corrections: &mut [f64],
    influence_meat: &mut [f64],
    influence_meat_corrections: &mut [f64],
) -> Result<()> {
    finish_group(
        fit,
        score,
        score_corrections,
        false,
        raw_transformed,
        raw_meat,
        raw_meat_corrections,
    )?;
    finish_group(
        fit,
        score,
        score_corrections,
        true,
        influence_transformed,
        influence_meat,
        influence_meat_corrections,
    )
}

fn finalize_subset_meat(meat: &mut [f64], corrections: Vec<f64>, dimension: usize) -> Result<()> {
    let square = checked_square(
        dimension,
        "prepared clustered finalized subset dimensions overflow usize",
    )?;
    if meat.len() != square || corrections.len() != square {
        return Err(invalid_argument(
            "prepared clustered finalized subset dimensions are inconsistent",
        ));
    }
    for left in 0..dimension {
        for right in left..dimension {
            let position = left * dimension + right;
            meat[position] += corrections[position];
            if !meat[position].is_finite() {
                return Err(invalid_data(
                    "prepared clustered finalized subset meat is not finite",
                ));
            }
            meat[right * dimension + left] = meat[position];
        }
    }
    Ok(())
}

fn finish_group(
    fit: &CertifiedPreparedOlsFit,
    score: &[f64],
    score_corrections: &[f64],
    transform_by_bread: bool,
    transformed: &mut [f64],
    meat: &mut [f64],
    meat_corrections: &mut [f64],
) -> Result<()> {
    let p = score.len();
    if score_corrections.len() != p
        || transformed.len() != p
        || meat.len() != p * p
        || meat_corrections.len() != p * p
    {
        return Err(invalid_argument(
            "prepared clustered group aggregation dimensions are inconsistent",
        ));
    }
    if transform_by_bread {
        for (row, target) in transformed.iter_mut().enumerate().take(p) {
            let mut value = 0.0_f64;
            let mut correction = 0.0_f64;
            for column in 0..p {
                compensated_add(
                    fit.bread().values()[row * p + column]
                        * (score[column] + score_corrections[column]),
                    &mut value,
                    &mut correction,
                )?;
            }
            *target = value + correction;
        }
    } else {
        for index in 0..p {
            transformed[index] = score[index] + score_corrections[index];
        }
    }
    for left in 0..p {
        for right in left..p {
            let position = left * p + right;
            compensated_add(
                transformed[left] * transformed[right],
                &mut meat[position],
                &mut meat_corrections[position],
            )?;
        }
    }
    Ok(())
}

fn compare_cluster_rows(dimensions: &[&[u32]], mask: usize, left: usize, right: usize) -> Ordering {
    for (dimension, codes) in dimensions.iter().enumerate() {
        if mask & (1_usize << dimension) == 0 {
            continue;
        }
        let ordering = codes[left].cmp(&codes[right]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn cluster_reduction_chunks(
    work_items: usize,
    minimum_items: usize,
    partial_stride: usize,
) -> Result<usize> {
    if work_items == 0 || minimum_items == 0 || partial_stride == 0 {
        return Err(invalid_argument(
            "prepared clustered reduction dimensions are empty",
        ));
    }
    let partial_bytes = partial_stride
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or_else(|| allocation("prepared clustered reduction partial bytes overflow usize"))?;
    let memory_blocks = (CLUSTER_PARTIAL_BUDGET_BYTES / partial_bytes.max(1)).max(1);
    Ok(work_items
        .div_ceil(minimum_items)
        .clamp(1, CLUSTER_REDUCTION_MAX_BLOCKS)
        .min(memory_blocks))
}

fn sandwich<R: PreparedInferenceReservation>(
    bread: &[f64],
    meat: &[f64],
    p: usize,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = checked_square(p, "prepared clustered sandwich dimensions overflow usize")?;
    if bread.len() != square || meat.len() != square {
        return Err(invalid_argument(
            "prepared clustered sandwich dimensions are inconsistent",
        ));
    }
    let mut left = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered bread-meat product",
    )?;
    for row in 0..p {
        for column in 0..p {
            let mut value = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..p {
                compensated_add(
                    bread[row * p + inner] * meat[inner * p + column],
                    &mut value,
                    &mut correction,
                )?;
            }
            left[row * p + column] = value + correction;
        }
    }
    let mut output = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered uncorrected covariance",
    )?;
    for row in 0..p {
        for column in 0..p {
            let mut value = 0.0_f64;
            let mut correction = 0.0_f64;
            for inner in 0..p {
                compensated_add(
                    left[row * p + inner] * bread[column * p + inner],
                    &mut value,
                    &mut correction,
                )?;
            }
            output[row * p + column] = value + correction;
        }
    }
    symmetrize(&mut output, p)?;
    Ok(output)
}

fn certify_with_group_influences(raw: &RawClusterCovariance) -> Result<f64> {
    let p = raw.dimension;
    let square = checked_square(
        p,
        "prepared clustered independent certificate dimensions overflow usize",
    )?;
    if raw.uncorrected.len() != square
        || raw.independent_uncorrected.len() != square
        || raw.subsets.len() != raw.independent_subsets.len()
    {
        return Err(invalid_argument(
            "prepared clustered independent certificate dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..p {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..p {
            let position = row * p + column;
            let production = raw.uncorrected[position];
            let independent = raw.independent_uncorrected[position];
            row_error += (production - independent).abs();
            row_scale += production.abs().max(independent.abs());
        }
        error = error.max(row_error / row_scale);
    }
    if raw.subsets != raw.independent_subsets {
        error = 1.0;
    }
    if error.is_finite() {
        Ok(error)
    } else {
        Err(invalid_data(
            "prepared clustered independent certificate norm is not finite",
        ))
    }
}

fn certify_corrected_covariance(raw: &RawClusterCovariance) -> Result<f64> {
    if raw.uncorrected.len() != raw.corrected.len()
        || !raw.correction.is_finite()
        || raw.correction <= 0.0
    {
        return Err(invalid_argument(
            "prepared clustered corrected covariance dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    let mut scale = 1.0_f64;
    for (uncorrected, corrected) in raw.uncorrected.iter().zip(&raw.corrected) {
        let expected = uncorrected * raw.correction;
        if !expected.is_finite() || !corrected.is_finite() {
            return Err(invalid_data(
                "prepared clustered corrected covariance is not finite",
            ));
        }
        error += (corrected - expected).abs();
        scale += corrected.abs().max(expected.abs());
    }
    Ok(error / scale)
}

fn apply_psd_policy<P: PreparedOlsClusteredView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    raw: &[f64],
    tolerance: f64,
    reservation: &mut R,
) -> Result<(Vec<f64>, PreparedOlsClusteredPsdReport)> {
    let p = fit.bread().dimension();
    let square = checked_square(p, "prepared clustered PSD dimensions overflow usize")?;
    if raw.len() != square {
        return Err(invalid_argument(
            "prepared clustered PSD dimensions are inconsistent",
        ));
    }
    if p == 0 {
        let state = if prepared.clustered_design().len() == 1 {
            PreparedOlsClusteredPsdState::OneWayStrict
        } else {
            PreparedOlsClusteredPsdState::MultiwayRawPsd
        };
        return Ok((
            Vec::new(),
            PreparedOlsClusteredPsdReport {
                state,
                full_adjusted: false,
                slope_block_restored: false,
                slope_block_adjusted: false,
                full_clipped_eigenvalues: 0,
                slope_clipped_eigenvalues: 0,
                minimum_raw_full_eigenvalue: 0.0,
                minimum_pre_restore_full_eigenvalue: 0.0,
                minimum_raw_slope_eigenvalue: None,
                minimum_restored_slope_eigenvalue: None,
                minimum_final_full_eigenvalue: 0.0,
                pre_restore_full_rank: 0,
                restored_slope_rank: None,
                strict_final_full_psd: true,
                transform_reconstruction_relative_error: 0.0,
                eigensystem_relative_error: 0.0,
                orthogonality_relative_error: 0.0,
            },
        ));
    }
    let mut symmetric = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered symmetric covariance",
    )?;
    symmetric.copy_from_slice(raw);
    symmetrize(&mut symmetric, p)?;
    let raw_eigen = symmetric_eigensystem(&symmetric, p, reservation)?;
    let minimum_raw = raw_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let scale = covariance_scale(&symmetric, p)?;
    if prepared.clustered_design().len() == 1 && minimum_raw < -tolerance * scale {
        return Err(invalid_data(
            "one-way prepared clustered covariance is materially indefinite",
        ));
    }

    let cluster_dimensions = prepared.clustered_design().len();
    let mut output = symmetric;
    let mut full_adjusted = false;
    let mut full_clipped = 0_usize;
    let mut slope_block_restored = false;
    let mut slope_block_adjusted = false;
    let mut slope_clipped = 0_usize;
    let mut minimum_raw_slope = None;
    let mut minimum_restored_slope = None;
    let mut restored_slope_rank = None;
    let mut transform_error = 0.0_f64;
    let mut eig_error = raw_eigen.reconstruction_error;
    let mut orthogonality_error = raw_eigen.orthogonality_error;
    if cluster_dimensions > 1 && minimum_raw < 0.0 {
        let (repaired, clipped_count) = reconstruct_clipped(&raw_eigen, p, reservation)?;
        transform_error =
            transform_error.max(certify_clipped_reconstruction(&raw_eigen, &repaired, p)?);
        output = repaired;
        full_clipped = clipped_count;
        full_adjusted = clipped_count > 0;

        let intercept = reported_intercept_position(prepared.clustered_ols(), fit)?;
        if let Some(intercept) = intercept {
            let slope_dimension = p.checked_sub(1).ok_or_else(|| {
                invalid_argument("prepared clustered slope dimension underflowed")
            })?;
            if slope_dimension > 0 {
                let slope_square = checked_square(
                    slope_dimension,
                    "prepared clustered slope-block dimensions overflow usize",
                )?;
                let mut positions = reserved_empty(
                    reservation,
                    slope_dimension,
                    "unable to reserve prepared clustered slope-block positions",
                )?;
                for position in 0..p {
                    if position != intercept {
                        push_reserved(
                            &mut positions,
                            position,
                            "prepared clustered slope-position capacity exhausted",
                        )?;
                    }
                }
                let mut slope = reserved_filled(
                    reservation,
                    slope_square,
                    0.0_f64,
                    "unable to reserve prepared clustered raw slope block",
                )?;
                for row in 0..slope_dimension {
                    for column in 0..slope_dimension {
                        slope[row * slope_dimension + column] =
                            raw_eigen_input(raw, p, positions[row], positions[column])?;
                    }
                }
                symmetrize(&mut slope, slope_dimension)?;
                let slope_eigen = symmetric_eigensystem(&slope, slope_dimension, reservation)?;
                minimum_raw_slope = Some(
                    slope_eigen
                        .values
                        .iter()
                        .copied()
                        .fold(f64::INFINITY, f64::min),
                );
                eig_error = eig_error.max(slope_eigen.reconstruction_error);
                orthogonality_error = orthogonality_error.max(slope_eigen.orthogonality_error);
                let (fixed_slope, clipped_count) =
                    reconstruct_clipped(&slope_eigen, slope_dimension, reservation)?;
                transform_error = transform_error.max(certify_clipped_reconstruction(
                    &slope_eigen,
                    &fixed_slope,
                    slope_dimension,
                )?);
                slope_clipped = clipped_count;
                slope_block_adjusted = clipped_count > 0;
                let fixed_slope_eigen =
                    symmetric_eigensystem(&fixed_slope, slope_dimension, reservation)?;
                minimum_restored_slope = Some(
                    fixed_slope_eigen
                        .values
                        .iter()
                        .copied()
                        .fold(f64::INFINITY, f64::min),
                );
                let slope_scale = covariance_scale(&fixed_slope, slope_dimension)?;
                if minimum_restored_slope.unwrap_or(0.0) < -tolerance * slope_scale {
                    return Err(invalid_data(
                        "prepared clustered restored slope block is materially indefinite",
                    ));
                }
                restored_slope_rank = Some(
                    fixed_slope_eigen
                        .values
                        .iter()
                        .filter(|value| **value > tolerance * slope_scale)
                        .count(),
                );
                eig_error = eig_error.max(fixed_slope_eigen.reconstruction_error);
                orthogonality_error =
                    orthogonality_error.max(fixed_slope_eigen.orthogonality_error);
                for row in 0..slope_dimension {
                    for column in 0..slope_dimension {
                        output[positions[row] * p + positions[column]] =
                            fixed_slope[row * slope_dimension + column];
                    }
                }
                slope_block_restored = true;
            }
        }
    }
    let (pre_restore_eigen, pre_restore_scale) = if full_adjusted {
        let mut pre_restore = reconstruct_clipped(&raw_eigen, p, reservation)?.0;
        symmetrize(&mut pre_restore, p)?;
        let eigen = symmetric_eigensystem(&pre_restore, p, reservation)?;
        let pre_restore_minimum = eigen.values.iter().copied().fold(f64::INFINITY, f64::min);
        let pre_restore_scale = covariance_scale(&pre_restore, p)?;
        if pre_restore_minimum < -tolerance * pre_restore_scale {
            return Err(invalid_data(
                "prepared clustered clipped full covariance is materially indefinite",
            ));
        }
        (eigen, pre_restore_scale)
    } else {
        (
            symmetric_eigensystem(&output, p, reservation)?,
            covariance_scale(&output, p)?,
        )
    };
    eig_error = eig_error.max(pre_restore_eigen.reconstruction_error);
    orthogonality_error = orthogonality_error.max(pre_restore_eigen.orthogonality_error);
    let minimum_pre_restore = pre_restore_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    if minimum_pre_restore < -tolerance * pre_restore_scale {
        return Err(invalid_data(
            "prepared clustered pre-restore full covariance is materially indefinite",
        ));
    }
    let pre_restore_full_rank = pre_restore_eigen
        .values
        .iter()
        .filter(|value| **value > tolerance * pre_restore_scale)
        .count();
    symmetrize(&mut output, p)?;
    let final_eigen = symmetric_eigensystem(&output, p, reservation)?;
    eig_error = eig_error.max(final_eigen.reconstruction_error);
    orthogonality_error = orthogonality_error.max(final_eigen.orthogonality_error);
    let minimum_final = final_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let final_scale = covariance_scale(&output, p)?;
    let strict_final_full_psd = !(cluster_dimensions > 1
        && full_adjusted
        && reported_intercept_position(prepared.clustered_ols(), fit)?.is_some()
        && p > 1
        && slope_block_restored);
    if strict_final_full_psd && minimum_final < -tolerance * final_scale {
        return Err(invalid_data(format!(
            "prepared clustered final covariance is materially indefinite: min={minimum_final:.17e}, scale={final_scale:.17e}, adjusted={full_adjusted}, slope_restored={slope_block_restored}",
        )));
    }
    let state = if cluster_dimensions == 1 {
        PreparedOlsClusteredPsdState::OneWayStrict
    } else if !full_adjusted {
        PreparedOlsClusteredPsdState::MultiwayRawPsd
    } else if slope_block_restored {
        PreparedOlsClusteredPsdState::MultiwayFullClippedSlopeRestored
    } else {
        PreparedOlsClusteredPsdState::MultiwayFullClipped
    };
    Ok((
        output,
        PreparedOlsClusteredPsdReport {
            state,
            full_adjusted,
            slope_block_restored,
            slope_block_adjusted,
            full_clipped_eigenvalues: full_clipped,
            slope_clipped_eigenvalues: slope_clipped,
            minimum_raw_full_eigenvalue: minimum_raw,
            minimum_pre_restore_full_eigenvalue: minimum_pre_restore,
            minimum_raw_slope_eigenvalue: minimum_raw_slope,
            minimum_restored_slope_eigenvalue: minimum_restored_slope,
            minimum_final_full_eigenvalue: minimum_final,
            pre_restore_full_rank,
            restored_slope_rank,
            strict_final_full_psd,
            transform_reconstruction_relative_error: transform_error,
            eigensystem_relative_error: eig_error,
            orthogonality_relative_error: orthogonality_error,
        },
    ))
}

fn raw_eigen_input(raw: &[f64], p: usize, row: usize, column: usize) -> Result<f64> {
    let value = 0.5 * (raw[row * p + column] + raw[column * p + row]);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid_data(
            "prepared clustered raw slope-block value is not finite",
        ))
    }
}

fn reported_intercept_position<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
) -> Result<Option<usize>> {
    let mut intercept = None;
    for (position, coordinate) in fit.bread().coordinates().iter().enumerate() {
        let is_intercept = coordinate.kind
            == PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept
            || matches!(
                prepared
                    .fit_model()
                    .columns
                    .get(coordinate.generated_column_index)
                    .map(|column| &column.kind),
                Some(PreparedOlsColumnKind::Intercept)
            );
        if is_intercept && intercept.replace(position).is_some() {
            return Err(invalid_argument(
                "prepared clustered covariance has multiple reported intercepts",
            ));
        }
    }
    Ok(intercept)
}

fn symmetric_eigensystem<R: PreparedInferenceReservation>(
    matrix: &[f64],
    dimension: usize,
    reservation: &mut R,
) -> Result<SymmetricEigensystem> {
    let square = checked_square(
        dimension,
        "prepared clustered eigensystem dimensions overflow usize",
    )?;
    if matrix.len() != square {
        return Err(invalid_argument(
            "prepared clustered eigensystem dimensions are inconsistent",
        ));
    }
    let mut work = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered eigensystem work matrix",
    )?;
    work.copy_from_slice(matrix);
    symmetrize(&mut work, dimension)?;
    let mut vectors = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered eigenvectors",
    )?;
    for index in 0..dimension {
        vectors[index * dimension + index] = 1.0;
    }
    if dimension > 1 {
        let scale = covariance_scale(&work, dimension)?;
        let threshold = 128.0 * f64::EPSILON * scale;
        let maximum_iterations = dimension
            .checked_mul(dimension)
            .and_then(|value| value.checked_mul(128))
            .ok_or_else(|| allocation("prepared clustered Jacobi iteration limit overflows"))?;
        let mut converged = false;
        for _ in 0..maximum_iterations {
            let mut pivot_row = 0_usize;
            let mut pivot_column = 1_usize;
            let mut maximum = 0.0_f64;
            for row in 0..dimension {
                for column in (row + 1)..dimension {
                    let value = work[row * dimension + column].abs();
                    if value > maximum {
                        maximum = value;
                        pivot_row = row;
                        pivot_column = column;
                    }
                }
            }
            if maximum <= threshold {
                converged = true;
                break;
            }
            jacobi_rotate(&mut work, &mut vectors, dimension, pivot_row, pivot_column)?;
        }
        if !converged {
            return Err(invalid_data(
                "prepared clustered symmetric eigensystem did not converge",
            ));
        }
    }
    let mut values = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared clustered eigenvalues",
    )?;
    for index in 0..dimension {
        values[index] = work[index * dimension + index];
        if !values[index].is_finite() {
            return Err(invalid_data("prepared clustered eigenvalue is not finite"));
        }
    }
    let (reconstruction_error, orthogonality_error) =
        certify_eigensystem(matrix, &values, &vectors, dimension)?;
    Ok(SymmetricEigensystem {
        values,
        vectors,
        reconstruction_error,
        orthogonality_error,
    })
}

fn jacobi_rotate(
    matrix: &mut [f64],
    vectors: &mut [f64],
    dimension: usize,
    p: usize,
    q: usize,
) -> Result<()> {
    let app = matrix[p * dimension + p];
    let aqq = matrix[q * dimension + q];
    let apq = matrix[p * dimension + q];
    if apq == 0.0 {
        return Ok(());
    }
    let tau = (aqq - app) / (2.0 * apq);
    let t = if tau >= 0.0 {
        1.0 / (tau + (1.0 + tau * tau).sqrt())
    } else {
        -1.0 / (-tau + (1.0 + tau * tau).sqrt())
    };
    let cosine = 1.0 / (1.0 + t * t).sqrt();
    let sine = t * cosine;
    if !cosine.is_finite() || !sine.is_finite() {
        return Err(invalid_data(
            "prepared clustered Jacobi rotation is not finite",
        ));
    }
    for index in 0..dimension {
        if index == p || index == q {
            continue;
        }
        let aip = matrix[index * dimension + p];
        let aiq = matrix[index * dimension + q];
        let new_p = cosine * aip - sine * aiq;
        let new_q = sine * aip + cosine * aiq;
        matrix[index * dimension + p] = new_p;
        matrix[p * dimension + index] = new_p;
        matrix[index * dimension + q] = new_q;
        matrix[q * dimension + index] = new_q;
    }
    matrix[p * dimension + p] = app - t * apq;
    matrix[q * dimension + q] = aqq + t * apq;
    matrix[p * dimension + q] = 0.0;
    matrix[q * dimension + p] = 0.0;
    for row in 0..dimension {
        let vip = vectors[row * dimension + p];
        let viq = vectors[row * dimension + q];
        vectors[row * dimension + p] = cosine * vip - sine * viq;
        vectors[row * dimension + q] = sine * vip + cosine * viq;
    }
    Ok(())
}

fn certify_eigensystem(
    matrix: &[f64],
    values: &[f64],
    vectors: &[f64],
    dimension: usize,
) -> Result<(f64, f64)> {
    let mut reconstruction_error = 0.0_f64;
    let mut orthogonality_error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        let mut orthogonal_row_error = 0.0_f64;
        for column in 0..dimension {
            let mut reconstructed = 0.0_f64;
            let mut reconstructed_correction = 0.0_f64;
            let mut inner_product = 0.0_f64;
            let mut inner_correction = 0.0_f64;
            for eigen in 0..dimension {
                compensated_add(
                    vectors[row * dimension + eigen]
                        * values[eigen]
                        * vectors[column * dimension + eigen],
                    &mut reconstructed,
                    &mut reconstructed_correction,
                )?;
                compensated_add(
                    vectors[row * dimension + eigen] * vectors[column * dimension + eigen],
                    &mut inner_product,
                    &mut inner_correction,
                )?;
            }
            reconstructed += reconstructed_correction;
            inner_product += inner_correction;
            row_error += (matrix[row * dimension + column] - reconstructed).abs();
            row_scale += matrix[row * dimension + column]
                .abs()
                .max(reconstructed.abs());
            orthogonal_row_error += (inner_product - f64::from(u8::from(row == column))).abs();
        }
        reconstruction_error = reconstruction_error.max(row_error / row_scale);
        orthogonality_error = orthogonality_error.max(orthogonal_row_error);
    }
    Ok((reconstruction_error, orthogonality_error))
}

fn reconstruct_clipped<R: PreparedInferenceReservation>(
    eigen: &SymmetricEigensystem,
    dimension: usize,
    reservation: &mut R,
) -> Result<(Vec<f64>, usize)> {
    let square = checked_square(
        dimension,
        "prepared clustered clipped reconstruction dimensions overflow usize",
    )?;
    let mut output = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered clipped reconstruction",
    )?;
    let clipped = eigen.values.iter().filter(|value| **value < 0.0).count();
    for row in 0..dimension {
        for column in row..dimension {
            let mut value = 0.0_f64;
            let mut correction = 0.0_f64;
            for index in 0..dimension {
                compensated_add(
                    eigen.vectors[row * dimension + index]
                        * eigen.values[index].max(0.0)
                        * eigen.vectors[column * dimension + index],
                    &mut value,
                    &mut correction,
                )?;
            }
            output[row * dimension + column] = value + correction;
            output[column * dimension + row] = value + correction;
        }
    }
    Ok((output, clipped))
}

fn certify_clipped_reconstruction(
    eigen: &SymmetricEigensystem,
    actual: &[f64],
    dimension: usize,
) -> Result<f64> {
    let square = checked_square(
        dimension,
        "prepared clustered clipped-certificate dimensions overflow usize",
    )?;
    if actual.len() != square || eigen.values.len() != dimension || eigen.vectors.len() != square {
        return Err(invalid_argument(
            "prepared clustered clipped-certificate dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..dimension {
            let mut expected = 0.0_f64;
            let mut correction = 0.0_f64;
            for index in 0..dimension {
                compensated_add(
                    eigen.vectors[row * dimension + index]
                        * eigen.values[index].max(0.0)
                        * eigen.vectors[column * dimension + index],
                    &mut expected,
                    &mut correction,
                )?;
            }
            expected += correction;
            let observed = actual[row * dimension + column];
            if !observed.is_finite() || !expected.is_finite() {
                return Err(invalid_data(
                    "prepared clustered clipped reconstruction is not finite",
                ));
            }
            row_error += (observed - expected).abs();
            row_scale += observed.abs().max(expected.abs());
        }
        error = error.max(row_error / row_scale);
    }
    Ok(error)
}

fn certify_psd_state<P: PreparedOlsClusteredView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    covariance: &PreparedOlsClusteredCovariance,
    tolerance: f64,
    reservation: &mut R,
) -> Result<(f64, f64, f64)> {
    let p = covariance.dimension();
    let square = checked_square(
        p,
        "prepared clustered independent PSD dimensions overflow usize",
    )?;
    if covariance.raw_values.len() != square || covariance.values.len() != square {
        return Err(invalid_argument(
            "prepared clustered independent PSD dimensions are inconsistent",
        ));
    }
    let q = prepared.clustered_design().len();
    let intercept = reported_intercept_position(prepared.clustered_ols(), fit)?;
    if p == 0 {
        let expected_state = if q == 1 {
            PreparedOlsClusteredPsdState::OneWayStrict
        } else {
            PreparedOlsClusteredPsdState::MultiwayRawPsd
        };
        let state_error = f64::from(u8::from(
            covariance.psd.state != expected_state
                || covariance.psd.full_adjusted
                || covariance.psd.slope_block_restored
                || !covariance.psd.strict_final_full_psd,
        ));
        return Ok((0.0, 0.0, state_error));
    }

    let mut raw = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered independent raw PSD matrix",
    )?;
    raw.copy_from_slice(&covariance.raw_values);
    symmetrize(&mut raw, p)?;
    let raw_eigen = symmetric_eigensystem(&raw, p, reservation)?;
    let raw_minimum = raw_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let expected_full_adjustment = q > 1 && raw_minimum < 0.0;
    let (mut expected, expected_full_clipped) = if expected_full_adjustment {
        reconstruct_clipped(&raw_eigen, p, reservation)?
    } else {
        (raw, 0)
    };
    let pre_restore_eigen = symmetric_eigensystem(&expected, p, reservation)?;
    let pre_restore_minimum = pre_restore_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let pre_restore_scale = covariance_scale(&expected, p)?;
    let pre_restore_error = (-pre_restore_minimum).max(0.0) / pre_restore_scale;
    let expected_pre_restore_rank = pre_restore_eigen
        .values
        .iter()
        .filter(|value| **value > tolerance * pre_restore_scale)
        .count();

    let expected_slope_restore = expected_full_adjustment && intercept.is_some() && p > 1;
    let mut raw_slope_minimum = None;
    let mut restored_slope_minimum = None;
    let mut expected_slope_rank = None;
    let mut expected_slope_clipped = 0_usize;
    let mut restored_slope_error = 0.0_f64;
    if expected_slope_restore {
        let intercept = intercept.ok_or_else(|| {
            invalid_argument("prepared clustered independent intercept state is inconsistent")
        })?;
        let slopes = p - 1;
        let slope_square = checked_square(
            slopes,
            "prepared clustered independent slope dimensions overflow usize",
        )?;
        let mut positions = reserved_empty(
            reservation,
            slopes,
            "unable to reserve prepared clustered independent slope positions",
        )?;
        for position in 0..p {
            if position != intercept {
                push_reserved(
                    &mut positions,
                    position,
                    "prepared clustered independent slope-position capacity exhausted",
                )?;
            }
        }
        let mut slope = reserved_filled(
            reservation,
            slope_square,
            0.0_f64,
            "unable to reserve prepared clustered independent raw slope block",
        )?;
        for row in 0..slopes {
            for column in 0..slopes {
                slope[row * slopes + column] =
                    raw_eigen_input(&covariance.raw_values, p, positions[row], positions[column])?;
            }
        }
        symmetrize(&mut slope, slopes)?;
        let slope_eigen = symmetric_eigensystem(&slope, slopes, reservation)?;
        raw_slope_minimum = Some(
            slope_eigen
                .values
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min),
        );
        let (restored, clipped) = reconstruct_clipped(&slope_eigen, slopes, reservation)?;
        expected_slope_clipped = clipped;
        let restored_eigen = symmetric_eigensystem(&restored, slopes, reservation)?;
        let restored_minimum = restored_eigen
            .values
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let slope_scale = covariance_scale(&restored, slopes)?;
        restored_slope_error = (-restored_minimum).max(0.0) / slope_scale;
        restored_slope_minimum = Some(restored_minimum);
        expected_slope_rank = Some(
            restored_eigen
                .values
                .iter()
                .filter(|value| **value > tolerance * slope_scale)
                .count(),
        );
        for row in 0..slopes {
            for column in 0..slopes {
                expected[positions[row] * p + positions[column]] = restored[row * slopes + column];
            }
        }
    }
    symmetrize(&mut expected, p)?;
    let transform_error = matrix_relative_infinity_norm(&expected, covariance.values(), p)?;
    let final_eigen = symmetric_eigensystem(covariance.values(), p, reservation)?;
    let final_minimum = final_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let final_scale = covariance_scale(covariance.values(), p)?;
    let expected_strict = !expected_slope_restore;
    let expected_state = if q == 1 {
        PreparedOlsClusteredPsdState::OneWayStrict
    } else if !expected_full_adjustment {
        PreparedOlsClusteredPsdState::MultiwayRawPsd
    } else if expected_slope_restore {
        PreparedOlsClusteredPsdState::MultiwayFullClippedSlopeRestored
    } else {
        PreparedOlsClusteredPsdState::MultiwayFullClipped
    };
    let diagnostics_error = [
        relative_difference(
            covariance.psd.minimum_raw_full_eigenvalue,
            raw_minimum,
            raw_minimum.abs().max(1.0),
        )?,
        relative_difference(
            covariance.psd.minimum_pre_restore_full_eigenvalue,
            pre_restore_minimum,
            pre_restore_minimum.abs().max(1.0),
        )?,
        relative_difference(
            covariance.psd.minimum_final_full_eigenvalue,
            final_minimum,
            final_minimum.abs().max(1.0),
        )?,
        optional_error(
            covariance.psd.minimum_raw_slope_eigenvalue,
            raw_slope_minimum,
        )?,
        optional_error(
            covariance.psd.minimum_restored_slope_eigenvalue,
            restored_slope_minimum,
        )?,
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    let state_error = f64::from(u8::from(
        covariance.psd.state != expected_state
            || covariance.psd.full_adjusted != expected_full_adjustment
            || covariance.psd.full_clipped_eigenvalues != expected_full_clipped
            || covariance.psd.slope_block_restored != expected_slope_restore
            || covariance.psd.slope_block_adjusted != (expected_slope_clipped > 0)
            || covariance.psd.slope_clipped_eigenvalues != expected_slope_clipped
            || covariance.psd.pre_restore_full_rank != expected_pre_restore_rank
            || covariance.psd.restored_slope_rank != expected_slope_rank
            || covariance.psd.strict_final_full_psd != expected_strict,
    ));
    let mut finite_diagonal_error = 0.0_f64;
    for index in 0..p {
        let diagonal = covariance.values[index * p + index];
        if !diagonal.is_finite()
            || !covariance.standard_errors[index].is_finite()
            || diagonal < 0.0
            || covariance.standard_errors[index] < 0.0
        {
            finite_diagonal_error = 1.0;
        }
    }
    if covariance.values().iter().any(|value| !value.is_finite()) {
        finite_diagonal_error = 1.0;
    }
    let strict_psd_error = if expected_strict {
        (-final_minimum).max(0.0) / final_scale
    } else {
        0.0
    };
    let final_structure_error = transform_error
        .max(diagnostics_error)
        .max(state_error)
        .max(finite_diagonal_error)
        .max(symmetry_error(covariance.values(), p)?)
        .max(standard_error_error(covariance)?)
        .max(strict_psd_error);
    Ok((
        pre_restore_error,
        restored_slope_error,
        final_structure_error,
    ))
}

fn matrix_relative_infinity_norm(left: &[f64], right: &[f64], dimension: usize) -> Result<f64> {
    let square = checked_square(
        dimension,
        "prepared clustered matrix-certificate dimensions overflow usize",
    )?;
    if left.len() != square || right.len() != square {
        return Err(invalid_argument(
            "prepared clustered matrix-certificate dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..dimension {
            let left_value = left[row * dimension + column];
            let right_value = right[row * dimension + column];
            if !left_value.is_finite() || !right_value.is_finite() {
                return Err(invalid_data(
                    "prepared clustered matrix certificate is not finite",
                ));
            }
            row_error += (left_value - right_value).abs();
            row_scale += left_value.abs().max(right_value.abs());
        }
        error = error.max(row_error / row_scale);
    }
    Ok(error)
}

fn build_standard_errors<R: PreparedInferenceReservation>(
    covariance: &[f64],
    dimension: usize,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let square = checked_square(
        dimension,
        "prepared clustered standard-error dimensions overflow usize",
    )?;
    if covariance.len() != square {
        return Err(invalid_argument(
            "prepared clustered standard-error dimensions are inconsistent",
        ));
    }
    let mut standard_errors = reserved_filled(
        reservation,
        dimension,
        0.0_f64,
        "unable to reserve prepared clustered standard errors",
    )?;
    for index in 0..dimension {
        let diagonal = covariance[index * dimension + index];
        if !diagonal.is_finite() || diagonal < 0.0 {
            return Err(invalid_data(
                "prepared clustered covariance diagonal is invalid",
            ));
        }
        standard_errors[index] = diagonal.sqrt();
    }
    Ok(standard_errors)
}

fn build_wald<P: PreparedOlsFitView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    covariance: &PreparedOlsClusteredCovariance,
    slope_dimension: usize,
    tolerance: f64,
    reservation: &mut R,
) -> Result<WaldCalculation> {
    if slope_dimension == 0 {
        return Ok(WaldCalculation {
            statistic: None,
            solve_relative_infinity_norm: 0.0,
        });
    }
    let dimension = covariance.dimension();
    let mut positions = reserved_empty(
        reservation,
        slope_dimension,
        "unable to reserve prepared clustered Wald slope coordinates",
    )?;
    for (position, coordinate) in fit.bread().coordinates().iter().enumerate() {
        if coordinate.kind == PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept {
            continue;
        }
        let column = prepared
            .fit_model()
            .columns
            .get(coordinate.generated_column_index)
            .ok_or_else(|| {
                invalid_argument("prepared clustered Wald coordinate is out of range")
            })?;
        if matches!(column.kind, PreparedOlsColumnKind::Intercept) {
            continue;
        }
        push_reserved(
            &mut positions,
            position,
            "prepared clustered Wald slope-coordinate capacity exhausted",
        )?;
    }
    if positions.len() != slope_dimension {
        return Err(invalid_argument(
            "prepared clustered Wald slope count differs from finite model DoF",
        ));
    }
    let square = checked_square(
        slope_dimension,
        "prepared clustered Wald dimensions overflow usize",
    )?;
    let mut factor = reserved_filled(
        reservation,
        square,
        0.0_f64,
        "unable to reserve prepared clustered Wald factor",
    )?;
    let scale = positions
        .iter()
        .map(|position| covariance.values[*position * dimension + *position].abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let threshold = tolerance * scale;
    for row in 0..slope_dimension {
        for column in 0..=row {
            let mut value = covariance.values[positions[row] * dimension + positions[column]];
            for inner in 0..column {
                value -= factor[row * slope_dimension + inner]
                    * factor[column * slope_dimension + inner];
            }
            if row == column {
                if value < -threshold {
                    return Err(invalid_data(
                        "prepared clustered Wald slope covariance is indefinite",
                    ));
                }
                if value <= threshold {
                    return Ok(WaldCalculation {
                        statistic: None,
                        solve_relative_infinity_norm: 0.0,
                    });
                }
                factor[row * slope_dimension + column] = value.sqrt();
            } else {
                factor[row * slope_dimension + column] =
                    value / factor[column * slope_dimension + column];
            }
        }
    }
    let mut solution = reserved_filled(
        reservation,
        slope_dimension,
        0.0_f64,
        "unable to reserve prepared clustered Wald solution",
    )?;
    for (slope, position) in positions.iter().copied().enumerate() {
        solution[slope] = fitted_coordinate_value(fit, position)?;
    }
    for row in 0..slope_dimension {
        let mut value = solution[row];
        for column in 0..row {
            value -= factor[row * slope_dimension + column] * solution[column];
        }
        solution[row] = value / factor[row * slope_dimension + row];
    }
    for row in (0..slope_dimension).rev() {
        let mut value = solution[row];
        for column in (row + 1)..slope_dimension {
            value -= factor[column * slope_dimension + row] * solution[column];
        }
        solution[row] = value / factor[row * slope_dimension + row];
    }
    let mut quadratic = 0.0_f64;
    let mut quadratic_correction = 0.0_f64;
    let mut solve_error = 0.0_f64;
    for row in 0..slope_dimension {
        let coefficient = fitted_coordinate_value(fit, positions[row])?;
        compensated_add(
            coefficient * solution[row],
            &mut quadratic,
            &mut quadratic_correction,
        )?;
        let mut reconstructed = 0.0_f64;
        let mut correction = 0.0_f64;
        let mut row_scale = coefficient.abs().max(1.0);
        for column in 0..slope_dimension {
            let product = covariance.values[positions[row] * dimension + positions[column]]
                * solution[column];
            compensated_add(product, &mut reconstructed, &mut correction)?;
            row_scale += product.abs();
        }
        solve_error = solve_error.max((reconstructed + correction - coefficient).abs() / row_scale);
    }
    quadratic += quadratic_correction;
    if !quadratic.is_finite() || quadratic < 0.0 || !solve_error.is_finite() {
        return Err(invalid_data(
            "prepared clustered Wald quadratic or solve certificate is invalid",
        ));
    }
    let statistic = quadratic / exact_integer_as_f64(slope_dimension as u128)?;
    if !statistic.is_finite() {
        return Err(invalid_data(
            "prepared clustered Wald statistic is not finite",
        ));
    }
    Ok(WaldCalculation {
        statistic: Some(statistic),
        solve_relative_infinity_norm: solve_error,
    })
}

fn fitted_coordinate_value(fit: &CertifiedPreparedOlsFit, position: usize) -> Result<f64> {
    let coordinate =
        fit.bread().coordinates().get(position).ok_or_else(|| {
            invalid_argument("prepared clustered fitted coordinate is out of range")
        })?;
    let fitted = fit
        .coefficients()
        .iter()
        .find(|coefficient| {
            coefficient.generated_column_index == Some(coordinate.generated_column_index)
        })
        .ok_or_else(|| {
            invalid_argument("prepared clustered fitted coordinate has no coefficient")
        })?;
    if fitted.status != PreparedOlsFitStatus::Kept {
        return Err(invalid_argument(
            "prepared clustered Wald coordinate is not a kept finite coefficient",
        ));
    }
    fitted
        .value
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid_data("prepared clustered Wald coefficient is not finite"))
}

fn certify_statistics(
    actual: PreparedOlsFitStatistics,
    rss: f64,
    tss: f64,
    within_tss: f64,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    wald: WaldCalculation,
) -> Result<f64> {
    let mut expected = build_statistics(rss, tss, within_tss, weights, dof)?;
    expected.f_statistic = wald.statistic;
    let mut error = 0.0_f64;
    for (left, right) in [
        (actual.rss, expected.rss),
        (actual.tss, expected.tss),
        (actual.within_tss, expected.within_tss),
        (
            actual.root_mean_squared_error,
            expected.root_mean_squared_error,
        ),
    ] {
        error = error.max(relative_difference(
            left,
            right,
            left.abs().max(right.abs()).max(1.0),
        )?);
    }
    for (left, right) in [
        (actual.r_squared, expected.r_squared),
        (actual.adjusted_r_squared, expected.adjusted_r_squared),
        (actual.within_r_squared, expected.within_r_squared),
        (
            actual.adjusted_within_r_squared,
            expected.adjusted_within_r_squared,
        ),
        (actual.f_statistic, expected.f_statistic),
        (
            actual.gaussian_log_likelihood,
            expected.gaussian_log_likelihood,
        ),
        (
            actual.gaussian_null_log_likelihood,
            expected.gaussian_null_log_likelihood,
        ),
    ] {
        error = error.max(optional_error(left, right)?);
    }
    Ok(error.max(wald.solve_relative_infinity_norm))
}

fn optional_error(actual: Option<f64>, expected: Option<f64>) -> Result<f64> {
    match (actual, expected) {
        (Some(actual), Some(expected)) if actual.is_finite() && expected.is_finite() => {
            relative_difference(actual, expected, actual.abs().max(expected.abs()).max(1.0))
        }
        (None, None) => Ok(0.0),
        _ => Ok(1.0),
    }
}

fn symmetrize(matrix: &mut [f64], dimension: usize) -> Result<()> {
    if matrix.len() != dimension * dimension {
        return Err(invalid_argument(
            "prepared clustered symmetry dimensions are inconsistent",
        ));
    }
    for row in 0..dimension {
        for column in (row + 1)..dimension {
            let value = 0.5 * (matrix[row * dimension + column] + matrix[column * dimension + row]);
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared clustered symmetrized value is not finite",
                ));
            }
            matrix[row * dimension + column] = value;
            matrix[column * dimension + row] = value;
        }
    }
    Ok(())
}

fn symmetry_error(matrix: &[f64], dimension: usize) -> Result<f64> {
    if matrix.len() != dimension * dimension {
        return Err(invalid_argument(
            "prepared clustered symmetry certificate dimensions are inconsistent",
        ));
    }
    let mut error = 0.0_f64;
    for row in 0..dimension {
        let mut row_error = 0.0_f64;
        let mut row_scale = 1.0_f64;
        for column in 0..dimension {
            let left = matrix[row * dimension + column];
            let right = matrix[column * dimension + row];
            row_error += (left - right).abs();
            row_scale += left.abs().max(right.abs());
        }
        error = error.max(row_error / row_scale);
    }
    Ok(error)
}

fn standard_error_error(covariance: &PreparedOlsClusteredCovariance) -> Result<f64> {
    let dimension = covariance.dimension();
    let mut error = 0.0_f64;
    for index in 0..dimension {
        let expected = covariance.values[index * dimension + index].max(0.0).sqrt();
        error = error.max(relative_difference(
            covariance.standard_errors[index],
            expected,
            covariance.standard_errors[index]
                .abs()
                .max(expected.abs())
                .max(1.0),
        )?);
    }
    Ok(error)
}

fn covariance_scale(matrix: &[f64], dimension: usize) -> Result<f64> {
    if matrix.len() != dimension * dimension {
        return Err(invalid_argument(
            "prepared clustered covariance scale dimensions are inconsistent",
        ));
    }
    let scale = matrix
        .iter()
        .copied()
        .map(f64::abs)
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    if scale.is_finite() {
        Ok(scale)
    } else {
        Err(invalid_data(
            "prepared clustered covariance scale is not finite",
        ))
    }
}

fn reserved_empty<T, R: PreparedInferenceReservation>(
    reservation: &mut R,
    elements: usize,
    label: &'static str,
) -> Result<Vec<T>> {
    let mut output = Vec::new();
    reservation.reserve(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "prepared clustered reservation returned insufficient capacity",
        ));
    }
    Ok(output)
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ols::prepare_ols_clustered_data;

    struct CountingReservation {
        calls: usize,
        fail_at: Option<usize>,
        undersize: bool,
    }

    impl CountingReservation {
        const fn succeeds() -> Self {
            Self {
                calls: 0,
                fail_at: None,
                undersize: false,
            }
        }

        const fn fails_at(call: usize) -> Self {
            Self {
                calls: 0,
                fail_at: Some(call),
                undersize: false,
            }
        }

        const fn undersized() -> Self {
            Self {
                calls: 0,
                fail_at: None,
                undersize: true,
            }
        }
    }

    impl PreparedInferenceReservation for CountingReservation {
        fn reserve<T>(
            &mut self,
            target: &mut Vec<T>,
            elements: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected prepared clustered inference reservation failure",
                ));
            }
            let requested = if self.undersize && elements > 0 {
                elements - 1
            } else {
                elements
            };
            target
                .try_reserve_exact(requested)
                .map_err(|_| allocation("test prepared clustered reservation failed"))
        }
    }

    #[test]
    fn dense_subset_plan_is_mixed_radix_and_strictly_scratch_bounded() {
        let first = [0_u32, 1, 0, 1];
        let second = [0_u32, 0, 1, 1];
        let dimensions = [&first[..], &second[..]];
        let plan = dense_subset_plan(&dimensions, 0b11, 4, 2)
            .expect("valid dense plan")
            .expect("small dense plan is admitted");
        assert_eq!(plan.radices[..2], [2, 2]);
        assert_eq!(plan.group_slots, 4);
        assert_eq!(
            (0..4)
                .map(|row| dense_subset_group(&dimensions, 0b11, plan, row))
                .collect::<Result<Vec<_>>>()
                .expect("mixed-radix groups"),
            [0, 2, 1, 3],
        );

        let sparse_high_code = [0_u32, 1_000_000];
        assert!(dense_subset_plan(&[&sparse_high_code], 1, 2, 32)
            .expect("valid sparse plan")
            .is_none());
    }

    #[test]
    fn radix_subset_order_matches_lexicographic_cluster_order_with_stable_ties() {
        let first = [1_u32, 0, 1, 0, 1, 0];
        let second = [1_u32, 1, 0, 0, 1, 0];
        let dimensions = [&first[..], &second[..]];
        let mut reservation = CountingReservation::succeeds();
        let joint = build_subset_radix_order(&dimensions, 0b11, 6, &mut reservation)
            .expect("valid radix ordering")
            .expect("compact radix ordering is admitted");
        assert_eq!(joint, [3, 5, 1, 2, 0, 4]);

        let second_only = build_subset_radix_order(&dimensions, 0b10, 6, &mut reservation)
            .expect("valid one-dimension radix ordering")
            .expect("compact radix ordering is admitted");
        assert_eq!(second_only, [2, 3, 5, 0, 1, 4]);
    }

    #[test]
    fn radix_subset_order_covers_more_than_four_cluster_dimensions() {
        let first = [1_u32, 0, 1, 0, 1, 0, 1, 0];
        let second = [0_u32, 1, 1, 0, 0, 1, 1, 0];
        let third = [1_u32, 1, 0, 0, 1, 1, 0, 0];
        let fourth = [0_u32, 0, 0, 0, 1, 1, 1, 1];
        let fifth = [1_u32, 0, 1, 0, 1, 0, 1, 0];
        let dimensions = [&first[..], &second[..], &third[..], &fourth[..], &fifth[..]];
        let mask = 0b1_1111;
        let mut reservation = CountingReservation::succeeds();
        let actual = build_subset_radix_order(&dimensions, mask, first.len(), &mut reservation)
            .expect("valid five-dimensional radix ordering")
            .expect("compact five-dimensional ordering is admitted");
        let mut expected = (0..first.len()).collect::<Vec<_>>();
        expected.sort_unstable_by(|left, right| {
            compare_cluster_rows(&dimensions, mask, *left, *right).then_with(|| left.cmp(right))
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_clustered_inference_reservation_failure_and_undersize_fails_closed() {
        let (request, loaded) = super::super::prepared_clusters::tests::fixture();
        let prepared =
            prepare_ols_clustered_data(&request, &loaded).expect("prepared clustered fixture");
        let mut count = CountingReservation::succeeds();
        fit_with_reservation(&prepared, PreparedOlsClusteredConfig::default(), &mut count)
            .expect("count clustered inference reservations");
        assert!(count.calls >= 20);
        for fail_at in 0..count.calls {
            let mut reservation = CountingReservation::fails_at(fail_at);
            let error = fit_with_reservation(
                &prepared,
                PreparedOlsClusteredConfig::default(),
                &mut reservation,
            )
            .expect_err("injected clustered inference allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "fail_at={fail_at}");
        }
        let error = fit_with_reservation(
            &prepared,
            PreparedOlsClusteredConfig::default(),
            &mut CountingReservation::undersized(),
        )
        .expect_err("undersized clustered inference allocation must fail");
        assert_eq!(error.code, ErrorCode::Allocation);
    }

    #[test]
    fn every_clustered_scientific_mutation_fails_closed() {
        let (mut request, loaded) = super::super::prepared_clusters::tests::fixture();
        request.ols.include_intercept = true;
        let prepared =
            prepare_ols_clustered_data(&request, &loaded).expect("prepared clustered fixture");
        for perturbation in [
            ClusteredPerturbations {
                score_design: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                residual_dof: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                subset_sign: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                subset_groups: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                uncorrected_covariance: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                corrected_covariance: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                correction: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                final_covariance: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                covariance_symmetry: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                standard_error: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                psd_state: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                rss: true,
                ..ClusteredPerturbations::default()
            },
            ClusteredPerturbations {
                f_statistic: true,
                ..ClusteredPerturbations::default()
            },
        ] {
            let mut reservation = SystemPreparedInferenceReservation;
            let error = compose_for_test(&prepared, &mut reservation, perturbation)
                .expect_err("injected clustered scientific corruption must fail");
            assert_eq!(error.code, ErrorCode::InvalidData, "{perturbation:?}");
        }
    }

    fn compose_for_test<R: PreparedInferenceReservation>(
        prepared: &PreparedOlsClusteredData,
        reservation: &mut R,
        perturbations: ClusteredPerturbations,
    ) -> Result<CertifiedPreparedOlsClustered> {
        let config = PreparedOlsClusteredConfig::default();
        let fit = fit_prepared_ols_coefficients_view_map(prepared.ols(), config.fit)?;
        compose(prepared, fit, config, reservation, perturbations)
    }

    #[test]
    fn singular_wald_slope_block_is_explicitly_undefined() {
        let (request, loaded) = super::super::prepared_clusters::tests::fixture();
        let prepared =
            prepare_ols_clustered_data(&request, &loaded).expect("prepared clustered fixture");
        let result =
            fit_prepared_ols_clustered_map(&prepared, PreparedOlsClusteredConfig::default())
                .expect("clustered fixture fit");
        let dimension = result.covariance.dimension();
        let zero = PreparedOlsClusteredCovariance {
            correction: result.covariance.correction,
            raw_values: vec![0.0; dimension * dimension],
            values: vec![0.0; dimension * dimension],
            standard_errors: vec![0.0; dimension],
            psd: PreparedOlsClusteredPsdReport {
                state: PreparedOlsClusteredPsdState::OneWayStrict,
                full_adjusted: false,
                slope_block_restored: false,
                slope_block_adjusted: false,
                full_clipped_eigenvalues: 0,
                slope_clipped_eigenvalues: 0,
                minimum_raw_full_eigenvalue: 0.0,
                minimum_pre_restore_full_eigenvalue: 0.0,
                minimum_raw_slope_eigenvalue: None,
                minimum_restored_slope_eigenvalue: None,
                minimum_final_full_eigenvalue: 0.0,
                pre_restore_full_rank: 0,
                restored_slope_rank: None,
                strict_final_full_psd: true,
                transform_reconstruction_relative_error: 0.0,
                eigensystem_relative_error: 0.0,
                orthogonality_relative_error: 0.0,
            },
        };
        let mut reservation = SystemPreparedInferenceReservation;
        let wald = build_wald(
            prepared.ols(),
            result.fit(),
            &zero,
            result.dof().finite_model_dof,
            PreparedOlsClusteredConfig::default().certificate_tolerance,
            &mut reservation,
        )
        .expect("singular Wald policy");
        assert_eq!(wald.statistic, None);
        assert_eq!(wald.solve_relative_infinity_norm, 0.0);
    }
}
