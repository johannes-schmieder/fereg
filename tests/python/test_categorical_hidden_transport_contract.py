from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "crates/stfixest-plugin/src"
STATA = ROOT / "stata/_stfixest_estimate_ols_test.ado"


class CategoricalHiddenTransportContractTests(unittest.TestCase):
    def test_hidden_dispatch_routes_all_three_vce_families(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        for symbol in (
            "prepare_categorical_ols_data",
            "prepare_categorical_ols_clustered_data",
            "fit_prepared_categorical_ols_unadjusted_map",
            "fit_prepared_categorical_ols_robust_map",
            "fit_prepared_categorical_ols_clustered_map",
            "CategoricalIid",
            "CategoricalRobust",
            "CategoricalClustered",
        ):
            self.assertIn(symbol, estimate)
        self.assertIn("categorical_sources.len() > 1", estimate)
        self.assertNotIn('ACTION_ESTIMATE_OLS: &str', estimate)

    def test_result_wire_retains_dimensions_and_interaction_identity(self):
        result = (PLUGIN / "result.rs").read_text(encoding="utf-8")
        view = (PLUGIN / "result_view.rs").read_text(encoding="utf-8")
        self.assertIn("const FE_COLUMNS: usize = 8", result)
        self.assertIn("const RECEIPT_COLUMNS: usize = 16", result)
        self.assertIn("_ => (3.0, f64::from(term_id.get()), 0.0)", result)
        self.assertIn("Categorical(&'a PreparedCategoricalOlsData)", view)
        self.assertIn(
            "Categorical(&'a PreparedCategoricalOlsClusteredData)", view
        )
        self.assertNotIn("unsafe {", result + view)

    def test_stata_gate_is_intercept_only_but_allows_categorical_tuples(self):
        stata = STATA.read_text(encoding="utf-8")
        self.assertIn("`absorb_terms'[`row',2] < 1", stata)
        self.assertIn("`absorb_terms'[`row',3] != 1", stata)
        self.assertIn(
            "hidden prepared OLS supports only all-categorical intercept fixed effects",
            stata,
        )
        self.assertNotIn("only single-source intercept fixed effects", stata)


if __name__ == "__main__":
    unittest.main()
