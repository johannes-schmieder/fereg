from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
CASE_ID = "target_keepsingletons"
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
FIXTURE = (
    ROOT
    / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_keepsingletons_parity.do"
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


class FeInteractionKeepsingletonsDifferentialContractTests(unittest.TestCase):
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
            "reghdfe y c.x1, absorb(worker firm#year) keepsingletons "
            "residuals(__stfx_probe_resid)",
        )

    def test_keepsingletons_oracle_is_fully_pinned(self) -> None:
        self.assertEqual(
            self.case["scalars"],
            {
                "N": "+1.8000000000000X+004",
                "N_full": "+1.8000000000000X+004",
                "N_hdfe": "+1.0000000000000X+001",
                "df_a": "+1.6000000000000X+004",
                "df_m": "+0.0000000000000X-3ff",
                "df_r": "+1.0000000000000X+001",
                "num_singletons": "+0.0000000000000X-3ff",
                "sumweights": "+1.8000000000000X+004",
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
                "o.x1": "+0.0000000000000X-3ff",
                "_cons": "+1.9eaaaaa000000X+002",
            },
        )
        const_variance = next(
            entry["value_hex"]
            for entry in matrices
            if entry["name"] == "V" and entry["row"] == 2 and entry["column"] == 2
        )
        self.assertEqual(const_variance, "+1.b4e828f5c551eX-00c")

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
            ("1.worker", "+1.8000000000000X+003"),
        )
        self.assertEqual(dof_entries[(1, 2)][1], "+0.0000000000000X-3ff")
        self.assertEqual(dof_entries[(1, 3)][1], "+1.8000000000000X+003")
        self.assertEqual(
            dof_entries[(2, 1)],
            ("1.firm#1.year", "+1.4000000000000X+004"),
        )
        self.assertEqual(dof_entries[(2, 2)][1], "+1.4000000000000X+003")
        self.assertEqual(dof_entries[(2, 3)][1], "+1.4000000000000X+003")

        observations = self.case["observations"]
        sample_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): round(
                _stata_hex_number(entry["value_hex"])
            )
            for entry in observations
            if entry["field"] == "sample"
        }
        self.assertEqual(sample_by_id, {obs_id: 1 for obs_id in range(1, 25)})

        residual_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): entry["value_hex"]
            for entry in observations
            if entry["field"] == "residual"
        }
        zero = "+0.0000000000000X-3ff"
        expected = {obs_id: zero for obs_id in range(1, 25)}
        expected.update(
            {
                3: "-1.99997ffffffb6X-005",
                4: "+1.99997ffffffb6X-005",
                11: "+1.99997ffffffb6X-005",
                12: "-1.99997ffffffb6X-005",
                15: "+1.9999bffffffe7X-005",
                16: "-1.9999bffffffe7X-005",
                23: "-1.9999c00000047X-005",
                24: "+1.9999c00000047X-005",
            }
        )
        self.assertEqual(residual_by_id, expected)

    def test_fixture_is_bound_to_every_keepsingletons_oracle_family(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        for token in (
            SOURCE_SHA,
            PAYLOAD_SHA256,
            CASE_ID,
            "keepsingletons residual(__frg_reference_resid)",
            "e(N) == 24",
            "e(num_singletons) == 0",
            "e(drop_singletons) == 0",
            "e(df_a) == 22",
            "e(fereg_status)[1,2] == 3",
            "e(fereg_status)[2,2] == 5",
            "+1.9eaaaaa000000X+002",
            "+1.b4e828f5c551eX-00c",
            "1.worker 1.firm#1.year",
            "END_FIXTURE|estimate_ols_categorical_interaction_keepsingletons_parity|rc=0",
        ):
            self.assertIn(token, text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_fixture_is_registered_in_qualification_profiles(self) -> None:
        relative = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_categorical_interaction_keepsingletons_parity.do"
        )
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn(relative, profiles[name]["fixtures"])

    def test_readme_keeps_the_gate_fail_closed(self) -> None:
        text = README.read_text(encoding="utf-8")
        self.assertIn(CASE_ID, text)
        self.assertIn("does not close `multi_fe_interactions`", text)
        self.assertIn("does not invoke `reghdfe`", text)


if __name__ == "__main__":
    unittest.main()
