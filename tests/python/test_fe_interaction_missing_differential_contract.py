from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
CASE_ID = "missing_interaction_components"
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
FIXTURE = (
    ROOT
    / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_missing_parity.do"
)
PROFILES = ROOT / "ci/stata_profiles.json"
README = ROOT / "tests/stata/parity_reghdfe/README.md"


def _stata_hex_number(value: str) -> float:
    mantissa, exponent = value.split("X", 1)
    if mantissa.lstrip("+-").startswith("0."):
        return 0.0
    if any(character in exponent.lower() for character in "abcdef"):
        raise ValueError(f"special Stata hex value is not finite: {value}")
    return float.fromhex(f"{mantissa}p{int(exponent)}")


class FeInteractionMissingDifferentialContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = json.loads(RECEIPT.read_text(encoding="utf-8"))
        cls.payload = cls.document["payload"]
        cls.case = next(
            case for case in cls.payload["cases"] if case["id"] == CASE_ID
        )

    def test_case_is_from_the_accepted_sealed_receipt(self) -> None:
        self.assertEqual(self.payload["status"], "success")
        self.assertEqual(self.payload["source_sha"], SOURCE_SHA)
        self.assertEqual(self.document["seal"]["payload_sha256"], PAYLOAD_SHA256)
        self.assertFalse(self.payload["raw_capture_committed"])
        self.assertEqual(self.case["return_code"], 0)
        self.assertEqual(
            self.case["macros"],
            {
                "absvars": "worker firm#year",
                "cmdline": "reghdfe y c.x, absorb(worker firm#year) keepsingletons residuals(__stfx_probe_resid)",
                "vce": "ols",
            },
        )

    def test_missing_component_oracle_is_fully_pinned(self) -> None:
        self.assertEqual(
            self.case["scalars"],
            {
                "N": "+1.2000000000000X+003",
                "N_full": "+1.2000000000000X+003",
                "N_hdfe": "+1.0000000000000X+001",
                "df_a": "+1.0000000000000X+002",
                "df_m": "+1.0000000000000X+000",
                "df_r": "+1.0000000000000X+002",
                "num_singletons": "+0.0000000000000X-3ff",
                "sumweights": "+1.2000000000000X+003",
            },
        )
        matrices = self.case["matrices"]
        coefficients = {
            entry["column_name"]: entry["value_hex"]
            for entry in matrices
            if entry["name"] == "b"
        }
        self.assertEqual(
            coefficients,
            {
                "x": "+1.8ccccccccccd1X+000",
                "_cons": "+1.47db97530eca8X+002",
            },
        )
        covariance = {
            (entry["row"], entry["column"]): entry["value_hex"]
            for entry in matrices
            if entry["name"] == "V"
        }
        self.assertEqual(
            covariance,
            {
                (1, 1): "+1.47ae147ae1493X-009",
                (1, 2): "-1.64ce9ed572779X-00a",
                (2, 1): "-1.64ce9ed572779X-00a",
                (2, 2): "+1.cd5750cad6873X-00b",
            },
        )
        dof_entries = {
            (entry["row"], entry["column"]): (
                entry["row_name"], entry["value_hex"]
            )
            for entry in matrices
            if entry["name"] == "dof_table"
        }
        self.assertEqual(dof_entries[(1, 1)], ("1.worker", "+1.8000000000000X+001"))
        self.assertEqual(dof_entries[(1, 2)][1], "+0.0000000000000X-3ff")
        self.assertEqual(dof_entries[(1, 3)][1], "+1.8000000000000X+001")
        self.assertEqual(
            dof_entries[(2, 1)],
            ("1.firm#1.year", "+1.8000000000000X+001"),
        )
        self.assertEqual(dof_entries[(2, 2)][1], "+1.0000000000000X+001")
        self.assertEqual(dof_entries[(2, 3)][1], "+1.0000000000000X+000")

        observations = self.case["observations"]
        sample_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): round(
                _stata_hex_number(entry["value_hex"])
            )
            for entry in observations
            if entry["field"] == "sample"
        }
        self.assertEqual(
            sample_by_id,
            {
                201: 1,
                202: 1,
                203: 1,
                204: 1,
                205: 1,
                206: 1,
                207: 1,
                208: 1,
                209: 0,
                210: 0,
                211: 0,
                212: 1,
            },
        )
        residual_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): entry["value_hex"]
            for entry in observations
            if entry["field"] == "residual"
            and sample_by_id[round(_stata_hex_number(entry["obs_id_hex"]))]
        }
        self.assertEqual(
            residual_by_id,
            {
                201: "+1.2f98fe2d949feX-033",
                202: "+1.9999999999938X-006",
                203: "-1.9999999999928X-006",
                204: "-1.2f98fe2d949feX-033",
                205: "+1.9999999999928X-006",
                206: "-1.99999999999e9X-005",
                207: "-1.d843527fcabfcX-033",
                208: "+1.9999999999a14X-006",
                212: "+0.0000000000000X-3ff",
            },
        )

    def test_fixture_binds_missingness_sample_and_numerical_oracles(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        for token in (
            SOURCE_SHA,
            PAYLOAD_SHA256,
            CASE_ID,
            "obs_id = 200 + _n",
            "8.40 9.00 . 9.80",
            "10 10 20 20 . 30 30 30",
            "2020 . 2022 2022",
            "e(N) == 9",
            "e(N_full) == 9",
            "e(num_singletons) == 0",
            "e(df_a) == 4",
            "e(df_m) == 1",
            "e(df_r) == 4",
            "x _cons",
            "e(stfixest_status)[1,2] == 1",
            "e(stfixest_status)[2,2] == 5",
            "+1.8ccccccccccd1X+000",
            "+1.47db97530eca8X+002",
            "+1.47ae147ae1493X-009",
            "e(dof_table)[2,2] == 2",
            "e(dof_table)[2,3] == 1",
            "obs_id == 212",
            "1.worker 1.firm#1.year",
            "missing(__stfx_reference_resid) if !__stfx_expected_sample",
            "END_FIXTURE|estimate_ols_categorical_interaction_missing_parity|rc=0",
        ):
            self.assertIn(token, text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_fixture_is_registered_in_qualification_profiles(self) -> None:
        relative = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_categorical_interaction_missing_parity.do"
        )
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn(relative, profiles[name]["fixtures"])

    def test_readme_keeps_the_gate_fail_closed(self) -> None:
        text = README.read_text(encoding="utf-8")
        self.assertIn(CASE_ID, text)
        self.assertIn("does not invoke `reghdfe`", text)
        self.assertIn("does not close `multi_fe_interactions`", text)


if __name__ == "__main__":
    unittest.main()
