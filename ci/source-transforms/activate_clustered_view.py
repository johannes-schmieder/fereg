#!/usr/bin/env python3
"""Activate the shared cluster-code view without changing numerical behavior."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"
CLUSTERED = ROOT / "crates/stfixest-core/src/ols/prepared_clustered.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


ols_mod = OLS_MOD.read_text(encoding="utf-8")
ols_mod = replace_once(
    ols_mod,
    "mod prepared_clustered;\nmod prepared_clusters;\n",
    "mod prepared_clustered;\nmod prepared_clustered_view;\nmod prepared_clusters;\n",
    "clustered view module declaration",
)
OLS_MOD.write_text(ols_mod, encoding="utf-8")

clustered = CLUSTERED.read_text(encoding="utf-8")
clustered = replace_once(
    clustered,
    "use super::prepared_dof::compute_prepared_ols_absorbed_dof_with_nesting;\n",
    "use super::prepared_clustered_view::PreparedOlsClusteredView;\n"
    "use super::prepared_dof::compute_prepared_ols_absorbed_dof_with_nesting;\n",
    "clustered view import",
)
clustered = replace_once(
    clustered,
    "fn validate_config(\n"
    "    prepared: &PreparedOlsClusteredData,\n",
    "fn validate_config<P: PreparedOlsClusteredView + ?Sized>(\n"
    "    prepared: &P,\n",
    "clustered config generic boundary",
)
clustered = replace_once(
    clustered,
    "if !(1..=10).contains(&prepared.clusters().len()) {\n",
    "if !(1..=10).contains(&prepared.clustered_design().len()) {\n",
    "clustered config dimension view",
)
clustered = replace_once(
    clustered,
    "fn borrow_cluster_dimensions<'a, R: PreparedInferenceReservation>(\n"
    "    prepared: &'a PreparedOlsClusteredData,\n"
    "    reservation: &mut R,\n"
    ") -> Result<Vec<&'a [u32]>> {\n"
    "    let mut dimensions = reserved_empty(\n"
    "        reservation,\n"
    "        prepared.clusters().len(),\n"
    "        \"unable to reserve prepared clustered code views\",\n"
    "    )?;\n"
    "    for index in 0..prepared.clusters().len() {\n"
    "        push_reserved(\n"
    "            &mut dimensions,\n"
    "            prepared.cluster_codes(index)?,\n",
    "fn borrow_cluster_dimensions<\n"
    "    'a,\n"
    "    P: PreparedOlsClusteredView + ?Sized,\n"
    "    R: PreparedInferenceReservation,\n"
    ">(\n"
    "    prepared: &'a P,\n"
    "    reservation: &mut R,\n"
    ") -> Result<Vec<&'a [u32]>> {\n"
    "    let mut dimensions = reserved_empty(\n"
    "        reservation,\n"
    "        prepared.clustered_design().len(),\n"
    "        \"unable to reserve prepared clustered code views\",\n"
    "    )?;\n"
    "    for index in 0..prepared.clustered_design().len() {\n"
    "        push_reserved(\n"
    "            &mut dimensions,\n"
    "            prepared.clustered_codes(index)?,\n",
    "cluster-code borrowing view",
)
CLUSTERED.write_text(clustered, encoding="utf-8")

print("activated shared clustered-data view")
