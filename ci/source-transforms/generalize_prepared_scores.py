#!/usr/bin/env python3
"""Generalize the ephemeral robust score design over PreparedOlsFitView."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCORES = ROOT / "crates/stfixest-core/src/ols/prepared_scores.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


text = SCORES.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use crate::fe::{\n"
    "    absorb_borrowed_intercept_map_with_structural_constant_certified, BorrowedMapWeights, MapConfig,\n"
    "};\n",
    "use crate::fe::{BorrowedMapWeights, MapConfig};\n"
    "use crate::prepared_fit_view::PreparedOlsFitView;\n",
    "score imports",
)
text = replace_once(
    text,
    "use super::{CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind, PreparedOlsData};\n",
    "use super::{CertifiedPreparedOlsFit, PreparedOlsBreadCoordinateKind};\n",
    "score super imports",
)
text = replace_once(
    text,
    "pub(super) fn build_prepared_ols_score_design<R: PreparedInferenceReservation>(\n"
    "    prepared: &PreparedOlsData,\n",
    "pub(super) fn build_prepared_ols_score_design<\n"
    "    P: PreparedOlsFitView + ?Sized,\n"
    "    R: PreparedInferenceReservation,\n"
    ">(\n"
    "    prepared: &P,\n",
    "score builder signature",
)
text = replace_once(
    text,
    "    let nrows = prepared.sample.n_active();\n",
    "    let nrows = prepared.fit_sample().n_active();\n",
    "score sample access",
)
text = replace_once(
    text,
    "            let value = prepared.column_value(generated_columns[column], row)?;\n",
    "            let value = prepared.fit_column_value(generated_columns[column], row)?;\n",
    "score generated-column access",
)
text = replace_once(
    text,
    "        let design = prepared.borrowed_intercept_fe_design()?;\n"
    "        let map_weights = match &prepared.sample.weights {\n"
    "            PreparedWeights::None => BorrowedMapWeights::Unit,\n"
    "            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),\n"
    "        };\n"
    "        let absorbed = absorb_borrowed_intercept_map_with_structural_constant_certified(\n"
    "            &design,\n"
    "            map_weights,\n"
    "            raw,\n"
    "            map,\n"
    "        )?;\n",
    "        let map_weights = match &prepared.fit_sample().weights {\n"
    "            PreparedWeights::None => BorrowedMapWeights::Unit,\n"
    "            PreparedWeights::Provided { values, .. } => BorrowedMapWeights::Provided(values),\n"
    "        };\n"
    "        let absorbed = prepared.fit_absorb_matrix(map_weights, raw, map)?;\n",
    "score absorption boundary",
)
for forbidden in [
    "prepared.sample",
    "prepared.column_value(",
    "prepared.borrowed_intercept_fe_design()",
    "absorb_borrowed_intercept_map_with_structural_constant_certified",
    "PreparedOlsData",
]:
    if forbidden in text:
        raise SystemExit(f"prepared_scores.rs retains legacy-only access: {forbidden}")
SCORES.write_text(text, encoding="utf-8")
print("generalized prepared robust score design over PreparedOlsFitView")
