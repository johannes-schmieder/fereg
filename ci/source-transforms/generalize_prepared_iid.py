#!/usr/bin/env python3
"""Repair clustered DoF composition after the shared iid-view refactor.

The iid boundary remains nesting-free. Clustered inference receives a separate,
fully validated summary for the nested absorbed-DoF report that it constructs in
the same call.
"""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VIEW = ROOT / "crates/stfixest-core/src/ols/prepared_inference_view.rs"
UNADJUSTED = ROOT / "crates/stfixest-core/src/ols/prepared_unadjusted.rs"
CLUSTERED = ROOT / "crates/stfixest-core/src/ols/prepared_clustered.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


view = VIEW.read_text(encoding="utf-8")
view = replace_once(
    view,
    "    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffectIdentity,\n"
    "    PreparedOlsAbsorbedDofPolicy, PreparedOlsAbsorbedDofReport, PreparedOlsData,\n",
    "    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffect,\n"
    "    PreparedOlsAbsorbedDofEffectIdentity, PreparedOlsAbsorbedDofPolicy,\n"
    "    PreparedOlsAbsorbedDofReport, PreparedOlsData,\n",
    "legacy DoF effect import",
)
view = replace_once(
    view,
    "pub(crate) struct PreparedOlsAbsorbedDofSummary {\n"
    "    pub initial_categories: usize,\n"
    "    pub redundant_categories: usize,\n"
    "    pub absorbed_rank: usize,\n"
    "    pub nested_categories: usize,\n"
    "    pub is_approximate: bool,\n"
    "}\n\n"
    "pub(crate) trait PreparedOlsInferenceView: PreparedOlsFitView {\n",
    "pub(crate) struct PreparedOlsAbsorbedDofSummary {\n"
    "    pub initial_categories: usize,\n"
    "    pub redundant_categories: usize,\n"
    "    pub absorbed_rank: usize,\n"
    "    pub nested_categories: usize,\n"
    "    pub is_approximate: bool,\n"
    "}\n\n"
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n"
    "enum LegacyNestingState {\n"
    "    Iid,\n"
    "    ClusterAware,\n"
    "}\n\n"
    "pub(crate) trait PreparedOlsInferenceView: PreparedOlsFitView {\n",
    "legacy nesting state",
)
view = replace_once(
    view,
    "        validate_legacy_report(self, report, expected_policy)\n",
    "        validate_legacy_report(\n"
    "            self,\n"
    "            report,\n"
    "            expected_policy,\n"
    "            LegacyNestingState::Iid,\n"
    "        )\n",
    "iid legacy summary call",
)
view = replace_once(
    view,
    "fn validate_legacy_report(\n"
    "    prepared: &PreparedOlsData,\n"
    "    report: &PreparedOlsAbsorbedDofReport,\n"
    "    expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    ") -> Result<PreparedOlsAbsorbedDofSummary> {\n",
    "pub(super) fn validate_cluster_aware_legacy_absorbed_dof_summary(\n"
    "    prepared: &PreparedOlsData,\n"
    "    report: &PreparedOlsAbsorbedDofReport,\n"
    "    expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    ") -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "    validate_legacy_report(\n"
    "        prepared,\n"
    "        report,\n"
    "        expected_policy,\n"
    "        LegacyNestingState::ClusterAware,\n"
    "    )\n"
    "}\n\n"
    "fn validate_legacy_report(\n"
    "    prepared: &PreparedOlsData,\n"
    "    report: &PreparedOlsAbsorbedDofReport,\n"
    "    expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    nesting_state: LegacyNestingState,\n"
    ") -> Result<PreparedOlsAbsorbedDofSummary> {\n",
    "cluster-aware legacy summary entry point",
)

categorical_marker = "\nfn validate_categorical_report("
if view.count(categorical_marker) != 1:
    raise SystemExit("categorical validator marker is not unique")
legacy, categorical_tail = view.split(categorical_marker, 1)
legacy = replace_once(
    legacy,
    "    if report.effects.len() != expected_rows || report.nested_categories != 0 {\n",
    "    if report.effects.len() != expected_rows\n"
    "        || (nesting_state == LegacyNestingState::Iid && report.nested_categories != 0)\n"
    "    {\n",
    "legacy row-count and iid nesting gate",
)
legacy = replace_once(
    legacy,
    "    let mut rank = 0_usize;\n"
    "    let mut approximate = false;\n"
    "    let mut rows = report.effects.as_slice();\n",
    "    let mut rank = 0_usize;\n"
    "    let mut nested_categories = 0_usize;\n"
    "    let mut approximate = false;\n"
    "    let mut rows = report.effects.as_slice();\n",
    "legacy nested-category accumulator",
)
legacy = replace_once(
    legacy,
    "            || row.coefficients != row.categories - row.redundant\n"
    "            || row.nested\n"
    "        {\n",
    "            || row.coefficients != row.categories - row.redundant\n"
    "            || !legacy_nesting_row_is_valid(nesting_state, row)\n"
    "        {\n",
    "legacy explicit-row nesting validation",
)
legacy = replace_once(
    legacy,
    "        approximate |= !row.redundancy_exact;\n"
    "    }\n"
    "    validate_totals(\n",
    "        if row.nested {\n"
    "            nested_categories = nested_categories\n"
    "                .checked_add(row.categories)\n"
    "                .ok_or_else(|| allocation(\"prepared absorbed nested category total overflows\"))?;\n"
    "        }\n"
    "        approximate |= !row.redundancy_exact;\n"
    "    }\n"
    "    if nested_categories != report.nested_categories {\n"
    "        return Err(invalid(\n"
    "            \"prepared absorbed-DoF nested category total is inconsistent\",\n"
    "        ));\n"
    "    }\n"
    "    validate_totals(\n",
    "legacy nested-category certificate",
)
legacy += (
    "\nfn legacy_nesting_row_is_valid(\n"
    "    nesting_state: LegacyNestingState,\n"
    "    row: &PreparedOlsAbsorbedDofEffect,\n"
    ") -> bool {\n"
    "    match (nesting_state, row.nested) {\n"
    "        (LegacyNestingState::Iid, false)\n"
    "        | (LegacyNestingState::ClusterAware, false) => true,\n"
    "        (LegacyNestingState::Iid, true) => false,\n"
    "        (LegacyNestingState::ClusterAware, true) => {\n"
    "            row.redundant == row.categories\n"
    "                && row.coefficients == 0\n"
    "                && row.redundancy_exact\n"
    "        }\n"
    "    }\n"
    "}\n"
)
VIEW.write_text(legacy + categorical_marker + categorical_tail, encoding="utf-8")

unadjusted = UNADJUSTED.read_text(encoding="utf-8")
unadjusted = replace_once(
    unadjusted,
    "use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, PreparedOlsInferenceView};\n",
    "use super::prepared_inference_view::{\n"
    "    validate_cluster_aware_legacy_absorbed_dof_summary, PreparedOlsAbsorbedDofSummary,\n"
    "    PreparedOlsInferenceView,\n"
    "};\n",
    "cluster-aware summary import",
)
unadjusted = replace_once(
    unadjusted,
    "pub(super) fn build_residual_dof_view<P: PreparedOlsInferenceView + ?Sized>(\n",
    "pub(super) fn build_clustered_base_residual_dof(\n"
    "    prepared: &PreparedOlsData,\n"
    "    fit: &CertifiedPreparedOlsFit,\n"
    "    absorbed: &PreparedOlsAbsorbedDofReport,\n"
    "    weights: PreparedOlsReferenceWeightReport,\n"
    ") -> Result<PreparedOlsResidualDofReport> {\n"
    "    let summary = validate_cluster_aware_legacy_absorbed_dof_summary(\n"
    "        prepared,\n"
    "        absorbed,\n"
    "        absorbed.policy,\n"
    "    )?;\n"
    "    build_residual_dof_view(prepared, fit, summary, weights)\n"
    "}\n\n"
    "pub(super) fn build_residual_dof_view<P: PreparedOlsInferenceView + ?Sized>(\n",
    "clustered base residual DoF builder",
)
UNADJUSTED.write_text(unadjusted, encoding="utf-8")

clustered = CLUSTERED.read_text(encoding="utf-8")
clustered = replace_once(
    clustered,
    "    build_reference_weight_report, build_residual_dof, build_statistics, checked_square,\n",
    "    build_clustered_base_residual_dof, build_reference_weight_report, build_statistics,\n"
    "    checked_square,\n",
    "clustered residual DoF import",
)
clustered = replace_once(
    clustered,
    "    let base_dof = build_residual_dof(prepared.ols(), &fit, &absorbed_dof, weights)?;\n",
    "    let base_dof =\n"
    "        build_clustered_base_residual_dof(prepared.ols(), &fit, &absorbed_dof, weights)?;\n",
    "clustered residual DoF call",
)
CLUSTERED.write_text(clustered, encoding="utf-8")

print("separated iid-only and cluster-aware absorbed-DoF validation")
