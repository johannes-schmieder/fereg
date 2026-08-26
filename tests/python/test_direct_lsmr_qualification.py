import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "direct_lsmr_qualification.py"
SPEC = importlib.util.spec_from_file_location("direct_lsmr_qualification", MODULE_PATH)
QUALIFICATION = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(QUALIFICATION)


class DirectLsmrQualificationTests(unittest.TestCase):
    def test_manifest_freezes_candidate_before_holdouts(self):
        manifest = QUALIFICATION.load_manifest()
        frozen = manifest["frozen_candidate"]
        self.assertEqual(frozen["algorithm"], "direct_lsmr")
        self.assertEqual(frozen["cg_refinement_iterations"], 0)
        self.assertIsNone(frozen["coefficient_cap"])
        self.assertEqual(frozen["pilot_min_contraction"], 0.008)

    def test_profiles_separate_calibration_from_new_seed_holdouts(self):
        manifest = QUALIFICATION.load_manifest()
        plans = {
            profile: QUALIFICATION.expand_profile(manifest, profile)
            for profile in ("calibration_1m", "holdout_1m", "freeze_10m")
        }
        calibration_seeds = {run["design"]["seed"] for run in plans["calibration_1m"]}
        holdout_seeds = {run["design"]["seed"] for run in plans["holdout_1m"]}
        self.assertTrue(calibration_seeds.isdisjoint(holdout_seeds))
        self.assertEqual({run["scale"]["n"] for run in plans["freeze_10m"]}, {10_000_000})


if __name__ == "__main__":
    unittest.main()
