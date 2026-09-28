import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "benchmarks/performance"))
import slope_rotation as rotation
import slope_summary as summary
from slope_probe import stata_threads


class SlopeComparisonTests(unittest.TestCase):
    def test_stata_capacity_is_explicit_and_independent_of_native_team(self):
        self.assertEqual(stata_threads(dict(threads=8)), 8)
        self.assertEqual(stata_threads(dict(threads=8, stata_processors=4)), 4)
        for processors in [0, -1, 9]:
            with self.assertRaisesRegex(ValueError, "control-plane capacity"):
                stata_threads(dict(threads=8, stata_processors=processors))
        with patch.dict(rotation.os.environ, NSLOTS="8"), patch.object(rotation.os, "sched_getaffinity", create=True, return_value=set(range(28))):
            rotation.require_allocation(True, smoke_slots=8)

    def test_shared_smoke_and_isolated_timing_have_different_affinity_contracts(self):
        for slots, available, smoke, accepted in ((4, 28, True, True), (4, 4, True, True),
                                                  (4, 2, True, False), (28, 28, False, True),
                                                  (28, 56, False, False), (4, 28, False, False)):
            with patch.dict(rotation.os.environ, NSLOTS=str(slots)), patch.object(rotation.os, "sched_getaffinity", create=True, return_value=set(range(available))):
                if accepted:
                    rotation.require_allocation(smoke)
                else:
                    with self.assertRaisesRegex(ValueError, "slot/affinity"):
                        rotation.require_allocation(smoke)

    def test_summary_requires_every_registered_pair_and_retains_disagreement(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rotation.write(root / "plan.json", dict(cases=[dict(id="test", arms=["baseline", "candidate"])],
                                                   seeds=[7, 11], rotations_per_seed=1))
            result = dict(process_rc=0, sentinel=True, timed_out=False, metadata=dict(seconds=1.),
                          process_seconds=2., process_peak_rss_bytes=1024)
            records = [dict(case="test", seed=seed, repetition=0, arm=arm, result=result)
                       for seed in [7, 11] for arm in ["baseline", "candidate"]]
            pairs = [dict(case="test", seed=seed, repetition=0,
                          comparisons=dict(baseline=dict(ranked=seed == 7, checks=dict(covariance=seed == 7),
                                                        ratio=1. if seed == 7 else None))) for seed in [7, 11]]
            for name, data in [("results.json", records), ("comparisons.json", pairs),
                               ("complete.json", dict(complete=True, fits=4, pairs=2)), ("identity.json", {})]:
                rotation.write(root / name, data)
            row = summary.summarize(root)["cases"][0]
            self.assertEqual(row["comparisons"]["baseline"]["eligible"], 1)
            self.assertIsNone(row["comparisons"]["baseline"]["ratios"])
            self.assertEqual(row["comparisons"]["baseline"]["failed_checks"], ["covariance"])
            for incomplete in (records[:2], [records[0], *records[1:3], records[0]]):
                rotation.write(root / "results.json", incomplete)
                with self.assertRaisesRegex(ValueError, "incomplete or duplicated"):
                    summary.summarize(root)

    def test_requested_outputs_and_failures_control_eligibility(self):
        tolerance = json.loads((ROOT / "benchmarks/systematic/manifest-light-v2.json").read_text())["science"]
        with tempfile.TemporaryDirectory() as temporary:
            a, b = (Path(temporary) / name for name in ("candidate", "stock"))
            result = dict(process_rc=0, sentinel=True, timed_out=False, competing_work=[],
                          metadata=dict(seconds=1., N=3, df_r=1, converged=1, rss=2.))
            rows = pd.DataFrame(dict(obs_id=[1, 2, 3], sample=[1, 1, 1], fitted=[1., 2., 3.], error=[1., 0., -1.]))
            for directory in (a, b):
                directory.mkdir()
                rotation.write(directory / "receipt.json", result)
                np.savetxt(directory / "b.csv", [1., 2.], delimiter=",")
                np.savetxt(directory / "V.csv", np.eye(2), delimiter=",")
                rows.to_csv(directory / "rows.csv", index=False)
            self.assertTrue(rotation.compare(a, b, tolerance)["ranked"])
            altered = rows.copy()
            altered.loc[1, "fitted"] += 1e-4
            altered.to_csv(b / "rows.csv", index=False)
            comparison = rotation.compare(a, b, tolerance)
            self.assertFalse(comparison["ranked"])
            self.assertIsNone(comparison["ratio"])
            self.assertTrue(comparison["checks"]["coefficients"])
            self.assertFalse(comparison["checks"]["fitted"])
            rows.iloc[:2].to_csv(b / "rows.csv", index=False)
            self.assertFalse(rotation.compare(a, b, tolerance)["checks"]["sample"])
            rows.to_csv(b / "rows.csv", index=False)
            np.savetxt(b / "V.csv", [[float("nan"), 0], [0, 1]], delimiter=",")
            self.assertFalse(rotation.compare(a, b, tolerance)["checks"]["covariance"])
            result["sentinel"] = False
            rotation.write(b / "receipt.json", result)
            self.assertEqual(rotation.compare(a, b, tolerance)["checks"], {"application": False})


if __name__ == "__main__":
    unittest.main()
