from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "crates/fereg-plugin/src"
STATA = ROOT / "stata/_fereg_estimate_ols.ado"
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
        self.assertIn('ACTION_ESTIMATE_OLS: &str = "estimate-ols"', estimate)

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
            ROOT / "legacy/fereg-core/src/ols/prepared_unadjusted.rs"
        ).read_text(encoding="utf-8")
        exports = (
            ROOT / "legacy/fereg-core/src/ols/mod.rs"
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

    def test_stata_gate_allows_multiple_and_slope_only_channels(self):
        stata = STATA.read_text(encoding="utf-8")
        self.assertIn("`absorb_terms'[`row',2] < 1", stata)
        self.assertIn("!inlist(`absorb_terms'[`row',3], 0, 1)", stata)
        self.assertIn(
            "at least one intercept or slope channel",
            stata,
        )
        self.assertIn("local n_absorb_intercepts 0", stata)
        self.assertIn("`n_absorb_intercepts' + `n_absorb_slopes'", stata)
        self.assertNotIn("if `n_absorb_slopes' > 1", stata)
        self.assertNotIn("absorbed slope is qualified only for iid inference", stata)
        self.assertNotIn("absorbed slope is not yet qualified for weights", stata)
        self.assertNotIn(
            "absorbed slope requires pairwise or continuous absorbed DoF", stata
        )
        self.assertNotIn("only single-source intercept fixed effects", stata)

    def test_licensed_slope_fixtures_are_in_every_qualification_profile(self):
        fixtures = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_intercept_slope_parity.do",
            "tests/stata/parity_reghdfe/"
            "estimate_ols_intercept_slope_overabsorbed_fail_closed.do",
            "tests/stata/parity_reghdfe/"
            "estimate_ols_slope_only_multiple_parity.do",
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

    def test_fixed_width_string_ingestion_is_native_and_qualified(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        ffi = (PLUGIN / "ffi.rs").read_text(encoding="utf-8")
        stata = STATA.read_text(encoding="utf-8")
        fixture = (
            "tests/stata/parity_reghdfe/estimate_ols_string_fe_parity.do"
        )
        self.assertIn("OlsSourceLoader::FixedWidthString", estimate)
        self.assertIn("provisional_codes: HashMap<Vec<u8>, u32>", estimate)
        self.assertIn("levels.sort_unstable_by", estimate)
        self.assertIn("fereg_spi_read_string", ffi)
        self.assertIn("variable_is_strl", ffi)
        self.assertNotIn("supports only numeric sources", stata)
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            with self.subTest(profile=name):
                self.assertIn(fixture, profiles[name]["fixtures"])

    def test_time_series_materialization_is_narrow_metadata_preserving_exception(self):
        stata = STATA.read_text(encoding="utf-8")
        parser = (ROOT / "stata/fereg_parse.ado").read_text(encoding="utf-8")
        core = (ROOT / "legacy/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        fixture = (
            "tests/stata/parity_reghdfe/estimate_ols_time_series_parity.do"
        )
        self.assertIn("generate double `materialized_ts'", stata)
        self.assertIn("original_model_components", stata)
        self.assertIn("matrix `model_components'[`component_row',9] = 3", stata)
        self.assertNotIn("fvrevar", stata.lower())
        self.assertNotIn("generate double `materialized_ts'", parser)
        self.assertIn("TimeSeriesState::Materialized", core)
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            with self.subTest(profile=name):
                self.assertIn(fixture, profiles[name]["fixtures"])

    def test_iweight_probe_is_an_explicit_pinned_rejection(self):
        adapter = (ROOT / "stata/_fereg_ols.ado").read_text(encoding="utf-8")
        fixture = "tests/stata/parity_reghdfe/estimate_ols_iweight_parity.do"
        self.assertIn("[fw aw pw iw]", adapter)
        self.assertIn('`"`weight\'"\' == "iweight"', adapter)
        self.assertIn("exit 101", adapter)
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            with self.subTest(profile=name):
                self.assertIn(fixture, profiles[name]["fixtures"])


if __name__ == "__main__":
    unittest.main()
