#!/usr/bin/env python3
"""Route cluster-aware absorbed DoF through the shared inference view."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
INFERENCE = ROOT / "crates/stfixest-core/src/ols/prepared_inference_view.rs"
CLUSTERED = ROOT / "crates/stfixest-core/src/ols/prepared_clustered.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


text = INFERENCE.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use super::{\n"
    "    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffect,\n",
    "use super::prepared_dof::compute_prepared_ols_absorbed_dof_with_nesting;\n"
    "use super::{\n"
    "    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffect,\n",
    "cluster-aware DoF import",
)
text = replace_once(
    text,
    "enum LegacyNestingState {\n"
    "    Iid,\n"
    "    ClusterAware,\n"
    "}\n",
    "enum LegacyNestingState {\n"
    "    Iid,\n"
    "    ClusterAware,\n"
    "}\n\n"
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n"
    "enum CategoricalNestingState {\n"
    "    Iid,\n"
    "    ClusterAware,\n"
    "}\n",
    "categorical nesting state",
)
text = replace_once(
    text,
    "    fn inference_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary>;\n\n"
    "    fn inference_has_explicit_absorbed_effects(&self) -> bool;\n",
    "    fn inference_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary>;\n\n"
    "    fn inference_compute_clustered_absorbed_dof(\n"
    "        &self,\n"
    "        policy: PreparedOlsAbsorbedDofPolicy,\n"
    "        cluster_dimensions: &[&[u32]],\n"
    "    ) -> Result<Self::AbsorbedDofReport>;\n\n"
    "    fn inference_clustered_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary>;\n\n"
    "    fn inference_has_explicit_absorbed_effects(&self) -> bool;\n",
    "cluster-aware inference trait methods",
)
text = replace_once(
    text,
    "    fn inference_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "        validate_legacy_report(self, report, expected_policy, LegacyNestingState::Iid)\n"
    "    }\n\n"
    "    fn inference_has_explicit_absorbed_effects(&self) -> bool {\n",
    "    fn inference_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "        validate_legacy_report(self, report, expected_policy, LegacyNestingState::Iid)\n"
    "    }\n\n"
    "    fn inference_compute_clustered_absorbed_dof(\n"
    "        &self,\n"
    "        policy: PreparedOlsAbsorbedDofPolicy,\n"
    "        cluster_dimensions: &[&[u32]],\n"
    "    ) -> Result<Self::AbsorbedDofReport> {\n"
    "        compute_prepared_ols_absorbed_dof_with_nesting(self, policy, cluster_dimensions)\n"
    "    }\n\n"
    "    fn inference_clustered_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "        validate_legacy_report(\n"
    "            self,\n"
    "            report,\n"
    "            expected_policy,\n"
    "            LegacyNestingState::ClusterAware,\n"
    "        )\n"
    "    }\n\n"
    "    fn inference_has_explicit_absorbed_effects(&self) -> bool {\n",
    "legacy cluster-aware inference implementation",
)
text = replace_once(
    text,
    "    fn inference_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "        validate_categorical_report(self, report, expected_policy)\n"
    "    }\n\n"
    "    fn inference_has_explicit_absorbed_effects(&self) -> bool {\n",
    "    fn inference_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "        validate_categorical_report(\n"
    "            self,\n"
    "            report,\n"
    "            expected_policy,\n"
    "            CategoricalNestingState::Iid,\n"
    "        )\n"
    "    }\n\n"
    "    fn inference_compute_clustered_absorbed_dof(\n"
    "        &self,\n"
    "        policy: PreparedOlsAbsorbedDofPolicy,\n"
    "        cluster_dimensions: &[&[u32]],\n"
    "    ) -> Result<Self::AbsorbedDofReport> {\n"
    "        self.absorbed_dof(categorical_policy(policy), cluster_dimensions)\n"
    "    }\n\n"
    "    fn inference_clustered_absorbed_dof_summary(\n"
    "        &self,\n"
    "        report: &Self::AbsorbedDofReport,\n"
    "        expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    ) -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "        validate_categorical_report(\n"
    "            self,\n"
    "            report,\n"
    "            expected_policy,\n"
    "            CategoricalNestingState::ClusterAware,\n"
    "        )\n"
    "    }\n\n"
    "    fn inference_has_explicit_absorbed_effects(&self) -> bool {\n",
    "categorical cluster-aware inference implementation",
)
text = replace_once(
    text,
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
    "}\n",
    "pub(super) fn validate_cluster_aware_legacy_absorbed_dof_summary(\n"
    "    prepared: &PreparedOlsData,\n"
    "    report: &PreparedOlsAbsorbedDofReport,\n"
    "    expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    ") -> Result<PreparedOlsAbsorbedDofSummary> {\n"
    "    prepared.inference_clustered_absorbed_dof_summary(report, expected_policy)\n"
    "}\n",
    "legacy cluster-aware summary routing",
)

start = text.index("fn validate_categorical_report(")
end = text.index("\n#[allow(clippy::too_many_arguments)]", start)
block = text[start:end]
block = replace_once(
    block,
    "    expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    ") -> Result<PreparedOlsAbsorbedDofSummary> {\n",
    "    expected_policy: PreparedOlsAbsorbedDofPolicy,\n"
    "    nesting_state: CategoricalNestingState,\n"
    ") -> Result<PreparedOlsAbsorbedDofSummary> {\n",
    "categorical validator nesting argument",
)
block = replace_once(
    block,
    "        || report.nested_categories != 0\n",
    "        || (nesting_state == CategoricalNestingState::Iid\n"
    "            && report.nested_categories != 0)\n",
    "categorical iid nesting condition",
)
block = replace_once(
    block,
    "    let mut rank = 0_usize;\n"
    "    let mut approximate = false;\n",
    "    let mut rank = 0_usize;\n"
    "    let mut nested_categories = 0_usize;\n"
    "    let mut approximate = false;\n",
    "categorical nested total accumulator",
)
block = replace_once(
    block,
    "            || row.coefficients != row.categories - row.redundant\n"
    "            || row.nested\n",
    "            || row.coefficients != row.categories - row.redundant\n"
    "            || !categorical_nesting_row_is_valid(nesting_state, row)\n",
    "categorical nested row validation",
)
block = replace_once(
    block,
    "        rank = rank\n"
    "            .checked_add(row.coefficients)\n"
    "            .ok_or_else(|| allocation(\"categorical absorbed rank total overflows\"))?;\n"
    "        approximate |= !row.redundancy_exact;\n"
    "    }\n"
    "    validate_totals(\n",
    "        rank = rank\n"
    "            .checked_add(row.coefficients)\n"
    "            .ok_or_else(|| allocation(\"categorical absorbed rank total overflows\"))?;\n"
    "        if row.nested {\n"
    "            nested_categories = nested_categories.checked_add(row.categories).ok_or_else(|| {\n"
    "                allocation(\"categorical absorbed nested category total overflows\")\n"
    "            })?;\n"
    "        }\n"
    "        approximate |= !row.redundancy_exact;\n"
    "    }\n"
    "    if nested_categories != report.nested_categories {\n"
    "        return Err(invalid(\n"
    "            \"categorical absorbed-DoF nested category total is inconsistent\",\n"
    "        ));\n"
    "    }\n"
    "    validate_totals(\n",
    "categorical nested total certificate",
)
text = text[:start] + block + text[end:]
text = replace_once(
    text,
    "}\n\n#[allow(clippy::too_many_arguments)]\n"
    "fn validate_totals(\n",
    "}\n\n"
    "fn categorical_nesting_row_is_valid(\n"
    "    nesting_state: CategoricalNestingState,\n"
    "    row: &crate::topology_dof::CategoricalTopologyDofEffect,\n"
    ") -> bool {\n"
    "    match (nesting_state, row.nested) {\n"
    "        (CategoricalNestingState::Iid, false)\n"
    "        | (CategoricalNestingState::ClusterAware, false) => true,\n"
    "        (CategoricalNestingState::Iid, true) => false,\n"
    "        (CategoricalNestingState::ClusterAware, true) => {\n"
    "            row.redundant == row.categories && row.coefficients == 0 && row.redundancy_exact\n"
    "        }\n"
    "    }\n"
    "}\n\n"
    "#[allow(clippy::too_many_arguments)]\n"
    "fn validate_totals(\n",
    "categorical nested row helper",
)
INFERENCE.write_text(text, encoding="utf-8")

clustered = CLUSTERED.read_text(encoding="utf-8")
clustered = replace_once(
    clustered,
    "use super::prepared_clustered_view::PreparedOlsClusteredView;\n"
    "use super::prepared_dof::compute_prepared_ols_absorbed_dof_with_nesting;\n",
    "use super::prepared_clustered_view::PreparedOlsClusteredView;\n"
    "use super::prepared_inference_view::PreparedOlsInferenceView;\n",
    "clustered inference view import",
)
clustered = replace_once(
    clustered,
    "    let absorbed_dof = compute_prepared_ols_absorbed_dof_with_nesting(\n"
    "        prepared.ols(),\n"
    "        config.absorbed_dof,\n"
    "        nesting_dimensions,\n"
    "    )?;\n",
    "    let absorbed_dof = prepared.ols().inference_compute_clustered_absorbed_dof(\n"
    "        config.absorbed_dof,\n"
    "        nesting_dimensions,\n"
    "    )?;\n",
    "clustered absorbed DoF view call",
)
CLUSTERED.write_text(clustered, encoding="utf-8")

print("activated cluster-aware absorbed-DoF inference view")
