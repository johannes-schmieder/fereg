import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from summarize_rotations import summarize
from rotation_threads import variant_cell, native_threads_match, selected_threads


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))


class RotationSummaryTests(unittest.TestCase):
    def test_variant_thread_override_is_explicit_and_bounded(self):
        case = {"cell": {"model": "ppml", "threads": 28, "k": 6},
                "arm_threads": {"baseline": 8, "candidate": 28, "stock": 28}}
        self.assertEqual(variant_cell(case, "baseline"), dict(model="ppml", threads=8, k=6))
        self.assertEqual(case["cell"]["threads"], 28)
        for override in [{"other": 8}, {"candidate": 29}, {"candidate": True}, {"candidate": 0}]:
            with self.assertRaises(ValueError):
                variant_cell(dict(case, arm_threads=override), "candidate")
        receipt = dict(cell=case["cell"], metadata=dict(feppml_threads=8, feppml_requested_threads=8))
        self.assertTrue(native_threads_match(receipt, 8))
        self.assertFalse(native_threads_match(receipt, 28))
        receipt["metadata"]["feppml_requested_threads"] = 28
        self.assertTrue(native_threads_match(receipt, 28, 8))
        self.assertFalse(native_threads_match(receipt, 8, 8))
        case["native_effective_threads"] = dict(candidate=8)
        self.assertEqual(selected_threads(case, "candidate"), 8)
        for counts in [dict(stock=8), dict(candidate=0), dict(candidate=29), dict(candidate=True)]:
            with self.assertRaises(ValueError):
                selected_threads(dict(case, native_effective_threads=counts), "candidate")

    def test_explicit_thread_pairs_require_actual_native_receipt(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            plan = json.loads((root / "plan.json").read_text())
            for case in plan["cases"]:
                case["arm_threads"] = dict(baseline=2, candidate=case["cell"]["threads"], stock=8)
                for rep in (1, 2):
                    for variant in ("baseline", "candidate", "stock"):
                        cell = variant_cell(case, variant)
                        self.mutate(root, f"{case['id']}/pair-{rep}/{variant}/result.json",
                                    lambda r: r.update(cell=cell, metadata=dict(fereg_threads=cell["threads"], fereg_requested_threads=cell["threads"])))
            write(root / "plan.json", plan)
            result = summarize(root)
            self.assertEqual(result["scaling"], [])
            self.assertEqual(result["cases"][0]["arm_threads"], dict(baseline=2, candidate=4, stock=8))
            self.mutate(root, "ols-t4/pair-1/candidate/result.json", lambda r: r["metadata"].update(fereg_threads=2))
            with self.assertRaisesRegex(ValueError, "native thread receipt"):
                summarize(root)

    def test_effective_ceiling_keeps_public_request_and_checks_each_pair(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            plan = json.loads((root / "plan.json").read_text())
            for case in plan["cases"]:
                case["native_effective_threads"] = dict(candidate=2)
                for rep in (1, 2):
                    for variant in ("baseline", "candidate"):
                        self.mutate(root, f"{case['id']}/pair-{rep}/{variant}/result.json",
                                    lambda r: r.update(metadata=dict(
                                        fereg_requested_threads=case["cell"]["threads"],
                                        fereg_threads=selected_threads(case, variant))))
            write(root / "plan.json", plan)
            result = summarize(root)
            self.assertEqual(result["scaling"], [])
            self.assertEqual(result["cases"][0]["cell"]["threads"], 4)
            self.assertEqual(result["cases"][0]["native_effective_threads"], dict(baseline=4, candidate=2))
            self.mutate(root, "ols-t4/pair-2/candidate/result.json", lambda r: r["metadata"].update(fereg_requested_threads=2))
            with self.assertRaisesRegex(ValueError, "native thread receipt"):
                summarize(root)

    def fixture(self, root):
        identity = {"sources": {"baseline": "old", "candidate": "new"}}
        fingerprint = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
        orders = [["baseline", "candidate", "stock"], ["stock", "candidate", "baseline"]]
        cases = []
        for threads in (4, 8):
            cell = {"model": "ols", "design": "sparse", "n": 100, "k": 2, "threads": threads}
            case = {"id": f"ols-t{threads}", "cell": cell, "seeds": [11, 13], "oracle": True, "stock_arm": "fixest"}
            cases.append(case)
            for rep, seed in enumerate(case["seeds"], 1):
                directory = root / case["id"] / f"pair-{rep}"
                record = {"case": case["id"], "repetition": rep, "seed": seed, "order": orders[rep - 1],
                    "comparisons": {arm: {"status": "pass", "checks": {"sample_exact": True, "coefficients": True}}
                                    for arm in ("baseline", "stock")},
                    "oracle_checks": {arm: {"sample": True, "coefficients": True} for arm in ("baseline", "candidate")}}
                write(directory / "comparison.json", record)
                for arm, seconds in (("baseline", 10), ("candidate", 4 * rep * 4 / threads), ("stock", 5)):
                    receipt = {"cell": cell, "arm": "fixest" if arm == "stock" else "fereg", "status": "pass",
                        "seconds": seconds, "elapsed_seconds": seconds + 2, "peak_rss_gib": 1,
                        "measurement_fingerprint": fingerprint, "input_sha256": f"input-{seed}", "sample_sha256": "sample"}
                    write(directory / arm / "result.json", receipt)
        write(root / "plan.json", {"cases": cases, "orders": orders})
        write(root / "identity.json", identity)
        write(root / "complete.json", {"complete": True, "pairs": 4})

    def mutate(self, root, name, edit):
        path = root / name
        value = json.loads(path.read_text())
        edit(value)
        write(path, value)

    def test_all_seed_distributions_and_same_input_scaling(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            result = summarize(root)
            self.assertEqual(result["cases"][0]["comparisons"]["stock"]["seconds_ratio"]["values"], [0.8, 1.6])
            self.assertEqual(result["scaling"][0]["higher_over_lower"]["values"], [0.5, 0.5])
            self.assertEqual(result["cases"][0]["measurements"]["candidate_elapsed_seconds"]["values"], [6, 10])
            self.assertIn("ols-t4/pair-1/candidate/result.json", result["bindings"])

    def test_failure_cannot_be_hidden_by_passing_seed(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            self.mutate(root, "ols-t4/pair-2/comparison.json", lambda r: r["comparisons"]["stock"]["checks"].update(coefficients=False))
            result = summarize(root)
            comparison = result["cases"][0]["comparisons"]["stock"]
            self.assertFalse(comparison["eligible"])
            self.assertIsNone(comparison["seconds_ratio"])
            self.assertTrue(result["cases"][0]["comparisons"]["baseline"]["eligible"])

    def test_repeated_seed_keeps_each_scaling_clock_and_input_guard(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            plan = json.loads((root / "plan.json").read_text())
            for case in plan["cases"]:
                case["seeds"] = [11, 11]
                self.mutate(root, case["id"] + "/pair-2/comparison.json", lambda r: r.update(seed=11))
                for arm in ("baseline", "candidate", "stock"):
                    self.mutate(root, case["id"] + "/pair-2/" + arm + "/result.json",
                                lambda r: r.update(input_sha256="input-11"))
            write(root / "plan.json", plan)
            self.mutate(root, "ols-t8/pair-1/candidate/result.json", lambda r: r.update(seconds=8))
            scaling = summarize(root)["scaling"][0]
            self.assertEqual(scaling["higher_over_lower"]["values"], [0.5, 2.0])
            self.assertFalse(scaling["slower_on_every_repetition"])
            self.mutate(root, "ols-t8/pair-1/candidate/result.json", lambda r: r.update(input_sha256="changed"))
            scaling = summarize(root)["scaling"][0]
            self.assertFalse(scaling["eligible"])
            self.assertIn("seed-11-rep-1:different_input_or_sample", scaling["reasons"])

    def test_missing_oracle_or_exact_sample_prevents_ranking(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            self.mutate(root, "ols-t4/pair-1/comparison.json", lambda r: r.pop("oracle_checks"))
            self.mutate(root, "ols-t8/pair-1/stock/result.json", lambda r: r.pop("sample_sha256"))
            result = summarize(root)
            self.assertEqual(len(result["oracle_failures"]), 2)
            self.assertFalse(result["cases"][0]["comparisons"]["baseline"]["eligible"])
            self.assertFalse(result["cases"][1]["comparisons"]["stock"]["eligible"])

    def test_timeout_and_partial_results_are_visible(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            self.mutate(root, "ols-t4/pair-1/candidate/result.json", lambda r: r.update(status="command_timeout", seconds=None))
            (root / "ols-t8/pair-2/comparison.json").unlink()
            (root / "complete.json").unlink()
            with self.assertRaisesRegex(ValueError, "incomplete rotation"):
                summarize(root)
            result = summarize(root, partial=True)
            self.assertEqual(result["statuses"]["candidate:command_timeout"], 1)
            self.assertEqual(result["completed_pairs"], 3)
            self.assertFalse(result["complete"])
            self.assertIsNone(result["scaling"][0]["higher_over_lower"])

    def test_wrong_identity_or_order_rejects_summary(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            self.mutate(root, "ols-t4/pair-1/comparison.json", lambda r: r.update(order=["stock", "candidate", "baseline"]))
            with self.assertRaisesRegex(ValueError, "order"):
                summarize(root)
            self.mutate(root, "ols-t4/pair-1/comparison.json", lambda r: r.update(order=["baseline", "candidate", "stock"]))
            self.mutate(root, "ols-t4/pair-1/candidate/result.json", lambda r: r.update(measurement_fingerprint="wrong"))
            with self.assertRaisesRegex(ValueError, "identity"):
                summarize(root)

    def test_changed_input_blocks_thread_scaling(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            self.mutate(root, "ols-t8/pair-2/candidate/result.json", lambda r: r.update(input_sha256="different"))
            result = summarize(root)
            self.assertFalse(result["scaling"][0]["eligible"])
            self.assertIsNone(result["scaling"][0]["higher_over_lower"])


if __name__ == "__main__":
    unittest.main()
