#!/usr/bin/env python3
"""Route the legacy hidden OLS serializer through borrowed result views."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESULT = ROOT / "crates/stfixest-plugin/src/result.rs"
ESTIMATE = ROOT / "crates/stfixest-plugin/src/estimate.rs"
LIB = ROOT / "crates/stfixest-plugin/src/lib.rs"


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


result = RESULT.read_text(encoding="utf-8")
result = replace_once(
    result,
    "use crate::input::{PreparedMatrixStoreName, SpiApi};\n",
    "use crate::input::{PreparedMatrixStoreName, SpiApi};\n"
    "use crate::result_view::{PreparedOlsClusteredResultData, PreparedOlsResultData};\n",
    "result-view import",
)
result = replace_once(
    result,
    "    PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate, PreparedOlsClusteredData,\n"
    "    PreparedOlsClusteredPsdState, PreparedOlsData, PreparedOlsFitStatistics, PreparedOlsFitStatus,\n",
    "    PreparedOlsAbsorbedDofReport, PreparedOlsBreadCoordinate,\n"
    "    PreparedOlsClusteredPsdState, PreparedOlsFitStatistics, PreparedOlsFitStatus,\n",
    "legacy prepared-type imports",
)
result = replace_once(
    result,
    "pub(crate) fn build_shadow_result<'a>(\n"
    "    prepared: &'a PreparedOlsData,\n"
    "    clustered_prepared: Option<&'a PreparedOlsClusteredData>,\n",
    "pub(crate) fn build_shadow_result<'a>(\n"
    "    prepared: PreparedOlsResultData<'a>,\n"
    "    clustered_prepared: Option<PreparedOlsClusteredResultData<'a>>,\n",
    "shadow-result prepared view signature",
)
result = replace_once(
    result,
    "    let coefficient_count = prepared.model.coefficients.len();\n",
    "    let coefficient_count = prepared.model().coefficients.len();\n",
    "shadow-result coefficient count",
)
result = result.replace("prepared.sample.", "prepared.sample().")
result = result.replace("prepared.singletons.", "prepared.singletons().")
result = regex_once(
    result,
    r"fn validate_alignment\(.*?\n}\n\n(?=fn build_fit_statistics)",
    '''fn validate_alignment(
    prepared: PreparedOlsResultData<'_>,
    clustered: Option<PreparedOlsClusteredResultData<'_>>,
    inference: &CertifiedOlsInference<'_>,
) -> Result<()> {
    match (inference, clustered) {
        (CertifiedOlsInference::Clustered(_), Some(value)) if value.is_bound_to(prepared) => {}
        (CertifiedOlsInference::Clustered(_), _) => {
            return Err(invalid(
                "clustered inference is not sample-bound to its preparation",
            ));
        }
        (CertifiedOlsInference::Iid(_) | CertifiedOlsInference::Robust(_), None) => {}
        _ => {
            return Err(invalid(
                "nonclustered inference unexpectedly has cluster preparation",
            ))
        }
    }
    if !inference.fit().rank_certificate().passed
        || !inference.fit().score_certificate().passed
        || !inference.fit().bread().certificate().passed
    {
        return Err(invalid("prepared OLS fit certificate is not complete"));
    }
    let vce_passed = match inference {
        CertifiedOlsInference::Iid(value) => value.certificate().passed,
        CertifiedOlsInference::Robust(value) => value.certificate().passed,
        CertifiedOlsInference::Clustered(value) => value.certificate().passed,
    };
    if !vce_passed {
        return Err(invalid(
            "prepared OLS inference certificate is not complete",
        ));
    }
    Ok(())
}

''',
    "alignment view boundary",
)
result = replace_once(
    result,
    "fn build_dof(\n"
    "    prepared: &PreparedOlsData,\n",
    "fn build_dof(\n"
    "    prepared: PreparedOlsResultData<'_>,\n",
    "DoF prepared view signature",
)
result = replace_once(
    result,
    "fn build_clusters(\n"
    "    prepared: Option<&PreparedOlsClusteredData>,\n",
    "fn build_clusters(\n"
    "    prepared: Option<PreparedOlsClusteredResultData<'_>>,\n",
    "cluster metadata view signature",
)
result = replace_once(
    result,
    "fn build_receipt(\n"
    "    prepared: &PreparedOlsData,\n",
    "fn build_receipt(\n"
    "    prepared: PreparedOlsResultData<'_>,\n",
    "receipt prepared view signature",
)
RESULT.write_text(result, encoding="utf-8")

estimate = ESTIMATE.read_text(encoding="utf-8")
estimate = replace_once(
    estimate,
    "use crate::result::{\n"
    "    build_shadow_result, CertifiedOlsInference, OlsShadowResult, OlsVceKind, CERTIFICATE_ROWS,\n"
    "    CLUSTER_COLUMNS, COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, FE_COLUMNS, FIT_ROWS, RECEIPT_COLUMNS,\n"
    "    RECEIPT_COMPLETE_COLUMN, RESULT_MAGIC, SOLVER_COLUMNS, SOLVER_ROWS, SUBSET_COLUMNS,\n"
    "};\n",
    "use crate::result::{\n"
    "    build_shadow_result, CertifiedOlsInference, OlsShadowResult, OlsVceKind, CERTIFICATE_ROWS,\n"
    "    CLUSTER_COLUMNS, COEFFICIENT_STATUS_COLUMNS, DOF_ROWS, FE_COLUMNS, FIT_ROWS, RECEIPT_COLUMNS,\n"
    "    RECEIPT_COMPLETE_COLUMN, RESULT_MAGIC, SOLVER_COLUMNS, SOLVER_ROWS, SUBSET_COLUMNS,\n"
    "};\n"
    "use crate::result_view::{PreparedOlsClusteredResultData, PreparedOlsResultData};\n",
    "estimate result-view import",
)
legacy_nonclustered = (
    "            let shadow = build_shadow_result(\n"
    "                &prepared,\n"
    "                None,\n"
)
if estimate.count(legacy_nonclustered) != 2:
    raise SystemExit(
        f"legacy nonclustered shadow calls: expected two occurrences, found {estimate.count(legacy_nonclustered)}"
    )
estimate = estimate.replace(
    legacy_nonclustered,
    "            let shadow = build_shadow_result(\n"
    "                PreparedOlsResultData::legacy(&prepared),\n"
    "                None,\n",
)
estimate = replace_once(
    estimate,
    "            let shadow = build_shadow_result(\n"
    "                prepared.ols(),\n"
    "                Some(&prepared),\n",
    "            let shadow = build_shadow_result(\n"
    "                PreparedOlsResultData::legacy(prepared.ols()),\n"
    "                Some(PreparedOlsClusteredResultData::legacy(&prepared)),\n",
    "legacy clustered shadow call",
)
ESTIMATE.write_text(estimate, encoding="utf-8")

lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "mod result;\n",
    "mod result;\nmod result_view;\n",
    "plugin result-view module declaration",
)
LIB.write_text(lib, encoding="utf-8")

print("activated legacy hidden OLS result views")
