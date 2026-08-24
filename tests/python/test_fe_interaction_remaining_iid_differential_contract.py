from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
CASE_IDS = (
    "missing_interaction_components",
    "repeated_identical_interaction",
    "interaction_order_year_firm",
    "dof_none",
    "dof_firstpair",
)
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
FIXTURE = (
    ROOT
    / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_remaining_iid_parity.do"
)
PROFILES = ROOT / "ci/stata_profiles.json"
README = ROOT / "tests/stata/parity_reghdfe/README.md"


class FeInteractionRemainingIidDifferentialContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = json.loads(RECEIPT.read_text(encoding="utf-8"))
        cls.payload = cls.document["payload"]
        cls.cases = {case["id"]: case for case in cls.payload["cases"]}

    def test_cases_are_from_the_accepted_sealed_receipt(self) -> None:
        self.assertEqual(self.payload["status"], "success")
        self.assertEqual(self.payload["source_sha"], SOURCE_SHA)
        self.assertEqual(self.document["seal"]["payload_sha256"], PAYLOAD_SHA256)
        self.assertFalse(self.payload["raw_capture_committed"])
        self.assertEqual(set(CASE_IDS), set(CASE_IDS) & self.cases.keys())
        for case_id in CASE_IDS:
            self.assertEqual(self.cases[case_id]["return_code"], 0)

    def test_fixture_binds_every_oracle_family(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        self.assertIn(SOURCE_SHA, text)
        self.assertIn(PAYLOAD_SHA256, text)
        for case_id in CASE_IDS:
            case = self.cases[case_id]
            self.assertIn(case_id, text)
            for name, value in case["scalars"].items():
                self.assertIn(f"e({name})", text)
                self.assertIn(value, text)
            for name, value in case["macros"].items():
                if name != "cmdline":
                    self.assertIn(value, text)
            for entry in case["matrices"]:
                self.assertIn(entry["value_hex"], text)
            for entry in case["observations"]:
                if entry["field"] == "residual" and entry["value_hex"] != "+1.0000000000000X+3ff":
                    self.assertIn(entry["value_hex"], text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_committed_fixtures_and_receipt_retain_complete_contract(self) -> None:
        target = (
            ROOT
            / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_parity.do"
        )
        cascade = (
            ROOT
            / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_cascade_parity.do"
        )
        target_text = target.read_text(encoding="utf-8")
        cascade_text = cascade.read_text(encoding="utf-8")
        fixture_text = FIXTURE.read_text(encoding="utf-8")
        self.assertIn(
            'import delimited using `"`root\'/tests/fixtures/ols/ols_core.csv"\', clear',
            target_text,
        )
        self.assertIn("set obs 12", cascade_text)
        self.assertIn("generate long obs_id = 100 + _n", cascade_text)
        self.assertIn("generate byte __stfx_expected_sample", cascade_text)
        self.assertIn("generate double __stfx_expected_resid", cascade_text)
        self.assertIn("Categories Redundant Num Coefs Inexact? Nested?", fixture_text)
        self.assertIn("assert colsof(__stfx_S) == 4", fixture_text)
        self.assertEqual(
            self.cases["missing_interaction_components"]["return_code"], 0
        )

    def test_fixture_is_registered_in_all_qualification_profiles(self) -> None:
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn(
                "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_remaining_iid_parity.do",
                profiles[name]["fixtures"],
            )

    def test_readme_keeps_the_gate_fail_closed(self) -> None:
        text = README.read_text(encoding="utf-8")
        for case_id in CASE_IDS:
            self.assertIn(case_id, text)
        self.assertIn("does not invoke `reghdfe`", text)
        self.assertIn("does not close `multi_fe_interactions`", text)


if __name__ == "__main__":
    unittest.main()
