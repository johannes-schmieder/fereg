from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CORE = ROOT / "crates/stfixest-core/src"
VIEW = CORE / "ols/prepared_fit_view.rs"
LIB = CORE / "lib.rs"


class PreparedFitViewContractTests(unittest.TestCase):
    def test_shared_view_covers_legacy_and_categorical_prepared_data(self):
        text = VIEW.read_text(encoding="utf-8")
        self.assertEqual(text.count("impl PreparedOlsFitView for PreparedOlsData"), 1)
        self.assertEqual(
            text.count("impl PreparedOlsFitView for PreparedCategoricalOlsData"), 1
        )
        for method in (
            "fit_model",
            "fit_sample",
            "fit_structural_intercept",
            "fit_report_intercept",
            "fit_column_value",
            "fit_build_rhs",
            "fit_absorb_matrix",
        ):
            self.assertIn(f"fn {method}", text)
        self.assertIn("absorb_borrowed_intercept_map_with_structural_constant_certified", text)
        self.assertIn("absorb_finalized_categorical_topology_map_certified", text)

    def test_shared_view_remains_private_and_source_bound(self):
        text = VIEW.read_text(encoding="utf-8")
        lib = LIB.read_text(encoding="utf-8")
        self.assertIn("pub(crate) trait PreparedOlsFitView", text)
        self.assertNotIn("pub trait PreparedOlsFitView", text)
        self.assertIn(
            '#[path = "ols/prepared_fit_view.rs"]\npub(crate) mod prepared_fit_view;',
            lib,
        )
        self.assertNotIn("unsafe {", text)
        for public_action in (
            "estimate-ols",
            "stfeols",
            "estimate-ppml",
            "stfepois",
        ):
            self.assertNotIn(public_action, text)


if __name__ == "__main__":
    unittest.main()
