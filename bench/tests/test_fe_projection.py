"""FE benchmark scope, lossless inputs, and fail-closed admission accounting."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import numpy as np
import pandas as pd

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from fe_projection import ARMS, CELLS, export_input
from fe_projection_compare import group, measurement, summarize
from run import load_suite


class FeProjectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.cell = {"id": "test", "rows": 4, "regressors": 2}
        self.identity = {"commit": "a" * 40, "dirty": False, "partial": False,
                         "binary_sha256": "b" * 64, "seed": 739391,
                         "numpy": "test", "pandas": "test", "tolerance": 1e-8,
                         "reference_tolerance": 1e-12, "repetitions": 2,
                         "cells": [self.cell], "threads": [1, 8]}

    def write(self, arm, threads, repetition, status="ok", coefficient=0.3):
        path = self.root / "commands/test" / f"{arm}-t{threads}-{repetition}"
        path.mkdir(parents=True)
        (path / "process.json").write_text(json.dumps({"process_status": status}))
        (path / "result.tsv").write_text(
            "rows\t4\nrank\t2\nrhs_count\t4\nprojection_seconds\t3.0\n"
            "used\tlsmr\niterations\t20\nfallbacks\t0\n")
        (path / "process.log").write_text(
            "FEREG_PHASE|fit|rhs=0.5|raw_moments=0.2|initial_absorb=1.3|finite_solve=9\n")
        np.array([coefficient, -0.2], dtype="<f8").tofile(path / "coefficients.f64")
        np.array([0.1, -0.1, 0.2, -0.2], dtype="<f8").tofile(path / "residuals.f64")
        return path

    def references(self):
        for arm in ("legacy-auto", "next-auto"):
            self.write(arm, 1, "reference")

    def test_binary_export_is_the_declared_dta_without_transformation(self):
        suite = load_suite()
        cell = next({**suite["defaults"], **c, "rows": 400, "regressors": 2}
                    for c in suite["cells"] if c["id"] == "ols-2fe-slope")
        path = export_input(cell, suite["seed"], self.root)
        frame = pd.read_stata(self.root / "input.dta")
        with path.open("rb") as stream:
            self.assertEqual(stream.read(8), b"FEREGFE1")
            self.assertEqual(struct.unpack("<QQQ", stream.read(24)), (400, 2, 2))
            for j in (1, 2):
                levels, slopes = struct.unpack("<QQ", stream.read(16))
                self.assertEqual(levels, frame[f"g{j}"].max())
                self.assertEqual(slopes, int(j == 1))
                np.testing.assert_array_equal(np.frombuffer(stream.read(400*4), dtype="<u4"),
                                              frame[f"g{j}"].to_numpy() - 1)
                if slopes:
                    np.testing.assert_array_equal(np.frombuffer(stream.read(400*8), dtype="<f8"),
                                                  frame.slope.to_numpy())
            for name in ("y", "x1", "x2"):
                np.testing.assert_array_equal(np.frombuffer(stream.read(400*8), dtype="<f8"),
                                              frame[name].to_numpy())
            self.assertEqual(stream.read(), b"")

    def test_legacy_timer_excludes_finite_fit_and_requires_one_complete_trace(self):
        path = self.write("legacy-auto", 1, 1)
        self.assertEqual(measurement(path, self.cell, "legacy-auto")[0], 2.0)
        trace = path / "process.log"
        trace.write_text(trace.read_text() * 2)
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            measurement(path, self.cell, "legacy-auto")
        trace.write_text("FEREG_PHASE|fit|rhs=-1|raw_moments=2|initial_absorb=3\n")
        with self.assertRaisesRegex(ValueError, "invalid legacy"):
            measurement(path, self.cell, "legacy-auto")

    def test_failed_repetition_retains_completed_count_without_a_passing_median(self):
        self.references()
        self.write("next-auto", 1, "warmup")
        self.write("next-auto", 1, 1)
        self.write("next-auto", 1, 2, status="timeout")
        record = group(self.root, self.identity, self.cell, "next-auto", 1)
        self.assertEqual((record["status"], record["completed"]), ("timeout", 1))
        self.assertFalse(record["accurate"])
        self.assertEqual(record["median_seconds"], "")

    def test_reference_disagreement_and_thread_drift_cannot_enter_ratios(self):
        self.references()
        for threads in (1, 8):
            for arm in ARMS:
                for rep in ("warmup", 1, 2):
                    self.write(arm, threads, rep,
                               coefficient=0.3 + (1e-12 if threads == 8 else 0.))
        (self.root / "run.json").write_text(json.dumps(self.identity))
        records = summarize(self.root, self.root / "summary.csv")
        self.assertEqual(len(records), 8)
        self.assertTrue(all(r["status"] == "thread-mismatch" for r in records
                            if r["arm"].startswith("next-")))
        self.assertTrue(all(r["ratio_to_legacy"] == "" for r in records
                            if r["arm"].startswith("next-")))
        reference = self.root / "commands/test/next-auto-t1-reference/coefficients.f64"
        np.array([1., -0.2], dtype="<f8").tofile(reference)
        record = group(self.root, self.identity, self.cell, "legacy-auto", 1)
        self.assertEqual(record["status"], "tight-reference disagreement")
        self.assertFalse(record["accurate"])

    def test_missing_arms_remain_in_the_formal_thirty_cell_summary(self):
        suite = load_suite()
        self.identity["cells"] = [{**suite["defaults"], **c} for c in suite["cells"]
                                  if c["id"] in CELLS]
        (self.root / "run.json").write_text(json.dumps(self.identity))
        records = summarize(self.root, self.root / "summary.csv")
        self.assertEqual(len(records), 30)
        self.assertTrue(all(r["status"] == "missing" and not r["accurate"] for r in records))


if __name__ == "__main__":
    unittest.main()
