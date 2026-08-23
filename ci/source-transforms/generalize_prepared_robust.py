#!/usr/bin/env python3
"""Generalize certified HC1 composition over PreparedOlsInferenceView."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ROBUST = ROOT / "crates/stfixest-core/src/ols/prepared_robust.rs"
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"
LIB = ROOT / "crates/stfixest-core/src/lib.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


text = ROBUST.read_text(encoding="utf-8")
marker = "\n#[cfg(test)]\nmod tests {"
if text.count(marker) != 1:
    raise SystemExit("prepared_robust.rs test-module marker is not unique")
production, tests = text.split(marker, 1)

production = replace_once(
    production,
    "use crate::error::{ErrorCode, Result, StFixestError};\n",
    "use crate::error::{ErrorCode, Result, StFixestError};\n"
    "use crate::prepared_categorical_data::PreparedCategoricalOlsData;\n"
    "use crate::topology_dof::CategoricalTopologyDofReport;\n",
    "categorical robust imports",
)
production = replace_once(
    production,
    "use super::prepared_scores::{\n",
    "use super::prepared_inference_view::{\n"
    "    PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView,\n"
    "};\n"
    "use super::prepared_scores::{\n",
    "inference-view import",
)
production = replace_once(
    production,
    "use super::prepared_unadjusted::{\n"
    "    build_reference_weight_report, build_residual_dof, build_statistics, checked_square,\n"
    "    compensated_add, exact_integer_as_f64, relative_difference, validate_fit_and_absorbed_dof,\n"
    "    validate_sum_of_squares, weighted_mean, weighted_sum_of_squares, ReferenceWeightView,\n"
    "};\n",
    "use super::prepared_unadjusted::{\n"
    "    build_reference_weight_report_view, build_residual_dof_view, build_statistics,\n"
    "    checked_square, compensated_add, exact_integer_as_f64, relative_difference,\n"
    "    validate_fit_and_absorbed_dof_view, validate_sum_of_squares, weighted_mean,\n"
    "    weighted_sum_of_squares, ReferenceWeightView,\n"
    "};\n",
    "shared inference helper imports",
)
production = replace_once(
    production,
    "use super::{\n"
    "    compute_prepared_ols_absorbed_dof, fit_prepared_ols_coefficients_map, CertifiedPreparedOlsFit,\n",
    "use super::{\n"
    "    compute_prepared_ols_absorbed_dof, fit_prepared_categorical_ols_coefficients_map,\n"
    "    fit_prepared_ols_coefficients_map, CertifiedPreparedOlsFit,\n",
    "categorical coefficient-fit import",
)

result_marker = "\n#[derive(Debug, Clone, Copy, Default)]\nstruct RobustPerturbations {"
if production.count(result_marker) != 1:
    raise SystemExit("robust perturbation marker is not unique")
categorical_result = """

/// Same-call categorical-interaction fit, absorbed rank, HC1 covariance, and
/// fit statistics. The categorical topology DoF report remains exact and is
/// retained without changing the legacy result or plugin wire identity.
#[derive(Debug, PartialEq)]
pub struct CertifiedPreparedCategoricalOlsRobust {
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: CategoricalTopologyDofReport,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsRobustCovariance,
    certificate: PreparedOlsRobustCertificate,
}

impl CertifiedPreparedCategoricalOlsRobust {
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
    pub const fn covariance(&self) -> &PreparedOlsRobustCovariance {
        &self.covariance
    }

    #[must_use]
    pub fn covariance_coordinates(&self) -> &[PreparedOlsBreadCoordinate] {
        self.fit.bread().coordinates()
    }

    #[must_use]
    pub const fn certificate(&self) -> PreparedOlsRobustCertificate {
        self.certificate
    }
}

#[derive(Debug, PartialEq)]
struct PreparedOlsRobustComponents {
    fit: CertifiedPreparedOlsFit,
    weights: PreparedOlsReferenceWeightReport,
    dof: PreparedOlsResidualDofReport,
    statistics: PreparedOlsFitStatistics,
    covariance: PreparedOlsRobustCovariance,
    certificate: PreparedOlsRobustCertificate,
}
"""
production = production.replace(result_marker, categorical_result + result_marker, 1)

legacy_public = """pub fn fit_prepared_ols_robust_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsRobustConfig,
) -> Result<CertifiedPreparedOlsRobust> {
    validate_config(config)?;
    let fit = fit_prepared_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = compute_prepared_ols_absorbed_dof(prepared, config.absorbed_dof)?;
    let mut reservation = SystemPreparedInferenceReservation;
    compose_internal(
        prepared,
        fit,
        absorbed_dof,
        config,
        &mut reservation,
        RobustPerturbations::default(),
    )
}
"""
categorical_public = legacy_public + """

/// Fit and certify categorical-interaction HC1 covariance through the same
/// numerical composition used by the legacy prepared path.
pub fn fit_prepared_categorical_ols_robust_map(
    prepared: &PreparedCategoricalOlsData,
    config: PreparedOlsRobustConfig,
) -> Result<CertifiedPreparedCategoricalOlsRobust> {
    validate_config(config)?;
    let fit = fit_prepared_categorical_ols_coefficients_map(prepared, config.fit)?;
    let absorbed_dof = prepared.inference_compute_absorbed_dof(config.absorbed_dof)?;
    let absorbed_summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let mut reservation = SystemPreparedInferenceReservation;
    let components = compose_view_internal(
        prepared,
        fit,
        absorbed_summary,
        config,
        &mut reservation,
        RobustPerturbations::default(),
    )?;
    Ok(CertifiedPreparedCategoricalOlsRobust {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}
"""
production = replace_once(
    production,
    legacy_public,
    categorical_public,
    "categorical robust public wrapper",
)

old_compose_start = """fn compose_internal<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
) -> Result<CertifiedPreparedOlsRobust> {
    validate_fit_and_absorbed_dof(prepared, &fit, &absorbed_dof)?;
    let (weights, weight_view, reference_weight_error) =
        build_reference_weight_report(prepared, &fit, config.certificate_tolerance, true)?;
    let mut dof = build_residual_dof(prepared, &fit, &absorbed_dof, weights)?;
"""
new_compose_start = """fn compose_internal<R: PreparedInferenceReservation>(
    prepared: &PreparedOlsData,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofReport,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
) -> Result<CertifiedPreparedOlsRobust> {
    let absorbed_summary =
        prepared.inference_absorbed_dof_summary(&absorbed_dof, config.absorbed_dof)?;
    let components = compose_view_internal(
        prepared,
        fit,
        absorbed_summary,
        config,
        reservation,
        perturbations,
    )?;
    Ok(CertifiedPreparedOlsRobust {
        fit: components.fit,
        absorbed_dof,
        weights: components.weights,
        dof: components.dof,
        statistics: components.statistics,
        covariance: components.covariance,
        certificate: components.certificate,
    })
}

fn compose_view_internal<
    P: PreparedOlsInferenceView + ?Sized,
    R: PreparedInferenceReservation,
>(
    prepared: &P,
    fit: CertifiedPreparedOlsFit,
    absorbed_dof: PreparedOlsAbsorbedDofSummary,
    config: PreparedOlsRobustConfig,
    reservation: &mut R,
    perturbations: RobustPerturbations,
) -> Result<PreparedOlsRobustComponents> {
    validate_fit_and_absorbed_dof_view(prepared, &fit, absorbed_dof)?;
    let (weights, weight_view, reference_weight_error) = build_reference_weight_report_view(
        prepared,
        &fit,
        config.certificate_tolerance,
        true,
    )?;
    let mut dof = build_residual_dof_view(prepared, &fit, absorbed_dof, weights)?;
"""
production = replace_once(
    production,
    old_compose_start,
    new_compose_start,
    "shared robust composition boundary",
)
production = production.replace(
    "&prepared.sample.outcome",
    "&prepared.fit_sample().outcome",
)
production = replace_once(
    production,
    "    let within_tss = if prepared.fe_topology.effects().is_empty() {\n",
    "    let within_tss = if !prepared.inference_has_explicit_absorbed_effects() {\n",
    "shared robust within-TSS boundary",
)
old_result = """    Ok(CertifiedPreparedOlsRobust {
        fit,
        absorbed_dof,
        weights,
        dof,
        statistics,
        covariance,
        certificate,
    })
}
"""
new_result = """    Ok(PreparedOlsRobustComponents {
        fit,
        weights,
        dof,
        statistics,
        covariance,
        certificate,
    })
}
"""
production = replace_once(
    production,
    old_result,
    new_result,
    "shared robust component result",
)
production = replace_once(
    production,
    "fn certify_dof_identity(\n"
    "    prepared: &PreparedOlsData,\n",
    "fn certify_dof_identity<P: PreparedOlsInferenceView + ?Sized>(\n"
    "    prepared: &P,\n",
    "shared robust DoF certificate signature",
)
production = production.replace(
    "prepared.report_intercept && !prepared.structural_intercept",
    "prepared.fit_report_intercept() && !prepared.fit_structural_intercept()",
)
production = production.replace(
    "prepared.structural_intercept",
    "prepared.fit_structural_intercept()",
)
production = replace_once(
    production,
    "fn build_robust_wald<R: PreparedInferenceReservation>(\n"
    "    prepared: &PreparedOlsData,\n",
    "fn build_robust_wald<\n"
    "    P: PreparedOlsInferenceView + ?Sized,\n"
    "    R: PreparedInferenceReservation,\n"
    ">(\n"
    "    prepared: &P,\n",
    "shared robust Wald signature",
)
production = production.replace(
    "        let column = prepared\n            .model\n            .columns\n",
    "        let column = prepared\n            .fit_model()\n            .columns\n",
)
for forbidden in [
    "prepared.sample",
    "prepared.fe_topology",
    "prepared.report_intercept",
    "prepared.structural_intercept",
    ".model\n            .columns",
    "build_reference_weight_report(prepared",
    "build_residual_dof(prepared",
    "validate_fit_and_absorbed_dof(prepared",
]:
    if forbidden in production:
        raise SystemExit(f"prepared_robust.rs retains legacy-only composition access: {forbidden}")
ROBUST.write_text(production + marker + tests, encoding="utf-8")

ols_mod = OLS_MOD.read_text(encoding="utf-8")
ols_mod = replace_once(
    ols_mod,
    "pub use prepared_robust::{\n"
    "    fit_prepared_ols_robust_map, CertifiedPreparedOlsRobust, PreparedOlsRobustCertificate,\n"
    "    PreparedOlsRobustConfig, PreparedOlsRobustCovariance,\n"
    "};\n",
    "pub use prepared_robust::{\n"
    "    fit_prepared_categorical_ols_robust_map, fit_prepared_ols_robust_map,\n"
    "    CertifiedPreparedCategoricalOlsRobust, CertifiedPreparedOlsRobust,\n"
    "    PreparedOlsRobustCertificate, PreparedOlsRobustConfig, PreparedOlsRobustCovariance,\n"
    "};\n",
    "OLS robust exports",
)
OLS_MOD.write_text(ols_mod, encoding="utf-8")

lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_clustered_map,\n"
    "    fit_prepared_ols_coefficients_map, fit_prepared_ols_robust_map,\n",
    "    fit_prepared_categorical_ols_robust_map, fit_prepared_categorical_ols_unadjusted_map,\n"
    "    fit_prepared_ols_clustered_map, fit_prepared_ols_coefficients_map,\n"
    "    fit_prepared_ols_robust_map,\n",
    "crate categorical robust function export",
)
lib = replace_once(
    lib,
    "    solve_rank_revealing, CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsFit,\n"
    "    CertifiedPreparedOlsRobust, CertifiedPreparedOlsUnadjusted, ClusterSubsetReport, DofConfig,\n",
    "    solve_rank_revealing, CertifiedPreparedCategoricalOlsRobust,\n"
    "    CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsFit,\n"
    "    CertifiedPreparedOlsRobust, CertifiedPreparedOlsUnadjusted, ClusterSubsetReport, DofConfig,\n",
    "crate categorical robust result export",
)
LIB.write_text(lib, encoding="utf-8")

print("generalized certified HC1 composition over PreparedOlsInferenceView")
