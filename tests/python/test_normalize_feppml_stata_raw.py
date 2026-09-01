from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "scripts/normalize_feppml_stata_raw.py"
SPEC = importlib.util.spec_from_file_location("normalize_feppml_raw_test", PATH)
assert SPEC is not None and SPEC.loader is not None
NORMALIZE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NORMALIZE)


def fixture(arm: str = "candidate") -> dict:
    return {
        "arm": arm, "rc": 0, "converged": True,
        "sample_identity": {
            "algorithm": "sha256", "encoding": "feppml-sample-mask-v1",
            "row_count": 2, "eligible_count": 2, "retained_count": 1,
            "separated_count": 1, "sha256": "a" * 64,
            "probes": [[1, 1, 1, 0], [2, 1, 0, 1]],
        },
        "separation_ledger": {
            "algorithm": "sha256", "encoding": "feppml-generic-separation-mask-v1",
            "row_count": 2, "separated_count": 1, "generic_reason_code": 1,
            "sha256": "b" * 64, "probes": [[2, 1]],
        },
        "coefficient_names": ["x", "o.x2", "_cons"],
        "coefficient_status_codes": [1, 6, 1],
        "coefficients": [0.2, 0.0, 0.1],
        "vce": [0.04, 0.0, 0.01, 0.0, 0.0, 0.0, 0.01, 0.0, 0.03],
        "dof": {"N_physical": 1, "N_effective": 1, "finite_rank": 2, "absorbed_rank": 0, "df": -1, "cluster_counts": []},
        "objective": {"deviance": 1.0, "log_likelihood": -2.0, "null_log_likelihood": -3.0, "pseudo_r2": 1 / 3},
        "outputs": {"prediction_probe": [1.2], "total_absorbed_effect_probe": None, "individual_fe_probe": None},
        "certificates": {"two_consecutive_iterations": True, "strict_final_solve": True, "rank_frozen": True, "score_linf": 1e-10, "fe_score_linf": 0.0, "vce_symmetry_linf": 0.0, "reconstruction_linf": 1e-12},
        "source_inventory": ["e(b)", "e(V)"],
        "complete_sentinel": "FEPPML_STATA_RAW_COMPLETE",
    }


class NormalizeFeppmlRawTests(unittest.TestCase):
    def test_candidate_normalization_is_deterministic_and_exact(self) -> None:
        first = NORMALIZE.normalize(fixture())
        second = NORMALIZE.normalize(fixture())
        self.assertEqual(first, second)
        self.assertEqual(first["coefficient_status"], ["identified", "numerically_rank_deficient", "identified"])
        self.assertEqual(first["vce_shape"], [3, 3])
        self.assertEqual(len(first["sample_identity_sha256"]), 64)

    def test_missing_native_certificate_fails_closed(self) -> None:
        raw = fixture()
        raw["certificates"]["strict_final_solve"] = False
        with self.assertRaisesRegex(NORMALIZE.RawNormalizationError, "mandatory"):
            NORMALIZE.normalize(raw)

    def test_nonidentified_slot_must_be_exact_zero(self) -> None:
        raw = fixture()
        raw["coefficients"][1] = 1e-30
        with self.assertRaisesRegex(NORMALIZE.RawNormalizationError, "exact zero"):
            NORMALIZE.normalize(raw)

    def test_comparator_accepts_frozen_recipe_status_metadata(self) -> None:
        raw = fixture("comparator")
        raw["coefficient_status_codes"] = [1, 2, 1]
        normalized = NORMALIZE.normalize(raw)
        self.assertEqual(normalized["coefficient_status"][1], "base_level")


if __name__ == "__main__":
    unittest.main()
