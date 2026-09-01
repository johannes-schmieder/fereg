from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.feppml_public_state import ACTIVATED, public_state


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ROOT / "ci" / "stata_profiles.json"
WORKFLOW = ROOT / ".github" / "workflows" / "stata-ci.yml"
PPML_FIXTURE = "tests/stata/parity_ppmlhdfe/feppml_fail_closed.do"
HIDDEN_TRANSACTION = (
    "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do"
)
HIDDEN_RUNTIME = (
    "tests/stata/parity_ppmlhdfe/estimate_ppml_runtime_validator.do"
)
PUBLIC_STAGED_INSTALL = "tests/stata/installation/feppml_installation_smoke.do"
TRANSACTION_FIXTURE = (
    "tests/stata/installation/estimate_ols_eclass_transaction.do"
)
PUBLIC_PPML_FIXTURES = [
    "tests/stata/parity_ppmlhdfe/feppml_command_contract.do",
    "tests/stata/parity_ppmlhdfe/feppml_saved_slopes.do",
    "tests/stata/parity_ppmlhdfe/feppml_scientific_contract.do",
    "tests/stata/parity_ppmlhdfe/feppml_separation_methods.do",
    "tests/stata/parity_ppmlhdfe/feppml_separation_outputs.do",
    "tests/stata/parity_ppmlhdfe/feppml_time_series.do",
    "tests/stata/parity_ppmlhdfe/feppml_weights_inference.do",
]
PUBLIC_HELP_FIXTURE = "tests/stata/documentation/feppml_help_examples.do"


def load_profiles() -> dict[str, dict[str, object]]:
    return json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]


class StataProfileSeparationContractTests(unittest.TestCase):
    def test_licensed_runner_is_manual_and_defaults_to_smoke(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("  workflow_dispatch:", workflow)
        self.assertNotIn("  push:", workflow)
        self.assertIn("        default: smoke", workflow)
        self.assertNotIn("inputs.profile ||", workflow)
        reference = (
            ROOT / ".github/workflows/fe-interaction-reference-probe.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("  workflow_dispatch:", reference)
        self.assertNotIn("  push:", reference)

    def test_scc_development_profiles_are_small_and_boundary_explicit(self) -> None:
        profiles = load_profiles()
        expected = {
            "scc-dev-ols": ["ci/fereg_smoke.do"],
            "scc-dev-ppml": ["ci/feppml_smoke.do"],
            "scc-dev-shared": ["ci/fereg_smoke.do", "ci/feppml_smoke.do"],
        }
        for name, fixtures in expected.items():
            profile = profiles[name]
            self.assertTrue(profile["build_plugin"])
            self.assertEqual(profile["fixtures"], fixtures)
            self.assertEqual(profile["timeout_seconds"], 1800)
        ppml_smoke = (ROOT / "ci/feppml_smoke.do").read_text(encoding="utf-8")
        self.assertIn("threads(4)", ppml_smoke)
        self.assertIn("END_FIXTURE|feppml_smoke|rc=0", ppml_smoke)

    def test_fereg_regression_is_a_complete_ols_only_verdict(self) -> None:
        profiles = load_profiles()
        regression = profiles["fereg-regression"]
        full = profiles["full"]
        fixtures = regression["fixtures"]

        self.assertEqual(regression["driver"], "fereg-fixtures")
        self.assertTrue(regression["build_plugin"])
        self.assertTrue(regression["run_break_fixture"])
        self.assertIn(TRANSACTION_FIXTURE, fixtures)
        self.assertNotIn(PPML_FIXTURE, fixtures)
        self.assertTrue(set(fixtures).issubset(set(full["fixtures"])))

        for relative in fixtures:
            path = ROOT / relative
            self.assertTrue(path.is_file(), relative)
            self.assertNotIn("parity_ppmlhdfe", relative)
            self.assertNotIn("feppml", path.read_text(encoding="utf-8").lower())

        for relative in full["fixtures"]:
            text = (ROOT / relative).read_text(encoding="utf-8").lower()
            if "parity_ppmlhdfe" not in relative and "feppml" not in text:
                self.assertIn(relative, fixtures)

    def test_public_ppml_profile_matches_public_state(self) -> None:
        profiles = load_profiles()
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("- fereg-regression", workflow)
        if public_state(ROOT) == ACTIVATED:
            self.assertNotIn("ppml-fail-closed", profiles)
            ppml = profiles["ppml-scientific"]
            self.assertEqual(ppml["driver"], "fereg-fixtures")
            self.assertTrue(ppml["build_plugin"])
            self.assertEqual(ppml["fixtures"], PUBLIC_PPML_FIXTURES)
            self.assertNotIn(PPML_FIXTURE, profiles["quick"]["fixtures"])
            self.assertNotIn(PPML_FIXTURE, profiles["full"]["fixtures"])
            self.assertTrue(
                set(PUBLIC_PPML_FIXTURES).issubset(profiles["full"]["fixtures"])
            )
            self.assertIn(PUBLIC_HELP_FIXTURE, profiles["full"]["fixtures"])
            self.assertIn("- ppml-scientific", workflow)
            self.assertNotIn("- ppml-fail-closed", workflow)
        else:
            ppml = profiles["ppml-fail-closed"]
            self.assertEqual(ppml["driver"], "fereg-fixtures")
            self.assertFalse(ppml["build_plugin"])
            self.assertEqual(ppml["fixtures"], [PPML_FIXTURE])
            self.assertEqual(ppml["required_outputs"], [])
            for name in ("quick", "full"):
                self.assertIn(PPML_FIXTURE, profiles[name]["fixtures"])
            self.assertIn("- ppml-fail-closed", workflow)
            self.assertNotIn("- ppml-scientific", workflow)

    def test_hidden_ppml_profile_is_preserved_across_public_states(self) -> None:
        profiles = load_profiles()
        hidden = profiles["ppml-hidden"]
        self.assertTrue(hidden["build_plugin"])
        self.assertTrue(hidden["run_ppml_break_fixture"])
        self.assertIn(HIDDEN_TRANSACTION, hidden["fixtures"])
        self.assertIn(HIDDEN_RUNTIME, hidden["fixtures"])
        self.assertTrue(set(hidden["fixtures"]).issubset(profiles["full"]["fixtures"]))
        self.assertTrue(profiles["full"]["run_ppml_break_fixture"])

        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("- ppml-hidden", workflow)
        if public_state(ROOT) == ACTIVATED:
            self.assertIn("ppml-scientific", profiles)
            self.assertIn("- ppml-scientific", workflow)
        else:
            self.assertNotIn("ppml-scientific", profiles)
            self.assertNotIn("- ppml-scientific", workflow)

        for name, profile in profiles.items():
            self.assertNotIn(
                PUBLIC_STAGED_INSTALL,
                profile.get("fixtures", []),
                f"{name} must not schedule the public staged install before activation",
            )

    def test_ppml_fixture_proves_failures_are_atomic(self) -> None:
        text = (ROOT / PPML_FIXTURE).read_text(encoding="utf-8")
        for required in (
            "quietly regress y x",
            "assert _rc == 498",
            "assert _rc == 198",
            "assert _N == `before_n'",
            "assert c(k) == `before_k'",
            "assert mreldif(e(b), `before_b') == 0",
            "assert mreldif(e(V), `before_v') == 0",
            "confirm new variable __ppml_resid",
            "confirm new variable __ppml_fe",
            "datasignature",
            'END_FIXTURE|feppml_fail_closed|rc=0',
        ):
            self.assertIn(required, text)


if __name__ == "__main__":
    unittest.main()
