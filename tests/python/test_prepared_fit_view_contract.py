from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CORE = ROOT / "crates/fereg-core/src"
VIEW = CORE / "ols/prepared_fit_view.rs"
FIT = CORE / "ols/prepared_fit.rs"
SESSION = CORE / "ols/linear_session.rs"
SCRATCH = CORE / "ols/prepared_scratch.rs"
OLS_MOD = CORE / "ols/mod.rs"
RHS = CORE / "ols/prepared_rhs.rs"
CATEGORICAL = CORE / "ols/prepared_categorical_data.rs"
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
            "fit_build_rhs_with_response",
            "fit_build_rhs_block_with_response",
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
            "fereg",
            "estimate-ppml",
            "feppml",
        ):
            self.assertNotIn(public_action, text)

    def test_weighted_linear_frame_is_borrowed_private_and_allocation_free(self):
        fit = FIT.read_text(encoding="utf-8")
        lib = LIB.read_text(encoding="utf-8")
        declaration = fit.split("pub(crate) struct LinearSolveFrame<'a>", 1)[1].split(
            "}", 1
        )[0]
        self.assertIn("response: &'a [f64]", declaration)
        self.assertIn("weights: NormalizedWeights<'a>", declaration)
        self.assertIn("bread_weight_scale: PreparedOlsBreadWeightScale", declaration)
        self.assertNotIn("Vec<", declaration)
        self.assertIn("pub(crate) fn from_parts", fit)
        self.assertIn("BorrowedMapWeights<'a>", fit)
        self.assertIn("fit_prepared_ols_coefficients_view_map_with_frame", fit)
        self.assertNotIn("LinearSolveFrame", lib)
        self.assertNotIn("Box<dyn PreparedOlsFitView", fit)

    def test_explicit_response_reuses_layout_specific_rhs_builders(self):
        view = VIEW.read_text(encoding="utf-8")
        rhs = RHS.read_text(encoding="utf-8")
        categorical = CATEGORICAL.read_text(encoding="utf-8")
        self.assertIn("fit_build_rhs_with_response", view)
        self.assertIn("fit_build_rhs_block_with_response", view)
        self.assertIn("build_prepared_ols_rhs_with_response", rhs)
        self.assertIn("fill_rhs_single_thread(prepared, response", rhs)
        self.assertIn("destination[0] = response[row]", rhs)
        self.assertIn("pub(crate) fn build_rhs_with_response", categorical)
        self.assertIn("destination[0] = response[row]", categorical)
        self.assertIn(
            "explicit_response_replaces_only_column_zero_with_one_allocation", rhs
        )

    def test_iterative_session_is_crate_private_and_absent_from_public_ols_adapters(self):
        session = SESSION.read_text(encoding="utf-8")
        fit = FIT.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        ols_mod = OLS_MOD.read_text(encoding="utf-8")
        lib = LIB.read_text(encoding="utf-8")
        self.assertIn("pub(crate) struct CertifiedLinearSession", session)
        self.assertIn("pub(crate) fn solve_fresh", session)
        self.assertIn("pub(crate) fn solve_frozen", session)
        self.assertIn("pub(crate) fn strictify", session)
        self.assertIn("kernel_moment_workspace_reused: false", session)
        self.assertIn("steady_state_allocation_free: false", session)
        self.assertIn("mod linear_session;", ols_mod)
        self.assertIn("pub(crate) use linear_session", ols_mod)
        public_reexports = ols_mod.split("#[allow(unused_imports)]", 1)[0]
        self.assertNotIn("CertifiedLinearSession", public_reexports)
        self.assertNotIn("CertifiedLinearSession", lib)
        for adapter in (
            "fit_prepared_ols_coefficients_map",
            "fit_prepared_categorical_ols_coefficients_map",
            "fit_prepared_ols_coefficients_view_map",
        ):
            body = fit.split(f"fn {adapter}", 1)[1].split("\n}", 1)[0]
            self.assertNotIn("CertifiedLinearSession", body)
            self.assertNotIn("solve_frozen", body)
            self.assertNotIn("working_response", body)

    def test_phase_allocator_probe_is_session_only_and_ols_defaults_to_none(self):
        session = SESSION.read_text(encoding="utf-8")
        fit = FIT.read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]
        self.assertIn("pub enum LinearSessionAllocationPhase", session)
        self.assertIn("pub enum LinearSessionPhaseAllocationEvent", session)
        self.assertIn("pub type LinearSessionPhaseAllocatorProbe", session)
        self.assertIn("phase_allocator_probe: None", session)
        self.assertIn("fn set_phase_allocator_probe", session)
        self.assertIn(
            "fit_prepared_weighted_linear_once(prepared, frame, config, false, None, None)",
            fit,
        )
        for adapter in (
            "fit_prepared_ols_coefficients_map",
            "fit_prepared_categorical_ols_coefficients_map",
            "fit_prepared_ols_coefficients_view_map",
        ):
            body = fit.split(f"fn {adapter}", 1)[1].split("\n}", 1)[0]
            self.assertNotIn("phase_allocator_probe", body)
            self.assertNotIn("LinearSessionPhaseAllocatorProbe", body)
            self.assertNotIn("Ppml", body)

    def test_session_generation_and_owned_final_product_contract(self):
        session = SESSION.read_text(encoding="utf-8")
        for generation in ("sample", "topology", "weights", "response", "basis"):
            self.assertIn(f"pub(crate) {generation}: u64", session)
        for invalidator in (
            "refresh_response",
            "refresh_weights",
            "refresh_basis",
            "invalidate_sample",
            "invalidate_topology",
            "refresh_config",
        ):
            self.assertIn(f"fn {invalidator}", session)
        self.assertIn("pub(crate) fn into_current_products", session)
        self.assertIn("pub(crate) struct CertifiedLinearSessionProducts", session)
        self.assertIn("pub(crate) fn retained_within_memory", session)
        self.assertIn("pub(crate) fn workspace_capacities", session)

    def test_low_memory_final_design_has_thread_safe_positional_row_access(self):
        session = SESSION.read_text(encoding="utf-8")
        scratch = SCRATCH.read_text(encoding="utf-8")
        self.assertIn("pub(crate) struct CertifiedLinearWithinDesign", session)
        self.assertIn("impl SandwichDesign for CertifiedLinearWithinDesign", session)
        self.assertIn("#[derive(Debug, Clone, Copy)]", session)
        self.assertIn("pub(crate) const fn finite_within_design", session)
        self.assertIn("read_linear_shared", scratch)
        self.assertIn("read_at(output, offset)", scratch)
        self.assertNotIn("Scratch(RefCell<PreparedOlsScratch>)", session)


if __name__ == "__main__":
    unittest.main()
