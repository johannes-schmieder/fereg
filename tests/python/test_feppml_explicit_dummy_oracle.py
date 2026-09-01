from __future__ import annotations

import csv
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import subprocess
import struct
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "scripts/feppml_explicit_dummy_oracle.py"
SPEC = importlib.util.spec_from_file_location("feppml_explicit_dummy_oracle_test", PATH)
assert SPEC is not None and SPEC.loader is not None
ORACLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ORACLE)


def model(*, clustered: bool = False) -> dict:
    finite = [
        {"name": "x", "column": "x"},
        {"name": "c2", "column": "c2"},
        {"name": "_cons", "column": None},
    ]
    statuses = [
        {"name": "x", "status_code": 1},
        {"name": "c2", "status_code": 6},
        {"name": "_cons", "status_code": 7},
    ]
    return {
        "schema_version": 1,
        "model_type": "feppml-explicit-dummy-model-v1",
        "cell_id": "oracle_multiway" if clustered else "oracle_robust",
        "row_count": 72,
        "outcome": "y",
        "finite_terms": finite,
        "fixed_effects": [
            {"column": "g", "slope": None},
            {"column": "h", "slope": None},
        ],
        "adjustment": {"kind": "offset", "column": "offset"},
        "weight": {"kind": "pweight", "column": "pw"},
        "clusters": ["cluster1", "cluster2"] if clustered else [],
        "vce": "cluster" if clustered else "robust",
        "separation": {
            "kind": "zero_fe_levels",
            "fe_columns": ["g", "h"],
            "reason_code": 1,
        },
        "expected_status_map": statuses,
        "expected_output": "total_absorbed_effect",
        "probe_count": 3,
        "complete_sentinel": "FEPPML_EXPLICIT_DUMMY_MODEL_COMPLETE",
    }


def rows() -> list[dict[str, str]]:
    result = []
    for obs_id in range(1, 73):
        g = (obs_id - 1) % 6
        h = ((obs_id - 1) // 3) % 5
        x = ((17 * obs_id) % 43 - 21) / 11
        offset = 0.03 * ((obs_id % 7) - 3)
        eta = 0.2 + 0.17 * x + 0.08 * g - 0.04 * h + offset
        mu = math.exp(eta)
        y = math.floor(mu) + int((obs_id * 37) % 100 < 100 * (mu - math.floor(mu)))
        if g == 0:
            y = 0
        result.append(
            {
                "obs_id": str(obs_id),
                "y": str(y),
                "x": format(x, ".17g"),
                "c2": format(2.0 * x, ".17g"),
                "g": str(g),
                "h": str(h),
                "offset": format(offset, ".17g"),
                "pw": format(0.5 + (obs_id % 9) / 7, ".17g"),
                "cluster1": str((obs_id - 1) % 7),
                "cluster2": str(((obs_id - 1) // 2) % 8),
            }
        )
    return result


class FeppmlExplicitDummyOracleTests(unittest.TestCase):
    def test_mask_digests_use_the_frozen_ascii_wire_representation(self) -> None:
        mask = [0, 1, 2, 1]
        expected_sample = hashlib.sha256(
            b"FEPPML_SAMPLE_MASK_V1\0" + struct.pack(">Q", 4) + b"0121"
        ).hexdigest()
        expected_separation = hashlib.sha256(
            b"FEPPML_GENERIC_SEPARATION_MASK_V1\0" + struct.pack(">Q", 4) + b"0010"
        ).hexdigest()
        self.assertEqual(ORACLE.sample_mask_digest(mask), expected_sample)
        self.assertEqual(ORACLE.separation_mask_digest(mask), expected_separation)

    def write_fixture(self, root: Path, *, clustered: bool = False) -> tuple[Path, Path]:
        data = root / "fixture.csv"
        with data.open("w", encoding="ascii", newline="") as target:
            writer = csv.DictWriter(target, fieldnames=list(rows()[0]))
            writer.writeheader()
            writer.writerows(rows())
        model_path = root / "model.json"
        model_path.write_text(json.dumps(model(clustered=clustered)), encoding="ascii")
        return data, model_path

    def test_golden_small_explicit_dummy_fit_is_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            data, model_path = self.write_fixture(Path(temporary))
            first = ORACLE.evaluate(data, model_path)
            second = ORACLE.evaluate(data, model_path)
        self.assertEqual(first, second)
        self.assertEqual(
            first["coefficient_status"],
            ["identified", "numerically_rank_deficient", "normalized_structural_constant"],
        )
        self.assertEqual(first["dof"]["N_physical"], 72)
        self.assertEqual(first["dof"]["N_effective"], 72)
        self.assertEqual(first["vce_shape"], [3, 3])
        self.assertEqual(first["coefficients"][1:], [0.0, 0.0])
        self.assertTrue(math.isclose(first["coefficients"][0], 0.15409094803879428, rel_tol=1e-11))
        self.assertEqual(
            first["sample_identity_sha256"],
            "5fdad6d48c8ff442fb7285660d86d04688413a582b72c5684990371c8c8466f8",
        )
        self.assertEqual(len(first["outputs"]["total_absorbed_effect_probe"]), 3)
        self.assertLess(first["certificates"]["score_linf"], 1e-10)
        self.assertLess(first["certificates"]["reconstruction_linf"], 1e-12)

    def test_two_way_cluster_oracle_uses_common_minimum_group_correction(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            data, model_path = self.write_fixture(Path(temporary), clustered=True)
            result = ORACLE.evaluate(data, model_path)
        self.assertEqual(result["dof"]["cluster_counts"], [7, 8])
        self.assertEqual(result["dof"]["df"], 6)
        covariance = result["vce"]
        self.assertTrue(all(math.isfinite(value) for value in covariance))
        self.assertEqual(covariance[1], 0.0)
        self.assertEqual(covariance[2], 0.0)

    def test_null_likelihood_excludes_fitted_offset_and_saved_fe_is_channel_identified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data, model_path = self.write_fixture(root)
            individual_model = model()
            individual_model["expected_output"] = "individual_fe"
            model_path.write_text(json.dumps(individual_model), encoding="ascii")
            result = ORACLE.evaluate(data, model_path)
            total_model = model()
            total_model["expected_output"] = "total_absorbed_effect"
            model_path.write_text(json.dumps(total_model), encoding="ascii")
            total = ORACLE.evaluate(data, model_path)
            retained = [row for row in rows() if row["g"] != "0"]
            raw_weights = [float(row["pw"]) for row in retained]
            scale = len(retained) / sum(raw_weights)
            weights = [value * scale for value in raw_weights]
            outcomes = [float(row["y"]) for row in retained]
            null_mean = sum(weight * outcome for weight, outcome in zip(weights, outcomes)) / sum(weights)
            expected = sum(
                weight * (outcome * math.log(null_mean) - null_mean - math.lgamma(outcome + 1.0))
                for weight, outcome in zip(weights, outcomes)
            )
        self.assertTrue(math.isclose(result["objective"]["null_log_likelihood"], expected, rel_tol=2e-13))
        self.assertEqual(len(result["outputs"]["individual_fe_probe"]), 6)
        channels = result["outputs"]["individual_fe_probe"]
        self.assertTrue(all(
            math.isclose(channels[index] + channels[index + 3], expected, abs_tol=2e-15)
            for index, expected in enumerate(total["outputs"]["total_absorbed_effect_probe"])
        ))
        self.assertNotEqual(channels[:3], channels[3:])
        swapped = channels[3:] + channels[:3]
        shifted = [value + 0.5 for value in channels[:3]] + [value - 0.5 for value in channels[3:]]
        self.assertNotEqual(swapped, channels)
        self.assertNotEqual(shifted, channels)
        self.assertTrue(all(
            math.isclose(shifted[index] + shifted[index + 3], expected, abs_tol=2e-15)
            for index, expected in enumerate(total["outputs"]["total_absorbed_effect_probe"])
        ))

    def test_model_and_data_mutations_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data, model_path = self.write_fixture(root)
            invalid = model()
            invalid["unexpected"] = True
            model_path.write_text(json.dumps(invalid), encoding="ascii")
            with self.assertRaisesRegex(ORACLE.OracleFailure, "not closed"):
                ORACLE.evaluate(data, model_path)
            valid = model()
            valid["row_count"] = 71
            model_path.write_text(json.dumps(valid), encoding="ascii")
            with self.assertRaisesRegex(ORACLE.OracleFailure, "row count"):
                ORACLE.evaluate(data, model_path)

    def test_cli_writes_once_and_refuses_output_reuse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data, model_path = self.write_fixture(root)
            output = root / "oracle.json"
            command = [
                sys.executable,
                str(PATH),
                "--data", str(data),
                "--model", str(model_path),
                "--output", str(output),
            ]
            first = subprocess.run(command, check=False, stdout=subprocess.PIPE, text=True)
            second = subprocess.run(command, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            self.assertEqual(first.returncode, 0)
            self.assertEqual(second.returncode, 1)
            self.assertIn("refusing to overwrite", second.stderr)
            self.assertEqual(json.loads(output.read_text(encoding="ascii"))["status"], "success")


if __name__ == "__main__":
    unittest.main()
