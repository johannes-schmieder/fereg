"""Scientific inputs and failure accounting, without licensed runtime dependencies."""
import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare import accuracy, summarize
from generate import generate, sha256
from run import launch, load_suite, selected_cells, thread_counts


class SuiteTests(unittest.TestCase):
    def test_manifest_has_required_cells_and_thread_ladder(self):
        suite = load_suite()
        self.assertEqual(len(suite["cells"]), 22)
        self.assertEqual(len(selected_cells(suite, "smoke")), 4)
        self.assertEqual(len(selected_cells(suite, "standard")), 19)
        cells = selected_cells(suite, "full")
        for cell in cells:
            expected = [1,2,4,8,14,28] if cell["id"] in suite["tiers"]["full"]["ladder_cells"] else [1,8]
            self.assertEqual(thread_counts(suite,"full",cell), expected)

    def test_generator_is_byte_reproducible_and_keeps_first_controls(self):
        import pandas as pd
        suite = load_suite()
        cell = {**selected_cells(suite,"smoke")[1], "rows": 400, "regressors": 2}
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            a, b = root/"a.dta", root/"b.dta"
            generate(cell, suite["seed"], a)
            generate(cell, suite["seed"], b)
            self.assertEqual(sha256(a),sha256(b))
            generate({**cell,"regressors":5},suite["seed"],b)
            pa,pb = pd.read_stata(a),pd.read_stata(b)
            self.assertEqual(pa.x1.tolist(),pb.x1.tolist())
            cell.update(model="ppml", separation=True)
            generate(cell,suite["seed"],b)
            data = pd.read_stata(b)
            self.assertTrue((data.loc[data.g1<=2,"y"]==0).all())
            self.assertTrue((data.y>0).any())

    def test_timeout_terminates_the_command_without_retry(self):
        with tempfile.TemporaryDirectory() as temp:
            result = launch([sys.executable,"-c","import time; time.sleep(30)"],Path(temp),0.05)
        self.assertEqual(result["process_status"],"timeout")
        self.assertIsNone(result["process_rc"])
        self.assertLess(result["wall_seconds"],5)

    def test_failed_reference_or_wrong_sample_cannot_pass(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ["a","b"]:
                (root/name).mkdir()
                (root/name/"coefficients.csv").write_text("name,value\nx1,0.3\n")
                (root/name/"sample.csv").write_text("obs_id\n1\n2\n")
            record = {"status":"ok","directory":str(root/"a")}
            reference = {"status":"ok","directory":str(root/"b")}
            policy = load_suite()["protocol"]
            self.assertEqual(accuracy(record,reference,policy)[0],"pass")
            reference["status"]="error"
            self.assertEqual(accuracy(record,reference,policy)[0],"unavailable")
            reference["status"]="ok"
            (root/"b/sample.csv").write_text("obs_id\n1\n3\n")
            self.assertEqual(accuracy(record,reference,policy)[0],"disagreement")
            (root/"b/sample.csv").write_text("obs_id\n1\n2\n")
            (root/"a/coefficients.csv").write_text("name,value\nx1,nan\n")
            self.assertEqual(accuracy(record,reference,policy)[0],"disagreement")

    def test_missing_repetitions_and_entire_missing_arms_remain_visible(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            campaign = {"source_sha":"a"*40,"source_dirty":True,"platform":"test","tier":"smoke",
                        "partial":True,"engine":"legacy","solver":"auto","protocol":load_suite()["protocol"],
                        "expected_groups":[["ols-1fe",4,"fereg"],["ols-1fe",4,"fixest"]],
                        "records":[{"cell":"ols-1fe","threads":4,"arm":"fereg","reference":False,
                                    "repetition":"warmup","status":"timeout","metadata":{}}]}
            path = root/"results.json"
            path.write_text(json.dumps(campaign))
            with contextlib.redirect_stdout(io.StringIO()):
                rows = summarize(path,root/"summary.csv")
            self.assertEqual([row["status"] for row in rows],["timeout","error"])
            self.assertTrue(all(row["accuracy"]=="unavailable" and row["ratio_fereg"]=="" for row in rows))
            self.assertLess((root/"summary.csv").stat().st_size,100000)
            with self.assertRaises(ValueError):
                summarize(path,root/"summary.csv")


if __name__ == "__main__":
    unittest.main()
