from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
FIXTURE = ROOT / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_parity.do"
PROFILES = ROOT / "ci/stata_profiles.json"
README = ROOT / "tests/stata/parity_reghdfe/README.md"
CASE_ID = "target_default_firm_year"


def _canonical_payload_sha256(payload: object) -> str:
    encoded = (
        json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        )
        + "\n"
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _stata_hex_number(value: str) -> float:
    mantissa, exponent = value.split("X", 1)
    if mantissa.lstrip("+-").startswith("0."):
        return 0.0
    if any(character in exponent.lower() for character in "abcdef"):
        raise ValueError(f"special Stata hex value is not finite: {value}")
    return float.fromhex(f"{mantissa}p{int(exponent)}")


class FeInteractionDifferentialContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = json.loads(RECEIPT.read_text(encoding="utf-8"))
        cls.payload = cls.document["payload"]
        cls.case = next(
            case for case in cls.payload["cases"] if case["id"] == CASE_ID
        )

    def test_oracle_is_successful_source_bound_and_sealed(self) -> None:
        self.assertEqual(self.payload["status"], "success")
        self.assertEqual(self.payload["source_sha"], SOURCE_SHA)
        self.assertFalse(self.payload["raw_capture_committed"])
        self.assertEqual(
            self.document["seal"],
            {
                "algorithm": "sha256",
                "canonicalization": "json-sort-keys-compact-ascii-v1",
                "payload_sha256": PAYLOAD_SHA256,
            },
        )
        self.assertEqual(_canonical_payload_sha256(self.payload), PAYLOAD_SHA256)
        self.assertEqual(
            self.payload["references"]["reghdfe"]["commit"],
            "29b2b203f534413369d675d0c0f674d61291f536",
        )
        self.assertEqual(
            self.payload["references"]["ftools"]["commit"],
            "d6e24d66fd02ba1b37325c82c5040444c5b4c636",
        )
        self.assertEqual(
            self.payload["references"]["require"]["commit"],
            "f4760b86e55cf73b82041ce59cf20e8e8224afb0",
        )

    def test_default_case_matches_the_committed_oracle(self) -> None:
        self.assertEqual(self.case["return_code"], 0)
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
                "_cons": "+1.b99999c000000X+002",
            },
        )
        const_variance = next(
            entry["value_hex"]
            for entry in matrices
            if entry["name"] == "V" and entry["row"] == 2 and entry["column"] == 2
        )
        self.assertEqual(const_variance, "+1.47ae1eb853fc5X-00a")
        self.assertEqual(
            self.case["scalars"],
            {
                "N": "+1.0000000000000X+003",
                "N_full": "+1.8000000000000X+004",
                "N_hdfe": "+1.0000000000000X+001",
                "df_a": "+1.8000000000000X+002",
                "df_m": "+0.0000000000000X-3ff",
                "df_r": "+1.0000000000000X+001",
                "num_singletons": "+1.0000000000000X+004",
                "sumweights": "+1.0000000000000X+003",
            },
        )

        observations = self.case["observations"]
        sample_by_id = {
            round(_stata_hex_number(entry["obs_id_hex"])): round(
                _stata_hex_number(entry["value_hex"])
            )
            for entry in observations
            if entry["field"] == "sample"
        }
        self.assertEqual(
            [obs_id for obs_id, included in sample_by_id.items() if included],
            [3, 4, 11, 12, 15, 16, 23, 24],
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
                3: "-1.99997fffffffdX-005",
                4: "+1.99997fffffffdX-005",
                11: "+1.99997fffffffdX-005",
                12: "-1.99997fffffffdX-005",
                15: "+1.9999bfffffff3X-005",
                16: "-1.9999bfffffff3X-005",
                23: "-1.9999bffffff7fX-005",
                24: "+1.9999bffffff7fX-005",
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
        self.assertEqual(dof_entries[(1, 1)], ("1.worker", "+1.0000000000000X+002"))
        self.assertEqual(
            dof_entries[(2, 1)],
            ("1.firm#1.year", "+1.0000000000000X+002"),
        )
        self.assertEqual(dof_entries[(2, 2)][1], "+1.0000000000000X+001")
        self.assertEqual(dof_entries[(2, 3)][1], "+1.0000000000000X+001")

    def test_fixture_is_explicitly_bound_to_the_oracle(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        for token in (
            SOURCE_SHA,
            PAYLOAD_SHA256,
            CASE_ID,
            "o.x1 _cons",
            "e(stfixest_status)[1,2] == 3",
            "e(stfixest_status)[2,2] == 5",
            "+1.b99999c000000X+002",
            "+1.47ae1eb853fc5X-00a",
            "inlist(obs_id, 3, 4, 11, 12, 15, 16, 23, 24)",
            "1.worker 1.firm#1.year",
            "END_FIXTURE|estimate_ols_categorical_interaction_parity|rc=0",
        ):
            self.assertIn(token, text)
        self.assertIn("_stfixest_estimate_ols_test", text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_fixture_is_in_quick_differential_and_full_profiles(self) -> None:
        relative = "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_parity.do"
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn(relative, profiles[name]["fixtures"])

    def test_readme_preserves_the_fail_closed_boundary(self) -> None:
        text = README.read_text(encoding="utf-8")
        self.assertIn(SOURCE_SHA, text)
        self.assertIn(PAYLOAD_SHA256, text)
        self.assertIn("stfeols` remains disabled", text)
        self.assertIn("does not invoke `reghdfe`", text)
        self.assertIn("does not close `multi_fe_interactions`", text)


if __name__ == "__main__":
    unittest.main()
