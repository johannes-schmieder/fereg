from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "crates/fereg-plugin/src"
STATA = ROOT / "stata/_fereg_estimate_ols_test.ado"
PROFILES = ROOT / "ci/stata_profiles.json"


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
        for forbidden in (
            "fit_prepared_categorical_ols_signed_unadjusted_map",
            "CategoricalSignedIid",
            "allow_signed_negative_dof",
        ):
            self.assertNotIn(forbidden, estimate)
        self.assertIn("categorical_sources.len() > 1", estimate)
        self.assertIn("plan_categorical_absorbed_request", estimate)
        self.assertIn("plan.availability_request().validate_and_compile()", estimate)
        self.assertIn("validate_cluster_request_shape(&requested_clustered)", estimate)
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

    def test_signed_residual_dof_publication_path_is_absent(self):
        core = (
            ROOT / "crates/fereg-core/src/ols/prepared_unadjusted.rs"
        ).read_text(encoding="utf-8")
        exports = (
            ROOT / "crates/fereg-core/src/ols/mod.rs"
        ).read_text(encoding="utf-8")
        result = (PLUGIN / "result.rs").read_text(encoding="utf-8")
        stata = STATA.read_text(encoding="utf-8")
        combined = core + exports + result
        for forbidden in (
            "PreparedOlsSignedResidualDofReport",
            "PreparedOlsSignedFitStatistics",
            "PreparedOlsSignedUnadjustedCovariance",
            "CertifiedPreparedCategoricalOlsSignedUnadjusted",
            "fit_prepared_categorical_ols_signed_unadjusted_map",
            "CategoricalSignedIid",
        ):
            self.assertNotIn(forbidden, combined)
        self.assertNotIn("ereturn scalar df_r = `df_r'", stata)
        self.assertIn("shadow.covariance[row * dimension + row] < 0.0", (
            PLUGIN / "estimate.rs"
        ).read_text(encoding="utf-8"))

    def test_native_fe_shadow_validator_accepts_only_structured_interactions(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        self.assertIn("validate_shadow_fixed_effect_row(row)?", estimate)
        self.assertIn("3 => term > 0 && source == 0", estimate)
        self.assertIn("4 => term > 0 && source > 0", estimate)
        self.assertIn("coefficients != categories - redundant", estimate)

    def test_stata_gate_allows_one_guarded_intercept_slope_channel(self):
        stata = STATA.read_text(encoding="utf-8")
        self.assertIn("`absorb_terms'[`row',2] < 1", stata)
        self.assertIn("`absorb_terms'[`row',3] != 1", stata)
        self.assertIn(
            "hidden prepared OLS supports categorical intercept effects and one guarded slope",
            stata,
        )
        self.assertIn("if `n_absorb_slopes' > 1", stata)
        self.assertIn("absorbed slope is qualified only for iid inference", stata)
        self.assertIn("absorbed slope is not yet qualified for weights", stata)
        self.assertIn("absorbed slope currently requires pairwise absorbed DoF", stata)
        self.assertNotIn("only single-source intercept fixed effects", stata)

    def test_licensed_slope_fixtures_are_in_every_qualification_profile(self):
        fixtures = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_intercept_slope_parity.do",
            "tests/stata/parity_reghdfe/"
            "estimate_ols_intercept_slope_overabsorbed_fail_closed.do",
        )
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            with self.subTest(profile=name):
                for fixture in fixtures:
                    self.assertIn(fixture, profiles[name]["fixtures"])

    def test_licensed_slope_fixture_checks_reference_inexact_dof_metadata(self):
        fixture = (
            ROOT
            / "tests/stata/parity_reghdfe/estimate_ols_intercept_slope_parity.do"
        ).read_text(encoding="utf-8")
        self.assertIn("e(dof_table)[2,4] == 1", fixture)
        self.assertNotIn("e(dof_table)[2,4] == 0", fixture)


if __name__ == "__main__":
    unittest.main()
