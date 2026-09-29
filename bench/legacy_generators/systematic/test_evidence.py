"""Negative tests for evidence acceptance; no estimators run locally."""
import tempfile
import unittest
from pathlib import Path

from accounting import parse
from common import HERE, read_json, write_json
from report import composite, panel, quantiles
from runner import compare


class EvidenceTests(unittest.TestCase):
    def test_accounting_missing_and_duplicate_tasks(self):
        text = "jobnumber 123\ntaskid 1\nhostname scc-wc4\nfailed 0\nexit_status 0\nslots 28\nru_wallclock 9\nmaxvmem 1G\n"
        self.assertEqual(parse(text, 123, [1])["status"], "pass")
        self.assertEqual(parse(text, 123, [1, 2])["status"], "pending")
        self.assertEqual(parse(text.replace("exit_status 0", "exit_status 1"), 123, [1])["status"], "failed")
        self.assertEqual(parse(text.replace("slots 28", "slots 4"), 123, [1])["status"], "failed")
        with self.assertRaises(ValueError):
            parse(text + "===================\n" + text, 123, [1])

    def test_six_distinct_warm_repetitions_required(self):
        rows = [{"eligible": True, "repetition": r, "seconds": r} for r in range(1, 7)]
        self.assertEqual(quantiles(rows, "seconds")[1], 3.5)
        self.assertIsNone(quantiles(rows[:5], "seconds"))
        self.assertIsNone(quantiles(rows[:5] + rows[:1], "seconds"))
        rows[-1]["eligible"] = False
        self.assertIsNone(quantiles(rows, "seconds"))
        rows[-1].update(eligible=True, seconds=float("inf"))
        self.assertIsNone(quantiles(rows, "seconds"))

    def fixture(self, path, arm, model="ols"):
        path.mkdir()
        meta = {"N": 100, "converged": 1, "df_a": 20, "df_r": 79, "rss": 42}
        write_json(path / "result.json", {"arm": arm, "status": "pass", "cell": {"k": 1, "model": model},
                   "metadata": meta, "input_sha256": "same-input", "sample_sha256": "same-sample",
                   "host": "test-host", "job_id": "123", "task_id": "1",
                   "measurement_fingerprint": "same-runtime"})
        (path / "coefficients.csv").write_text("name,value,se\nx1,0.3,0.1\n")
        (path / "covariance.csv").write_text("row,column,value\nx1,x1,0.01\n")

    def test_missing_covariance_never_passes(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            self.fixture(a, "fereg"); self.fixture(b, "reghdfe")
            settings = read_json(HERE / "manifest.json")["science"]
            self.assertEqual(compare(a, b, settings)["status"], "pass")
            (b / "covariance.csv").unlink()
            self.assertEqual(compare(a, b, settings)["status"], "accuracy_failure")

    def test_cross_node_or_task_pairing_never_passes(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            self.fixture(a, "fereg"); self.fixture(b, "reghdfe")
            settings = read_json(HERE / "manifest.json")["science"]
            record = read_json(b / "result.json")
            record["task_id"] = "2"
            write_json(b / "result.json", record)
            self.assertFalse(compare(a, b, settings)["checks"]["same_node_task"])

    def test_missing_df_objective_or_convergence_never_passes(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            self.fixture(a, "fereg"); self.fixture(b, "reghdfe")
            settings = read_json(HERE / "manifest.json")["science"]
            original = read_json(a / "result.json")
            for key in ("df_a", "rss", "converged"):
                record = read_json(a / "result.json")
                record["metadata"] = {k: v for k, v in original["metadata"].items() if k != key}
                write_json(a / "result.json", record)
                self.assertEqual(compare(a, b, settings)["status"], "accuracy_failure", key)

    def test_ppml_normalized_constant_preserves_total_rank(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            self.fixture(a, "feppml", "ppml"); self.fixture(b, "ppmlhdfe", "ppml")
            left, right = read_json(a / "result.json"), read_json(b / "result.json")
            left["metadata"].update(rank=2, df_a=19, df_resid=79, ll=-123, deviance=100)
            right["metadata"].update(rank=1, df_a=20, df=79, ll=-123, deviance=100)
            write_json(a / "result.json", left); write_json(b / "result.json", right)
            settings = read_json(HERE / "manifest.json")["science"]
            self.assertEqual(compare(a, b, settings)["status"], "pass")
            right["metadata"]["df_a"] = 19
            write_json(b / "result.json", right)
            self.assertEqual(compare(a, b, settings)["status"], "accuracy_failure")

    def test_explicit_reference_nonconvergence_overrides_numerical_agreement(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            self.fixture(a, "fereg"); self.fixture(b, "areg")
            settings = read_json(HERE / "manifest.json")["science"]
            record = read_json(b / "result.json")
            # Every reported number and row remains identical to the candidate.
            for arm in ("areg", "reghdfe", "greg", "pyfixest"):
                record["arm"] = arm
                for flag in (0, False, None):
                    with self.subTest(arm=arm, converged=flag):
                        record["metadata"]["converged"] = flag
                        write_json(b / "result.json", record)
                        result = compare(a, b, settings)
                        self.assertTrue(result["checks"]["coefficients"])
                        self.assertFalse(result["checks"]["comparator_converged"])
                        self.assertEqual(result["status"], "accuracy_failure")
                record["metadata"]["converged"] = 1
                write_json(b / "result.json", record)
                self.assertEqual(compare(a, b, settings)["status"], "pass")

    def test_reference_missing_convergence_keeps_existing_required_surfaces(self):
        with tempfile.TemporaryDirectory() as folder:
            a, b = Path(folder) / "a", Path(folder) / "b"
            self.fixture(a, "fereg"); self.fixture(b, "fixest")
            settings = read_json(HERE / "manifest.json")["science"]
            record = read_json(b / "result.json")
            del record["metadata"]["converged"]
            write_json(b / "result.json", record)
            self.assertFalse(compare(a, b, settings)["checks"]["comparator_converged"])
            # Missing pyfixest OLS metadata is not a fabricated positive flag.
            record["arm"] = "pyfixest"
            write_json(b / "result.json", record)
            self.assertNotIn("comparator_converged", compare(a, b, settings)["checks"])

    def test_latex_panels_remain_individual(self):
        result = composite("Example", ["a.pdf", "b.pdf", "c.pdf", "d.pdf"], ["a", "b", "c", "d"], "Note")
        self.assertEqual(result.count(r"\begin{subfigure}"), 4)
        self.assertEqual(result.count(r"\includegraphics"), 4)
        self.assertIn("panels/a.pdf", result)

    def test_completely_missing_panel_renders_without_fake_points(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / "missing.pdf"
            manifest = read_json(HERE / "manifest.json")
            panel(target, [], manifest["arms"]["ols"], "arm", "maximum", "seconds", "Empty", manifest)
            self.assertTrue(target.read_bytes().startswith(b"%PDF"))


if __name__ == "__main__":
    unittest.main()
