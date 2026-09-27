import json
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "benchmarks/performance"))
import slope_rotation as rotation


class SlopeComparisonTests(unittest.TestCase):
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
