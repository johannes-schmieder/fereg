#!/usr/bin/env python3
"""Repair stale source anchors, run the frozen iid transform, and update contracts."""

from __future__ import annotations

import runpy
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
BASE = HERE / "generalize_prepared_iid_base.py"
PATCHED = HERE / "_generalize_prepared_iid_patched.py"
BOUNDARY_TEST = ROOT / "tests/python/test_prepared_ols_boundary.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


script = BASE.read_text(encoding="utf-8")
script = replace_once(
    script,
    '''lib = replace_once(
    lib,
    "    fit_prepared_categorical_ols_coefficients_map, fit_prepared_ols_clustered_map,\\n",
    "    fit_prepared_categorical_ols_coefficients_map,\\n"
    "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_clustered_map,\\n",
    "crate categorical iid function export",
)
''',
    '''lib = replace_once(
    lib,
    "    compute_prepared_ols_absorbed_dof, fit_ols, fit_prepared_categorical_ols_coefficients_map,\\n",
    "    compute_prepared_ols_absorbed_dof, fit_ols, fit_prepared_categorical_ols_coefficients_map,\\n"
    "    fit_prepared_categorical_ols_unadjusted_map,\\n",
    "crate categorical iid function export",
)
''',
    "categorical iid function export anchor",
)
script = replace_once(
    script,
    '''lib = replace_once(
    lib,
    "    PreparedOlsClusteredPsdState, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,\\n",
    "    PreparedOlsClusteredPsdState, PreparedOlsCoefficient, PreparedOlsCoefficientStatus,\\n"
    "    CertifiedPreparedCategoricalOlsUnadjusted,\\n",
    "crate categorical iid result export",
)
''',
    '''lib = replace_once(
    lib,
    "    solve_rank_revealing, CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust,\\n",
    "    solve_rank_revealing, CertifiedPreparedCategoricalOlsUnadjusted,\\n"
    "    CertifiedPreparedOlsFit, CertifiedPreparedOlsRobust,\\n",
    "crate categorical iid result export",
)
''',
    "categorical iid result export anchor",
)
PATCHED.write_text(script, encoding="utf-8")

try:
    runpy.run_path(str(PATCHED), run_name="__main__")

    boundary = BOUNDARY_TEST.read_text(encoding="utf-8")
    boundary = replace_once(
        boundary,
        '''        production = inference.split("#[cfg(test)]", maxsplit=1)[0]
        fit = (ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs").read_text(
''',
        '''        production = inference.split("#[cfg(test)]", maxsplit=1)[0]
        inference_view = (
            ROOT / "crates/stfixest-core/src/ols/prepared_inference_view.rs"
        ).read_text(encoding="utf-8")
        fit = (ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs").read_text(
''',
        "prepared iid source-contract view setup",
    )
    boundary = replace_once(
        boundary,
        '''        self.assertIn("fit_prepared_ols_unadjusted_map", production)
        self.assertIn("fit_prepared_ols_coefficients_map", production)
        self.assertIn("compute_prepared_ols_absorbed_dof", production)
''',
        '''        self.assertIn("fit_prepared_ols_unadjusted_map", production)
        self.assertIn("fit_prepared_categorical_ols_unadjusted_map", production)
        self.assertIn("fit_prepared_ols_coefficients_map", production)
        self.assertIn("PreparedOlsInferenceView", inference_view)
        self.assertIn("compute_prepared_ols_absorbed_dof", inference_view)
        self.assertIn("inference_compute_absorbed_dof", inference_view)
''',
        "prepared iid source-contract assertions",
    )
    boundary = replace_once(
        boundary,
        '''        self.assertNotIn("fit_prepared_ols_unadjusted_map", plugin)
        self.assertIn('\"estimate-ols\" | \"estimate-ppml\"', dispatch)
''',
        '''        self.assertNotIn("fit_prepared_ols_unadjusted_map", plugin)
        self.assertNotIn("fit_prepared_categorical_ols_unadjusted_map", plugin)
        self.assertIn('\"estimate-ols\" | \"estimate-ppml\"', dispatch)
''',
        "prepared iid plugin isolation assertion",
    )
    BOUNDARY_TEST.write_text(boundary, encoding="utf-8")

    BASE.unlink()
finally:
    PATCHED.unlink(missing_ok=True)
