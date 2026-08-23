#!/usr/bin/env python3
"""Remove private legacy iid wrappers made unreachable by shared HC1 composition."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "crates/stfixest-core/src/ols/prepared_unadjusted.rs"


def remove_once(text: str, block: str, label: str) -> str:
    count = text.count(block)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(block, "", 1)


text = PATH.read_text(encoding="utf-8")
text = remove_once(
    text,
    "pub(super) fn validate_fit_and_absorbed_dof(\n"
    "    prepared: &PreparedOlsData,\n"
    "    fit: &CertifiedPreparedOlsFit,\n"
    "    absorbed: &PreparedOlsAbsorbedDofReport,\n"
    ") -> Result<()> {\n"
    "    let summary = prepared.inference_absorbed_dof_summary(absorbed, absorbed.policy)?;\n"
    "    validate_fit_and_absorbed_dof_view(prepared, fit, summary)\n"
    "}\n\n",
    "obsolete fit/DoF wrapper",
)
text = remove_once(
    text,
    "pub(super) fn build_residual_dof(\n"
    "    prepared: &PreparedOlsData,\n"
    "    fit: &CertifiedPreparedOlsFit,\n"
    "    absorbed: &PreparedOlsAbsorbedDofReport,\n"
    "    weights: PreparedOlsReferenceWeightReport,\n"
    ") -> Result<PreparedOlsResidualDofReport> {\n"
    "    let summary = prepared.inference_absorbed_dof_summary(absorbed, absorbed.policy)?;\n"
    "    build_residual_dof_view(prepared, fit, summary, weights)\n"
    "}\n\n",
    "obsolete residual-DoF wrapper",
)
PATH.write_text(text, encoding="utf-8")
print("removed obsolete private iid wrappers")
