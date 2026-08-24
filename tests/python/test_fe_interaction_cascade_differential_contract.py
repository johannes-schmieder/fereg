from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
CASE_ID = "tuple_second_fe_cascade"
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
FIXTURE = (
    ROOT
    / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_cascade_parity.do"
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


class FeInteractionCascadeDifferentialContractTests(unittest.TestCase):
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
        self.assertEqual(
            self.document["seal"]["payload_sha256"],
            PAYLOAD_SHA256,
        )
        self.assertFalse(self.payload["raw_capture_committed"])
        self.assertEqual(self.case["return_code"], 0)
        self.assertEqual(
            self.case["macros"]["cmdline"],
            "reghdfe y c.x, absorb(worker firm#year) residuals(__stfx_probe_resid)",
        )

    def test_cascade_oracle_is_fully_pinned(self) -> None:
        self.assertEqual(
            self.case["scalars"],
            {
                "N": "+1.0000000000000X+003",
                "N_full": "+1.8000000000000X+003",
                "N_hdfe": "+1.0000000000000X+001",
                "df_a": "+1.8000000000000X+001",
                "df_m": "+1.0000000000000X+000",
                "df_r": "+1.0000000000000X+002",
                "num_singletons": "+1.0000000000000X+002",
                "sumweights": "+1.0000000000000X+003",
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
                "x": "+1.576510c34bad4X+000",
                "_cons": "+1.53e9bc7e29351X+002",
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
                (1, 1): "+1.2b07f9473bbddX-005",
                (1, 2): "-1.93b12a202a404X-008",
                (2, 1): "-1.93b12a202a404X-008",
                (2, 2): "+1.ae3b26f81645dX-009",
            },
        )

        dof_entries = {
            (entry["row"], entry["column"]): (
                entry["row_name"],
                entry["value_hex"],
            )
            for entry in matrices
            if entry["name"] == "dof_table"
        }
        self.assertEqual(
            dof_entries[(1, 1)],
            ("1.worker", "+1.0000000000000X+001"),
        )
        self.assertEqual(dof_entries[(1, 2)][1], "+0.0000000000000X-3ff")
        self.assertEqual(dof_entries[(1, 3)][1], "+1.0000000000000X+001")
        self.assertEqual(
            dof_entries[(2, 1)],
            ("1.firm#1.year", "+1.0000000000000X+001"),
        )
        self.assertEqual(dof_entries[(2, 2)][1], "+1.0000000000000X+000")
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
                101: 1,
                102: 1,
                103: 1,
                104: 1,
                105: 1,
                106: 1,
                107: 1,
                108: 1,
                109: 0,
                110: 0,
                111: 0,
                112: 0,
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
                101: "-1.2e93f80ed7b3fX-006",
                102: "+1.49660abdc3223X-003",
                103: "-1.f264f7c3d9580X-005",
                104: "-1.4df49b95e3ae6X-004",
                105: "+1.70b7d698eadb6X-008",
                106: "-1.2f194a70af820X-003",
                107: "+1.024f9d922dc42X-005",
                108: "+1.c5ff48aeb97b1X-004",
            },
        )

    def test_fixture_is_bound_to_every_cascade_oracle_family(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        for token in (
            SOURCE_SHA,
            PAYLOAD_SHA256,
            CASE_ID,
            "obs_id = 100 + _n",
            "e(N) == 8",
            "e(N_full) == 12",
            "e(num_singletons) == 4",
            "e(df_a) == 3",
            "e(df_m) == 1",
            "e(df_r) == 4",
            "x _cons",
            "e(stfixest_status)[1,2] == 1",
            "e(stfixest_status)[2,2] == 5",
            "+1.576510c34bad4X+000",
            "+1.53e9bc7e29351X+002",
            "+1.2b07f9473bbddX-005",
            "obs_id <= 108",
            "1.worker 1.firm#1.year",
            "END_FIXTURE|estimate_ols_categorical_interaction_cascade_parity|rc=0",
        ):
            self.assertIn(token, text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_fixture_is_registered_in_qualification_profiles(self) -> None:
        relative = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_categorical_interaction_cascade_parity.do"
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
