from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "0e2aa609f9cbb17f8a205261d80680e68a752cef"
PAYLOAD_SHA256 = "fab875a998edc8f91bbeebfca8ca8ef92c09f43f6044fbbcef0b1e7da8fcf387"
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{SOURCE_SHA}.json"
STATA_RECEIPT = ROOT / ".ci/stata/full-results" / f"{SOURCE_SHA}.json"
GATE_RECEIPT = (
    ROOT / ".ci/staged-validation/final-sample-rebase" / f"{SOURCE_SHA}.json"
)
FIXTURE = (
    ROOT
    / "tests/stata/parity_reghdfe/estimate_ols_categorical_final_sample_rebase.do"
)
PROFILES = ROOT / "ci/stata_profiles.json"
FACTOR_CASE = "final_sample_default_factor_rebase"
INTERACTION_CASE = "final_sample_default_interaction_rebase"


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
    return float.fromhex(f"{mantissa}p{int(exponent)}")


class FinalSampleRebaseDifferentialContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = json.loads(RECEIPT.read_text(encoding="utf-8"))
        cls.payload = cls.document["payload"]
        cls.cases = {case["id"]: case for case in cls.payload["cases"]}

    def test_exact_source_reference_receipt_is_successful_and_sealed(self) -> None:
        self.assertEqual(self.payload["status"], "success")
        self.assertEqual(self.payload["source_sha"], SOURCE_SHA)
        self.assertFalse(self.payload["raw_capture_committed"])
        self.assertEqual(_canonical_payload_sha256(self.payload), PAYLOAD_SHA256)
        self.assertEqual(
            self.document["seal"],
            {
                "algorithm": "sha256",
                "canonicalization": "json-sort-keys-compact-ascii-v1",
                "payload_sha256": PAYLOAD_SHA256,
            },
        )
        self.assertEqual(
            self.payload["references"]["reghdfe"]["commit"],
            "29b2b203f534413369d675d0c0f674d61291f536",
        )
        self.assertEqual(
            list(self.cases)[-2:],
            [FACTOR_CASE, INTERACTION_CASE],
        )

    def test_factor_and_interaction_oracles_are_exact(self) -> None:
        expected = {
            FACTOR_CASE: {
                "x": "+1.1c5bec29a16c4X+000",
                "2b.f": "+0.0000000000000X-3ff",
                "3.f": "+1.2b33c240ae588X+000",
                "_cons": "-1.aa4b4c5891d8bX-002",
            },
            INTERACTION_CASE: {
                "x": "+1.666666666666cX+000",
                "2b.f": "+0.0000000000000X-3ff",
                "3.f": "+1.6666666666658X-001",
                "1b.z": "+0.0000000000000X-3ff",
                "2.z": "-1.3333333333346X-002",
                "2b.f#1b.z": "+0.0000000000000X-3ff",
                "2b.f#2o.z": "+0.0000000000000X-3ff",
                "3o.f#1b.z": "+0.0000000000000X-3ff",
                "3.f#2.z": "+1.999999999999bX-003",
                "_cons": "-1.999999999993dX-004",
            },
        }
        for case_id, expected_coefficients in expected.items():
            case = self.cases[case_id]
            self.assertEqual(case["return_code"], 0)
            coefficients = {
                entry["column_name"]: entry["value_hex"]
                for entry in case["matrices"]
                if entry["name"] == "b"
            }
            self.assertEqual(coefficients, expected_coefficients)
            sample = [
                round(_stata_hex_number(entry["obs_id_hex"]))
                for entry in case["observations"]
                if entry["field"] == "sample"
                and round(_stata_hex_number(entry["value_hex"])) == 1
            ]
            self.assertEqual(sample, list(range(2, 10)))
            self.assertEqual(case["scalars"]["N"], "+1.0000000000000X+003")
            self.assertEqual(
                case["scalars"]["num_singletons"],
                "+1.0000000000000X+001",
            )

    def test_fixture_consumes_the_exact_oracles_and_failure_contract(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        for token in (
            "x 2b.f 3.f _cons",
            "x 2b.f 3.f 1b.z 2.z 2b.f#1b.z 2b.f#2o.z 3o.f#1b.z 3.f#2.z _cons",
            "+1.1c5bec29a16c4X+000",
            "+1.2b33c240ae588X+000",
            "+1.666666666666cX+000",
            "+1.999999999999bX-003",
            'foreach specification in "ib1.f" "ib3.f" "ibn.f"',
            "assert _rc == 498",
            "assert mreldif(e(b), `accepted_b') == 0",
            "assert mreldif(e(V), `accepted_V') == 0",
            "END_FIXTURE|estimate_ols_categorical_final_sample_rebase|rc=0",
        ):
            self.assertIn(token, text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_full_licensed_receipt_is_exact_green_and_redacted(self) -> None:
        receipt = json.loads(STATA_RECEIPT.read_text(encoding="utf-8"))
        self.assertEqual(receipt["tested_sha"], SOURCE_SHA)
        self.assertEqual(receipt["status"], "success")
        self.assertEqual(receipt["stata_status"], "success")
        self.assertEqual(receipt["rust_status"], "success")
        self.assertEqual(receipt["tests_passed"], 25)
        self.assertEqual(receipt["tests_failed"], 0)
        self.assertNotIn("stata_executable", receipt)
        self.assertNotIn("runner_name", receipt)
        self.assertEqual(
            receipt["privacy"],
            {
                "local_paths_committed": False,
                "raw_logs_committed": False,
                "license_identity_committed": False,
            },
        )

    def test_fixture_is_in_all_licensed_qualification_profiles(self) -> None:
        relative = (
            "tests/stata/parity_reghdfe/"
            "estimate_ols_categorical_final_sample_rebase.do"
        )
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn(relative, profiles[name]["fixtures"])

    def test_acceptance_receipt_binds_exact_evidence_without_public_activation(self) -> None:
        document = json.loads(GATE_RECEIPT.read_text(encoding="utf-8"))
        payload = document["payload"]
        encoded = json.dumps(
            payload,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        ).encode("utf-8")
        self.assertEqual(
            hashlib.sha256(encoded).hexdigest(), document["payload_sha256"]
        )
        self.assertEqual(payload["source_sha"], SOURCE_SHA)
        self.assertEqual(payload["status"], "success")
        self.assertTrue(payload["activation_committed"])
        self.assertFalse(payload["public_activation"])
        self.assertFalse(payload["performance_claim"])
        for path_key, digest_key in (
            ("licensed_stata_full_receipt", "licensed_stata_full_receipt_sha256"),
            ("pinned_reference_receipt", "pinned_reference_receipt_sha256"),
        ):
            evidence_path = ROOT / payload["evidence"][path_key]
            self.assertEqual(
                hashlib.sha256(evidence_path.read_bytes()).hexdigest(),
                payload["evidence"][digest_key],
            )


if __name__ == "__main__":
    unittest.main()
