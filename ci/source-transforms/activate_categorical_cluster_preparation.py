#!/usr/bin/env python3
"""Activate categorical clustered-data preparation without changing legacy behavior."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATEGORICAL = ROOT / "crates/stfixest-core/src/ols/prepared_categorical_data.rs"
CLUSTERS = ROOT / "crates/stfixest-core/src/ols/prepared_clusters.rs"
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"
LIB = ROOT / "crates/stfixest-core/src/lib.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


categorical = CATEGORICAL.read_text(encoding="utf-8")
categorical = replace_once(
    categorical,
    "    prepare_ols_data, LoadedOlsInput, OlsRequestV1, PreparedCategoricalSource,\n",
    "    prepare_ols_data_with_cluster_sources, LoadedOlsInput, OlsRequestV1,\n"
    "    PreparedCategoricalSource,\n",
    "categorical preparation import",
)
categorical = replace_once(
    categorical,
    "pub fn prepare_categorical_ols_data(\n"
    "    request: &OlsRequestV1,\n"
    "    loaded: &LoadedOlsInput,\n"
    ") -> Result<PreparedCategoricalOlsData> {\n"
    "    let plan = plan_categorical_absorbed_request(request)?;\n"
    "    let availability = prepare_ols_data(plan.availability_request(), loaded)?;\n",
    "pub fn prepare_categorical_ols_data(\n"
    "    request: &OlsRequestV1,\n"
    "    loaded: &LoadedOlsInput,\n"
    ") -> Result<PreparedCategoricalOlsData> {\n"
    "    prepare_categorical_ols_data_with_cluster_sources(request, loaded, &[])\n"
    "}\n\n"
    "pub(crate) fn prepare_categorical_ols_data_with_cluster_sources(\n"
    "    request: &OlsRequestV1,\n"
    "    loaded: &LoadedOlsInput,\n"
    "    cluster_sources: &[SourceId],\n"
    ") -> Result<PreparedCategoricalOlsData> {\n"
    "    let plan = plan_categorical_absorbed_request(request)?;\n"
    "    let availability = prepare_ols_data_with_cluster_sources(\n"
    "        plan.availability_request(),\n"
    "        loaded,\n"
    "        cluster_sources,\n"
    "    )?;\n",
    "cluster-aware categorical preparation boundary",
)
CATEGORICAL.write_text(categorical, encoding="utf-8")

clusters = CLUSTERS.read_text(encoding="utf-8")
clusters = replace_once(
    clusters,
    "enum PreparedOlsClusterCodes {\n",
    "pub(super) enum PreparedOlsClusterCodes {\n",
    "cluster code visibility",
)
clusters = replace_once(
    clusters,
    "pub struct PreparedOlsClusterDimension {\n"
    "    label: String,\n"
    "    sources: Vec<SourceId>,\n"
    "    codes: PreparedOlsClusterCodes,\n"
    "    groups: usize,\n"
    "}\n",
    "pub struct PreparedOlsClusterDimension {\n"
    "    pub(super) label: String,\n"
    "    pub(super) sources: Vec<SourceId>,\n"
    "    pub(super) codes: PreparedOlsClusterCodes,\n"
    "    pub(super) groups: usize,\n"
    "}\n",
    "cluster dimension field visibility",
)
clusters = replace_once(
    clusters,
    "pub struct PreparedOlsClusterDesign {\n"
    "    dimensions: Vec<PreparedOlsClusterDimension>,\n"
    "}\n",
    "pub struct PreparedOlsClusterDesign {\n"
    "    pub(super) dimensions: Vec<PreparedOlsClusterDimension>,\n"
    "}\n",
    "cluster design field visibility",
)
clusters = replace_once(
    clusters,
    "fn validate_request(request: &OlsClusteredRequestV1) -> Result<Vec<SourceId>> {\n",
    "pub(super) fn validate_request(\n"
    "    request: &OlsClusteredRequestV1,\n"
    ") -> Result<Vec<SourceId>> {\n",
    "cluster request validator visibility",
)
CLUSTERS.write_text(clusters, encoding="utf-8")

ols_mod = OLS_MOD.read_text(encoding="utf-8")
ols_mod = replace_once(
    ols_mod,
    "mod prepared_clustered;\nmod prepared_clusters;\n",
    "mod prepared_categorical_clusters;\nmod prepared_clustered;\nmod prepared_clusters;\n",
    "categorical cluster module declaration",
)
ols_mod = replace_once(
    ols_mod,
    "pub use prepared_clustered::{\n",
    "pub use prepared_categorical_clusters::{\n"
    "    prepare_categorical_ols_clustered_data, PreparedCategoricalOlsClusteredData,\n"
    "};\n"
    "pub use prepared_clustered::{\n",
    "categorical cluster exports",
)
OLS_MOD.write_text(ols_mod, encoding="utf-8")

lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "    fit_prepared_categorical_ols_coefficients_map, fit_prepared_categorical_ols_robust_map,\n",
    "    fit_prepared_categorical_ols_coefficients_map, fit_prepared_categorical_ols_robust_map,\n"
    "    prepare_categorical_ols_clustered_data,\n",
    "crate categorical cluster function export",
)
lib = replace_once(
    lib,
    "    PreparedOlsClusteredPsdState, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,\n",
    "    PreparedCategoricalOlsClusteredData, PreparedOlsClusteredPsdState,\n"
    "    PreparedOlsCoefficient, PreparedOlsCoefficientStatus,\n",
    "crate categorical clustered-data type export",
)
LIB.write_text(lib, encoding="utf-8")

print("activated categorical clustered-data preparation")
