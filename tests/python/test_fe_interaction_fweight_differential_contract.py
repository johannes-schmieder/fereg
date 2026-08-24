from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
CASE_ID = "fweight_frequency_cascade"
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
FIXTURE = (
    ROOT
    / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_fweight_parity.do"
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


class FeInteractionFweightDifferentialContractTests(unittest.TestCase):
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
                "cmdline": "reghdfe y c.x [fw=fw], absorb(worker firm#year) residuals(__stfx_probe_resid)",
                "vce": "ols",
                "wexp": "= fw",
                "wtype": "fweight",
            },
        )

    def test_frequency_weight_oracle_is_fully_pinned(self) -> None:
        self.assertEqual(
            self.case["scalars"],
            {
                "N": "+1.a000000000000X+003",
                "N_full": "+1.a000000000000X+003",
                "N_hdfe": "+1.0000000000000X+001",
                "df_a": "+1.c000000000000X+002",
                "df_m": "+1.0000000000000X+000",
                "df_r": "+1.4000000000000X+002",
                "num_singletons": "+0.0000000000000X-3ff",
                "sumweights": "+1.a000000000000X+003",
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
                "x": "+1.576510c34bacfX+000",
                "_cons": "+1.a01158a444c63X+002",
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
                (1, 1): "+1.de732871f9384X-006",
                (1, 2): "-1.4b3c084ee79d1X-007",
                (2, 1): "-1.4b3c084ee79d1X-007",
                (2, 2): "+1.2dae85efd719dX-008",
            },
        )
        dof_entries = {
            (entry["row"], entry["column"]): (
                entry["row_name"], entry["value_hex"]
            )
            for entry in matrices
            if entry["name"] == "dof_table"
        }
        self.assertEqual(dof_entries[(1, 1)], ("1.worker", "+1.0000000000000X+002"))
        self.assertEqual(dof_entries[(1, 2)][1], "+0.0000000000000X-3ff")
        self.assertEqual(dof_entries[(1, 3)][1], "+1.0000000000000X+002")
        self.assertEqual(
            dof_entries[(2, 1)],
            ("1.firm#1.year", "+1.0000000000000X+002"),
        )
        self.assertEqual(dof_entries[(2, 2)][1], "+1.0000000000000X+000")
        self.assertEqual(dof_entries[(2, 3)][1], "+1.8000000000000X+001")

        observations = self.case["observations"]
        sample_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): round(
                _stata_hex_number(entry["value_hex"])
            )
            for entry in observations
            if entry["field"] == "sample"
        }
        self.assertEqual(sample_by_id, {obs_id: 1 for obs_id in range(101, 113)})
        residual_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): entry["value_hex"]
            for entry in observations
            if entry["field"] == "residual"
        }
        self.assertEqual(
            residual_by_id,
            {
                101: "-1.2e93f80ed79c5X-006",
                102: "+1.49660abdc325bX-003",
                103: "-1.f264f7c3d9567X-005",
                104: "-1.4df49b95e3adaX-004",
                105: "+1.70b7d698ea7ecX-008",
                106: "-1.2f194a70af81fX-003",
                107: "+1.024f9d922ddb0X-005",
                108: "+1.c5ff48aeb9877X-004",
                109: "+1.7be70c648845bX-032",
                110: "+1.8d3021e9c959fX-033",
                111: "-1.bf35d236b16dbX-033",
                112: "+1.a37e7531f95dbX-032",
            },
        )

    def test_fixture_binds_weight_sample_and_numerical_oracles(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        for token in (
            SOURCE_SHA,
            PAYLOAD_SHA256,
            CASE_ID,
            "fweight(fw)",
            "e(wtype)",
            '"fweight"',
            "e(wexp)",
            '"= fw"',
            "e(N) == 13",
            "e(N_full) == 13",
            "e(num_singletons) == 0",
            "e(df_a) == 7",
            "e(df_m) == 1",
            "e(df_r) == 5",
            "e(sumweights) == 13",
            "x _cons",
            "e(stfixest_status)[1,2] == 1",
            "e(stfixest_status)[2,2] == 5",
            "+1.576510c34bacfX+000",
            "+1.a01158a444c63X+002",
            "+1.de732871f9384X-006",
            "1.worker 1.firm#1.year",
            "END_FIXTURE|estimate_ols_categorical_interaction_fweight_parity|rc=0",
        ):
            self.assertIn(token, text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_fixture_is_registered_in_qualification_profiles(self) -> None:
        relative = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_categorical_interaction_fweight_parity.do"
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
