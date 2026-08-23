#!/usr/bin/env python3
"""Enable categorical clustered inference through the existing numerical engine."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VIEW = ROOT / "crates/stfixest-core/src/ols/prepared_clustered_view.rs"
FIT = ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs"
CLUSTERED = ROOT / "crates/stfixest-core/src/ols/prepared_clustered.rs"
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"
LIB = ROOT / "crates/stfixest-core/src/lib.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def regex_once(text: str, pattern: str, replacement: str, label: str) -> str:
    new, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return new


view = VIEW.read_text(encoding="utf-8")
view = replace_once(
    view,
    "use crate::error::Result;\n\nuse super::{\n"
    "    PreparedCategoricalOlsClusteredData, PreparedOlsClusterDesign, PreparedOlsClusteredData,\n"
    "};\n",
    "use crate::error::Result;\n"
    "use crate::prepared_categorical_data::PreparedCategoricalOlsData;\n\n"
    "use super::prepared_inference_view::PreparedOlsInferenceView;\n"
    "use super::{\n"
    "    PreparedCategoricalOlsClusteredData, PreparedOlsClusterDesign, PreparedOlsClusteredData,\n"
    "    PreparedOlsData,\n"
    "};\n",
    "clustered view imports",
)
view = replace_once(
    view,
    "pub(crate) trait PreparedOlsClusteredView {\n"
    "    fn clustered_design(&self) -> &PreparedOlsClusterDesign;\n",
    "pub(crate) trait PreparedOlsClusteredView {\n"
    "    type Ols: PreparedOlsInferenceView + ?Sized;\n\n"
    "    fn clustered_ols(&self) -> &Self::Ols;\n\n"
    "    fn clustered_design(&self) -> &PreparedOlsClusterDesign;\n",
    "clustered view associated OLS",
)
view = replace_once(
    view,
    "impl PreparedOlsClusteredView for PreparedOlsClusteredData {\n"
    "    fn clustered_design(&self) -> &PreparedOlsClusterDesign {\n",
    "impl PreparedOlsClusteredView for PreparedOlsClusteredData {\n"
    "    type Ols = PreparedOlsData;\n\n"
    "    fn clustered_ols(&self) -> &Self::Ols {\n"
    "        self.ols()\n"
    "    }\n\n"
    "    fn clustered_design(&self) -> &PreparedOlsClusterDesign {\n",
    "legacy clustered OLS view",
)
view = replace_once(
    view,
    "impl PreparedOlsClusteredView for PreparedCategoricalOlsClusteredData {\n"
    "    fn clustered_design(&self) -> &PreparedOlsClusterDesign {\n",
    "impl PreparedOlsClusteredView for PreparedCategoricalOlsClusteredData {\n"
    "    type Ols = PreparedCategoricalOlsData;\n\n"
    "    fn clustered_ols(&self) -> &Self::Ols {\n"
    "        self.ols()\n"
    "    }\n\n"
    "    fn clustered_design(&self) -> &PreparedOlsClusterDesign {\n",
    "categorical clustered OLS view",
)
VIEW.write_text(view, encoding="utf-8")

fit = FIT.read_text(encoding="utf-8")
fit = replace_once(
    fit,
    "fn fit_prepared_ols_coefficients_view_map<P: PreparedOlsFitView + ?Sized>(\n",
    "pub(crate) fn fit_prepared_ols_coefficients_view_map<P: PreparedOlsFitView + ?Sized>(\n",
    "generic coefficient-fit visibility",
)
FIT.write_text(fit, encoding="utf-8")

clustered = CLUSTERED.read_text(encoding="utf-8")
clustered = replace_once(
    clustered,
    "use crate::error::{ErrorCode, Result, StFixestError};\n\n"
    "use super::prepared_clustered_view::PreparedOlsClusteredView;\n"
    "use super::prepared_inference_view::PreparedOlsInferenceView;\n",
    "use crate::error::{ErrorCode, Result, StFixestError};\n"
    "use crate::prepared_fit_view::PreparedOlsFitView;\n"
    "use crate::topology_dof::CategoricalTopologyDofReport;\n\n"
    "use super::prepared_clustered_view::PreparedOlsClusteredView;\n"
    "use super::prepared_fit::fit_prepared_ols_coefficients_view_map;\n"
    "use super::prepared_inference_view::{\n"
    "    PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView,\n"
    "};\n",
    "clustered generic imports",
)
clustered = replace_once(
    clustered,
    "use super::prepared_unadjusted::{\n"
    "    build_clustered_base_residual_dof, build_reference_weight_report, build_statistics,\n"
    "    checked_square, compensated_add, exact_integer_as_f64, relative_difference,\n"
    "    validate_sum_of_squares, weighted_mean, weighted_sum_of_squares, ReferenceWeightView,\n"
    "};\n",
    "use super::prepared_unadjusted::{\n"
    "    build_reference_weight_report_view, build_residual_dof_view, build_statistics,\n"
    "    checked_square, compensated_add, exact_integer_as_f64, relative_difference,\n"
    "    validate_sum_of_squares, weighted_mean, weighted_sum_of_squares, ReferenceWeightView,\n"
    "};\n",
    "clustered generic inference helpers",
)
clustered = replace_once(
    clustered,
    "use super::{\n"
    "    fit_prepared_ols_coefficients_map, CertifiedPreparedOlsFit, PreparedOlsAbsorbedDofPolicy,\n"
    "    PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind,\n"
    "    PreparedOlsClusteredData, PreparedOlsColumnKind, PreparedOlsFitConfig,\n",
    "use super::{\n"
    "    CertifiedPreparedOlsFit, PreparedCategoricalOlsClusteredData,\n"
    "    PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport,\n"
    "    PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind, PreparedOlsClusteredData,\n"
    "    PreparedOlsColumnKind, PreparedOlsFitConfig,\n",
    "clustered public type imports",
)

legacy_marker = "#[derive(Debug)]\nstruct RawClusterCovariance"
legacy_end = clustered.index(legacy_marker)
legacy_prefix = clustered[:legacy_end]
legacy_suffix = clustered[legacy_end:]
insert = r'''
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

'''
clustered = legacy_prefix + insert + legacy_suffix

start = clustered.index("/// Fit and certify one-/multiway clustered covariance")
end = clustered.index("fn validate_config", start)
new_pipeline = r'''/// Fit and certify one-/multiway clustered covariance through the prepared
/// strict-MAP path. This entry point owns the complete scientific composition.
pub fn fit_prepared_ols_clustered_map(
    prepared: &PreparedOlsClusteredData,
    config: PreparedOlsClusteredConfig,
) -> Result<CertifiedPreparedOlsClustered> {
    let mut reservation = SystemPreparedInferenceReservation;
    fit_with_reservation(prepared, config, &mut reservation)
}

/// Fit and certify categorical-interaction OLS/WLS with one-/multiway
/// clustered covariance through the same numerical pipeline.
pub fn fit_prepared_categorical_ols_clustered_map(
    prepared: &PreparedCategoricalOlsClusteredData,
    config: PreparedOlsClusteredConfig,
) -> Result<CertifiedPreparedCategoricalOlsClustered> {
    let mut reservation = SystemPreparedInferenceReservation;
    let components = fit_clustered_view_with_reservation(prepared, config, &mut reservation)?;
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
) -> Result<PreparedOlsClusteredComponents<<P::Ols as PreparedOlsInferenceView>::AbsorbedDofReport>> {
    validate_config(prepared, config)?;
    let fit = fit_prepared_ols_coefficients_view_map(prepared.clustered_ols(), config.fit)?;
    compose_clustered_view(
        prepared,
        fit,
        config,
        reservation,
        ClusteredPerturbations::default(),
    )
}

fn compose<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsClusteredData,
    fit: CertifiedPreparedOlsFit,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
) -> Result<CertifiedPreparedOlsClustered> {
    let components =
        compose_clustered_view(prepared, fit, config, reservation, perturbations)?;
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

fn compose_clustered_view<
    P: PreparedOlsClusteredView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &P,
    fit: CertifiedPreparedOlsFit,
    config: PreparedOlsClusteredConfig,
    reservation: &mut R,
    perturbations: ClusteredPerturbations,
) -> Result<PreparedOlsClusteredComponents<<P::Ols as PreparedOlsInferenceView>::AbsorbedDofReport>> {
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

    let (weights, weight_view, weight_error) = build_reference_weight_report_view(
        ols,
        &fit,
        config.certificate_tolerance,
        true,
    )?;
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
        &fit,
        weight_view,
        config.fit.map,
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
    let independent_error = certify_with_group_influences(
        &fit,
        &scores,
        weight_view,
        &dimensions,
        &raw.uncorrected,
        &raw.subsets,
        reservation,
    )?;
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

'''
clustered = clustered[:start] + new_pipeline + clustered[end:]

# Remove the legacy-only absorbed-DoF validator; the inference view now owns it.
clustered = regex_once(
    clustered,
    r"fn validate_cluster_aware_absorbed_dof\(.*?\n}\n\n(?=fn build_clustered_dof)",
    "",
    "legacy cluster-aware DoF validator removal",
)

clustered = regex_once(
    clustered,
    r"fn build_clustered_dof\(\n    prepared: &PreparedOlsClusteredData,\n    weights: PreparedOlsReferenceWeightReport,\n    base: PreparedOlsResidualDofReport,\n    absorbed: &PreparedOlsAbsorbedDofReport,\n\) -> Result<PreparedOlsClusteredDofReport> \{.*?\n}\n\n(?=fn borrow_cluster_dimensions)",
    r'''fn build_clustered_dof<P: PreparedOlsClusteredView + ?Sized>(
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
        .ok_or_else(|| invalid_argument("prepared clustered model has no cluster dimensions"))?;
    let reported_residual_dof = minimum_cluster_count
        .checked_sub(1)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid_data("prepared clustered model has no positive reported residual DoF")
        })?;
    let fit_residual_dof = base
        .residual_dof
        .checked_sub(absorbed.nested_categories)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid_data("prepared clustered model has no positive fit residual DoF"))?;
    let correction_denominator_dof = weights
        .effective_observations
        .checked_sub(absorbed.absorbed_rank)
        .checked_sub(base.finite_rank)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid_data("prepared clustered correction denominator DoF is not positive")
        })?;
    if correction_denominator_dof != base.residual_dof {
        return Err(invalid_argument(
            "prepared clustered base and correction residual DoF disagree",
        ));
    }
    Ok(PreparedOlsClusteredDofReport {
        finite_rank: base.finite_rank,
        finite_model_dof: base.finite_model_dof,
        absorbed_rank: absorbed.absorbed_rank,
        nested_absorbed_categories: absorbed.nested_categories,
        unclustered_residual_dof: base.residual_dof,
        reported_residual_dof,
        fit_residual_dof,
        correction_denominator_dof,
        minimum_cluster_count,
        has_structural_intercept: base.has_structural_intercept,
        has_normalized_absorbed_intercept: base.has_normalized_absorbed_intercept,
        absorbed_rank_is_approximate: absorbed.is_approximate,
    })
}

''',
    "generic clustered DoF builder",
)

clustered = replace_once(
    clustered,
    "fn apply_psd_policy<R: PreparedInferenceReservation>(\n"
    "    prepared: &PreparedOlsClusteredData,\n",
    "fn apply_psd_policy<\n"
    "    P: PreparedOlsClusteredView + ?Sized,\n"
    "    R: PreparedInferenceReservation,\n"
    ">(\n"
    "    prepared: &P,\n",
    "generic PSD policy signature",
)
clustered = clustered.replace("prepared.clusters().len()", "prepared.clustered_design().len()")
clustered = clustered.replace("reported_intercept_position(prepared.ols(), fit)", "reported_intercept_position(prepared.clustered_ols(), fit)")
clustered = replace_once(
    clustered,
    "fn reported_intercept_position(\n"
    "    prepared: &super::PreparedOlsData,\n",
    "fn reported_intercept_position<P: PreparedOlsFitView + ?Sized>(\n"
    "    prepared: &P,\n",
    "generic intercept-position signature",
)
clustered = clustered.replace("prepared\n                    .model\n", "prepared\n                    .fit_model()\n")
clustered = replace_once(
    clustered,
    "fn certify_psd_state<R: PreparedInferenceReservation>(\n"
    "    prepared: &PreparedOlsClusteredData,\n",
    "fn certify_psd_state<\n"
    "    P: PreparedOlsClusteredView + ?Sized,\n"
    "    R: PreparedInferenceReservation,\n"
    ">(\n"
    "    prepared: &P,\n",
    "generic PSD certificate signature",
)
clustered = replace_once(
    clustered,
    "fn build_wald<R: PreparedInferenceReservation>(\n"
    "    prepared: &super::PreparedOlsData,\n",
    "fn build_wald<P: PreparedOlsFitView + ?Sized, R: PreparedInferenceReservation>(\n"
    "    prepared: &P,\n",
    "generic Wald signature",
)
clustered = clustered.replace("prepared\n            .model\n", "prepared\n            .fit_model()\n")
CLUSTERED.write_text(clustered, encoding="utf-8")

ols_mod = OLS_MOD.read_text(encoding="utf-8")
ols_mod = replace_once(
    ols_mod,
    "pub use prepared_clustered::{\n"
    "    fit_prepared_ols_clustered_map, CertifiedPreparedOlsClustered,\n",
    "pub use prepared_clustered::{\n"
    "    fit_prepared_categorical_ols_clustered_map, fit_prepared_ols_clustered_map,\n"
    "    CertifiedPreparedCategoricalOlsClustered, CertifiedPreparedOlsClustered,\n",
    "OLS categorical clustered exports",
)
OLS_MOD.write_text(ols_mod, encoding="utf-8")

lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "    fit_prepared_categorical_ols_robust_map, fit_prepared_categorical_ols_unadjusted_map,\n"
    "    fit_prepared_ols_clustered_map, prepare_categorical_ols_clustered_data,\n",
    "    fit_prepared_categorical_ols_clustered_map, fit_prepared_categorical_ols_robust_map,\n"
    "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_clustered_map,\n"
    "    prepare_categorical_ols_clustered_data,\n",
    "crate categorical clustered function export",
)
lib = replace_once(
    lib,
    "    CertifiedPreparedCategoricalOlsRobust, CertifiedPreparedCategoricalOlsUnadjusted,\n"
    "    CertifiedPreparedOlsClustered,\n",
    "    CertifiedPreparedCategoricalOlsClustered, CertifiedPreparedCategoricalOlsRobust,\n"
    "    CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsClustered,\n",
    "crate categorical clustered result export",
)
LIB.write_text(lib, encoding="utf-8")

print("activated categorical clustered inference through shared numerical engine")
