from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class PreparedOlsBoundaryContractTests(unittest.TestCase):
    def assert_public_fereg_is_wired(self):
        fereg = (ROOT / "stata/fereg.ado").read_text(encoding="utf-8")
        public = (ROOT / "stata/_fereg_estimate_public.ado").read_text(
            encoding="utf-8"
        )
        package = (ROOT / "stata/fereg.pkg").read_text(encoding="utf-8")
        self.assertIn("_fereg_estimate_public `0'", fereg)
        self.assertIn("_fereg_ols `varlist'", public)
        self.assertIn("f _fereg_ols.ado", package)
        self.assertIn("f _fereg_estimate_ols.ado", package)

    def test_preparation_action_is_developer_guarded_and_not_packaged(self):
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(encoding="utf-8")
        package = (ROOT / "stata/fereg.pkg").read_text(encoding="utf-8")
        input_source = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("confirm=developer-preparation-test", helper)
        self.assertIn('const CONFIRMATION: &str = "developer-preparation-test"', input_source)
        self.assertNotIn("f _fereg_prepare_ols_test.ado", package)

    def test_spi_session_is_explicitly_thread_confined(self):
        ffi = (ROOT / "crates/fereg-plugin/src/ffi.rs").read_text(encoding="utf-8")
        self.assertIn("struct SpiSession", ffi)
        self.assertIn("PhantomData<Rc<()>>", ffi)
        self.assertIn("fereg_spi_is_main_thread", ffi)

    def test_public_fereg_is_wired_and_feppml_remains_fail_closed(self):
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn('"estimate-ppml"', dispatch)
        self.assert_public_fereg_is_wired()
        feppml = (ROOT / "stata/feppml.ado").read_text(encoding="utf-8").lower()
        self.assertIn("exit 498", feppml)
        self.assertNotIn("plugin call", feppml)

    def test_licensed_fixture_locks_missing_rows_and_retained_count(self):
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        self.assertIn("r(n_retained) == 24", fixture)
        self.assertIn("inlist(_n, 2, 5, 6, 7)", fixture)
        self.assertIn("assert _rc == 498", fixture)

    def test_analytic_weight_slice_is_explicit_and_remains_preparation_only(self):
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
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
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
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
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
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
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
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
        self.assert_public_fereg_is_wired()
        self.assertNotIn("fvrevar", helper.lower())

    def test_all_categorical_interaction_slice_is_recipe_only_and_stays_internal(self):
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("PreparedOlsCategoricalComponent", core)
        self.assertIn("PreparedOlsColumnKind::CategoricalInteraction", core)
        self.assertNotIn("interaction_codes", core)
        self.assertIn("categorical_interaction_fixture", plugin)
        self.assertIn("ib2.g##ib1.h", fixture)
        self.assertIn("i.g#c.x", fixture)
        self.assertIn('\"estimate-ols\" | \"estimate-ppml\"', dispatch)
        self.assert_public_fereg_is_wired()

    def test_numeric_fixed_effect_preparation_is_bounded_and_stays_internal(self):
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
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
        sample = (ROOT / "crates/fereg-core/src/data/sample.rs").read_text(
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
            ROOT / "crates/fereg-plugin/src/dispatch.rs"
        ).read_text(encoding="utf-8"))

    def test_singleton_preparation_is_fixed_point_aligned_and_not_an_estimator(self):
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        sample = (ROOT / "crates/fereg-core/src/data/sample.rs").read_text(
            encoding="utf-8"
        )
        singleton = (ROOT / "crates/fereg-core/src/fe/singleton.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
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
            ROOT / "crates/fereg-plugin/src/dispatch.rs"
        ).read_text(encoding="utf-8"))
        self.assert_public_fereg_is_wired()

    def test_prepared_fe_topology_is_deduplicated_fallible_and_not_a_solver(self):
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        design = (ROOT / "crates/fereg-core/src/fe/design.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("PreparedOlsFeTopology", core)
        self.assertIn("topology_source_index", core)
        self.assertIn("build_prepared_fe_topology", core)
        self.assertIn("try_reserve_exact", core)
        self.assertIn("LevelIndex::from_codes", core)
        self.assertIn("from_codes_with_reservation", design)
        self.assertIn("checked_members", design)
        self.assertIn("fereg.prepare_ols.n_fe_topology_sources", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assertNotIn("absorb_fixed_effects(", plugin)
        self.assert_public_fereg_is_wired()

    def test_borrowed_fe_adapter_is_intercept_only_and_does_not_invoke_a_solver(self):
        core = (ROOT / "crates/fereg-core/src/ols/prepared.rs").read_text(
            encoding="utf-8"
        )
        design = (ROOT / "crates/fereg-core/src/fe/design.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("BorrowedInterceptEffect", design)
        self.assertIn("BorrowedInterceptFeDesign", design)
        self.assertIn("ValidatedBorrowedInterceptSource", design)
        self.assertIn("borrowed_intercept_fe_design", core)
        self.assertIn("build_borrowed_intercept_fe_design_with_controls", core)
        self.assertIn("CountingBorrowedSourceValidation", core)
        self.assertIn("repeated_fe_terms_validate_their_unique_source_once", core)
        self.assertIn("unable to reserve validated borrowed FE sources", core)
        self.assertIn("unable to reserve borrowed FE design effects", core)
        self.assertIn("checked_intercept_term_slots", design)
        self.assertNotIn("BorrowedInterceptFeDesign", plugin)
        self.assertNotIn("absorb_fixed_effects(", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assert_public_fereg_is_wired()

    def test_prepared_rhs_map_check_is_opt_in_fallible_and_not_an_estimator(self):
        rhs = (ROOT / "crates/fereg-core/src/ols/prepared_rhs.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        helper = (ROOT / "stata/_fereg_prepare_ols_test.ado").read_text(
            encoding="utf-8"
        )
        fixture = (ROOT / "tests/stata/installation/prepare_ols_boundary.do").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("build_prepared_ols_rhs", rhs)
        self.assertIn("absorb_prepared_ols_rhs_map", rhs)
        self.assertIn("try_reserve_exact", rhs)
        self.assertIn("checked_mul", rhs)
        self.assertIn("SyntaxOmitted", rhs)
        self.assertIn("BorrowedMapWeights::Unit", rhs)
        self.assertIn("run_borrowed_map", plugin)
        self.assertIn("write_diagnostics", plugin)
        self.assertLess(
            plugin.index("run_borrowed_map_check(&prepared"),
            plugin.index("write_diagnostics(&action.diagnostics"),
        )
        self.assertIn("MAPCHECK", helper)
        self.assertIn('"run_borrowed_map=`run_borrowed_map\'"', helper)
        self.assertIn("if 0, mapcheck", fixture)
        self.assertIn("_rc == 459", fixture)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assert_public_fereg_is_wired()

    def test_prepared_coefficient_fit_is_certified_core_only_and_not_public(self):
        fit = (ROOT / "crates/fereg-core/src/ols/prepared_fit.rs").read_text(
            encoding="utf-8"
        )
        production = fit.split("#[cfg(test)]", maxsplit=1)[0]
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("fit_prepared_ols_coefficients_map", production)
        self.assertIn("fit_prepared_categorical_ols_coefficients_map", production)
        self.assertIn("PreparedOlsFitView", production)
        self.assertIn("fit_build_rhs", production)
        self.assertIn("fit_absorb_matrix", production)
        self.assertNotIn("absorb_borrowed_intercept_map_certified(", production)
        self.assertIn("AbsorbedInterceptDeferred", production)
        self.assertIn("fit_scaled_pivoted_qr", production)
        self.assertIn("certify_scores", production)
        self.assertIn("try_reserve_exact", production)
        for forbidden in (
            "fit_ols(",
            "solve_rank_revealing",
            "compute_dof",
            "FeDesign",
            "vec![",
            ".collect",
            ".to_vec",
            "with_capacity",
        ):
            self.assertNotIn(forbidden, production)
        self.assertNotIn("fit_prepared_ols_coefficients_map", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assert_public_fereg_is_wired()

    def test_prepared_absorbed_dof_is_structural_fallible_and_not_public(self):
        dof = (ROOT / "crates/fereg-core/src/ols/prepared_dof.rs").read_text(
            encoding="utf-8"
        )
        production = dof.split("#[cfg(test)]", maxsplit=1)[0]
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("PreparedOlsAbsorbedDofReport", production)
        self.assertIn("PreparedOlsAbsorbedDofPolicy", production)
        self.assertIn("PreparedOlsAbsorbedDofPolicy::None", production)
        self.assertIn("PreparedOlsAbsorbedDofPolicy::FirstPair", production)
        self.assertIn("PreparedOlsAbsorbedDofPolicy::Pairwise", production)
        self.assertIn("prepared.fe_topology", production)
        self.assertIn(".categorical_sources", production)
        self.assertIn("try_reserve_exact", production)
        self.assertIn("checked_add", production)
        self.assertIn("fn find_root", production)
        for forbidden in (
            "prepared.model",
            "prepared.sample.outcome",
            "prepared.sample.regressors",
            "prepared.sample.weights",
            "PreparedOlsRhs",
            "fit_ols(",
            "compute_dof(",
            "FeDesign",
            "vec![",
            ".collect",
            ".to_vec",
            "with_capacity",
        ):
            self.assertNotIn(forbidden, production)
        self.assertIn("ols/20260821T030000Z-a5181f9", dof)
        self.assertNotIn("compute_prepared_ols_absorbed_dof", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assert_public_fereg_is_wired()

    def test_prepared_iid_inference_is_same_call_certified_and_not_public(self):
        inference = (
            ROOT / "crates/fereg-core/src/ols/prepared_unadjusted.rs"
        ).read_text(encoding="utf-8")
        production = inference.split(
            "\n#[cfg(test)]\nmod tests {", maxsplit=1
        )[0]
        inference_view = (
            ROOT / "crates/fereg-core/src/ols/prepared_inference_view.rs"
        ).read_text(encoding="utf-8")
        fit = (ROOT / "crates/fereg-core/src/ols/prepared_fit.rs").read_text(
            encoding="utf-8"
        )
        plugin = (ROOT / "crates/fereg-plugin/src/input.rs").read_text(
            encoding="utf-8"
        )
        dispatch = (ROOT / "crates/fereg-plugin/src/dispatch.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("fit_prepared_ols_unadjusted_map", production)
        self.assertIn("fit_prepared_categorical_ols_unadjusted_map", production)
        self.assertIn("fit_prepared_ols_coefficients_map", production)
        self.assertIn("PreparedOlsInferenceView", inference_view)
        self.assertIn("compute_prepared_ols_absorbed_dof", inference_view)
        self.assertIn("inference_compute_absorbed_dof", inference_view)
        self.assertIn("initial_absorbed_outcome_sum_of_squares", fit)
        self.assertIn("PreparedOlsUnadjustedCertificate", production)
        self.assertIn("PreparedOlsReferenceWeightReport", production)
        self.assertIn("WeightKind::Probability", production)
        self.assertIn("requires robust inference", production)
        self.assertIn("try_reserve_exact", production)
        self.assertIn("checked_mul", production)
        for forbidden in (
            "fit_ols(",
            "compute_vce(",
            "compute_fit_statistics",
            "super::vce",
            "super::fit",
            "super::dof",
            "vec![",
            ".collect",
            ".to_vec",
            "with_capacity",
        ):
            self.assertNotIn(forbidden, production)
        self.assertNotIn("fit_prepared_ols_unadjusted_map", plugin)
        self.assertNotIn("fit_prepared_categorical_ols_unadjusted_map", plugin)
        self.assertIn('"estimate-ols" | "estimate-ppml"', dispatch)
        self.assert_public_fereg_is_wired()


if __name__ == "__main__":
    unittest.main()
