from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class PreparedOlsBoundaryContractTests(unittest.TestCase):
    def test_action_is_developer_guarded_and_packaged_as_internal_helper(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(encoding="utf-8")
        package = (ROOT / "stata/stfixest.pkg").read_text(encoding="utf-8")
        input_source = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("confirm=developer-preparation-test", helper)
        self.assertIn('const CONFIRMATION: &str = "developer-preparation-test"', input_source)
        self.assertIn("f _stfixest_prepare_ols_test.ado", package)

    def test_spi_session_is_explicitly_thread_confined(self):
        ffi = (ROOT / "crates/stfixest-plugin/src/ffi.rs").read_text(encoding="utf-8")
        self.assertIn("struct SpiSession", ffi)
        self.assertIn("PhantomData<Rc<()>>", ffi)
        self.assertIn("stfixest_spi_is_main_thread", ffi)

    def test_public_estimators_remain_fail_closed(self):
        dispatch = (ROOT / "crates/stfixest-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        for filename in ("stfeols.ado", "stfepois.ado"):
            text = (ROOT / "stata" / filename).read_text(encoding="utf-8").lower()
            self.assertIn("exit 498", text)
            self.assertNotIn("plugin call", text)

    def test_licensed_fixture_locks_missing_rows_and_retained_count(self):
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("r(n_retained) == 24", fixture)
        self.assertIn("inlist(_n, 2, 5, 6, 7)", fixture)
        self.assertIn("assert _rc == 498", fixture)

    def test_analytic_weight_slice_is_explicit_and_remains_preparation_only(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("AWEIGHT(varname numeric)", helper)
        self.assertIn("OlsInputRole::AnalyticWeight", core)
        self.assertIn("WeightKind::Analytic", core)
        self.assertIn("(4, 0) => OlsInputRole::AnalyticWeight", plugin)
        self.assertIn("_st_aw_reason[8] == 4", fixture)
        self.assertIn("_rc == 459", fixture)

    def test_frequency_weight_slice_is_bounded_and_remains_preparation_only(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("FWEIGHT(varname numeric)", helper)
        self.assertIn("OlsInputRole::FrequencyWeight", core)
        self.assertIn("WeightKind::Frequency", core)
        self.assertIn("(5, 0) => OlsInputRole::FrequencyWeight", plugin)
        self.assertIn("r(n_retained) == 12", fixture)
        self.assertIn("r(n_retained) == 11", fixture)
        self.assertIn('assert "`r(weight_type)\'" == "fweight"', fixture)
        self.assertIn("assert _rc == 459", fixture)

    def test_probability_weight_slice_is_bounded_and_importance_weight_is_rejected(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("PWEIGHT(varname numeric)", helper)
        self.assertNotIn("IWEIGHT(varname numeric)", helper)
        self.assertIn("OlsInputRole::ProbabilityWeight", core)
        self.assertIn("WeightKind::Probability", core)
        self.assertIn("(6, 0) => OlsInputRole::ProbabilityWeight", plugin)
        self.assertNotIn("(7, 0) =>", plugin)
        self.assertIn('assert "`r(weight_type)\'" == "pweight"', fixture)
        self.assertIn("iweight(aw)", fixture)
        self.assertIn("assert _rc == 198", fixture)

    def test_numeric_factor_main_effect_slice_reuses_role_three_and_stays_internal(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("NumericSource(SourceId)", core)
        self.assertNotIn("ContinuousSource(SourceId)", core)
        self.assertIn("PreparedOlsCoefficientStatus::SyntaxOmitted", core)
        self.assertIn("factorize_f64", core)
        self.assertIn("(3, source) if source > 0", plugin)
        self.assertIn("OlsInputRole::NumericSource", plugin)
        self.assertNotIn("(7, 0) =>", plugin)
        self.assertIn("c.g ib2.g", fixture)
        self.assertIn("ibn.g", fixture)
        self.assertIn("exit 498", (ROOT / "stata/stfeols.ado").read_text(encoding="utf-8"))
        self.assertNotIn("fvrevar", helper.lower())

    def test_all_categorical_interaction_slice_is_recipe_only_and_stays_internal(self):
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/stfixest-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("PreparedOlsCategoricalComponent", core)
        self.assertIn("PreparedOlsColumnKind::CategoricalInteraction", core)
        self.assertNotIn("interaction_codes", core)
        self.assertIn("categorical_interaction_fixture", plugin)
        self.assertIn("ib2.g##ib1.h", fixture)
        self.assertIn("i.g#c.x", fixture)
        self.assertIn('\"estimate-ols\" | \"estimate-ppml\"', dispatch)
        self.assertIn("exit 498", (ROOT / "stata/stfeols.ado").read_text(encoding="utf-8"))

    def test_numeric_fixed_effect_preparation_is_bounded_and_stays_internal(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("ABSorb(string asis)", helper)
        self.assertIn("does not support string model or fixed-effect sources", helper)
        self.assertIn('"absorb_term_matrix=', helper)
        self.assertIn("PreparedOlsEffect", core)
        self.assertIn("NumericSourceUse", core)
        sample = (ROOT / "crates/stfixest-core/src/data/sample.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("DropReason::MissingFixedEffect", sample)
        self.assertIn("absorb_component_count", plugin)
        self.assertIn("absorb(g h g)", fixture)
        self.assertIn("_st_shared_fe_reason[10] == 3", fixture)
        self.assertIn("_st_shared_fe_reason[11] == 5", fixture)
        self.assertIn("absorb(g#h)", fixture)
        self.assertIn("absorb(g##c.x)", fixture)
        self.assertIn("absorb(saved_fe=g)", fixture)
        self.assertIn("absorb(fe_string)", fixture)
        self.assertIn('"estimate-ols" | "estimate-ppml"', (
            ROOT / "crates/stfixest-plugin/src/dispatch.rs"
        ).read_text(encoding="utf-8"))

    def test_singleton_preparation_is_fixed_point_aligned_and_not_an_estimator(self):
        helper = (ROOT / "stata/_stfixest_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        sample = (ROOT / "crates/stfixest-core/src/data/sample.rs").read_text(
            encoding="utf-8"
        )
        singleton = (ROOT / "crates/stfixest-core/src/fe/singleton.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("KEEPsingletons", helper)
        self.assertIn('"keep_singletons=`keep_singletons\'"', helper)
        self.assertIn("DropReason::Singleton", core)
        self.assertIn("compact_active_rows", core)
        self.assertIn("compact_active_rows", sample)
        self.assertIn("SingletonMultiplicity::Frequency", core)
        self.assertIn("checked_add", singleton)
        self.assertIn('"keep_singletons"', plugin)
        self.assertIn("r(n_singletons) == 5", fixture)
        self.assertIn("_st_singleton_reason == 8 in 1/5", fixture)
        self.assertIn("fweight(fw)", fixture)
        self.assertIn("_st_fw_singleton_reason[2] == 8", fixture)
        self.assertIn('"estimate-ols" | "estimate-ppml"', (
            ROOT / "crates/stfixest-plugin/src/dispatch.rs"
        ).read_text(encoding="utf-8"))
        self.assertIn("exit 498", (ROOT / "stata/stfeols.ado").read_text(encoding="utf-8"))

    def test_prepared_fe_topology_is_deduplicated_fallible_and_not_a_solver(self):
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        design = (ROOT / "crates/stfixest-core/src/fe/design.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/stfixest-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("PreparedOlsFeTopology", core)
        self.assertIn("topology_source_index", core)
        self.assertIn("build_prepared_fe_topology", core)
        self.assertIn("try_reserve_exact", core)
        self.assertIn("LevelIndex::from_codes", core)
        self.assertIn("from_codes_with_reservation", design)
        self.assertIn("checked_members", design)
        self.assertIn("stfixest.prepare_ols.n_fe_topology_sources", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assertNotIn("absorb_fixed_effects(", plugin)
        self.assertIn("exit 498", (ROOT / "stata/stfeols.ado").read_text(encoding="utf-8"))

    def test_borrowed_fe_adapter_is_intercept_only_and_does_not_invoke_a_solver(self):
        core = (ROOT / "crates/stfixest-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        design = (ROOT / "crates/stfixest-core/src/fe/design.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/stfixest-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/stfixest-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("BorrowedInterceptEffect", design)
        self.assertIn("BorrowedInterceptFeDesign", design)
        self.assertIn("borrowed_intercept_fe_design", core)
        self.assertIn("unable to reserve borrowed FE design effects", core)
        self.assertIn("checked_intercept_term_slots", design)
        self.assertNotIn("BorrowedInterceptFeDesign", plugin)
        self.assertNotIn("absorb_fixed_effects(", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assertIn("exit 498", (ROOT / "stata/stfeols.ado").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
