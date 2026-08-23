#!/usr/bin/env python3
"""One-time exact refactor of the certified prepared OLS fit over PreparedOlsFitView."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIT = ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs"
VIEW = ROOT / "crates/stfixest-core/src/ols/prepared_fit_view.rs"
OLS_MOD = ROOT / "crates/stfixest-core/src/ols/mod.rs"
LIB = ROOT / "crates/stfixest-core/src/lib.rs"


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


text = FIT.read_text(encoding="utf-8")
marker = "\n#[cfg(test)]\nmod tests {"
if text.count(marker) != 1:
    raise SystemExit("prepared_fit.rs test-module marker is not unique")
production, tests = text.split(marker, 1)

production = replace_once(
    production,
    "use crate::fe::{\n"
    "    absorb_borrowed_intercept_map_with_structural_constant_certified, BorrowedMapWeights,\n"
    "    MapConfig, SolveReport,\n"
    "};\n",
    "use crate::fe::{BorrowedMapWeights, MapConfig, SolveReport};\n"
    "use crate::prepared_categorical_data::PreparedCategoricalOlsData;\n"
    "use crate::prepared_fit_view::PreparedOlsFitView;\n",
    "fit imports",
)
production = replace_once(
    production,
    "use super::{\n"
    "    build_prepared_ols_rhs, PreparedOlsCoefficientStatus, PreparedOlsColumnKind, PreparedOlsData,\n"
    "};\n",
    "use super::{PreparedOlsCoefficientStatus, PreparedOlsColumnKind, PreparedOlsData};\n",
    "fit super imports",
)

old_public = """pub fn fit_prepared_ols_coefficients_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    let mut reservation = SystemFitReservation;
    fit_with_controls_internal(
        prepared,
        config,
        &mut reservation,
        FitPerturbations::default(),
    )
}
"""
new_public = """pub fn fit_prepared_ols_coefficients_map(
    prepared: &PreparedOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map(prepared, config)
}

/// Fit the interaction-aware categorical prepared artifact through the exact
/// same certified MAP, scaled pivoted QR, bread, residual, and score path used
/// by the legacy prepared boundary.
pub fn fit_prepared_categorical_ols_coefficients_map(
    prepared: &PreparedCategoricalOlsData,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    fit_prepared_ols_coefficients_view_map(prepared, config)
}

fn fit_prepared_ols_coefficients_view_map<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    config: PreparedOlsFitConfig,
) -> Result<CertifiedPreparedOlsFit> {
    let mut reservation = SystemFitReservation;
    fit_with_controls_internal(
        prepared,
        config,
        &mut reservation,
        FitPerturbations::default(),
    )
}
"""
production = replace_once(production, old_public, new_public, "public fit wrappers")

signature_replacements = [
    (
        r"fn fit_with_controls_internal<R: FitReservation>\(\n    prepared: &PreparedOlsData,",
        "fn fit_with_controls_internal<P: PreparedOlsFitView + ?Sized, R: FitReservation>(\n    prepared: &P,",
        "fit_with_controls_internal signature",
    ),
    (
        r"fn validate_config\(prepared: &PreparedOlsData, config: PreparedOlsFitConfig\)",
        "fn validate_config<P: PreparedOlsFitView + ?Sized>(prepared: &P, config: PreparedOlsFitConfig)",
        "validate_config signature",
    ),
    (
        r"fn normalized_weights\(prepared: &PreparedOlsData\)",
        "fn normalized_weights<P: PreparedOlsFitView + ?Sized>(prepared: &P)",
        "normalized_weights signature",
    ),
    (
        r"fn fit_scaled_pivoted_qr<R: FitReservation>\(\n    prepared: &PreparedOlsData,",
        "fn fit_scaled_pivoted_qr<P: PreparedOlsFitView + ?Sized, R: FitReservation>(\n    prepared: &P,",
        "fit_scaled_pivoted_qr signature",
    ),
    (
        r"fn build_finite_bread_draft<R: FitReservation>\(\n    prepared: &PreparedOlsData,",
        "fn build_finite_bread_draft<P: PreparedOlsFitView + ?Sized, R: FitReservation>(\n    prepared: &P,",
        "build_finite_bread_draft signature",
    ),
    (
        r"fn prepared_bread_weight_scale\(\n    prepared: &PreparedOlsData,",
        "fn prepared_bread_weight_scale<P: PreparedOlsFitView + ?Sized>(\n    prepared: &P,",
        "prepared_bread_weight_scale signature",
    ),
    (
        r"fn finalize_reported_bread_and_intercept<R: FitReservation>\(\n    prepared: &PreparedOlsData,",
        "fn finalize_reported_bread_and_intercept<\n    P: PreparedOlsFitView + ?Sized,\n    R: FitReservation,\n>(\n    prepared: &P,",
        "finalize_reported_bread signature",
    ),
    (
        r"fn certify_scores\(\n    prepared: &PreparedOlsData,",
        "fn certify_scores<P: PreparedOlsFitView + ?Sized>(\n    prepared: &P,",
        "certify_scores signature",
    ),
    (
        r"fn build_coefficient_slots<R: FitReservation>\(\n    prepared: &PreparedOlsData,",
        "fn build_coefficient_slots<P: PreparedOlsFitView + ?Sized, R: FitReservation>(\n    prepared: &P,",
        "build_coefficient_slots signature",
    ),
]
for pattern, replacement, label in signature_replacements:
    production = regex_once(production, pattern, replacement, label)

production = replace_once(
    production,
    "    let design = prepared.borrowed_intercept_fe_design()?;\n    let weights = normalized_weights(prepared)?;\n\n    let raw = build_prepared_ols_rhs(prepared)?.into_values();\n",
    "    let weights = normalized_weights(prepared)?;\n\n    let raw = prepared.fit_build_rhs()?;\n",
    "shared fit input setup",
)
production = replace_once(
    production,
    "    let absorbed = absorb_borrowed_intercept_map_with_structural_constant_certified(\n"
    "        &design,\n"
    "        weights.borrowed(),\n"
    "        raw,\n"
    "        config.map,\n"
    "    )?;\n",
    "    let absorbed = prepared.fit_absorb_matrix(weights.borrowed(), raw, config.map)?;\n",
    "initial shared absorption",
)
production = replace_once(
    production,
    "    let residual_absorbed = absorb_borrowed_intercept_map_with_structural_constant_certified(\n"
    "        &design,\n"
    "        weights.borrowed(),\n"
    "        residual_matrix,\n"
    "        config.map,\n"
    "    )?;\n",
    "    let residual_absorbed =\n"
    "        prepared.fit_absorb_matrix(weights.borrowed(), residual_matrix, config.map)?;\n",
    "residual shared absorption",
)

field_replacements = [
    ("prepared.sample.n_active()", "prepared.fit_sample().n_active()"),
    ("prepared.sample.outcome", "prepared.fit_sample().outcome"),
    ("prepared.sample.weights", "prepared.fit_sample().weights"),
    ("prepared.model.columns", "prepared.fit_model().columns"),
    ("prepared.model.coefficients", "prepared.fit_model().coefficients"),
    ("prepared.structural_intercept", "prepared.fit_structural_intercept()"),
    ("prepared.report_intercept", "prepared.fit_report_intercept()"),
    ("prepared.column_value(", "prepared.fit_column_value("),
]
for old, new in field_replacements:
    production = production.replace(old, new)

for forbidden in [
    "prepared.sample",
    "prepared.model",
    "prepared.structural_intercept",
    "prepared.report_intercept",
    "prepared.column_value(",
    "build_prepared_ols_rhs(prepared)",
    "prepared.borrowed_intercept_fe_design()",
    "absorb_borrowed_intercept_map_with_structural_constant_certified",
]:
    if forbidden in production:
        raise SystemExit(f"production prepared_fit.rs still contains legacy-only access: {forbidden}")

FIT.write_text(production + marker + tests, encoding="utf-8")

view = VIEW.read_text(encoding="utf-8")
view = replace_once(
    view,
    "/// partially transformed matrix on failure. The temporary dead-code allowance\n"
    "/// is removed when `prepared_fit` is generalized over this view.\n"
    "#[allow(dead_code)]\n",
    "/// partially transformed matrix on failure.\n",
    "fit-view activation",
)
VIEW.write_text(view, encoding="utf-8")

ols_mod = OLS_MOD.read_text(encoding="utf-8")
ols_mod = replace_once(
    ols_mod,
    "    fit_prepared_ols_coefficients_map, CertifiedPreparedOlsFit, PreparedOlsBread,\n",
    "    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_coefficients_map,\n"
    "    CertifiedPreparedOlsFit, PreparedOlsBread,\n",
    "ols module fit export",
)
OLS_MOD.write_text(ols_mod, encoding="utf-8")

lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "    compute_prepared_ols_absorbed_dof, fit_ols, fit_prepared_ols_clustered_map,\n"
    "    fit_prepared_ols_coefficients_map, fit_prepared_ols_robust_map,\n",
    "    compute_prepared_ols_absorbed_dof, fit_ols,\n"
    "    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_clustered_map,\n"
    "    fit_prepared_ols_coefficients_map, fit_prepared_ols_robust_map,\n",
    "crate fit export",
)
LIB.write_text(lib, encoding="utf-8")

print("generalized prepared_fit.rs over PreparedOlsFitView")
