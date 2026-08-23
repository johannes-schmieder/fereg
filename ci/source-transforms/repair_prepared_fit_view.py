#!/usr/bin/env python3
"""Repair the five residual legacy field accesses after the fit-view refactor."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIT = ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs"
CONTRACT = ROOT / "tests/python/test_prepared_ols_boundary.py"


def regex_exact(text: str, pattern: str, replacement: str, expected: int, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != expected:
        raise SystemExit(f"{label}: expected {expected} matches, found {count}")
    return updated


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


fit = FIT.read_text(encoding="utf-8")
marker = "\n#[cfg(test)]\nmod tests {"
if fit.count(marker) != 1:
    raise SystemExit("prepared_fit.rs test-module marker is not unique")
production, tests = fit.split(marker, 1)
production = regex_exact(
    production,
    r"prepared\s*\.model\s*\.columns",
    "prepared.fit_model().columns",
    4,
    "legacy model field access",
)
production = regex_exact(
    production,
    r"prepared\s*\.sample\s*\.n_active\(\)",
    "prepared.fit_sample().n_active()",
    1,
    "legacy sample field access",
)
for forbidden in ("prepared.model", "prepared.sample"):
    if forbidden in production:
        raise SystemExit(f"prepared_fit.rs still contains legacy access: {forbidden}")
FIT.write_text(production + marker + tests, encoding="utf-8")

contract = CONTRACT.read_text(encoding="utf-8")
old = '''        self.assertIn("fit_prepared_ols_coefficients_map", production)
        self.assertIn("build_prepared_ols_rhs", production)
        self.assertIn(
            "absorb_borrowed_intercept_map_with_structural_constant_certified",
            production,
        )
'''
new = '''        self.assertIn("fit_prepared_ols_coefficients_map", production)
        self.assertIn("fit_prepared_categorical_ols_coefficients_map", production)
        self.assertIn("PreparedOlsFitView", production)
        self.assertIn("fit_build_rhs", production)
        self.assertIn("fit_absorb_matrix", production)
'''
contract = replace_once(contract, old, new, "prepared-fit source contract")
CONTRACT.write_text(contract, encoding="utf-8")

print("repaired five fit-view accesses and updated the source contract")
