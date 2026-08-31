import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "benchmarks/scc/fereg_f0_scc.py"
MANIFEST_PATH = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"


def load_module():
    spec = importlib.util.spec_from_file_location("fereg_f0_scc_test_module", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class FeregF0SccContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scc = load_module()

    def test_manifest_closes_over_fourteen_scc_cells(self):
        manifest = self.scc.load_regression_manifest(MANIFEST_PATH)
        cells = self.scc.scc_cells(manifest)
        self.assertEqual(len(cells), 14)
        self.assertEqual(cells[0]["id"], "dense_two_fe_t1_fast_fit")
        self.assertEqual(cells[-1]["id"], "constrained_memory_t1_low")
        self.assertTrue(all(cell["profile"] == "scc_governing" for cell in cells))

    def test_pair_cadence_and_confirmation_are_frozen(self):
        noisy = []
        ratios = [0.9, 1.25, 0.9, 1.25, 0.9, 1.25, 0.9, 1.25, 1.25]
        for index, (order, ratio) in enumerate(zip(self.scc.PAIR_ORDERS, ratios), 1):
            noisy.append({
                "index": index,
                "order": order,
                "baseline": {"seconds": 1.0, "peak_rss_bytes": 100},
                "candidate": {"seconds": ratio, "peak_rss_bytes": 100},
            })
        summary = self.scc.paired_summary(noisy)
        self.assertGreater(summary["ratio"], 1.05)
        self.assertLessEqual(summary["log_ci_lower"], 0.0)
        self.assertTrue(self.scc.confirmation_required(noisy))

        certain = [
            {
                "index": index,
                "order": order,
                "baseline": {"seconds": 1.0, "peak_rss_bytes": 100},
                "candidate": {"seconds": 1.06, "peak_rss_bytes": 100},
            }
            for index, order in enumerate(self.scc.PAIR_ORDERS, 1)
        ]
        self.assertGreater(self.scc.paired_summary(certain)["log_ci_lower"], 0.0)
        self.assertTrue(self.scc.confirmation_required(certain))

        precise_small_slowdown = [
            {
                "index": index,
                "order": order,
                "baseline": {"seconds": 1.0, "peak_rss_bytes": 100},
                "candidate": {"seconds": 1.02, "peak_rss_bytes": 100},
            }
            for index, order in enumerate(self.scc.PAIR_ORDERS, 1)
        ]
        precise_summary = self.scc.paired_summary(precise_small_slowdown)
        self.assertLess(precise_summary["ratio"], 1.05)
        self.assertGreater(precise_summary["log_ci_lower"], 0.0)
        self.assertTrue(self.scc.confirmation_required(precise_small_slowdown))

        parity = [
            {
                "index": index,
                "order": order,
                "baseline": {"seconds": 1.0, "peak_rss_bytes": 100},
                "candidate": {"seconds": 1.0, "peak_rss_bytes": 100},
            }
            for index, order in enumerate(self.scc.PAIR_ORDERS, 1)
        ]
        self.assertFalse(self.scc.confirmation_required(parity))

    def _qacct_record(self, job, task=None, failed="0", exit_status="0"):
        rows = [
            "==============================================================",
            "qname shared.q",
            "hostname scc-test",
            "project welfgr",
            f"jobnumber {job}",
        ]
        if task is not None:
            rows.append(f"taskid {task}")
        rows.extend([
            "slots 16",
            f"failed {failed}",
            f"exit_status {exit_status}",
            "ru_wallclock 12.0",
            "cpu 10.0",
            "maxvmem 2.0G",
        ])
        return "\n".join(rows) + "\n"

    def test_qacct_requires_closed_successful_array_inventory(self):
        text = "".join(self._qacct_record("456", task) for task in (1, 2, 3))
        records = self.scc.validate_qacct(text, "456", [1, 2, 3])
        self.assertEqual(len(records), 3)
        with self.assertRaisesRegex(self.scc.SccContractError, "incomplete"):
            self.scc.validate_qacct(text, "456", [1, 2, 3, 4])
        failed = self._qacct_record("456", 1, failed="1")
        with self.assertRaisesRegex(self.scc.SccContractError, "failed"):
            self.scc.validate_qacct(failed, "456", [1])

    def test_qacct_fetch_waits_for_the_complete_array_inventory(self):
        partial = self._qacct_record("456", 1)
        complete = partial + self._qacct_record("456", 2)
        responses = [
            subprocess.CompletedProcess(["qacct"], 0, stdout=partial),
            subprocess.CompletedProcess(["qacct"], 0, stdout=complete),
        ]
        original = self.scc.subprocess.run
        self.scc.subprocess.run = lambda *args, **kwargs: responses.pop(0)
        try:
            observed = self.scc._fetch_qacct(
                "456", [1, 2], attempts=2, delay_seconds=0.0
            )
        finally:
            self.scc.subprocess.run = original
        self.assertEqual(observed, complete)

    def test_solver_digest_excludes_only_environmental_timing_columns(self):
        captured = {
            "matrices": [
                {
                    "name": "fereg_solver",
                    "row": 1,
                    "column": column,
                    "row_name": "r1",
                    "column_name": f"c{column}",
                    "value_hex": f"+1X+{column:03x}",
                }
                for column in range(1, 12)
            ]
        }
        first = self.scc._matrix_digest(
            self.scc, captured, "fereg_solver", excluded_columns=(9, 10)
        )
        captured["matrices"][8]["value_hex"] = "+9X+999"
        captured["matrices"][9]["value_hex"] = "+9X+999"
        second = self.scc._matrix_digest(
            self.scc, captured, "fereg_solver", excluded_columns=(9, 10)
        )
        self.assertEqual(first, second)
        captured["matrices"][10]["value_hex"] = "+9X+999"
        third = self.scc._matrix_digest(
            self.scc, captured, "fereg_solver", excluded_columns=(9, 10)
        )
        self.assertNotEqual(second, third)

    def _fake_run(self, root):
        manifest = self.scc.load_regression_manifest(MANIFEST_PATH)
        manifest_bytes = MANIFEST_PATH.read_bytes()
        (root / "input").mkdir(parents=True)
        (root / "output/cells").mkdir(parents=True)
        (root / "receipts/tasks").mkdir(parents=True)
        (root / "input/manifest.json").write_bytes(manifest_bytes)
        arms = {
            arm: {
                "source": {
                    "commit": ("a" if arm == "baseline" else "b") * 40,
                    "tree": ("c" if arm == "baseline" else "d") * 40,
                    "cargo_lock_sha256": "e" * 64,
                    "references_lock_sha256": "f" * 64,
                },
                "artifact": {
                    "platform": "linux-x86_64",
                    "sha256": ("1" if arm == "baseline" else "2") * 64,
                    "size_bytes": 10,
                    "embedded_git_commit": ("a" if arm == "baseline" else "b") * 12,
                },
                "bundle": {"sha256": "3" * 64, "size_bytes": 10, "inventory": []},
            }
            for arm in ("baseline", "candidate")
        }
        setup = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-setup-v1",
            "run_id": root.name,
            "manifest": {"id": manifest["manifest_id"], "sha256": self.scc.sha256_bytes(manifest_bytes)},
            "preparation_receipt_sha256": "4" * 64,
            "toolchain": {},
            "arms": arms,
            "datasets": {},
            "sge": {},
            "terminal_sentinel": "FEREG_F0_SETUP_SUCCESS",
        }
        self.scc.write_sealed(root / "receipts/setup.json", setup)
        setup_sha = self.scc.sha256_file(root / "receipts/setup.json")
        cells = []
        for task_id, spec in enumerate(self.scc.scc_cells(manifest), 1):
            cell_id = spec["id"]
            cell = {
                "cell_id": cell_id,
                "environment": {},
                "scientific": {},
                "pairs": [],
                "confirmation_pairs": None,
                "memory": {},
            }
            cell_path = root / f"output/cells/{cell_id}.json"
            cell_path.write_text(json.dumps(cell), encoding="utf-8")
            receipt = {
                "schema_version": 1,
                "receipt_type": "fereg-f0-scc-task-v1",
                "run_id": root.name,
                "task_id": task_id,
                "cell_id": cell_id,
                "manifest": setup["manifest"],
                "setup_receipt_sha256": setup_sha,
                "dataset": {},
                "arms": arms,
                "environment": {},
                "cell_sha256": self.scc.sha256_file(cell_path),
                "sge": {},
                "terminal_sentinel": f"FEREG_F0_TASK_SUCCESS|{cell_id}",
            }
            self.scc.write_sealed(root / f"receipts/tasks/task-{task_id:03d}.json", receipt)
            (root / f"receipts/tasks/task-{task_id:03d}.SUCCESS").write_text(
                f"FEREG_F0_TASK_SUCCESS|{cell_id}\n", encoding="ascii"
            )
            cells.append(cell)
        return manifest, setup, cells

    def test_task_collector_rejects_missing_and_tampered_shards(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "20260831T120000Z-test"
            root.mkdir()
            manifest, setup, cells = self._fake_run(root)
            observed = self.scc.validate_task_inventory(root, manifest, setup)
            self.assertEqual(observed, cells)

            victim = root / "output/cells/dense_two_fe_t1_fast_fit.json"
            victim.write_text("{}\n", encoding="utf-8")
            with self.assertRaisesRegex(self.scc.SccContractError, "hash mismatch"):
                self.scc.validate_task_inventory(root, manifest, setup)

    def test_shell_templates_encode_scc_policy(self):
        setup = (ROOT / "benchmarks/scc/fereg_f0_setup_job.sh").read_text()
        task = (ROOT / "benchmarks/scc/fereg_f0_job.sh").read_text()
        collector = (ROOT / "benchmarks/scc/fereg_f0_collector_job.sh").read_text()
        submit = (ROOT / "benchmarks/scc/fereg_f0_submit.sh").read_text()
        deploy = (ROOT / "benchmarks/scc/fereg_f0_deploy.sh").read_text()
        close = (ROOT / "benchmarks/scc/fereg_f0_close.sh").read_text()
        for source in (setup, task, collector):
            self.assertIn("#$ -P welfgr", source)
        self.assertIn("module load stata-mp/19", setup)
        self.assertIn("module load stata-mp/19", task)
        self.assertIn("--task-id \"$SGE_TASK_ID\"", task)
        self.assertIn("-hold_jid \"$SETUP_JOB\"", submit)
        self.assertIn("-hold_jid \"$SMOKE_JOB\"", submit)
        self.assertIn("-hold_jid \"$ARRAY_JOB\"", submit)
        self.assertIn('-t "2-$TASK_COUNT"', submit)
        self.assertIn('--smoke-job "$SMOKE_JOB"', submit)
        self.assertNotIn(" -V", submit)
        self.assertNotIn("--delete", deploy)
        self.assertIn("verify-input", deploy)
        self.assertIn(" qacct", close)
        for source in (setup, task, collector, submit, close):
            self.assertIn("^/projectnb/welfgr/fereg/runs/", source)

    def test_exact_numeric_preimage_and_ereturn_inventory_are_explicit(self):
        source = MODULE_PATH.read_text(encoding="utf-8")
        self.assertIn('"fit": ordered_matrix_hex("fereg_fit")', source)
        self.assertIn('"certificates": ordered_matrix_hex("fereg_certificate")', source)
        self.assertIn(
            "sha256_bytes(reference.canonical_json_bytes(exact_hex))", source
        )
        rendered = self.scc._inject_ereturn_inventory(
            'file write __frg_capture "END_FIXTURE|ols_v1_reference_capture|rc=0" _n',
            Path("/tmp/fereg-f0-ereturn.inventory"),
        )
        for inventory in ("e(scalars)", "e(macros)", "e(matrices)"):
            self.assertIn(inventory, rendered)


if __name__ == "__main__":
    unittest.main()
