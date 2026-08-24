from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/validate-final-sample-rebase.yml"
TRANSFORM = ROOT / "ci/source-transforms/activate_final_sample_rebase.py"
CORE = ROOT / "crates/fereg-core/src/lib.rs"
DATA = ROOT / "crates/fereg-core/src/ols/prepared_categorical_data.rs"
STATA = ROOT / "stata/_fereg_estimate_ols_test.ado"
PROFILES = ROOT / "ci/stata_profiles.json"


class FinalSampleRebaseValidationContractTests(unittest.TestCase):
    def test_obsolete_branch_only_activation_path_is_removed(self) -> None:
        self.assertFalse(WORKFLOW.exists())
        self.assertFalse(TRANSFORM.exists())

    def test_rebase_is_compiled_and_called_by_production_preparation(self) -> None:
        core = CORE.read_text(encoding="utf-8")
        data = DATA.read_text(encoding="utf-8")
        self.assertIn("mod prepared_categorical_rebase;", core)
        self.assertIn(
            "pub use prepared_categorical_rebase::PreparedCategoricalOlsReportPlan;",
            core,
        )
        self.assertIn("rebase_categorical_model(", data)
        self.assertIn("report_plan: rebased.report_plan", data)
        self.assertNotIn("remap_and_resolve_model(", data)

    def test_stata_posts_only_the_final_logical_graph(self) -> None:
        text = STATA.read_text(encoding="utf-8")
        self.assertIn("local coefficient_capacity =", text)
        self.assertIn("local coefficient_count = r(coefficients)", text)
        self.assertIn("fvexpand `indepvars' if `sample_shadow'", text)
        self.assertIn('ereturn local fereg_result_wire "1.1"', text)

    def test_licensed_rebase_fixture_is_in_every_qualification_profile(self) -> None:
        import json

        relative = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_categorical_final_sample_rebase.do"
        )
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn(relative, profiles[name]["fixtures"])


if __name__ == "__main__":
    unittest.main()
