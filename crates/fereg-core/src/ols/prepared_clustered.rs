use super::prepared_wald::{calculate_wald, WaldCalculation};

use crate::error::{allocation, invalid_argument, invalid_data, Result};
use crate::memory::push_reserved;
use crate::parallel::ThreadTeam;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::sandwich::{
    bread_meat_bread, prepare_cluster_score_batch, reduce_qualified_cluster_subset,
    reduce_qualified_clustered_score_meats, QualifiedClusterCertificateRoute,
    QualifiedClusterRoutePolicy, QualifiedClusterScoreBatch, QualifiedClusterSubsetMeatProvider,
    QualifiedClusterSubsetMeats, SandwichCorrectionPolicy, SandwichProductMode,
    SandwichScoreMultiplier, SandwichScorePair,
};
use crate::topology_dof::CategoricalTopologyDofReport;

use super::prepared_clustered_view::PreparedOlsClusteredView;
use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView};
#[cfg(test)]
use super::prepared_scores::take_prepared_ols_score_product;
use super::prepared_scores::PreparedOlsScoreProduct;
use super::prepared_scores::{
    build_prepared_ols_score_design, reserved_filled, CertifiedPreparedOlsScoreDesign,
    PreparedInferenceReservation, SystemPreparedInferenceReservation,
};
use super::prepared_unadjusted::{
    build_reference_weight_report_view, build_residual_dof_view, build_statistics, checked_square,
    compensated_add, exact_integer_as_f64, independent_outcome_sum, independent_residual_sum,
    relative_difference, validate_sum_of_squares, ReferenceWeightView,
};
use super::{
    CertifiedPreparedOlsFit, PreparedCategoricalOlsClusteredData, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsBreadCoordinateKind, PreparedOlsClusteredData, PreparedOlsColumnKind,
    PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatistics,
    PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport,
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
    /// Historical wire state; next never restores a slope block after clipping.
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
    /// The final full covariance is always checked for positive semidefiniteness.
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

/// Same-call certified fit with its clustered covariance payload.
pub type CertifiedPreparedOlsClustered<A = CategoricalTopologyDofReport> =
    super::prepared_products::CertifiedPreparedOlsInference<
        A,
        PreparedOlsClusteredDofReport,
        PreparedOlsClusteredCovariance,
        PreparedOlsClusteredCertificate,
        PreparedOlsClusteredMetadata,
    >;

/// Canonical topology identities for categorical and ordinary consumers.
pub type CertifiedPreparedCategoricalOlsClustered = CertifiedPreparedOlsClustered;

impl super::prepared_products::PreparedOlsInferenceBuffers for PreparedOlsClusteredCovariance {
    fn inference_owned_bytes(&self) -> u128 {
        use crate::memory::vec_capacity_bytes as bytes;
        bytes(&self.values) + bytes(&self.standard_errors) + bytes(&self.raw_values)
    }
}

/// Cluster identities and subset certificates retained after score consumption.
#[derive(Debug, PartialEq)]
pub struct PreparedOlsClusteredMetadata {
    cluster_counts: Vec<usize>,
    subsets: Vec<PreparedOlsClusterSubsetCertificate>,
}

impl super::prepared_products::PreparedOlsInferenceBuffers for PreparedOlsClusteredMetadata {
    fn inference_owned_bytes(&self) -> u128 {
        crate::memory::vec_capacity_bytes(&self.cluster_counts)
            + crate::memory::vec_capacity_bytes(&self.subsets)
    }
}

impl<A> CertifiedPreparedOlsClustered<A> {
    #[must_use]
    pub fn cluster_counts(&self) -> &[usize] {
        &self.metadata.cluster_counts
    }
    #[must_use]
    pub fn subsets(&self) -> &[PreparedOlsClusterSubsetCertificate] {
        &self.metadata.subsets
    }
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

struct OlsQualifiedClusterSubsetProvider<'a> {
    fit: &'a CertifiedPreparedOlsFit,
    scores: &'a CertifiedPreparedOlsScoreDesign<'a>,
    weights: ReferenceWeightView<'a>,
    dimensions: &'a [&'a [u32]],
    group_batch: Option<&'a QualifiedClusterScoreBatch>,
}

#[derive(Clone, Copy)]
struct OlsClusterScoreMultiplier<'a> {
    residuals: &'a [f64],
    weights: ReferenceWeightView<'a>,
}

impl SandwichScoreMultiplier for OlsClusterScoreMultiplier<'_> {
    fn nrows(&self) -> usize {
        self.residuals.len()
    }
    fn score_pair(&self, row: usize) -> Result<SandwichScorePair> {
        let score = self.weights.value(row)?
            * *self.residuals.get(row).ok_or_else(|| {
                invalid_argument("prepared clustered residual row is out of range")
            })?;
        if !score.is_finite() {
            return Err(invalid_data(
                "prepared clustered observation score scale is not finite",
            ));
        }
        Ok(SandwichScorePair {
            production: score,
            certificate: score,
        })
    }
}

impl<R: PreparedInferenceReservation> QualifiedClusterSubsetMeatProvider<R>
    for OlsQualifiedClusterSubsetProvider<'_>
{
    type Report = PreparedOlsClusterSubsetCertificate;

    fn dimension(&self) -> usize {
        self.fit.bread().dimension()
    }

    fn cluster_dimensions(&self) -> usize {
        self.dimensions.len()
    }

    fn reduce_subset(
        &self,
        mask: usize,
        reservation: &mut R,
    ) -> Result<QualifiedClusterSubsetMeats> {
        let certificate = if self.scores.dense_view().is_some() {
            QualifiedClusterCertificateRoute::OlsIndependentRaw
        } else {
            QualifiedClusterCertificateRoute::OlsPerGroupBread(self.fit.bread().values())
        };
        let meats = if let Some(batch) = self.group_batch {
            batch.reduce_subset(mask, self.fit.bread().values(), reservation)?
        } else {
            reduce_qualified_cluster_subset(
                self.scores,
                self.scores.dense_view(),
                OlsClusterScoreMultiplier {
                    residuals: self.fit.residuals(),
                    weights: self.weights,
                },
                self.dimensions,
                mask,
                certificate,
                QualifiedClusterRoutePolicy {
                    maximum_dimensions: 10,
                    reduction_maximum_blocks: CLUSTER_REDUCTION_MAX_BLOCKS,
                    partial_budget_bytes: CLUSTER_PARTIAL_BUDGET_BYTES,
                    ordered_rows_per_block: 16_384,
                    dense_groups_per_block: 1_024,
                    // Inference runs inside the action's installed team.
                    planned_threads: rayon::current_num_threads(),
                },
                reservation,
            )?
        };
        Ok(QualifiedClusterSubsetMeats {
            production: meats.production,
            certificate: meats.certificate,
            groups: meats.groups,
        })
    }

    fn report(
        &self,
        mask: usize,
        dimensions: usize,
        sign: i8,
        groups: usize,
    ) -> Result<Self::Report> {
        Ok(PreparedOlsClusterSubsetCertificate {
            mask: u16::try_from(mask)
                .map_err(|_| allocation("prepared clustered subset mask exceeds u16"))?,
            dimensions,
            sign,
            groups,
        })
    }
}

#[derive(Debug)]
struct SymmetricEigensystem {
    values: Vec<f64>,
    vectors: Vec<f64>,
    reconstruction_error: f64,
    orthogonality_error: f64,
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
    fit_prepared_ols_clustered_map_with_loaded_moments(prepared, config, None)
}

/// Consume optional same-call numeric rows after authoritative sample validation.
pub fn fit_prepared_ols_clustered_map_with_loaded_moments(
    prepared: &PreparedOlsClusteredData,
    config: PreparedOlsClusteredConfig,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsClustered> {
    ThreadTeam::new(config.fit.execution.threads).install(|| {
        let mut reservation = SystemPreparedInferenceReservation;
        fit_with_loaded_reservation(prepared, config, &mut reservation, rows)
    })?
}

/// Fit and certify categorical-interaction OLS/WLS with one-/multiway
/// clustered covariance through the same numerical pipeline.
pub fn fit_prepared_categorical_ols_clustered_map(
    prepared: &PreparedCategoricalOlsClusteredData,
    config: PreparedOlsClusteredConfig,
) -> Result<CertifiedPreparedCategoricalOlsClustered> {
    fit_prepared_categorical_ols_clustered_map_with_loaded_moments(prepared, config, None)
}

/// Consume optional same-call numeric rows after authoritative sample validation.
pub fn fit_prepared_categorical_ols_clustered_map_with_loaded_moments(
    prepared: &PreparedCategoricalOlsClusteredData,
    config: PreparedOlsClusteredConfig,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedCategoricalOlsClustered> {
    let components = ThreadTeam::new(config.fit.execution.threads).install(|| {
        let mut reservation = SystemPreparedInferenceReservation;
        fit_clustered_view_with_reservation(prepared, config, &mut reservation, rows)
    })??;
    Ok(components)
}

/// Fit the canonical owner without duplicating the host's VCE dispatch.
pub fn fit_prepared_model_clustered_with_loaded_moments(
    prepared: &PreparedOlsData,
    config: PreparedOlsClusteredConfig,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsClustered> {
    if prepared.uses_categorical_preparation() {
        fit_prepared_categorical_ols_clustered_map_with_loaded_moments(prepared, config, rows)
    } else {
        fit_prepared_ols_clustered_map_with_loaded_moments(prepared, config, rows)
    }
}

#[cfg(test)]
fn fit_with_reservation<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsClusteredData,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
) -> Result<CertifiedPreparedOlsClustered> {
    fit_with_loaded_reservation(prepared, config, reservation, None)
}

fn fit_with_loaded_reservation<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsClusteredData,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsClustered> {
    let components = fit_clustered_view_with_reservation(
        &crate::prepared_fit_view::OrdinaryPrepared(prepared),
        config,
        reservation,
        rows,
    )?;
    Ok(components)
}

fn fit_clustered_view_with_reservation<
    P: PreparedOlsClusteredView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &P,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    rows: Option<super::LoadedOlsRows>,
) -> Result<CertifiedPreparedOlsClustered<CategoricalTopologyDofReport>> {
    validate_config(prepared, config)?;
    let ols = prepared.clustered_ols();
    let (fit, score_scratch) = super::prepared_fit::fit_prepared_ols_coefficients_view_map_for_inference_with_loaded_moments(ols, config.fit, rows)?;
    compose_clustered_view(
        prepared,
        fit,
        config,
        reservation,
        ClusteredPerturbations::default(),
        score_scratch,
    )
}

#[cfg(test)]
fn compose<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsClusteredData,
    mut fit: CertifiedPreparedOlsFit,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
) -> Result<CertifiedPreparedOlsClustered> {
    let scores = take_prepared_ols_score_product(&mut fit, None)?;
    let components = compose_clustered_view(
        &crate::prepared_fit_view::OrdinaryPrepared(prepared),
        fit,
        config,
        reservation,
        perturbations,
        scores,
    )?;
    Ok(components)
}

fn compose_clustered_view<P: PreparedOlsClusteredView + ?Sized, R: PreparedInferenceReservation>(
    prepared: &P,
    mut fit: CertifiedPreparedOlsFit,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
    score_scratch: PreparedOlsScoreProduct,
) -> Result<CertifiedPreparedOlsClustered<CategoricalTopologyDofReport>> {
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

    let tss = independent_outcome_sum(
        &ols.fit_sample().outcome,
        weight_view,
        dof.has_structural_intercept,
    )?;
    let within_tss = if ols.inference_has_explicit_absorbed_effects() {
        fit.initial_absorbed_outcome_sum_of_squares()
    } else {
        tss
    };
    let rss = independent_residual_sum(&fit, weight_view)?;
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
        config.certificate_tolerance,
        reservation,
        score_scratch,
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
    Ok(CertifiedPreparedOlsClustered {
        fit,
        absorbed_dof,
        weights,
        dof,
        metadata: PreparedOlsClusteredMetadata {
            cluster_counts,
            subsets: raw.subsets,
        },
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
    scores: &CertifiedPreparedOlsScoreDesign<'_>,
    weights: ReferenceWeightView<'_>,
    dimensions: &[&[u32]],
    dof: PreparedOlsClusteredDofReport,
    weight_report: PreparedOlsReferenceWeightReport,
    reservation: &mut R,
) -> Result<RawClusterCovariance> {
    let p = fit.bread().dimension();
    let square = checked_square(p, "prepared clustered meat dimensions overflow usize")?;
    let combined = {
        let group_batch = if scores.dense_view().is_none() {
            prepare_cluster_score_batch(
                scores,
                OlsClusterScoreMultiplier {
                    residuals: fit.residuals(),
                    weights,
                },
                dimensions,
                10,
                reservation,
            )?
        } else {
            None
        };
        reduce_qualified_clustered_score_meats(
            &OlsQualifiedClusterSubsetProvider {
                fit,
                scores,
                weights,
                dimensions,
                group_batch: group_batch.as_ref(),
            },
            reservation,
        )?
    };
    let combined_meat = combined.production;
    let mut combined_influence = combined.certificate;
    let subsets = combined.subsets;
    let independent_subsets = combined.certificate_subsets;
    let combined_influence = if scores.dense_view().is_some() {
        // Dense fast paths retain a second, independently ordered raw-score
        // meat.  Apply the bread only once after inclusion-exclusion instead
        // of once per cluster group; this is algebraically identical and
        // removes the dominant O(G K^2) matrix-vector work for wide designs.
        sandwich(fit.bread().values(), &combined_influence, p, reservation)?
    } else {
        symmetrize(&mut combined_influence, p)?;
        combined_influence
    };
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
    bread_meat_bread(
        bread,
        meat,
        p,
        SandwichCorrectionPolicy::None,
        SandwichProductMode::CompensatedFull,
        reservation,
    )
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

        // Repair the complete reported covariance, including the constant.
        // Restoring an independently clipped slope block can destroy PSD.
    }
    // The clipped reconstruction is exactly symmetric, so symmetrizing it is
    // the identity and the pre-restore and final matrices are the same bits.
    // Without an adjustment, `output` is the symmetrized raw matrix itself.
    // One eigensystem therefore serves both the pre-restore and final checks.
    symmetrize(&mut output, p)?;
    let adjusted_eigen = if full_adjusted {
        Some(symmetric_eigensystem(&output, p, reservation)?)
    } else {
        None
    };
    let output_eigen = adjusted_eigen.as_ref().unwrap_or(&raw_eigen);
    let output_scale = if full_adjusted {
        covariance_scale(&output, p)?
    } else {
        scale
    };
    let minimum_pre_restore = output_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    if full_adjusted && minimum_pre_restore < -tolerance * output_scale {
        return Err(invalid_data(
            "prepared clustered clipped full covariance is materially indefinite",
        ));
    }
    if let Some(eigen) = adjusted_eigen.as_ref() {
        eig_error = eig_error.max(eigen.reconstruction_error);
        orthogonality_error = orthogonality_error.max(eigen.orthogonality_error);
    }
    if minimum_pre_restore < -tolerance * output_scale {
        return Err(invalid_data(
            "prepared clustered pre-restore full covariance is materially indefinite",
        ));
    }
    let pre_restore_full_rank = output_eigen
        .values
        .iter()
        .filter(|value| **value > tolerance * output_scale)
        .count();
    let minimum_final = minimum_pre_restore;
    let final_scale = output_scale;
    if minimum_final < -tolerance * final_scale {
        return Err(invalid_data(format!(
            "prepared clustered final covariance is materially indefinite: min={minimum_final:.17e}, scale={final_scale:.17e}, adjusted={full_adjusted}",
        )));
    }
    let state = if cluster_dimensions == 1 {
        PreparedOlsClusteredPsdState::OneWayStrict
    } else if !full_adjusted {
        PreparedOlsClusteredPsdState::MultiwayRawPsd
    } else {
        PreparedOlsClusteredPsdState::MultiwayFullClipped
    };
    Ok((
        output,
        PreparedOlsClusteredPsdReport {
            state,
            full_adjusted,
            slope_block_restored: false,
            slope_block_adjusted: false,
            full_clipped_eigenvalues: full_clipped,
            slope_clipped_eigenvalues: 0,
            minimum_raw_full_eigenvalue: minimum_raw,
            minimum_pre_restore_full_eigenvalue: minimum_pre_restore,
            minimum_raw_slope_eigenvalue: None,
            minimum_restored_slope_eigenvalue: None,
            minimum_final_full_eigenvalue: minimum_final,
            pre_restore_full_rank,
            restored_slope_rank: None,
            strict_final_full_psd: true,
            transform_reconstruction_relative_error: transform_error,
            eigensystem_relative_error: eig_error,
            orthogonality_relative_error: orthogonality_error,
        },
    ))
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

    symmetrize(&mut expected, p)?;
    let transform_error = matrix_relative_infinity_norm(&expected, covariance.values(), p)?;
    let final_eigen = symmetric_eigensystem(covariance.values(), p, reservation)?;
    let final_minimum = final_eigen
        .values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let final_scale = covariance_scale(covariance.values(), p)?;
    let expected_state = if q == 1 {
        PreparedOlsClusteredPsdState::OneWayStrict
    } else if !expected_full_adjustment {
        PreparedOlsClusteredPsdState::MultiwayRawPsd
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
        optional_error(covariance.psd.minimum_raw_slope_eigenvalue, None)?,
        optional_error(covariance.psd.minimum_restored_slope_eigenvalue, None)?,
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    let state_error = f64::from(u8::from(
        covariance.psd.state != expected_state
            || covariance.psd.full_adjusted != expected_full_adjustment
            || covariance.psd.full_clipped_eigenvalues != expected_full_clipped
            || covariance.psd.slope_block_restored
            || covariance.psd.slope_block_adjusted
            || covariance.psd.slope_clipped_eigenvalues != 0
            || covariance.psd.pre_restore_full_rank != expected_pre_restore_rank
            || covariance.psd.restored_slope_rank.is_some()
            || !covariance.psd.strict_final_full_psd,
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
    let strict_psd_error = (-final_minimum).max(0.0) / final_scale;
    let final_structure_error = transform_error
        .max(diagnostics_error)
        .max(state_error)
        .max(finite_diagonal_error)
        .max(symmetry_error(covariance.values(), p)?)
        .max(standard_error_error(covariance)?)
        .max(strict_psd_error);
    Ok((pre_restore_error, 0.0, final_structure_error))
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
    calculate_wald(
        fit,
        &covariance.values,
        dimension,
        &positions,
        tolerance,
        reservation,
    )
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

#[cfg(test)]
mod tests;
