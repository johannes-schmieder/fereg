from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = ROOT / "tests" / "stata" / "parity_ppmlhdfe"
SCIENTIFIC_FIXTURES = (
    "feppml_command_contract.do",
    "feppml_scientific_contract.do",
    "feppml_weights_inference.do",
    "feppml_separation_outputs.do",
    "feppml_separation_methods.do",
    "feppml_saved_slopes.do",
    "feppml_time_series.do",
)
HIDDEN_WRAPPERS = (
    "estimate_ppml_hidden_command_contract.do",
    "estimate_ppml_hidden_scientific_contract.do",
    "estimate_ppml_hidden_weights_inference.do",
    "estimate_ppml_hidden_separation_outputs.do",
    "estimate_ppml_hidden_separation_methods.do",
    "estimate_ppml_hidden_saved_slopes.do",
    "estimate_ppml_hidden_time_series.do",
)


class PpmlCommandScientificMatrixTests(unittest.TestCase):
    def fixture(self, name: str) -> str:
        return (FIXTURE_DIRECTORY / name).read_text(encoding="utf-8")

    def test_fixtures_default_to_public_but_have_one_private_command_hook(self) -> None:
        for name in SCIENTIFIC_FIXTURES:
            with self.subTest(name=name):
                source = self.fixture(name)
                self.assertIn('local feppml_test_command `"$FEREG_PPML_TEST_COMMAND"\'', source)
                self.assertIn("local feppml_hidden_wrapper =", source)
                self.assertIn('local feppml_test_command "feppml"', source)
                self.assertIn("`feppml_test_command' ", source)
                self.assertNotIn("_fereg_estimate_ppml", source)
                self.assertIn(f"END_FIXTURE|{name.removesuffix('.do')}|rc=0", source)

    def test_outcome_sample_rank_and_atomicity_matrix_is_explicit(self) -> None:
        source = self.fixture("feppml_scientific_contract.do")
        required = (
            "0.5 0",
            "exposure(exposure)",
            "offset(offset)",
            "assert _rc == 459",
            "assert _rc == 2001",
            "dropreason(__sample_drop)",
            "__sample_drop == 1",
            "__sample_drop == 2",
            "__sample_drop == 3",
            "__sample_drop == 5",
            "__sample_drop == 6",
            "__sample_drop == 7",
            "e(feppml_status)[2,2] == 6",
            "ib0.factor_group",
            "== 2 local base_slot",
            "ib0.factor_vanish",
            "colsof(e(b)) == 5",
            "e(feppml_status)[2,2] == 3",
            "e(b)[1,2] == 0",
            "[aw=unsupported_weight]",
            "[iw=unsupported_weight]",
            "mreldif(e(b), `accepted_b') == 0",
            "e(feppml_receipt)[1,20] == 16",
            "e(feppml_receipt)[1,33] == 1",
        )
        for fragment in required:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, source)

    def test_weight_and_inference_matrix_is_explicit(self) -> None:
        source = self.fixture("feppml_weights_inference.do")
        required = (
            "summarize fw, meanonly",
            "expand fw",
            "[fw=fw]",
            "[pw=pw]",
            "[pw=pw_scaled]",
            "[pw=pw_huge]",
            "[pw=pw_tiny]",
            "17 * scalar(__pw_raw)",
            "e(log_raw_sumweights)",
            "e(log_weight_multiplier)",
            "e(feppml_objective_scale_code) == 1",
            "capture confirm scalar e(raw_sumweights)",
            "capture confirm scalar e(weight_multiplier)",
            "vce(robust)",
            "forvalues dimensions = 1/5",
            "2^`dimensions' - 1",
            "e(feppml_cluster)",
            "e(feppml_subset)",
            "r(chi2)",
        )
        for fragment in required:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, source)

    def test_separation_prediction_and_saved_fe_matrix_is_explicit(self) -> None:
        source = self.fixture("feppml_separation_outputs.do")
        required = (
            "residualtype(response)",
            "predict double __predict_default",
            "predict double __predict_score, score",
            "predict double __predict_pearson, pearson",
            "predict double __predict_working, working",
            "predict double __predict_deviance, deviance",
            "predict double __predict_anscombe, anscombe",
            "absorb(__saved_fe=fe)",
            "__saved_drop == 9",
            "e(separated) == 6",
            "e(feppml_status)[2,2] == 7",
            "e(b)[1,2] == 0",
            "e(df_resid) == e(N_effective) - e(rank) - e(df_a)",
            "mreldif(e(b), `saved_b')",
            "assert _rc == 110",
            "assert _rc == 430",
            "mreldif(e(V), `before_V') == 0",
        )
        for fragment in required:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, source)

    def test_every_separation_reason_and_method_order_is_explicit(self) -> None:
        source = self.fixture("feppml_separation_methods.do")
        required = (
            "__singleton_drop == 8",
            '"simplex 10 3"',
            '"ir 11 4"',
            '"mu 12 5"',
            "e(feppml_receipt)[1,16] == 6",
            "e(feppml_status)[2,2] == 6",
            "separation(fe simplex ir)",
            "__ordered_drop == 9",
        )
        for fragment in required:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, source)

    def test_ci_preserves_hidden_and_public_ppml_profiles(self) -> None:
        profiles = json.loads((ROOT / "ci" / "stata_profiles.json").read_text(encoding="utf-8"))[
            "profiles"
        ]
        self.assertIn("ppml-scientific", profiles)
        profile = profiles["ppml-hidden"]
        expected_wrappers = [
            f"tests/stata/parity_ppmlhdfe/{name}" for name in HIDDEN_WRAPPERS
        ]
        self.assertTrue(profile["build_plugin"])
        self.assertTrue(profile["run_ppml_break_fixture"])
        self.assertIn(
            "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do",
            profile["fixtures"],
        )
        self.assertIn(
            "tests/stata/parity_ppmlhdfe/estimate_ppml_runtime_validator.do",
            profile["fixtures"],
        )
        self.assertIn(
            "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_allocation.do",
            profile["fixtures"],
        )
        self.assertIn(
            "tests/stata/documentation/estimate_ppml_hidden_help_examples.do",
            profile["fixtures"],
        )
        self.assertTrue(set(expected_wrappers).issubset(profile["fixtures"]))
        self.assertGreaterEqual(profile["timeout_seconds"], 5400)
        for wrapper_name, public_name in zip(HIDDEN_WRAPPERS, SCIENTIFIC_FIXTURES):
            wrapper = self.fixture(wrapper_name)
            self.assertIn('global FEREG_PPML_TEST_COMMAND "_fereg_ppml"', wrapper)
            self.assertIn(public_name, wrapper)

        self.assertTrue(set(profile["fixtures"]).issubset(profiles["full"]["fixtures"]))
        self.assertTrue(profiles["full"]["run_ppml_break_fixture"])
        for name in SCIENTIFIC_FIXTURES:
            relative = f"tests/stata/parity_ppmlhdfe/{name}"
            self.assertIn(relative, profiles["full"]["fixtures"])
        self.assertFalse(
            any("parity_ppmlhdfe" in fixture for fixture in profiles["fereg-regression"]["fixtures"]),
            "the frozen fereg regression boundary must remain PPML-free",
        )

        workflow = (ROOT / ".github" / "workflows" / "stata-ci.yml").read_text(encoding="utf-8")
        self.assertIn("          - ppml-hidden", workflow)
        self.assertIn("          - ppml-scientific", workflow)
        self.assertNotIn("          - ppml-fail-closed", workflow)

    def test_production_config_cannot_inherit_ols_vce_or_dof_policy(self) -> None:
        core = (ROOT / "legacy" / "fereg-core" / "src" / "ppml" / "mod.rs").read_text(
            encoding="utf-8"
        )
        match = re.search(
            r"pub struct PpmlConfig \{(?P<body>.*?)\n\}", core, flags=re.DOTALL
        )
        self.assertIsNotNone(match)
        body = match.group("body")
        self.assertNotIn("VceKind", body)
        self.assertNotIn("DofConfig", body)
        self.assertNotIn("FiniteSolver", body)
        self.assertNotRegex(body, r"\bpub\s+finite_solver\s*:")
        self.assertNotRegex(body, r"\bpub\s+rank_tolerance\s*:")
        self.assertNotRegex(body, r"\bpub\s+vce\s*:")
        self.assertNotRegex(body, r"\bpub\s+dof\s*:")
        for internal in (
            "absorb", "fixed_effect_collinearity_tolerance", "score_tolerance",
            "inner_tolerance_initial", "inner_tolerance_final", "max_step_halvings",
            "minimum_step", "objective_slack", "bounds", "initialization",
        ):
            self.assertNotRegex(body, rf"\bpub\s+{internal}\s*:")
        self.assertIn("pub fn with_outer_controls", core)
        self.assertNotIn("pub enum PpmlInitialization", core)

        crate_root = (ROOT / "legacy/fereg-core/src/lib.rs").read_text(encoding="utf-8")
        for stale_export in ("means_from_eta", "PoissonBounds", "PpmlInitialization"):
            self.assertNotRegex(crate_root, rf"pub use ppml::\{{[^}}]*\b{stale_export}\b")

        driver = (
            ROOT / "legacy" / "fereg-core" / "src" / "ppml" / "prepared_estimator.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("pub vce: PpmlVceRequest", driver)
        self.assertIn("compute_ppml_inference", driver)

    def test_extreme_pweight_and_generation_ledger_contracts_are_source_bound(self) -> None:
        prepared = (ROOT / "legacy/fereg-core/src/ppml/prepared.rs").read_text(
            encoding="utf-8"
        )
        for fragment in (
            "pub raw_weight_sum: Option<f64>",
            "pub log_raw_weight_sum: Option<f64>",
            "pub raw_to_prepared_multiplier: Option<f64>",
            "pub log_raw_to_prepared_multiplier: f64",
            "let scaled_sum = compensated_nonnegative_sum",
            "let log_raw_sum = maximum.ln() + scaled_sum.ln()",
            "extreme_finite_pweights_normalize_even_when_raw_sum_and_multiplier_overflow",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, prepared)

        separation = (
            ROOT / "legacy/fereg-core/src/ppml/prepared_separation.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("hash ^= u64::try_from(original_row)", separation)
        self.assertIn("candidate_ledger_hash_binds_stable_original_rows", separation)
        self.assertIn("positive_maximum: boundary_violation", separation)
        self.assertIn("ir_no_separation_boundary_diagnostic_is_never_negative", separation)


if __name__ == "__main__":
    unittest.main()
