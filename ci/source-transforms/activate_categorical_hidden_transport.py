#!/usr/bin/env python3
"""Activate categorical-interaction transport for the hidden OLS developer action."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESULT_VIEW = ROOT / "crates/stfixest-plugin/src/result_view.rs"
RESULT = ROOT / "crates/stfixest-plugin/src/result.rs"
ESTIMATE = ROOT / "crates/stfixest-plugin/src/estimate.rs"
STATA = ROOT / "stata/_stfixest_estimate_ols_test.ado"
PROFILES = ROOT / "ci/stata_profiles.json"
FIXTURE = ROOT / "tests/stata/installation/estimate_ols_categorical_interaction.do"
CONTRACT = ROOT / "tests/python/test_categorical_hidden_transport_contract.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def regex_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return updated


RESULT_VIEW.write_text(
    r'''//! Borrowed prepared-result views for the hidden OLS transport.
//!
//! Legacy single-source and categorical-interaction preparation own different
//! topology artifacts but expose the same final model, sample, singleton, and
//! cluster metadata. These copyable views let one serializer borrow either path
//! without cloning observation-scale state or changing result-wire dimensions.

use stfixest_core::{
    PreparedCategoricalOlsClusteredData, PreparedCategoricalOlsData, PreparedNumericSample,
    PreparedOlsClusterDesign, PreparedOlsClusteredData, PreparedOlsData, PreparedOlsModel,
    PreparedOlsSingletons,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOlsResultData<'a> {
    Legacy(&'a PreparedOlsData),
    Categorical(&'a PreparedCategoricalOlsData),
}

impl<'a> PreparedOlsResultData<'a> {
    pub(crate) const fn legacy(value: &'a PreparedOlsData) -> Self {
        Self::Legacy(value)
    }

    pub(crate) const fn categorical(value: &'a PreparedCategoricalOlsData) -> Self {
        Self::Categorical(value)
    }

    pub(crate) const fn model(self) -> &'a PreparedOlsModel {
        match self {
            Self::Legacy(value) => &value.model,
            Self::Categorical(value) => value.model(),
        }
    }

    pub(crate) const fn sample(self) -> &'a PreparedNumericSample {
        match self {
            Self::Legacy(value) => &value.sample,
            Self::Categorical(value) => value.sample(),
        }
    }

    pub(crate) const fn singletons(self) -> PreparedOlsSingletons {
        match self {
            Self::Legacy(value) => value.singletons,
            Self::Categorical(value) => value.singletons(),
        }
    }

    fn is_same_legacy(self, value: &'a PreparedOlsData) -> bool {
        match self {
            Self::Legacy(prepared) => std::ptr::eq(prepared, value),
            Self::Categorical(_) => false,
        }
    }

    fn is_same_categorical(self, value: &'a PreparedCategoricalOlsData) -> bool {
        match self {
            Self::Legacy(_) => false,
            Self::Categorical(prepared) => std::ptr::eq(prepared, value),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreparedOlsClusteredResultData<'a> {
    Legacy(&'a PreparedOlsClusteredData),
    Categorical(&'a PreparedCategoricalOlsClusteredData),
}

impl<'a> PreparedOlsClusteredResultData<'a> {
    pub(crate) const fn legacy(value: &'a PreparedOlsClusteredData) -> Self {
        Self::Legacy(value)
    }

    pub(crate) const fn categorical(value: &'a PreparedCategoricalOlsClusteredData) -> Self {
        Self::Categorical(value)
    }

    pub(crate) const fn ols(self) -> PreparedOlsResultData<'a> {
        match self {
            Self::Legacy(value) => PreparedOlsResultData::Legacy(value.ols()),
            Self::Categorical(value) => PreparedOlsResultData::Categorical(value.ols()),
        }
    }

    pub(crate) const fn clusters(self) -> &'a PreparedOlsClusterDesign {
        match self {
            Self::Legacy(value) => value.clusters(),
            Self::Categorical(value) => value.clusters(),
        }
    }

    pub(crate) fn is_bound_to(self, prepared: PreparedOlsResultData<'a>) -> bool {
        match self.ols() {
            PreparedOlsResultData::Legacy(value) => prepared.is_same_legacy(value),
            PreparedOlsResultData::Categorical(value) => prepared.is_same_categorical(value),
        }
    }
}
''',
    encoding="utf-8",
)

result = RESULT.read_text(encoding="utf-8")
result = regex_once(
    result,
    r"use stfixest_core::ols::\{.*?\};\n\nuse crate::result_view",
    '''use stfixest_core::ols::{
    CertifiedPreparedCategoricalOlsClustered, CertifiedPreparedCategoricalOlsRobust,
    CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsClustered,
    CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust, CertifiedPreparedOlsUnadjusted,
    PreparedOlsAbsorbedDofEffectIdentity, PreparedOlsAbsorbedDofReport,
    PreparedOlsBreadCoordinate, PreparedOlsClusterSubsetCertificate,
    PreparedOlsClusteredCertificate, PreparedOlsClusteredCovariance, PreparedOlsClusteredDofReport,
    PreparedOlsClusteredPsdState, PreparedOlsFitStatistics, PreparedOlsFitStatus,
    PreparedOlsReferenceWeightReport, PreparedOlsResidualDofReport, PreparedOlsRobustCertificate,
    PreparedOlsUnadjustedCertificate,
};
use stfixest_core::{
    CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofReport,
};

use crate::result_view''',
    "categorical result imports",
)
result = regex_once(
    result,
    r"pub\(crate\) enum CertifiedOlsInference<'a> \{.*?\n}\n\n#\[derive\(Debug\)\]\npub\(crate\) struct OlsShadowResult",
    r'''#[derive(Debug, Clone, Copy)]
enum PreparedAbsorbedDofData<'a> {
    Legacy(&'a PreparedOlsAbsorbedDofReport),
    Categorical(&'a CategoricalTopologyDofReport),
}

impl PreparedAbsorbedDofData<'_> {
    fn effects_len(self) -> usize {
        match self {
            Self::Legacy(value) => value.effects.len(),
            Self::Categorical(value) => value.effects.len(),
        }
    }

    fn initial_categories(self) -> usize {
        match self {
            Self::Legacy(value) => value.initial_categories,
            Self::Categorical(value) => value.initial_categories,
        }
    }

    fn redundant_categories(self) -> usize {
        match self {
            Self::Legacy(value) => value.redundant_categories,
            Self::Categorical(value) => value.redundant_categories,
        }
    }

    fn nested_categories(self) -> usize {
        match self {
            Self::Legacy(value) => value.nested_categories,
            Self::Categorical(value) => value.nested_categories,
        }
    }
}

pub(crate) enum CertifiedOlsInference<'a> {
    Iid(&'a CertifiedPreparedOlsUnadjusted),
    Robust(&'a CertifiedPreparedOlsRobust),
    Clustered(&'a CertifiedPreparedOlsClustered),
    CategoricalIid(&'a CertifiedPreparedCategoricalOlsUnadjusted),
    CategoricalRobust(&'a CertifiedPreparedCategoricalOlsRobust),
    CategoricalClustered(&'a CertifiedPreparedCategoricalOlsClustered),
}

impl<'a> CertifiedOlsInference<'a> {
    pub(crate) const fn kind(&self) -> OlsVceKind {
        match self {
            Self::Iid(_) | Self::CategoricalIid(_) => OlsVceKind::Iid,
            Self::Robust(_) | Self::CategoricalRobust(_) => OlsVceKind::Robust,
            Self::Clustered(_) | Self::CategoricalClustered(_) => OlsVceKind::Clustered,
        }
    }

    pub(crate) const fn fit(&self) -> &'a CertifiedPreparedOlsFit {
        match self {
            Self::Iid(value) => value.fit(),
            Self::Robust(value) => value.fit(),
            Self::Clustered(value) => value.fit(),
            Self::CategoricalIid(value) => value.fit(),
            Self::CategoricalRobust(value) => value.fit(),
            Self::CategoricalClustered(value) => value.fit(),
        }
    }

    fn absorbed_dof(&self) -> PreparedAbsorbedDofData<'a> {
        match self {
            Self::Iid(value) => PreparedAbsorbedDofData::Legacy(value.absorbed_dof()),
            Self::Robust(value) => PreparedAbsorbedDofData::Legacy(value.absorbed_dof()),
            Self::Clustered(value) => PreparedAbsorbedDofData::Legacy(value.absorbed_dof()),
            Self::CategoricalIid(value) => {
                PreparedAbsorbedDofData::Categorical(value.absorbed_dof())
            }
            Self::CategoricalRobust(value) => {
                PreparedAbsorbedDofData::Categorical(value.absorbed_dof())
            }
            Self::CategoricalClustered(value) => {
                PreparedAbsorbedDofData::Categorical(value.absorbed_dof())
            }
        }
    }

    fn weights(&self) -> PreparedOlsReferenceWeightReport {
        match self {
            Self::Iid(value) => value.weights(),
            Self::Robust(value) => value.weights(),
            Self::Clustered(value) => value.weights(),
            Self::CategoricalIid(value) => value.weights(),
            Self::CategoricalRobust(value) => value.weights(),
            Self::CategoricalClustered(value) => value.weights(),
        }
    }

    fn statistics(&self) -> PreparedOlsFitStatistics {
        match self {
            Self::Iid(value) => value.statistics(),
            Self::Robust(value) => value.statistics(),
            Self::Clustered(value) => value.statistics(),
            Self::CategoricalIid(value) => value.statistics(),
            Self::CategoricalRobust(value) => value.statistics(),
            Self::CategoricalClustered(value) => value.statistics(),
        }
    }

    fn covariance_coordinates(&self) -> &'a [PreparedOlsBreadCoordinate] {
        match self {
            Self::Iid(value) => value.covariance_coordinates(),
            Self::Robust(value) => value.covariance_coordinates(),
            Self::Clustered(value) => value.covariance_coordinates(),
            Self::CategoricalIid(value) => value.covariance_coordinates(),
            Self::CategoricalRobust(value) => value.covariance_coordinates(),
            Self::CategoricalClustered(value) => value.covariance_coordinates(),
        }
    }

    fn covariance_values(&self) -> &'a [f64] {
        match self {
            Self::Iid(value) => value.covariance().values(),
            Self::Robust(value) => value.covariance().values(),
            Self::Clustered(value) => value.covariance().values(),
            Self::CategoricalIid(value) => value.covariance().values(),
            Self::CategoricalRobust(value) => value.covariance().values(),
            Self::CategoricalClustered(value) => value.covariance().values(),
        }
    }

    fn residual_dof(&self) -> Option<PreparedOlsResidualDofReport> {
        match self {
            Self::Iid(value) => Some(value.dof()),
            Self::Robust(value) => Some(value.dof()),
            Self::CategoricalIid(value) => Some(value.dof()),
            Self::CategoricalRobust(value) => Some(value.dof()),
            Self::Clustered(_) | Self::CategoricalClustered(_) => None,
        }
    }

    fn clustered_dof(&self) -> Option<PreparedOlsClusteredDofReport> {
        match self {
            Self::Clustered(value) => Some(value.dof()),
            Self::CategoricalClustered(value) => Some(value.dof()),
            _ => None,
        }
    }

    fn cluster_counts(&self) -> Option<&'a [usize]> {
        match self {
            Self::Clustered(value) => Some(value.cluster_counts()),
            Self::CategoricalClustered(value) => Some(value.cluster_counts()),
            _ => None,
        }
    }

    fn subsets(&self) -> Option<&'a [PreparedOlsClusterSubsetCertificate]> {
        match self {
            Self::Clustered(value) => Some(value.subsets()),
            Self::CategoricalClustered(value) => Some(value.subsets()),
            _ => None,
        }
    }

    fn clustered_covariance(&self) -> Option<&'a PreparedOlsClusteredCovariance> {
        match self {
            Self::Clustered(value) => Some(value.covariance()),
            Self::CategoricalClustered(value) => Some(value.covariance()),
            _ => None,
        }
    }

    fn iid_certificate(&self) -> Option<PreparedOlsUnadjustedCertificate> {
        match self {
            Self::Iid(value) => Some(value.certificate()),
            Self::CategoricalIid(value) => Some(value.certificate()),
            _ => None,
        }
    }

    fn robust_certificate(&self) -> Option<PreparedOlsRobustCertificate> {
        match self {
            Self::Robust(value) => Some(value.certificate()),
            Self::CategoricalRobust(value) => Some(value.certificate()),
            _ => None,
        }
    }

    fn clustered_certificate(&self) -> Option<PreparedOlsClusteredCertificate> {
        match self {
            Self::Clustered(value) => Some(value.certificate()),
            Self::CategoricalClustered(value) => Some(value.certificate()),
            _ => None,
        }
    }

    fn certificate_passed(&self) -> bool {
        self.iid_certificate().is_some_and(|value| value.passed)
            || self.robust_certificate().is_some_and(|value| value.passed)
            || self
                .clustered_certificate()
                .is_some_and(|value| value.passed)
    }

    fn is_clustered(&self) -> bool {
        matches!(self.kind(), OlsVceKind::Clustered)
    }
}

#[derive(Debug)]
pub(crate) struct OlsShadowResult''',
    "categorical inference variants",
)
result = regex_once(
    result,
    r"fn validate_alignment\(.*?\n}\n\n(?=fn build_fit_statistics)",
    r'''fn validate_alignment(
    prepared: PreparedOlsResultData<'_>,
    clustered: Option<PreparedOlsClusteredResultData<'_>>,
    inference: &CertifiedOlsInference<'_>,
) -> Result<()> {
    match (inference.is_clustered(), clustered) {
        (true, Some(value)) if value.is_bound_to(prepared) => {}
        (true, _) => {
            return Err(invalid(
                "clustered inference is not sample-bound to its preparation",
            ));
        }
        (false, None) => {}
        (false, Some(_)) => {
            return Err(invalid(
                "nonclustered inference unexpectedly has cluster preparation",
            ));
        }
    }
    if !inference.fit().rank_certificate().passed
        || !inference.fit().score_certificate().passed
        || !inference.fit().bread().certificate().passed
    {
        return Err(invalid("prepared OLS fit certificate is not complete"));
    }
    if !inference.certificate_passed() {
        return Err(invalid(
            "prepared OLS inference certificate is not complete",
        ));
    }
    Ok(())
}

''',
    "categorical alignment validation",
)
result = regex_once(
    result,
    r"fn build_dof\(.*?\n}\n\n(?=fn build_solver)",
    r'''fn build_dof(
    prepared: PreparedOlsResultData<'_>,
    inference: &CertifiedOlsInference<'_>,
) -> Result<Vec<f64>> {
    let mut output = reserved_filled(DOF_ROWS * 2, 0.0, "OLS DoF table")?;
    let weights = inference.weights();
    let absorbed = inference.absorbed_dof();
    let (
        finite_rank,
        finite_model_dof,
        absorbed_rank,
        model_rank,
        reported,
        fit,
        correction,
        nested,
        structural,
        normalized,
        approximate,
    ) = if let Some(dof) = inference.clustered_dof() {
        let model_rank = dof
            .finite_rank
            .checked_add(dof.absorbed_rank)
            .ok_or_else(|| allocation("clustered OLS model rank overflows"))?;
        (
            dof.finite_rank,
            dof.finite_model_dof,
            dof.absorbed_rank,
            model_rank,
            dof.reported_residual_dof,
            dof.fit_residual_dof,
            dof.correction_denominator_dof,
            dof.nested_absorbed_categories,
            dof.has_structural_intercept,
            dof.has_normalized_absorbed_intercept,
            dof.absorbed_rank_is_approximate,
        )
    } else {
        let dof = inference
            .residual_dof()
            .ok_or_else(|| invalid("nonclustered OLS residual DoF is absent"))?;
        (
            dof.finite_rank,
            dof.finite_model_dof,
            dof.absorbed_rank,
            dof.model_rank,
            dof.residual_dof,
            dof.residual_dof,
            dof.residual_dof,
            absorbed.nested_categories(),
            dof.has_structural_intercept,
            dof.has_normalized_absorbed_intercept,
            dof.absorbed_rank_is_approximate,
        )
    };
    let fields = [
        Some(weights.effective_observations as f64),
        Some(weights.physical_observations as f64),
        Some(prepared.sample().n_candidate as f64),
        Some(prepared.sample().n_active() as f64),
        weights.raw_weight_sum,
        Some(finite_rank as f64),
        Some(finite_model_dof as f64),
        Some(absorbed_rank as f64),
        Some(model_rank as f64),
        Some(reported as f64),
        Some(fit as f64),
        Some(correction as f64),
        Some(nested as f64),
        Some(absorbed.initial_categories() as f64),
        Some(absorbed.redundant_categories() as f64),
        Some(prepared.singletons().num_singletons as f64),
        Some(prepared.singletons().cascade_events as f64),
        Some(f64::from(structural)),
        Some(f64::from(normalized)),
        Some(f64::from(approximate)),
    ];
    for (row, value) in fields.into_iter().enumerate() {
        set_defined(&mut output, row, value, "OLS DoF field")?;
    }
    Ok(output)
}

''',
    "categorical DoF serialization",
)
result = regex_once(
    result,
    r"fn build_fixed_effects\(.*?\n}\n\n(?=fn build_clusters)",
    r'''fn build_fixed_effects(report: PreparedAbsorbedDofData<'_>) -> Result<(Vec<f64>, usize)> {
    let rows = report.effects_len().max(1);
    let mut output = reserved_filled(
        checked_product(rows, FE_COLUMNS, "OLS FE metadata")?,
        0.0,
        "OLS FE metadata",
    )?;
    match report {
        PreparedAbsorbedDofData::Legacy(report) => {
            for (row, effect) in report.effects.iter().enumerate() {
                let (identity, term, source) = match effect.identity {
                    PreparedOlsAbsorbedDofEffectIdentity::Explicit { term_id, source } => {
                        (1.0, f64::from(term_id.get()), f64::from(source.get()))
                    }
                    PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb => (2.0, 0.0, 0.0),
                };
                write_fixed_effect_row(
                    &mut output,
                    row,
                    identity,
                    term,
                    source,
                    effect.categories,
                    effect.redundant,
                    effect.coefficients,
                    effect.redundancy_exact,
                    effect.nested,
                );
            }
        }
        PreparedAbsorbedDofData::Categorical(report) => {
            for (row, effect) in report.effects.iter().enumerate() {
                let (identity, term, source) = match &effect.identity {
                    CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources } => {
                        match sources.as_slice() {
                            [] => {
                                return Err(invalid(
                                    "categorical absorbed effect has no source identity",
                                ));
                            }
                            [source] => {
                                (1.0, f64::from(term_id.get()), f64::from(source.get()))
                            }
                            _ => (3.0, f64::from(term_id.get()), 0.0),
                        }
                    }
                    CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb => (2.0, 0.0, 0.0),
                };
                write_fixed_effect_row(
                    &mut output,
                    row,
                    identity,
                    term,
                    source,
                    effect.categories,
                    effect.redundant,
                    effect.coefficients,
                    effect.redundancy_exact,
                    effect.nested,
                );
            }
        }
    }
    Ok((output, rows))
}

#[allow(clippy::too_many_arguments)]
fn write_fixed_effect_row(
    output: &mut [f64],
    row: usize,
    identity: f64,
    term: f64,
    source: f64,
    categories: usize,
    redundant: usize,
    coefficients: usize,
    redundancy_exact: bool,
    nested: bool,
) {
    let values = [
        identity,
        term,
        source,
        categories as f64,
        redundant as f64,
        coefficients as f64,
        f64::from(redundancy_exact),
        f64::from(nested),
    ];
    output[row * FE_COLUMNS..(row + 1) * FE_COLUMNS].copy_from_slice(&values);
}

''',
    "categorical FE metadata serialization",
)
result = regex_once(
    result,
    r"fn build_clusters\(.*?\n}\n\n(?=fn build_certificates)",
    r'''fn build_clusters(
    prepared: Option<PreparedOlsClusteredResultData<'_>>,
    inference: &CertifiedOlsInference<'_>,
) -> Result<(Vec<f64>, usize, Vec<f64>, usize)> {
    let cluster_counts = inference.cluster_counts().unwrap_or(&[]);
    let subset_reports = inference.subsets().unwrap_or(&[]);
    let cluster_rows = cluster_counts.len();
    let subset_rows = subset_reports.len();
    let physical_cluster_rows = cluster_rows.max(1);
    let physical_subset_rows = subset_rows.max(1);
    let mut clusters = reserved_filled(
        checked_product(
            physical_cluster_rows,
            CLUSTER_COLUMNS,
            "OLS cluster metadata",
        )?,
        0.0,
        "OLS cluster metadata",
    )?;
    let mut subsets = reserved_filled(
        checked_product(physical_subset_rows, SUBSET_COLUMNS, "OLS subset metadata")?,
        0.0,
        "OLS subset metadata",
    )?;
    if inference.is_clustered() {
        let prepared = prepared
            .ok_or_else(|| invalid("clustered OLS preparation is absent"))?;
        if prepared.clusters().dimensions().len() != cluster_counts.len() {
            return Err(invalid("prepared OLS cluster counts are inconsistent"));
        }
        for (row, (dimension, count)) in prepared
            .clusters()
            .dimensions()
            .iter()
            .zip(cluster_counts)
            .enumerate()
        {
            if dimension.groups() != *count {
                return Err(invalid(
                    "prepared OLS cluster group count changed after fit",
                ));
            }
            let values = [
                (row + 1) as f64,
                dimension.sources().len() as f64,
                *count as f64,
                f64::from(dimension.is_interaction()),
            ];
            clusters[row * CLUSTER_COLUMNS..(row + 1) * CLUSTER_COLUMNS].copy_from_slice(&values);
        }
        for (row, subset) in subset_reports.iter().enumerate() {
            let values = [
                f64::from(subset.mask),
                subset.dimensions as f64,
                f64::from(subset.sign),
                subset.groups as f64,
            ];
            subsets[row * SUBSET_COLUMNS..(row + 1) * SUBSET_COLUMNS].copy_from_slice(&values);
        }
    }
    Ok((
        clusters,
        physical_cluster_rows,
        subsets,
        physical_subset_rows,
    ))
}

''',
    "categorical cluster metadata serialization",
)
result = regex_once(
    result,
    r"fn build_certificates\(.*?\n}\n\n(?=/// Determinant of the exact covariance)",
    r'''fn build_certificates(
    inference: &CertifiedOlsInference<'_>,
    reported_covariance: &[f64],
    reported_dimension: usize,
) -> Result<Vec<f64>> {
    let mut output = reserved_filled(CERTIFICATE_ROWS * 2, 0.0, "OLS certificate diagnostics")?;
    let fit = inference.fit();
    let rank = fit.rank_certificate();
    let score = fit.score_certificate();
    let bread = fit.bread().certificate();
    let common = [
        Some(f64::from(rank.passed)),
        Some(f64::from(score.passed)),
        Some(f64::from(bread.passed)),
        Some(rank.relative_threshold),
        Some(rank.maximum_dropped_trailing_norm),
        Some(rank.condition_proxy),
        Some(score.relative_infinity_norm),
        Some(score.relative_two_norm),
        Some(bread.left_inverse_relative_infinity_norm),
        Some(bread.right_inverse_relative_infinity_norm),
        Some(bread.reported_left_inverse_relative_infinity_norm),
        Some(bread.reported_right_inverse_relative_infinity_norm),
        Some(bread.symmetry_relative_infinity_norm),
        Some(bread.minimum_diagonal),
        Some(bread.intercept_formula_relative_error),
        Some(bread.intercept_normalization_relative_error),
    ];
    for (row, value) in common.into_iter().enumerate() {
        set_defined(&mut output, row, value, "OLS fit certificate")?;
    }
    if let Some(certificate) = inference.iid_certificate() {
        let selected = [
            certificate.reference_weight_sum_relative_error,
            certificate.covariance_scale_relative_infinity_norm,
            certificate.covariance_symmetry_relative_infinity_norm,
            0.0,
            certificate.minimum_covariance_diagonal,
            0.0,
            certificate.statistic_formula_relative_infinity_norm,
            f64::from(certificate.passed),
        ];
        set_selected_certificates(&mut output, &selected)?;
    } else if let Some(certificate) = inference.robust_certificate() {
        let selected = [
            certificate.reference_weight_sum_relative_error,
            certificate.sandwich_formula_relative_infinity_norm,
            certificate.covariance_symmetry_relative_infinity_norm,
            certificate.standard_error_relative_infinity_norm,
            certificate.minimum_normalized_psd_pivot,
            certificate.wald_solve_relative_infinity_norm,
            certificate.statistic_formula_relative_infinity_norm,
            f64::from(certificate.passed),
        ];
        set_selected_certificates(&mut output, &selected)?;
    } else if let Some(certificate) = inference.clustered_certificate() {
        let selected = [
            certificate.reference_weight_sum_relative_error,
            certificate.corrected_covariance_relative_infinity_norm,
            certificate.covariance_symmetry_relative_infinity_norm,
            certificate.standard_error_relative_infinity_norm,
            certificate.pre_restore_full_psd_relative_error,
            certificate.wald_solve_relative_infinity_norm,
            certificate.statistic_formula_relative_infinity_norm,
            f64::from(certificate.passed),
        ];
        set_selected_certificates(&mut output, &selected)?;
        let covariance = inference
            .clustered_covariance()
            .ok_or_else(|| invalid("clustered OLS covariance is absent"))?;
        let psd = covariance.psd();
        let extras = [
            psd_state_code(psd.state),
            f64::from(psd.full_adjusted),
            f64::from(psd.slope_block_restored),
            psd.minimum_final_full_eigenvalue,
            f64::from(psd.strict_final_full_psd),
            covariance.correction(),
        ];
        for (offset, entry) in extras.into_iter().enumerate() {
            set_defined(
                &mut output,
                24 + offset,
                Some(entry),
                "clustered OLS PSD diagnostic",
            )?;
        }
    } else {
        return Err(invalid("prepared OLS inference certificate is absent"));
    }
    let determinant = covariance_determinant(reported_covariance, reported_dimension)?;
    set_defined(
        &mut output,
        COVARIANCE_DETERMINANT_CERTIFICATE_ROW,
        Some(determinant),
        "reported OLS covariance determinant",
    )?;
    set_defined(
        &mut output,
        COVARIANCE_DETERMINANT_PASSED_ROW,
        Some(1.0),
        "reported OLS covariance determinant certificate",
    )?;
    Ok(output)
}

''',
    "categorical certificate serialization",
)
RESULT.write_text(result, encoding="utf-8")

estimate = ESTIMATE.read_text(encoding="utf-8")
estimate = replace_once(
    estimate,
    "use stfixest_core::ols::{\n"
    "    fit_prepared_ols_clustered_map, fit_prepared_ols_robust_map, fit_prepared_ols_unadjusted_map,\n"
    "    prepare_ols_clustered_data, prepare_ols_data, LoadedNumericColumn, LoadedOlsInput,\n",
    "use stfixest_core::ols::{\n"
    "    fit_prepared_categorical_ols_clustered_map, fit_prepared_categorical_ols_robust_map,\n"
    "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_clustered_map,\n"
    "    fit_prepared_ols_robust_map, fit_prepared_ols_unadjusted_map,\n"
    "    prepare_categorical_ols_clustered_data, prepare_ols_clustered_data, prepare_ols_data,\n"
    "    LoadedNumericColumn, LoadedOlsInput,\n",
    "categorical estimator imports",
)
estimate = replace_once(
    estimate,
    "use stfixest_core::protocol::{\n",
    "use stfixest_core::prepare_categorical_ols_data;\n"
    "use stfixest_core::protocol::{\n",
    "categorical preparation import",
)
estimate = regex_once(
    estimate,
    r"pub fn estimate_ols_test\(.*?\n}\n\n(?=impl EstimateActionSpec)",
    r'''pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    let specification = EstimateActionSpec::parse(args)?;
    let action = specification.decode_and_validate(spi)?;
    let loaded = load_ols_input(&action.request, spi)?;
    let categorical = action
        .request
        .absorbed_terms
        .iter()
        .any(|term| term.categorical_sources.len() > 1);
    match (action.vce, categorical) {
        (OlsVceKind::Iid, false) => {
            let prepared = prepare_ols_data(&action.request, &loaded)?;
            let inference = fit_prepared_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                PreparedOlsResultData::legacy(&prepared),
                None,
                CertifiedOlsInference::Iid(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        (OlsVceKind::Robust, false) => {
            let prepared = prepare_ols_data(&action.request, &loaded)?;
            let inference = fit_prepared_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                PreparedOlsResultData::legacy(&prepared),
                None,
                CertifiedOlsInference::Robust(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        (OlsVceKind::Clustered, false) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_ols_clustered_data(&request, &loaded)?;
            let inference = fit_prepared_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                PreparedOlsResultData::legacy(prepared.ols()),
                Some(PreparedOlsClusteredResultData::legacy(&prepared)),
                CertifiedOlsInference::Clustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        (OlsVceKind::Iid, true) => {
            let prepared = prepare_categorical_ols_data(&action.request, &loaded)?;
            let inference = fit_prepared_categorical_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                PreparedOlsResultData::categorical(&prepared),
                None,
                CertifiedOlsInference::CategoricalIid(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        (OlsVceKind::Robust, true) => {
            let prepared = prepare_categorical_ols_data(&action.request, &loaded)?;
            let inference = fit_prepared_categorical_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                PreparedOlsResultData::categorical(&prepared),
                None,
                CertifiedOlsInference::CategoricalRobust(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
        (OlsVceKind::Clustered, true) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_categorical_ols_clustered_data(&request, &loaded)?;
            let inference = fit_prepared_categorical_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            let shadow = build_shadow_result(
                PreparedOlsResultData::categorical(prepared.ols()),
                Some(PreparedOlsClusteredResultData::categorical(&prepared)),
                CertifiedOlsInference::CategoricalClustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, spi)
        }
    }
}

''',
    "categorical hidden estimator dispatch",
)
ESTIMATE.write_text(estimate, encoding="utf-8")

stata = STATA.read_text(encoding="utf-8")
stata = replace_once(
    stata,
    "            if `absorb_terms'[`row',2] != 1 | `absorb_terms'[`row',3] != 1 | ///\n"
    "               `absorb_terms'[`row',4] != 0 {\n"
    "                di as error \"hidden prepared OLS supports only single-source intercept fixed effects\"\n",
    "            if `absorb_terms'[`row',2] != 1 | `absorb_terms'[`row',3] < 1 | ///\n"
    "               `absorb_terms'[`row',4] != 0 {\n"
    "                di as error \"hidden prepared OLS supports only all-categorical intercept fixed effects\"\n",
    "categorical Stata absorb gate",
)
STATA.write_text(stata, encoding="utf-8")

FIXTURE.write_text(
    r'''version 18.0
clear all
set more off

set obs 72
generate long obs_id = _n
generate int firm = mod(_n - 1, 6) + 1
generate int year = mod(floor((_n - 1) / 6), 4) + 1
generate int repetition = floor((_n - 1) / 24)
generate double x = sin(_n / 5) + .31 * repetition + .07 * firm - .04 * year
generate double z = cos(_n / 7) - .22 * repetition + .03 * firm
generate double cell = .8 * firm - .45 * year + .11 * firm * year
generate double y = 1.6 * x - .7 * z + cell + .03 * sin(_n / 3)
generate int cluster_a = mod(firm + 2 * year + repetition, 9) + 1
generate int cluster_b = mod(2 * firm + year + 3 * repetition, 7) + 1

quietly regress y c.x c.z i.firm#i.year
local reference_x = _b[x]
local reference_z = _b[z]
local reference_vx = e(V)[1,1]
local reference_vz = e(V)[2,2]

_stfixest_estimate_ols_test y c.x c.z, absorb(firm#year) ///
    residual(__stfx_interaction_resid)
assert `"`e(cmd)'"' == "_stfixest_estimate_ols_test"
assert e(stfixest_receipt)[1,16] == 1
assert e(stfixest_receipt)[1,4] == 1
assert e(stfixest_receipt)[1,11] == 1
assert e(stfixest_fe)[1,1] == 3
assert e(stfixest_fe)[1,2] == 1
assert e(stfixest_fe)[1,3] == 0
assert e(stfixest_fe)[1,4] == 24
assert e(stfixest_fe)[1,6] == e(df_a)
assert e(sample) == 1
assert !missing(__stfx_interaction_resid)
assert reldif(_b[x], `reference_x') < 1e-8
assert reldif(_b[z], `reference_z') < 1e-8
assert reldif(e(V)[1,1], `reference_vx') < 1e-7
assert reldif(e(V)[2,2], `reference_vz') < 1e-7
assert e(stfixest_certificate)[32,2] == 1

_stfixest_estimate_ols_test y c.x c.z, absorb(firm#year) ///
    vce(robust) noconstant
assert e(stfixest_receipt)[1,4] == 2
assert e(stfixest_receipt)[1,5] == 2
assert e(stfixest_fe)[1,1] == 3
assert e(stfixest_certificate)[24,2] == 1
assert e(stfixest_certificate)[32,2] == 1

_stfixest_estimate_ols_test y c.x c.z, absorb(firm#year) ///
    cluster(cluster_a cluster_b)
assert e(stfixest_receipt)[1,4] == 3
assert e(stfixest_receipt)[1,10] == 2
assert e(stfixest_cluster)[1,3] > 1
assert e(stfixest_cluster)[2,3] > 1
assert e(stfixest_subset)[1,1] == 1
assert e(stfixest_subset)[2,1] == 2
assert e(stfixest_subset)[3,1] == 3
assert e(stfixest_certificate)[24,2] == 1
assert e(stfixest_certificate)[32,2] == 1
assert e(N_hdfe) == 1
assert `"`e(absvars)'"' == "firm#year"

di as result "END_FIXTURE|estimate_ols_categorical_interaction|rc=0"
''',
    encoding="utf-8",
)

profiles = json.loads(PROFILES.read_text(encoding="utf-8"))
fixture = "tests/stata/installation/estimate_ols_categorical_interaction.do"
for profile_name in ("quick", "full"):
    fixtures = profiles["profiles"][profile_name]["fixtures"]
    if fixture not in fixtures:
        anchor = "tests/stata/installation/estimate_ols_eclass_transaction.do"
        if anchor in fixtures:
            fixtures.insert(fixtures.index(anchor) + 1, fixture)
        else:
            fixtures.append(fixture)
PROFILES.write_text(json.dumps(profiles, indent=2) + "\n", encoding="utf-8")

CONTRACT.write_text(
    r'''from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "crates/stfixest-plugin/src"
STATA = ROOT / "stata/_stfixest_estimate_ols_test.ado"


class CategoricalHiddenTransportContractTests(unittest.TestCase):
    def test_hidden_dispatch_routes_all_three_vce_families(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        for symbol in (
            "prepare_categorical_ols_data",
            "prepare_categorical_ols_clustered_data",
            "fit_prepared_categorical_ols_unadjusted_map",
            "fit_prepared_categorical_ols_robust_map",
            "fit_prepared_categorical_ols_clustered_map",
            "CategoricalIid",
            "CategoricalRobust",
            "CategoricalClustered",
        ):
            self.assertIn(symbol, estimate)
        self.assertIn("categorical_sources.len() > 1", estimate)
        self.assertNotIn('ACTION_ESTIMATE_OLS: &str', estimate)

    def test_result_wire_retains_dimensions_and_interaction_identity(self):
        result = (PLUGIN / "result.rs").read_text(encoding="utf-8")
        view = (PLUGIN / "result_view.rs").read_text(encoding="utf-8")
        self.assertIn("const FE_COLUMNS: usize = 8", result)
        self.assertIn("const RECEIPT_COLUMNS: usize = 16", result)
        self.assertIn("_ => (3.0, f64::from(term_id.get()), 0.0)", result)
        self.assertIn("Categorical(&'a PreparedCategoricalOlsData)", view)
        self.assertIn(
            "Categorical(&'a PreparedCategoricalOlsClusteredData)", view
        )
        self.assertNotIn("unsafe {", result + view)

    def test_stata_gate_is_intercept_only_but_allows_categorical_tuples(self):
        stata = STATA.read_text(encoding="utf-8")
        self.assertIn("`absorb_terms'[`row',3] < 1", stata)
        self.assertIn(
            "hidden prepared OLS supports only all-categorical intercept fixed effects",
            stata,
        )
        self.assertNotIn("only single-source intercept fixed effects", stata)


if __name__ == "__main__":
    unittest.main()
''',
    encoding="utf-8",
)

print("activated categorical hidden OLS transport")
