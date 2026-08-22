from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

from scripts.ols_hidden_numeric import (
    BLOCKERS,
    EXECUTABLE_CASE_IDS,
    Q_ANCHORS,
    SEALED_DEFAULT_V_ABS_CAPS,
    _validate_internal_candidate,
    load_projection,
    projected_manifest,
    stata_hex_to_float,
)
from scripts.ols_reference import ContractError


ROOT = Path(__file__).resolve().parents[2]


def internal_q_case(case_id: str) -> dict:
    one = "+1.0000000000000X+000"
    receipt = [
        {
            "name": "stfixest_receipt",
            "row": 1,
            "column": column,
            "row_name": "r1",
            "column_name": f"c{column}",
            "value_hex": (
                "+1.46b5490000000X+005" if column == 1 else one if column == 16 else "+0.0000000000000X-3ff"
            ),
        }
        for column in range(1, 17)
    ]
    certificate = [
        {
            "name": "stfixest_certificate",
            "row": row,
            "column": column,
            "row_name": f"r{row}",
            "column_name": f"c{column}",
            "value_hex": one,
        }
        for row in (1, 2, 3, 24)
        for column in (1, 2)
    ]
    axis = ("x1", "_cons")
    covariance = []
    for value, (row, column) in zip(
        Q_ANCHORS[case_id]["V"], ((1, 1), (1, 2), (2, 1), (2, 2))
    ):
        covariance.append(
            {
                "name": "V",
                "row": row,
                "column": column,
                "row_name": axis[row - 1],
                "column_name": axis[column - 1],
                "value_hex": value,
            }
        )
    return {
        "id": case_id,
        "return_code": 0,
        "matrices": [
            *receipt,
            *certificate,
            {
                "name": "stfixest_status",
                "row": 1,
                "column": 1,
                "row_name": "r1",
                "column_name": "c1",
                "value_hex": one,
            },
            *covariance,
        ],
        "scalars": {"stfixest_v_determinant": Q_ANCHORS[case_id]["determinant"]},
    }


class HiddenNumericProjectionTests(unittest.TestCase):
    def test_projection_exactly_partitions_parent_and_stays_developer_only(self):
        projection = load_projection()
        self.assertEqual(tuple(projection["executable_case_ids"]), EXECUTABLE_CASE_IDS)
        self.assertEqual(
            tuple((item["case_id"], item["reason"]) for item in projection["blockers"]),
            BLOCKERS,
        )
        manifest = projected_manifest(projection)
        self.assertEqual(len(manifest["semantic_cases"]), 30)
        self.assertFalse(manifest["benchmark_cases"])
        for case in manifest["semantic_cases"]:
            self.assertTrue(case["command"].startswith("_stfixest_ols_hidden_numeric_v1 "))
            self.assertNotIn("stfeols ", case["command"])

    def test_projection_mutations_fail_closed(self):
        source = json.loads(
            (ROOT / "benchmarks/manifests/ols_hidden_numeric_v1.json").read_text(encoding="utf-8")
        )
        mutations = []
        public = copy.deepcopy(source)
        public["public_activation"] = True
        mutations.append(public)
        reordered = copy.deepcopy(source)
        reordered["executable_case_ids"][0:2] = reversed(reordered["executable_case_ids"][0:2])
        mutations.append(reordered)
        blocker = copy.deepcopy(source)
        blocker["blockers"][0]["reason"] = "changed"
        mutations.append(blocker)
        tolerance = copy.deepcopy(source)
        tolerance["tolerances"]["vce_atol"] = 1e-4
        mutations.append(tolerance)
        with tempfile.TemporaryDirectory() as raw:
            for index, mutation in enumerate(mutations):
                candidate = Path(raw) / f"projection-{index}.json"
                candidate.write_text(json.dumps(mutation), encoding="utf-8")
                with self.assertRaises(ContractError):
                    load_projection(candidate)

    def test_q2_q3_internal_anchors_are_exact_and_mutation_fatal(self):
        for case_id in ("cluster_twoway", "cluster_threeway"):
            case = internal_q_case(case_id)
            _validate_internal_candidate(case)
            mutated = copy.deepcopy(case)
            next(item for item in mutated["matrices"] if item["name"] == "V")[
                "value_hex"
            ] = "+0.0000000000000X-3ff"
            with self.assertRaisesRegex(ContractError, "exact internal covariance"):
                _validate_internal_candidate(mutated)

    def test_receipt_caps_are_narrow_and_hex_decoder_is_exact(self):
        self.assertEqual(
            SEALED_DEFAULT_V_ABS_CAPS,
            {
                "cluster_oneway": 1e-7,
                "cluster_twoway": 2e-6,
                "cluster_threeway": 4e-6,
                "cluster_interaction": 1e-7,
            },
        )
        self.assertEqual(stata_hex_to_float("+1.0000000000000X+000"), 1.0)
        self.assertEqual(stata_hex_to_float("-1.0000000000000X-001"), -0.5)
        with self.assertRaises(ContractError):
            stata_hex_to_float("1.0")

    def test_hidden_adapter_never_claims_public_command_identity(self):
        adapter = (ROOT / "stata/_stfixest_ols_hidden_numeric_v1.ado").read_text(
            encoding="utf-8"
        )
        self.assertIn('ereturn local cmd "_stfixest_ols_hidden_numeric_v1"', adapter)
        self.assertIn('ereturn local stfixest_intended_public_command "stfeols"', adapter)
        self.assertNotIn('ereturn local cmd "stfeols"', adapter)


if __name__ == "__main__":
    unittest.main()
