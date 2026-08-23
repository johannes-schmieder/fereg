#!/usr/bin/env python3
"""Generalize certified iid inference over legacy and categorical prepared views."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
UNADJUSTED = ROOT / "crates/stfixest-core/src/ols/prepared_unadjusted.rs"
INFERENCE_VIEW = ROOT / "crates/stfixest-core/src/ols/prepared_inference_view.rs"
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"
LIB = ROOT / "crates/stfixest-core/src/lib.rs"

INFERENCE_VIEW_TEXT = '''//! Shared absorbed-DoF boundary for certified prepared OLS inference.
//!
//! The legacy single-source prepared path and the categorical-interaction path
//! retain different public report identities.  Numerical inference consumes a
//! validated common summary, while each result continues to retain its exact
//! source-bound absorbed-DoF report.

use crate::error::{ErrorCode, Result, StFixestError};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::topology_dof::{
    CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofPolicy,
    CategoricalTopologyDofReport,
};

use super::{
    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffectIdentity,
    PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport, PreparedOlsData,
};

/// Topology-validated summary consumed by common inference calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedOlsAbsorbedDofSummary {
    pub initial_categories: usize,
    pub redundant_categories: usize,
    pub absorbed_rank: usize,
    pub nested_categories: usize,
    pub is_approximate: bool,
}

pub(crate) trait PreparedOlsInferenceView: PreparedOlsFitView {
    type AbsorbedDofReport;

    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<Self::AbsorbedDofReport>;

    fn inference_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary>;

    fn inference_has_explicit_absorbed_effects(&self) -> bool;
}

impl PreparedOlsInferenceView for PreparedOlsData {
    type AbsorbedDofReport = PreparedOlsAbsorbedDofReport;

    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<Self::AbsorbedDofReport> {
        compute_prepared_ols_absorbed_dof(self, policy)
    }

    fn inference_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_legacy_report(self, report, expected_policy)
    }

    fn inference_has_explicit_absorbed_effects(&self) -> bool {
        !self.fe_topology.effects().is_empty()
    }
}

impl PreparedOlsInferenceView for PreparedCategoricalOlsData {
    type AbsorbedDofReport = CategoricalTopologyDofReport;

    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<Self::AbsorbedDofReport> {
        self.absorbed_dof(categorical_policy(policy), &[])
    }

    fn inference_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(self, report, expected_policy)
    }

    fn inference_has_explicit_absorbed_effects(&self) -> bool {
        !self.effect_specs().is_empty()
    }
}

fn validate_legacy_report(
    prepared: &PreparedOlsData,
    report: &PreparedOlsAbsorbedDofReport,
    expected_policy: PreparedOlsAbsorbedDofPolicy,
) -> Result<PreparedOlsAbsorbedDofSummary> {
    if report.policy != expected_policy {
        return Err(invalid(
            "prepared absorbed-DoF report policy differs from the requested policy",
        ));
    }
    let implicit_no_absorb = prepared.fit_structural_intercept()
        && prepared.fe_topology.effects().is_empty()
        && prepared.fit_sample().n_active() > 0;
    let expected_rows = prepared
        .fe_topology
        .effects()
        .len()
        .checked_add(usize::from(implicit_no_absorb))
        .ok_or_else(|| allocation("prepared absorbed-DoF row count overflows"))?;
    if report.effects.len() != expected_rows || report.nested_categories != 0 {
        return Err(invalid(
            "prepared absorbed-DoF report dimensions or iid nesting state are inconsistent",
        ));
    }

    let mut initial = 0_usize;
    let mut redundant = 0_usize;
    let mut rank = 0_usize;
    let mut approximate = false;
    let mut rows = report.effects.as_slice();
    if implicit_no_absorb {
        let (implicit, remaining) = rows
            .split_first()
            .ok_or_else(|| invalid("prepared implicit no-absorb DoF row is absent"))?;
        if implicit.identity != PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
            || implicit.label != "_cons"
            || implicit.categories != 1
            || implicit.redundant != 0
            || implicit.coefficients != 1
            || !implicit.redundancy_exact
            || implicit.nested
        {
            return Err(invalid(
                "prepared implicit no-absorb DoF row is inconsistent",
            ));
        }
        initial = 1;
        rank = 1;
        rows = remaining;
    }

    for (effect, row) in prepared.fe_topology.effects().iter().zip(rows) {
        let source = prepared
            .fe_topology
            .sources()
            .get(effect.topology_source_index)
            .ok_or_else(|| invalid("prepared absorbed-DoF FE source is out of range"))?;
        if row.identity
            != (PreparedOlsAbsorbedDofEffectIdentity::Explicit {
                term_id: effect.term_id,
                source: effect.source,
            })
            || row.label != effect.label
            || row.categories != source.n_levels()
            || row.redundant > row.categories
            || row.coefficients != row.categories - row.redundant
            || row.nested
        {
            return Err(invalid(
                "prepared absorbed-DoF explicit row is inconsistent",
            ));
        }
        initial = initial
            .checked_add(row.categories)
            .ok_or_else(|| allocation("prepared absorbed category total overflows"))?;
        redundant = redundant
            .checked_add(row.redundant)
            .ok_or_else(|| allocation("prepared absorbed redundancy total overflows"))?;
        rank = rank
            .checked_add(row.coefficients)
            .ok_or_else(|| allocation("prepared absorbed rank total overflows"))?;
        approximate |= !row.redundancy_exact;
    }
    validate_totals(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.is_approximate,
        initial,
        redundant,
        rank,
        approximate,
    )?;
    Ok(summary(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.nested_categories,
        report.is_approximate,
    ))
}

fn validate_categorical_report(
    prepared: &PreparedCategoricalOlsData,
    report: &CategoricalTopologyDofReport,
    expected_policy: PreparedOlsAbsorbedDofPolicy,
) -> Result<PreparedOlsAbsorbedDofSummary> {
    if report.policy != categorical_policy(expected_policy) {
        return Err(invalid(
            "categorical absorbed-DoF report policy differs from the requested policy",
        ));
    }
    let topology = prepared.fe_topology();
    let specs = prepared.effect_specs();
    let implicit_no_absorb = prepared.fit_structural_intercept()
        && specs.is_empty()
        && prepared.fit_sample().n_active() > 0;
    let expected_rows = specs
        .len()
        .checked_add(usize::from(implicit_no_absorb))
        .ok_or_else(|| allocation("categorical absorbed-DoF row count overflows"))?;
    if report.effects.len() != expected_rows
        || topology.effect_to_topology().len() != specs.len()
        || report.nested_categories != 0
    {
        return Err(invalid(
            "categorical absorbed-DoF report dimensions or iid nesting state are inconsistent",
        ));
    }

    let mut initial = 0_usize;
    let mut redundant = 0_usize;
    let mut rank = 0_usize;
    let mut approximate = false;
    let mut rows = report.effects.as_slice();
    if implicit_no_absorb {
        let (implicit, remaining) = rows
            .split_first()
            .ok_or_else(|| invalid("categorical implicit no-absorb DoF row is absent"))?;
        if implicit.identity != CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb
            || implicit.label != "_cons"
            || implicit.categories != 1
            || implicit.redundant != 0
            || implicit.coefficients != 1
            || !implicit.redundancy_exact
            || implicit.nested
        {
            return Err(invalid(
                "categorical implicit no-absorb DoF row is inconsistent",
            ));
        }
        initial = 1;
        rank = 1;
        rows = remaining;
    }

    for (effect_index, (spec, row)) in specs.iter().zip(rows).enumerate() {
        let topology_source_index = *topology
            .effect_to_topology()
            .get(effect_index)
            .ok_or_else(|| invalid("categorical absorbed-DoF topology map is out of range"))?;
        let source = topology.registry().source(topology_source_index)?;
        let identity_matches = matches!(
            &row.identity,
            CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources }
                if *term_id == spec.term_id && sources == &spec.sources
        );
        if !identity_matches
            || row.label != spec.label
            || row.categories != source.n_levels()
            || row.redundant > row.categories
            || row.coefficients != row.categories - row.redundant
            || row.nested
        {
            return Err(invalid(
                "categorical absorbed-DoF explicit row is inconsistent",
            ));
        }
        initial = initial
            .checked_add(row.categories)
            .ok_or_else(|| allocation("categorical absorbed category total overflows"))?;
        redundant = redundant
            .checked_add(row.redundant)
            .ok_or_else(|| allocation("categorical absorbed redundancy total overflows"))?;
        rank = rank
            .checked_add(row.coefficients)
            .ok_or_else(|| allocation("categorical absorbed rank total overflows"))?;
        approximate |= !row.redundancy_exact;
    }
    validate_totals(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.is_approximate,
        initial,
        redundant,
        rank,
        approximate,
    )?;
    Ok(summary(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.nested_categories,
        report.is_approximate,
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_totals(
    reported_initial: usize,
    reported_redundant: usize,
    reported_rank: usize,
    reported_approximate: bool,
    computed_initial: usize,
    computed_redundant: usize,
    computed_rank: usize,
    computed_approximate: bool,
) -> Result<()> {
    if reported_initial != computed_initial
        || reported_redundant != computed_redundant
        || reported_rank != computed_rank
        || reported_approximate != computed_approximate
        || reported_redundant > reported_initial
        || reported_rank != reported_initial - reported_redundant
    {
        return Err(invalid("absorbed-DoF report totals are inconsistent"));
    }
    Ok(())
}

const fn summary(
    initial_categories: usize,
    redundant_categories: usize,
    absorbed_rank: usize,
    nested_categories: usize,
    is_approximate: bool,
) -> PreparedOlsAbsorbedDofSummary {
    PreparedOlsAbsorbedDofSummary {
        initial_categories,
        redundant_categories,
        absorbed_rank,
        nested_categories,
        is_approximate,
    }
}

const fn categorical_policy(policy: PreparedOlsAbsorbedDofPolicy) -> CategoricalTopologyDofPolicy {
    match policy {
        PreparedOlsAbsorbedDofPolicy::None => CategoricalTopologyDofPolicy::None,
        PreparedOlsAbsorbedDofPolicy::FirstPair => CategoricalTopologyDofPolicy::FirstPair,
        PreparedOlsAbsorbedDofPolicy::Pairwise => CategoricalTopologyDofPolicy::Pairwise,
    }
}

fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}
'''

NEW_RESULT_BLOCK = '''
/// Certified categorical-interaction iid OLS/WLS result.
///
/// The numerical fit, covariance, statistics, and certificates are produced by
/// the same common pipeline as [`CertifiedPreparedOlsUnadjusted`].  This result
/// retains the interaction-aware absorbed-DoF identities rather than weakening
/// them to the legacy single-source representation.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedCategoricalOlsUnadjusted {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsUnadjustedCovariance,
    certificate: PreparedOlsUnadjustedCertificate,
}

impl CertifiedPreparedCategoricalOlsUnadjusted {
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
    pub const fn dof(&self) -> PreparedOlsResidualDofReport {
        self.dof
    }

    #[must_use]
    pub const fn statistics(&self) -> PreparedOlsFitStatistics {
        self.statistics
    }

    #[must_use]
    pub const fn covariance(&self) -> &PreparedOlsUnadjustedCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsUnadjustedCertificate {
        self.certificate
    }
}

#[derive(Debug, PartialEq)]
struct PreparedOlsUnadjustedComponents {
    fit: CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsUnadjustedCovariance,
    certificate: PreparedOlsUnadjustedCertificate,
}

'''

PIPELINE_BLOCK = '''/// Fit and certify the current prepared model with conventional homoskedastic
/// covariance. Probability weights fail closed because the pinned reference
/// changes them to robust inference instead of producing iid covariance.
pub fn fit_prepared_ols_unadjusted_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsUnadjustedConfig,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    validate_config(config)?;
    reject_probability_iid(prepared)?;
    let fit = fit_prepared_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = prepared.inference_compute_absorbed_dof(config.absorbed_dof)?;
    let mut reservation = SystemInferenceReservation;
    compose_internal(
        prepared,
        fit,
        absorbed_dof,
        config.certificate_tolerance,
        &mut reservation,
        InferencePerturbations::default(),
    )
}

/// Fit and certify categorical-interaction OLS/WLS with conventional
/// homoskedastic covariance through the same common inference pipeline.
pub fn fit_prepared_categorical_ols_unadjusted_map(
    prepared: &PreparedCategoricalOlsData,
    config: PreparedOlsUnadjustedConfig,
) -> Result<CertifiedPreparedCategoricalOlsUnadjusted> {
    validate_config(config)?;
    reject_probability_iid(prepared)?;
    let fit = fit_prepared_categorical_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = prepared.inference_compute_absorbed_dof(config.absorbed_dof)?;
    let summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let mut reservation = SystemInferenceReservation;
    let components = compose_view_internal(
        prepared,
        fit,
        summary,
        config.certificate_tolerance,
        &mut reservation,
        InferencePerturbations::default(),
    )?;
    Ok(CertifiedPreparedCategoricalOlsUnadjusted {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn reject_probability_iid<P: PreparedOlsInferenceView + ?Sized>(prepared: &P) -> Result<()> {
    if matches!(
        &prepared.fit_sample().weights,
        PreparedWeights::Provided {
            kind: WeightKind::Probability,
            ..
        }
    ) {
        return Err(unsupported(
            "prepared iid covariance is unavailable for probability weights; the pinned reference requires robust inference",
        ));
    }
    Ok(())
}

fn compose_internal<R: InferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    tolerance: f64,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<CertifiedPreparedOlsUnadjusted> {
    let summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, absorbed_dof.policy)?;
    let components = compose_view_internal(
        prepared,
        fit,
        summary,
        tolerance,
        reservation,
        perturbations,
    )?;
    Ok(CertifiedPreparedOlsUnadjusted {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn compose_view_internal<P: PreparedOlsInferenceView + ?Sized, R: InferenceReservation>(
    prepared: &P,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofSummary,
    tolerance: f64,
    reservation: &mut R,
    perturbations: InferencePerturbations,
) -> Result<PreparedOlsUnadjustedComponents> {
    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;
    let (weights, weight_view, weight_sum_relative_error) =
        build_reference_weight_report_view(prepared, &fit, tolerance, false)?;
    let mut dof = build_residual_dof_view(prepared, &fit, absorbed_dof, weights)?;
    if perturbations.residual_dof {
        dof.residual_dof = dof
            .residual_dof
            .checked_add(1)
            .ok_or_else(|| allocation("injected prepared iid residual DoF overflows"))?;
    }

    let outcome_mean = weighted_mean(&prepared.fit_sample().outcome, weight_view)?;
    let center = if dof.has_structural_intercept {
        outcome_mean
    } else {
        0.0
    };
    let certified_tss =
        weighted_sum_of_squares(&prepared.fit_sample().outcome, weight_view, center)?;
    let certified_within_tss = if prepared.inference_has_explicit_absorbed_effects() {
        fit.initial_absorbed_outcome_sum_of_squares()
    } else {
        certified_tss
    };
    let certified_rss = weighted_sum_of_squares(fit.residuals(), weight_view, 0.0)?;
    let mut tss = certified_tss;
    if perturbations.tss {
        tss *= 1.25;
    }
    let mut within_tss = certified_within_tss;
    if perturbations.within_tss {
        within_tss *= 1.25;
    }
    let mut rss = certified_rss;
    if perturbations.rss {
        rss *= 1.25;
    }
    validate_sum_of_squares(rss, tss, within_tss)?;

    let mut statistics = build_statistics(rss, tss, within_tss, weights, dof)?;
    if perturbations.r_squared {
        perturb_optional_statistic(&mut statistics.r_squared)?;
    }
    if perturbations.adjusted_r_squared {
        perturb_optional_statistic(&mut statistics.adjusted_r_squared)?;
    }
    if perturbations.within_r_squared {
        perturb_optional_statistic(&mut statistics.within_r_squared)?;
    }
    if perturbations.adjusted_within_r_squared {
        perturb_optional_statistic(&mut statistics.adjusted_within_r_squared)?;
    }
    if perturbations.root_mean_squared_error {
        statistics.root_mean_squared_error += 0.25;
    }
    if perturbations.gaussian_log_likelihood {
        perturb_optional_statistic(&mut statistics.gaussian_log_likelihood)?;
    }
    if perturbations.gaussian_null_log_likelihood {
        perturb_optional_statistic(&mut statistics.gaussian_null_log_likelihood)?;
    }
    let mut covariance = build_covariance(&fit, rss, dof.residual_dof, reservation)?;
    let wald = build_wald_f(prepared, &fit, &covariance, dof, reservation)?;
    statistics.f_statistic = wald.statistic;
    if perturbations.f_statistic {
        perturb_optional_statistic(&mut statistics.f_statistic)?;
    }
    if perturbations.variance_scale {
        covariance.variance_scale += 0.25;
    }
    if perturbations.covariance && !covariance.values.is_empty() {
        covariance.values[0] += 0.25;
    }
    if perturbations.standard_error && !covariance.standard_errors.is_empty() {
        covariance.standard_errors[0] += 0.25;
    }
    let certificate = certify_inference(
        prepared,
        &fit,
        weights,
        dof,
        statistics,
        &covariance,
        InferenceCertificateInputs {
            rss: certified_rss,
            tss: certified_tss,
            within_tss: certified_within_tss,
            wald_solve_relative_infinity_norm: wald.solve_relative_infinity_norm,
            reference_weight_sum_relative_error: weight_sum_relative_error,
            tolerance,
        },
    )?;
    if !certificate.passed {
        return Err(invalid_data(
            "prepared iid covariance or fit-statistic certificate did not pass",
        ));
    }
    Ok(PreparedOlsUnadjustedComponents {
        fit,
        weights,
        dof,
        statistics,
        covariance,
        certificate,
    })
}

'''

VALIDATION_BLOCK = '''pub(super) fn validate_fit_and_absorbed_dof(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    absorbed: &PreparedOlsAbsorbedDofReport,
) -> Result<()> {
    let summary = prepared.inference_absorbed_dof_summary(absorbed, absorbed.policy)?;
    validate_fit_and_absorbed_dof_view(prepared, fit, summary)
}

pub(super) fn validate_fit_and_absorbed_dof_view<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    absorbed: PreparedOlsAbsorbedDofSummary,
) -> Result<()> {
    let nobs = prepared.fit_sample().n_active();
    let reported_intercepts = prepared
        .fit_model()
        .columns
        .iter()
        .filter(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
        .count();
    if nobs == 0
        || reported_intercepts != usize::from(prepared.fit_report_intercept())
        || prepared.fit_report_intercept() && !prepared.fit_structural_intercept()
        || fit.residuals().len() != nobs
        || fit.coefficients().len() != prepared.fit_model().coefficients.len()
        || !fit.rank_certificate().passed
        || !fit.score_certificate().passed
        || !fit.bread().certificate().passed
        || !fit.absorption_report().certified()
        || !fit.residual_absorption_report().certified()
        || absorbed.nested_categories != 0
        || absorbed.redundant_categories > absorbed.initial_categories
        || absorbed.absorbed_rank
            != absorbed.initial_categories - absorbed.redundant_categories
    {
        return Err(invalid_argument(
            "prepared iid fit or absorbed rank is not complete for the final sample",
        ));
    }
    let bread_dimension = fit.bread().dimension();
    let bread_square = checked_square(
        bread_dimension,
        "prepared iid bread dimensions overflow usize",
    )?;
    if fit.bread().values().len() != bread_square
        || fit.bread().coordinates().len() != bread_dimension
    {
        return Err(invalid_argument(
            "prepared iid bread dimensions are inconsistent",
        ));
    }

    let mut kept = 0_usize;
    for (prepared_coefficient, fitted) in prepared
        .fit_model()
        .coefficients
        .iter()
        .zip(fit.coefficients())
    {
        if prepared_coefficient.coefficient_slot != fitted.coefficient_slot
            || prepared_coefficient.generated_column_index != fitted.generated_column_index
        {
            return Err(invalid_argument(
                "prepared iid coefficient identity changed after fitting",
            ));
        }
        if fitted.status == PreparedOlsFitStatus::Kept {
            kept = kept
                .checked_add(1)
                .ok_or_else(|| allocation("prepared iid kept-rank count overflows"))?;
        }
    }
    if kept != fit.finite_rank() {
        return Err(invalid_argument(
            "prepared iid kept coefficients disagree with finite rank",
        ));
    }

    let mut finite_coordinates = 0_usize;
    let mut normalized_coordinates = 0_usize;
    for coordinate in fit.bread().coordinates() {
        let prepared_column = prepared
            .fit_model()
            .columns
            .get(coordinate.generated_column_index)
            .ok_or_else(|| invalid_argument("prepared iid bread coordinate is out of range"))?;
        if prepared_column.coefficient_slot != coordinate.coefficient_slot {
            return Err(invalid_argument(
                "prepared iid bread coordinate slot is inconsistent",
            ));
        }
        let fitted = fit
            .coefficients()
            .iter()
            .find(|coefficient| {
                coefficient.generated_column_index == Some(coordinate.generated_column_index)
            })
            .ok_or_else(|| {
                invalid_argument("prepared iid bread coordinate has no fitted coefficient")
            })?;
        match coordinate.kind {
            PreparedOlsBreadCoordinateKind::FiniteRegressor => {
                if fitted.status != PreparedOlsFitStatus::Kept {
                    return Err(invalid_argument(
                        "prepared iid finite bread coordinate is not kept",
                    ));
                }
                finite_coordinates = finite_coordinates
                    .checked_add(1)
                    .ok_or_else(|| allocation("prepared iid finite-coordinate count overflows"))?;
            }
            PreparedOlsBreadCoordinateKind::NormalizedAbsorbedIntercept => {
                if fitted.status != PreparedOlsFitStatus::NormalizedAbsorbedIntercept
                    || !matches!(prepared_column.kind, PreparedOlsColumnKind::Intercept)
                {
                    return Err(invalid_argument(
                        "prepared iid normalized-intercept coordinate is inconsistent",
                    ));
                }
                normalized_coordinates =
                    normalized_coordinates.checked_add(1).ok_or_else(|| {
                        allocation("prepared iid normalized-intercept count overflows")
                    })?;
            }
        }
    }
    if finite_coordinates != fit.finite_rank()
        || normalized_coordinates > 1
        || (normalized_coordinates == 1
            && (!prepared.fit_structural_intercept() || !prepared.fit_report_intercept()))
    {
        return Err(invalid_argument(
            "prepared iid covariance-coordinate rank is inconsistent",
        ));
    }
    Ok(())
}

'''


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def regex_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return updated


INFERENCE_VIEW.write_text(INFERENCE_VIEW_TEXT, encoding="utf-8")

text = UNADJUSTED.read_text(encoding="utf-8")
marker = "\n#[cfg(test)]\nmod tests {"
if text.count(marker) != 1:
    raise SystemExit("prepared_unadjusted.rs test-module marker is not unique")
production, tests = text.split(marker, 1)

old_imports = """use crate::data::{PreparedWeights, WeightKind};
use crate::error::{ErrorCode, Result, StFixestError};

use super::{
    compute_prepared_ols_absorbed_dof, fit_prepared_ols_coefficients_map, CertifiedPreparedOlsFit,
    PreparedOlsAbsorbedDofEffectIdentity, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind,
    PreparedOlsColumnKind, PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatus,
};
"""
new_imports = """use crate::data::{PreparedWeights, WeightKind};
use crate::error::{ErrorCode, Result, StFixestError};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::topology_dof::CategoricalTopologyDofReport;

use super::prepared_inference_view::{
    PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView,
};
use super::{
    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_coefficients_map,
    CertifiedPreparedOlsFit, PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport,
    PreparedOlsBreadCoordinate, PreparedOlsBreadCoordinateKind, PreparedOlsColumnKind,
    PreparedOlsData, PreparedOlsFitConfig, PreparedOlsFitStatus,
};
"""
production = replace_once(production, old_imports, new_imports, "prepared iid imports")

production = replace_once(
    production,
    "\ntrait InferenceReservation {",
    NEW_RESULT_BLOCK + "trait InferenceReservation {",
    "categorical iid result insertion",
)

pipeline_start = production.index(
    "/// Fit and certify the current prepared model with conventional homoskedastic"
)
pipeline_end = production.index("fn validate_config(")
production = production[:pipeline_start] + PIPELINE_BLOCK + production[pipeline_end:]

validation_start = production.index("pub(super) fn validate_fit_and_absorbed_dof(")
reference_start = production.index("pub(super) fn build_reference_weight_report<'a>(")
production = production[:validation_start] + VALIDATION_BLOCK + production[reference_start:]

old_reference_header = """pub(super) fn build_reference_weight_report<'a>(
    prepared: &'a PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    tolerance: f64,
    allow_probability: bool,
) -> Result<(
    PreparedOlsReferenceWeightReport,
    ReferenceWeightView<'a>,
    f64,
)> {
"""
new_reference_header = """pub(super) fn build_reference_weight_report<'a>(
    prepared: &'a PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    tolerance: f64,
    allow_probability: bool,
) -> Result<(
    PreparedOlsReferenceWeightReport,
    ReferenceWeightView<'a>,
    f64,
)> {
    build_reference_weight_report_view(prepared, fit, tolerance, allow_probability)
}

pub(super) fn build_reference_weight_report_view<
    'a,
    P: PreparedOlsInferenceView + ?Sized,
>(
    prepared: &'a P,
    fit: &CertifiedPreparedOlsFit,
    tolerance: f64,
    allow_probability: bool,
) -> Result<(
    PreparedOlsReferenceWeightReport,
    ReferenceWeightView<'a>,
    f64,
)> {
"""
production = replace_once(
    production,
    old_reference_header,
    new_reference_header,
    "reference-weight generic wrapper",
)

old_residual_header = """pub(super) fn build_residual_dof(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    absorbed: &PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
) -> Result<PreparedOlsResidualDofReport> {
"""
new_residual_header = """pub(super) fn build_residual_dof(
    prepared: &PreparedOlsData,
    fit: &CertifiedPreparedOlsFit,
    absorbed: &PreparedOlsAbsorbedDofReport,
    weights: PreparedOlsReferenceWeightReport,
) -> Result<PreparedOlsResidualDofReport> {
    let summary = prepared.inference_absorbed_dof_summary(absorbed, absorbed.policy)?;
    build_residual_dof_view(prepared, fit, summary, weights)
}

pub(super) fn build_residual_dof_view<P: PreparedOlsInferenceView + ?Sized>(
    prepared: &P,
    fit: &CertifiedPreparedOlsFit,
    absorbed: PreparedOlsAbsorbedDofSummary,
    weights: PreparedOlsReferenceWeightReport,
) -> Result<PreparedOlsResidualDofReport> {
"""
production = replace_once(
    production,
    old_residual_header,
    new_residual_header,
    "residual-DoF generic wrapper",
)

production = regex_once(
    production,
    r"fn build_wald_f<R: InferenceReservation>\(\n    prepared: &PreparedOlsData,",
    "fn build_wald_f<P: PreparedOlsInferenceView + ?Sized, R: InferenceReservation>(\n"
    "    prepared: &P,",
    "Wald generic signature",
)
production = regex_once(
    production,
    r"fn certify_inference\(\n    prepared: &PreparedOlsData,",
    "fn certify_inference<P: PreparedOlsInferenceView + ?Sized>(\n"
    "    prepared: &P,",
    "iid certificate generic signature",
)

production = re.sub(r"prepared\s*\.\s*sample", "prepared.fit_sample()", production)
production = re.sub(r"prepared\s*\.\s*model", "prepared.fit_model()", production)
production = re.sub(
    r"prepared\s*\.\s*report_intercept",
    "prepared.fit_report_intercept()",
    production,
)
production = re.sub(
    r"prepared\s*\.\s*structural_intercept",
    "prepared.fit_structural_intercept()",
    production,
)
production = re.sub(
    r"prepared\s*\.\s*fe_topology\s*\.\s*effects\(\)\s*\.\s*is_empty\(\)",
    "!prepared.inference_has_explicit_absorbed_effects()",
    production,
)

for forbidden in [
    "prepared.sample",
    "prepared.model",
    "prepared.report_intercept",
    "prepared.structural_intercept",
    "prepared.fe_topology",
]:
    if forbidden in production:
        raise SystemExit(f"prepared_unadjusted.rs still contains legacy-only access: {forbidden}")

tests = replace_once(
    tests,
    "        OlsRequestV1,\n    };",
    "        OlsRequestV1, PreparedOlsAbsorbedDofEffectIdentity,\n    };",
    "test absorbed identity import",
)

UNADJUSTED.write_text(production + marker + tests, encoding="utf-8")

ols_mod = OLS_MOD.read_text(encoding="utf-8")
ols_mod = replace_once(
    ols_mod,
    "mod prepared_fit;\n",
    "mod prepared_fit;\nmod prepared_inference_view;\n",
    "OLS inference-view module",
)
ols_mod = replace_once(
    ols_mod,
    "pub use prepared_unadjusted::{\n"
    "    fit_prepared_ols_unadjusted_map, CertifiedPreparedOlsUnadjusted, PreparedOlsFitStatistics,\n",
    "pub use prepared_unadjusted::{\n"
    "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_unadjusted_map,\n"
    "    CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsUnadjusted,\n"
    "    PreparedOlsFitStatistics,\n",
    "OLS categorical iid exports",
)
OLS_MOD.write_text(ols_mod, encoding="utf-8")

lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_clustered_map,\n",
    "    fit_prepared_categorical_ols_coefficients_map,\n"
    "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_clustered_map,\n",
    "crate categorical iid function export",
)
lib = replace_once(
    lib,
    "    PreparedOlsClusteredPsdState, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,\n",
    "    PreparedOlsClusteredPsdState, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,\n"
    "    CertifiedPreparedCategoricalOlsUnadjusted,\n",
    "crate categorical iid result export",
)
LIB.write_text(lib, encoding="utf-8")

print("generalized prepared iid inference over categorical view")
