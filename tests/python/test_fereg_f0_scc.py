import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock


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

    def test_manifest_closes_over_forty_two_scc_cells(self):
        manifest = self.scc.load_regression_manifest(MANIFEST_PATH)
        cells = self.scc.scc_cells(manifest)
        self.assertEqual(len(cells), 42)
        self.assertEqual(cells[0]["id"], "dense_two_fe_t1_fast_fit")
        self.assertEqual(cells[-1]["id"], "solver_cmg_t1_fast_fit")
        self.assertTrue(all(cell["profile"] == "scc_governing" for cell in cells))

    def test_setup_smoke_runs_one_small_fixture_for_each_exact_arm(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "20260901T120000Z-test"
            root.mkdir()
            artifacts = {}
            for arm in ("baseline", "candidate"):
                source = root / f"work/setup/{arm}/source"
                (source / "ci").mkdir(parents=True)
                (source / "scripts").mkdir()
                (source / "dist").mkdir()
                (source / "ci/fereg_smoke.do").write_text(
                    "display \"END_FIXTURE|fereg_smoke|rc=0\"\n",
                    encoding="ascii",
                )
                (source / "scripts/run_stata_tests.py").write_text(
                    "# retained exact runner\n", encoding="ascii"
                )
                plugin = source / "dist/fereg_linux.plugin"
                plugin.write_bytes(f"plugin-{arm}".encode("ascii"))
                artifacts[arm] = {
                    "sha256": self.scc.sha256_file(plugin),
                    "size_bytes": plugin.stat().st_size,
                }

            def fake_run(command, **_kwargs):
                summary_path = Path(command[command.index("--summary-json") + 1])
                log_dir = Path(command[command.index("--log-dir") + 1])
                log_dir.mkdir(parents=True)
                (log_dir / "fereg_smoke.log").write_text(
                    "END_FIXTURE|fereg_smoke|rc=0\n", encoding="ascii"
                )
                summary_path.write_text(
                    json.dumps(
                        {
                            "completed": True,
                            "status": "success",
                            "stata_rc": 0,
                            "tests_passed": 1,
                            "tests_failed": 0,
                            "results": [
                                {
                                    "fixture": "ci/fereg_smoke.do",
                                    "status": "success",
                                    "process_rc": 0,
                                    "stata_rc": 0,
                                }
                            ],
                        }
                    ),
                    encoding="utf-8",
                )
                return subprocess.CompletedProcess(command, 0, stdout="PASS\n")

            with mock.patch.object(self.scc.subprocess, "run", side_effect=fake_run):
                smoke = {
                    arm: self.scc._run_setup_smoke(
                        root,
                        root / f"work/setup/{arm}/source",
                        artifacts[arm],
                        arm=arm,
                    )
                    for arm in ("baseline", "candidate")
                }
            setup = {
                "arms": {
                    arm: {"artifact": artifacts[arm]}
                    for arm in ("baseline", "candidate")
                },
                "smoke": smoke,
            }
            self.scc._verify_setup_smoke(root, setup)
            self.assertEqual(
                {arm: smoke[arm]["status"] for arm in smoke},
                {"baseline": "pass", "candidate": "pass"},
            )

    def test_frozen_harness_must_equal_candidate_but_diagnostic_may_differ(self):
        candidate = {
            "commit": "1" * 40, "tree": "2" * 40,
            "cargo_lock_sha256": "3" * 64,
            "references_lock_sha256": "4" * 64,
            "archive_sha256": "5" * 64, "archive_size_bytes": 123,
            "build_date": "2026-08-31",
        }
        harness = dict(candidate)
        self.scc._validate_frozen_harness_candidate_binding(
            "frozen", harness, candidate
        )
        harness["commit"] = "6" * 40
        self.scc._validate_frozen_harness_candidate_binding(
            "artifacts_sealed", harness, candidate
        )
        with self.assertRaisesRegex(self.scc.SccContractError, "harness differs"):
            self.scc._validate_frozen_harness_candidate_binding(
                "frozen", harness, candidate
            )

    def test_behavior_arm_binding_rejects_swapped_source(self):
        source = {
            "commit": "1" * 40, "tree": "2" * 40,
            "cargo_lock_sha256": "3" * 64,
            "references_lock_sha256": "4" * 64,
        }
        setup_arm = {
            "source": source,
            "artifact": {"platform": "linux-x86_64"},
        }
        observation = {
            "source": dict(source),
            "artifact": {"platform": "macos-universal", "sha256": "5" * 64, "size_bytes": 1},
        }
        self.scc._validate_behavior_arm_binding(observation, setup_arm, "candidate")
        observation["source"] = {**source, "commit": "6" * 40}
        with self.assertRaisesRegex(self.scc.SccContractError, "source differs"):
            self.scc._validate_behavior_arm_binding(observation, setup_arm, "candidate")

    def test_dataset_action_replays_after_collection_and_rejects_wrapper_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "collected-run"
            origin = Path("/projectnb/welfgr/johannes/fereg-f0/runs/collected-run")
            run_dir = root / "logs/generate-generated_dense"
            generator = root / "work/setup/baseline/source/benchmarks/generate.do"
            output = root / "work/datasets/generated_dense.dta"
            run_dir.mkdir(parents=True)
            (run_dir / "controller-home").mkdir()
            (run_dir / "controller-tmp").mkdir()
            generator.parent.mkdir(parents=True)
            output.parent.mkdir(parents=True)
            generator.write_text("exit 0\n", encoding="ascii")
            output.write_bytes(b"sealed-dta")
            wrapper = self.scc._render_dataset_wrapper(
                origin / generator.relative_to(root), [10, 2],
                origin / output.relative_to(root), "generated_dense", "dense",
            )
            (run_dir / "generate.do").write_text(wrapper, encoding="utf-8")
            (run_dir / "runner.stdout").write_text("", encoding="utf-8")
            sentinel = "FEREG_F0_DATASET_SUCCESS|generated_dense"
            (run_dir / "generate.log").write_text(sentinel + "\n", encoding="utf-8")
            self.scc.write_json_exclusive(run_dir / "status.json", {
                "schema_version": 1, "process_returncode": 0,
                "terminal_sentinels": [sentinel],
            })
            self.scc.write_json_exclusive(run_dir / "invocation.json", {
                "schema_version": 1,
                "stata": {"path": str(self.scc.SCC_STATA), "sha256": self.scc.SCC_STATA_SHA256, "size_bytes": self.scc.SCC_STATA_SIZE},
                "argv": [str(self.scc.SCC_STATA), "-b", "do", str(origin / "logs/generate-generated_dense/generate.do")],
                "cwd": str(origin / "logs/generate-generated_dense"),
                "environment": {
                    "HOME": str(origin / "logs/generate-generated_dense/controller-home"),
                    "TMPDIR": str(origin / "logs/generate-generated_dense/controller-tmp"),
                    "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
                },
                "stream": "merged-stdout-stderr-v1",
            })
            action = {
                "receipt_type": "fereg-f0-dataset-action-v1",
                **{name: self.scc._relative_file_record(root, path) for name, path in {
                    "wrapper": run_dir / "generate.do", "invocation": run_dir / "invocation.json",
                    "stream": run_dir / "runner.stdout", "log": run_dir / "generate.log",
                    "status": run_dir / "status.json", "output": output,
                }.items()},
                "sentinel": sentinel,
            }
            record = {"sha256": self.scc.sha256_file(output), "size_bytes": output.stat().st_size, "action": action}
            self.scc._verify_dataset_action(root, origin, "generated_dense", record, generator, [10, 2], "dense")
            (run_dir / "generate.do").write_text(wrapper + "* forged\n", encoding="utf-8")
            with self.assertRaisesRegex(self.scc.SccContractError, "wrapper|bytes"):
                self.scc._verify_dataset_action(root, origin, "generated_dense", record, generator, [10, 2], "dense")

    def test_extracted_source_tree_must_equal_retained_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            source = base / "source"
            extracted = base / "extracted"
            source.mkdir()
            (source / "src").mkdir()
            (source / "src/lib.rs").write_text("pub fn exact() {}\n", encoding="ascii")
            archive = base / "source.tar"
            with tarfile.open(archive, "w") as handle:
                handle.add(source / "src", arcname="src")
            self.scc._safe_extract_tar(archive, extracted)
            self.scc._verify_extracted_archive_files(archive, extracted)
            (extracted / "src/lib.rs").write_text("pub fn forged() {}\n", encoding="ascii")
            with self.assertRaisesRegex(self.scc.SccContractError, "inventory"):
                self.scc._verify_extracted_archive_files(archive, extracted)

    def test_read_only_source_transform_preserves_archive_execute_semantics(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            source = base / "source"
            source.mkdir()
            script = source / "run.sh"
            script.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
            script.chmod(0o755)
            archive = base / "source.tar"
            with tarfile.open(archive, "w") as handle:
                handle.add(script, arcname="run.sh")
            extracted = base / "extracted"
            self.scc._safe_extract_tar(archive, extracted)
            self.scc._verify_extracted_archive_files(archive, extracted)
            self.scc._make_tree_read_only(extracted)
            self.assertEqual((extracted / "run.sh").stat().st_mode & 0o777, 0o555)
            self.scc._verify_extracted_archive_files(
                archive, extracted, read_only=True
            )

    def test_formal_dist_rejects_extra_files_and_parent_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            source = base / "source"
            source.mkdir()
            (source / "Cargo.toml").write_text("[workspace]\n", encoding="ascii")
            archive = base / "source.tar"
            with tarfile.open(archive, "w") as handle:
                handle.add(source / "Cargo.toml", arcname="Cargo.toml")
            extracted = base / "extracted"
            self.scc._safe_extract_tar(archive, extracted)
            dist = extracted / "dist"
            dist.mkdir()
            plugin = dist / "fereg_linux.plugin"
            plugin.write_bytes(b"plugin")
            plugin.chmod(0o755)
            digest = dist / "fereg_linux.plugin.sha256"
            digest.write_text(
                f"{self.scc.sha256_file(plugin)}  /original/source/dist/fereg_linux.plugin\n",
                encoding="ascii",
            )
            digest.chmod(0o644)
            self.scc._verify_extracted_archive_files(archive, extracted, allow_formal_dist=True)
            read_only_copy = base / "read-only-copy"
            shutil.copytree(extracted, read_only_copy)
            self.scc._make_tree_read_only(read_only_copy)
            self.scc._verify_extracted_archive_files(
                archive, read_only_copy, allow_formal_dist=True, read_only=True
            )
            (dist / "extra").write_text("forged\n", encoding="ascii")
            with self.assertRaisesRegex(self.scc.SccContractError, "dist inventory"):
                self.scc._verify_extracted_archive_files(archive, extracted, allow_formal_dist=True)
            (dist / "extra").unlink()
            shutil.rmtree(dist)
            outside = base / "outside"
            outside.mkdir()
            dist.symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(self.scc.SccContractError, "symlink"):
                self.scc._verify_extracted_archive_files(archive, extracted, allow_formal_dist=True)

    def test_extracted_source_root_symlink_fails_containment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "run"
            outside = Path(temporary) / "outside"
            root.mkdir()
            outside.mkdir()
            (outside / "Cargo.toml").write_text("[workspace]\n", encoding="ascii")
            archive = root / "source.tar"
            with tarfile.open(archive, "w") as handle:
                handle.add(outside / "Cargo.toml", arcname="Cargo.toml")
            linked = root / "source"
            linked.symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(self.scc.SccContractError, "real directory|symlink"):
                self.scc._verify_extracted_archive_files(
                    archive, linked, containment_root=root
                )

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

    def _qacct_record(
        self,
        job,
        task=None,
        failed="0",
        exit_status="0",
        *,
        owner="johannes",
        jobname="f0a_test",
        qsub_time="Sun Aug 31 12:00:00 2026",
        submit_cmd_line=None,
        cwd=None,
        scriptfile=None,
    ):
        rows = [
            "==============================================================",
            "qname shared.q",
            "hostname scc-test",
            f"owner {owner}",
            "project welfgr",
            f"jobname {jobname}",
            f"jobnumber {job}",
        ]
        if task is not None:
            rows.append(f"taskid {task}")
        rows.extend([
            "slots 16",
            f"failed {failed}",
            f"exit_status {exit_status}",
            f"qsub_time {qsub_time}",
            "start_time Sun Aug 31 12:00:01 2026",
            "end_time Sun Aug 31 12:00:13 2026",
            "ru_wallclock 12.0",
            "cpu 10.0",
            "maxvmem 2.0G",
        ])
        if submit_cmd_line is not None:
            rows.append(f"submit_cmd_line {submit_cmd_line}")
        if cwd is not None:
            rows.append(f"cwd {cwd}")
        if scriptfile is not None:
            rows.append(f"scriptfile {scriptfile}")
        return "\n".join(rows) + "\n"

    @staticmethod
    def _qacct_arguments():
        return {
            "expected_slots": 16,
            "expected_job_name": "f0a_test",
            "expected_script": Path("/projectnb/welfgr/fereg/runs/test/job.sh"),
            "expected_run_root": Path("/projectnb/welfgr/fereg/runs/test"),
            "expected_argv": [],
            "submission_started_at_utc": "2026-08-31T15:59:00Z",
            "submission_completed_at_utc": "2026-08-31T16:01:00Z",
        }

    def test_qacct_requires_closed_successful_array_inventory(self):
        text = "".join(self._qacct_record("456", task) for task in (1, 2, 3))
        records = self.scc.validate_qacct(
            text, "456", [1, 2, 3], **self._qacct_arguments()
        )
        self.assertEqual(len(records), 3)
        with self.assertRaisesRegex(self.scc.SccContractError, "incomplete"):
            self.scc.validate_qacct(
                text, "456", [1, 2, 3, 4], **self._qacct_arguments()
            )
        failed = self._qacct_record("456", 1, failed="1")
        with self.assertRaisesRegex(self.scc.SccContractError, "failed"):
            self.scc.validate_qacct(
                failed, "456", [1], **self._qacct_arguments()
            )
        wrong_owner = self._qacct_record("456", 1, owner="somebody")
        with self.assertRaisesRegex(self.scc.SccContractError, "wrong-project"):
            self.scc.validate_qacct(
                wrong_owner, "456", [1], **self._qacct_arguments()
            )
        outside = self._qacct_record(
            "456", 1, qsub_time="Sun Aug 31 11:00:00 2026"
        )
        with self.assertRaisesRegex(self.scc.SccContractError, "submission window"):
            self.scc.validate_qacct(
                outside, "456", [1], **self._qacct_arguments()
            )
        wrong_command = self._qacct_record(
            "456", 1, submit_cmd_line="qsub --not-the-sealed-command"
        )
        with self.assertRaisesRegex(self.scc.SccContractError, "differs from submission"):
            self.scc.validate_qacct(
                wrong_command, "456", [1], **self._qacct_arguments()
            )

    def test_qacct_fetch_waits_for_the_complete_array_inventory(self):
        partial = self._qacct_record("456", 1)
        complete = partial + self._qacct_record("456", 2)
        responses = [
            subprocess.CompletedProcess(["qacct"], 0, stdout=partial),
            subprocess.CompletedProcess(["qacct"], 0, stdout=complete),
        ]
        original = self.scc.subprocess.run
        self.scc.subprocess.run = lambda *args, **kwargs: responses.pop(0)
        with tempfile.TemporaryDirectory() as temp:
            qacct = Path(temp) / "qacct"
            qacct.write_bytes(b"exact-qacct-client")
            original_path = self.scc.EXPECTED_QACCT
            original_sha = self.scc.EXPECTED_QACCT_SHA256
            original_resolved = self.scc.EXPECTED_QACCT_RESOLVED
            original_size = self.scc.EXPECTED_QACCT_SIZE
            self.scc.EXPECTED_QACCT = qacct
            self.scc.EXPECTED_QACCT_SHA256 = self.scc.sha256_file(qacct)
            self.scc.EXPECTED_QACCT_RESOLVED = str(qacct.resolve())
            self.scc.EXPECTED_QACCT_SIZE = qacct.stat().st_size
            try:
                observed = self.scc._fetch_qacct(
                    "456",
                    [1, 2],
                    **self._qacct_arguments(),
                    attempts=2,
                    delay_seconds=0.0,
                )
            finally:
                self.scc.subprocess.run = original
                self.scc.EXPECTED_QACCT = original_path
                self.scc.EXPECTED_QACCT_SHA256 = original_sha
                self.scc.EXPECTED_QACCT_RESOLVED = original_resolved
                self.scc.EXPECTED_QACCT_SIZE = original_size
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

    def test_memory_allocation_uses_physical_rows_not_weighted_e_n(self):
        class Paired:
            @staticmethod
            def stata_hex_number(value):
                return float(value)

        captured = {
            "scalars": {"N": "300"},
            "matrices": [
                {"name": "fereg_fe", "column": 4, "value_hex": "7"},
                {"name": "fereg_cluster", "column": 3, "value_hex": "2"},
            ],
        }
        observed = self.scc._allocation_classes(
            Paired(), captured, {"coefficient_names": ["x1", "x2"]}, 100
        )
        self.assertEqual(observed["observations"], 100)
        self.assertNotEqual(observed["observations"], 300)
        with self.assertRaisesRegex(
            self.scc.SccContractError, "physical dataset observation"
        ):
            self.scc._allocation_classes(
                Paired(), captured, {"coefficient_names": []}, True
            )

    def _fake_run(self, root):
        manifest = self.scc.load_regression_manifest(MANIFEST_PATH)
        manifest_bytes = MANIFEST_PATH.read_bytes()
        (root / "input").mkdir(parents=True)
        (root / "input/harness").mkdir(parents=True)
        (root / "output/cells").mkdir(parents=True)
        (root / "receipts/tasks").mkdir(parents=True)
        for name in (
            "fereg_f0_setup_job.sh",
            "fereg_f0_job.sh",
            "fereg_f0_collector_job.sh",
        ):
            (root / "input/harness" / name).write_text(
                f"#!/bin/bash\n# {name}\n", encoding="ascii"
            )
        (root / "input/manifest.json").write_bytes(manifest_bytes)
        preparation = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-preparation-v1",
            "run_id": root.name,
            "project": "welfgr",
            "manifest": {
                "id": manifest["manifest_id"],
                "sha256": self.scc.sha256_bytes(manifest_bytes),
            },
            "tasks": {
                "count": len(self.scc.scc_cells(manifest)),
                "sha256": "9" * 64,
            },
            "arms": {},
            "harness": {},
        }
        self.scc.write_sealed(root / "input/preparation.json", preparation)
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
            "sge": {"job_id": "101", "host_id": "setup-host", "slots": 16},
            "terminal_sentinel": "FEREG_F0_SETUP_SUCCESS",
        }
        self.scc.write_sealed(root / "receipts/setup.json", setup)
        setup_sha = self.scc.sha256_file(root / "receipts/setup.json")
        cells = []
        for task_id, spec in enumerate(self.scc.scc_cells(manifest), 1):
            cell_id = spec["id"]
            cell = {
                "cell_id": cell_id,
                "environment": {"host_id": "task-host"},
                "scientific": {},
                "pairs": [],
                "confirmation_pairs": None,
                "memory": {},
            }
            cell_path = root / f"output/cells/{cell_id}.json"
            cell_path.write_text(json.dumps(cell), encoding="utf-8")
            task_root = root / f"work/tasks/task-{task_id:03d}-{cell_id}"
            task_root.mkdir(parents=True)
            (task_root / "raw.txt").write_text(
                f"raw evidence for {cell_id}\n", encoding="ascii"
            )
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
                "environment": cell["environment"],
                "raw_files": self.scc._raw_inventory(task_root),
                "cell_sha256": self.scc.sha256_file(cell_path),
                "sge": {
                    "job_id": "102",
                    "task_id": str(task_id),
                    "host_id": "task-host",
                    "slots": 16,
                },
                "terminal_sentinel": f"FEREG_F0_TASK_SUCCESS|{cell_id}",
            }
            self.scc.write_sealed(root / f"receipts/tasks/task-{task_id:03d}.json", receipt)
            task_path = root / f"receipts/tasks/task-{task_id:03d}.json"
            (root / f"receipts/tasks/task-{task_id:03d}.SUCCESS").write_text(
                self.scc._receipt_sentinel(
                    task_path, f"FEREG_F0_TASK_SUCCESS|{cell_id}"
                ),
                encoding="ascii",
            )
            cells.append(cell)
        qsub_root = root / "receipts/qsub"
        qsub_root.mkdir()
        jobs = {
            "setup": "101",
            "array": "102",
            "collector": "103",
        }
        for stage, job_id in jobs.items():
            (qsub_root / f"{stage}.txt").write_text(
                f"{job_id}\n", encoding="ascii"
            )
            script = self.scc._submitted_script_records(root)[stage]["path"]
            name = self.scc._submission_job_names(root.name)[stage]
            h_rt = {"setup": 14400, "array": 42300, "collector": 1800}[stage]
            extra = []
            if stage != "collector":
                extra.append("parallel_environment: omp range: 16")
            if stage == "array":
                extra.append(f"job-array tasks: 1-{len(self.scc.scc_cells(manifest))}:1")
                extra.append(f"jid_predecessor_list (req): {jobs['setup']}")
            elif stage == "collector":
                extra.append(f"jid_predecessor_list (req): {jobs['array']}")
            (qsub_root / f"{stage}.qstat.txt").write_text(
                "\n".join(
                    (
                        f"job_number: {job_id}",
                        "submission_time: Mon Aug 31 12:00:00 2026",
                        "owner: johannes",
                        f"cwd: {root}",
                        "merge: y",
                        f"hard resource_list: h_rt={h_rt},mem_per_core=4G",
                        f"job_name: {name}",
                        f"stdout_path_list: NONE:NONE:{root}/logs",
                        f"env_list: RUN_ROOT={root}",
                        f"script_file: {root}/{script}",
                        "project: welfgr",
                        "shell_list: NONE:/bin/bash",
                        *extra,
                        "",
                    )
                ),
                encoding="utf-8",
            )
        (qsub_root / "started-at-utc.txt").write_text(
            "2026-08-31T15:59:00Z\n", encoding="ascii"
        )
        (qsub_root / "completed-at-utc.txt").write_text(
            "2026-08-31T16:01:00Z\n", encoding="ascii"
        )
        submission_ledger, parsed_jobs = self.scc._submission_ledger(root)
        self.assertEqual(parsed_jobs, jobs)
        submission = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-submission-v1",
            "run_id": root.name,
            "owner": "johannes",
            "project": "welfgr",
            "manifest": preparation["manifest"],
            "preparation_receipt_sha256": self.scc.sha256_file(
                root / "input/preparation.json"
            ),
            "jobs": jobs,
            "job_names": self.scc._submission_job_names(root.name),
            "scripts": self.scc._submitted_script_records(root),
            "array_tasks": len(self.scc.scc_cells(manifest)),
            "resources": {
                "setup": {
                    "slots": 16,
                    "mem_per_core": "4G",
                    "h_rt": "04:00:00",
                },
                "array": {
                    "slots": 16,
                    "mem_per_core": "4G",
                    "h_rt": "11:45:00",
                },
                "collector": {
                    "slots": 1,
                    "mem_per_core": "4G",
                    "h_rt": "00:30:00",
                },
            },
            "submission_ledger": submission_ledger,
        }
        self.scc.write_sealed(root / "receipts/submission.json", submission)
        return manifest, setup, cells

    def test_submission_replay_is_relocation_aware_and_origin_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            run_id = "20260831T120009Z-test"
            original = workspace / "scc" / run_id
            original.mkdir(parents=True)
            self._fake_run(original)
            preparation = self.scc.read_sealed(
                original / "input/preparation.json",
                "fereg-f0-scc-preparation-v1",
            )
            original_submission = self.scc.validate_submission_binding(
                original, preparation
            )

            collected = workspace / "collected" / run_id
            collected.parent.mkdir()
            shutil.copytree(original, collected)
            copied_preparation = self.scc.read_sealed(
                collected / "input/preparation.json",
                "fereg-f0-scc-preparation-v1",
            )
            replayed = self.scc.validate_submission_binding(
                collected, copied_preparation
            )
            self.assertEqual(replayed, original_submission)
            ledger = replayed["submission_ledger"]
            submitted_root = Path(ledger["submit_cwd"])
            stage = "array"
            script = submitted_root / replayed["scripts"][stage]["path"]
            qacct = self._qacct_record(
                replayed["jobs"][stage],
                1,
                jobname=replayed["job_names"][stage],
                submit_cmd_line=shlex.join(ledger["argv"][stage]),
                cwd=str(submitted_root),
                scriptfile=str(script),
            )
            records = self.scc.validate_qacct(
                qacct,
                replayed["jobs"][stage],
                [1],
                expected_slots=16,
                expected_job_name=replayed["job_names"][stage],
                expected_script=script,
                expected_run_root=submitted_root,
                expected_argv=ledger["argv"][stage],
                submission_started_at_utc=ledger["started_at_utc"],
                submission_completed_at_utc=ledger["completed_at_utc"],
            )
            self.assertEqual(len(records), 1)
            with self.assertRaisesRegex(
                self.scc.SccContractError, "qacct cwd differs"
            ):
                self.scc.validate_qacct(
                    qacct,
                    replayed["jobs"][stage],
                    [1],
                    expected_slots=16,
                    expected_job_name=replayed["job_names"][stage],
                    expected_script=collected
                    / replayed["scripts"][stage]["path"],
                    expected_run_root=collected,
                    expected_argv=ledger["argv"][stage],
                    submission_started_at_utc=ledger["started_at_utc"],
                    submission_completed_at_utc=ledger["completed_at_utc"],
                )

    def test_task_collector_rejects_missing_and_tampered_shards(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "20260831T120000Z-test"
            root.mkdir()
            manifest, setup, cells = self._fake_run(root)
            reconstruct = lambda run_root, _manifest, _setup, cell, _task_id, *_args: json.loads(
                (run_root / f"output/cells/{cell['id']}.json").read_text()
            )
            with mock.patch.object(
                self.scc, "_reconstruct_task_cell", side_effect=reconstruct
            ):
                observed = self.scc.validate_task_inventory(root, manifest, setup)
            self.assertEqual(observed, cells)

            victim = root / "output/cells/dense_two_fe_t1_fast_fit.json"
            victim.write_text("{}\n", encoding="utf-8")
            with mock.patch.object(
                self.scc, "_reconstruct_task_cell", side_effect=reconstruct
            ):
                with self.assertRaisesRegex(self.scc.SccContractError, "hash mismatch"):
                    self.scc.validate_task_inventory(root, manifest, setup)

    def test_submission_and_task_job_ids_are_cryptographically_cross_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "20260831T120001Z-test"
            root.mkdir()
            manifest, setup, _ = self._fake_run(root)
            preparation = self.scc.read_sealed(
                root / "input/preparation.json", "fereg-f0-scc-preparation-v1"
            )
            submission = self.scc.validate_submission_binding(root, preparation)
            reconstruct = lambda run_root, _manifest, _setup, cell, _task_id, *_args: json.loads(
                (run_root / f"output/cells/{cell['id']}.json").read_text()
            )
            with mock.patch.object(
                self.scc, "_reconstruct_task_cell", side_effect=reconstruct
            ):
                self.scc.validate_task_inventory(root, manifest, setup, submission)

            path = root / "receipts/submission.json"
            path.unlink()
            changed = dict(submission)
            changed["jobs"] = dict(changed["jobs"])
            changed["jobs"]["array"] = "999"
            self.scc.write_sealed(path, changed)
            with self.assertRaisesRegex(
                self.scc.SccContractError, "raw qsub job IDs"
            ):
                self.scc.validate_submission_binding(root, preparation)

    def test_lifecycle_has_distinct_nonqualifying_and_success_closure_markers(self):
        source = MODULE_PATH.read_text(encoding="utf-8")
        self.assertIn('"artifacts_sealed"', source)
        self.assertIn('"fereg-f0-scc-baseline-evidence-v1"', source)
        self.assertIn('"complete_nonqualifying"', source)
        self.assertIn("BASELINE_EVIDENCE.COMPLETE", source)
        self.assertIn('"FEREG_F0_SCC_BASELINE_EVIDENCE_COMPLETE"', source)
        self.assertIn("RUN.SUCCESS", source)
        self.assertIn('"FEREG_F0_SCC_RUN_SUCCESS"', source)

    def test_staged_bundle_is_reconstructed_from_exact_source_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source/stata"
            source.mkdir(parents=True)
            (source / "fereg.ado").write_text("program fereg\nend\n", encoding="ascii")
            archive = root / "source.tar"
            with tarfile.open(archive, "w") as handle:
                handle.add(source, arcname="stata")
            bundle = root / "bundle"
            shutil.copytree(source, bundle / "stata")
            plugin = bundle / "stata/fereg_linux.plugin"
            plugin.write_bytes(b"exact-plugin")

            def receipt():
                digest, size, inventory = self.scc._bundle_digest(bundle)
                return {
                    "artifact": {
                        "sha256": self.scc.sha256_file(plugin),
                        "size_bytes": plugin.stat().st_size,
                    },
                    "bundle": {
                        "sha256": digest,
                        "size_bytes": size,
                        "inventory": inventory,
                    },
                }

            self.scc._verify_staged_bundle(bundle, receipt(), archive)
            (bundle / "stata/fereg.ado").write_text(
                "program substituted\nend\n", encoding="ascii"
            )
            with self.assertRaisesRegex(
                self.scc.SccContractError, "exact source archive"
            ):
                self.scc._verify_staged_bundle(bundle, receipt(), archive)

    def test_sealed_preparation_materializes_a_closed_external_evidence_root(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            source_root = workspace / "external"
            destination = workspace / "copied"
            source_root.mkdir()

            def write(relative, content=b"evidence\n"):
                path = source_root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
                return {
                    "path": relative,
                    "sha256": self.scc.sha256_file(path),
                    "size_bytes": path.stat().st_size,
                }

            local = write("local/receipt.json", b"{}\n")
            observation = write("behavior/observation.json", b"{}\n")
            bundle = write("behavior/bundle.tar")
            attestation = write("platform/macos/attestation.json", b"{}\n")
            producer = write("platform/macos/producer.py")
            for suffix in (".status", ".stata.log", ".wrapper.do", ".invocation.json"):
                write(attestation["path"] + suffix)
            windows_artifact = write("platform/windows/plugin.plugin")
            (source_root / "platform/linux-run").mkdir(parents=True)
            write("platform/linux-run/receipts/close.json", b"{}\n")
            artifact_payload = {
                "evidence": {
                    "local_full_receipt": local,
                    "direct_behavior": {
                        "observation": observation,
                        "bundle": bundle,
                        "verified_observation": {},
                    },
                    "macos": {
                        "native_attestation": attestation,
                        "native_producer": producer,
                        "verified_receipt": {},
                    },
                    "linux": {
                        "run_root": "platform/linux-run",
                        "verified_close": {},
                    },
                    "windows": {
                        "artifact": windows_artifact,
                        "verified_receipt": {},
                    },
                }
            }
            artifact_path = source_root / "seals/artifact.json"
            artifact_path.parent.mkdir(parents=True)
            artifact_path.write_text(
                json.dumps({"payload": artifact_payload, "seal": {}}),
                encoding="ascii",
            )
            artifact_binding = {
                "path": "seals/artifact.json",
                "sha256": self.scc.sha256_file(artifact_path),
                "size_bytes": artifact_path.stat().st_size,
            }
            manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
            manifest["status"] = "artifacts_sealed"
            manifest["sealing"]["observed_local_evidence"][0][
                "receipt_path"
            ] = local["path"]
            manifest["sealing"]["observed_local_evidence"][0][
                "receipt_sha256"
            ] = local["sha256"]
            manifest["sealing"]["artifact_seal"] = artifact_binding
            manifest["sealing"]["qualification_seal"] = None

            closure = self.scc._materialize_seal_evidence(
                manifest, source_root, destination
            )
            expected = {
                path.relative_to(destination).as_posix()
                for path in destination.rglob("*")
                if path.is_file()
            }
            self.assertEqual(
                expected, {record["path"] for record in closure["files"]}
            )
            self.assertIn("platform/macos/attestation.json.status", expected)
            self.assertIn("platform/linux-run/receipts/close.json", expected)

    def test_shell_templates_encode_scc_policy(self):
        setup = (ROOT / "benchmarks/scc/fereg_f0_setup_job.sh").read_text()
        task = (ROOT / "benchmarks/scc/fereg_f0_job.sh").read_text()
        collector = (ROOT / "benchmarks/scc/fereg_f0_collector_job.sh").read_text()
        submit = (ROOT / "benchmarks/scc/fereg_f0_submit.sh").read_text()
        deploy = (ROOT / "benchmarks/scc/fereg_f0_deploy.sh").read_text()
        close = (ROOT / "benchmarks/scc/fereg_f0_close.sh").read_text()
        development = (ROOT / "benchmarks/scc/fereg_dev_smoke_job.sh").read_text()
        for source in (setup, task, collector):
            self.assertIn("#$ -P welfgr", source)
        self.assertIn("module load stata-mp/19", setup)
        self.assertIn("fereg-rust-1.85.1", setup)
        self.assertIn("RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu", setup)
        self.assertIn('test "$(rustc --version)" = "rustc 1.85.1', setup)
        self.assertIn("module load stata-mp/19", task)
        self.assertIn("--task-id \"$SGE_TASK_ID\"", task)
        self.assertIn("-hold_jid \"$SETUP_JOB\"", submit)
        self.assertIn("-hold_jid \"$ARRAY_JOB\"", submit)
        self.assertIn('-t "1-$TASK_COUNT"', submit)
        self.assertIn('-N "$SETUP_NAME"', submit)
        self.assertIn('-wd "$RUN_ROOT"', submit)
        self.assertNotIn('receipts/qsub/smoke.txt', submit)
        self.assertIn('receipts/qsub/started-at-utc.txt', submit)
        self.assertIn('record-submission \\\n    --run-root "$RUN_ROOT"', submit)
        self.assertEqual(submit.count('"$QSUB" -clear '), 3)
        self.assertIn("-S /bin/bash -j y", submit)
        self.assertEqual(submit.count("-pe omp 16"), 2)
        self.assertNotIn("-pe omp 1 ", submit)
        self.assertIn("h_rt=04:00:00", submit)
        self.assertEqual(submit.count("h_rt=11:45:00"), 1)
        self.assertIn("h_rt=00:30:00", submit)
        self.assertEqual(submit.count("mem_per_core=4G"), 3)
        self.assertIn("SCHEDULER_ENV=(/usr/bin/env -i", submit)
        self.assertIn("QSUB=/usr/local/bin/qsub", submit)
        self.assertNotIn('--smoke-job', submit)
        self.assertNotIn('SMOKE_JOB', submit)
        self.assertNotIn('--qacct-dir', MODULE_PATH.read_text(encoding="utf-8"))
        self.assertNotIn(" -V", submit)
        for source in (setup, task, collector, submit, close):
            self.assertNotIn('python3 "$RUN_ROOT', source)
            self.assertIn('python3 -I -B -S "$RUN_ROOT', source)
        self.assertNotIn("--delete", deploy)
        self.assertNotIn("--info=progress2", deploy)
        self.assertIn("--progress", deploy)
        self.assertIn("verify-input", deploy)
        self.assertIn(" qacct", close)
        for source in (setup, task, collector, submit, close):
            self.assertIn("^/projectnb/welfgr/fereg/runs/", source)
        self.assertIn("#$ -pe omp 4", development)
        self.assertIn("#$ -l h_rt=00:30:00", development)
        self.assertIn("#$ -l mem_per_core=4G", development)
        self.assertIn("FEREG_DEV_PROFILE must be ols, ppml, or shared", development)
        self.assertIn("ols) PROFILE=scc-dev-ols", development)
        self.assertIn("ppml) PROFILE=scc-dev-ppml", development)
        self.assertIn("shared) PROFILE=scc-dev-shared", development)
        self.assertNotIn("${FEREG_DEV_PROFILE:-", development)

    def test_controller_cli_rejects_direct_nonisolated_api(self):
        with self.assertRaisesRegex(SystemExit, "Python -I -B -S"):
            self.scc.main([])

    def test_formal_build_environment_drops_hostile_ambient_overrides(self):
        tools = {
            role: {"path": f"/sealed/tools/{role}"}
            for role in self.scc.BUILD_TOOL_ROLES
        }
        hostile = {
            "RUSTC_WRAPPER": "/tmp/forge-rustc",
            "RUSTFLAGS": "--cfg forged",
            "CARGO_TARGET_DIR": "/tmp/forged-target",
            "PYTHONPATH": "/tmp/sitecustomize",
            "LD_PRELOAD": "/tmp/forge.so",
        }
        with mock.patch.dict(os.environ, hostile, clear=False):
            env = self.scc._formal_build_environment(
                Path("/projectnb/welfgr/fereg/runs/20260831T000000Z-test"),
                {"commit": "a" * 40, "build_date": "2026-08-31"},
                tools,
                arm="candidate",
                slots=16,
            )
        for name in hostile:
            self.assertNotIn(name, env)
        self.assertEqual(env["RUSTC"], "/sealed/tools/rustc")
        self.assertEqual(env["CC"], "/sealed/tools/cc")
        self.assertEqual(env["PATH"].split(":")[0], "/sealed/tools")

    def test_git_dependency_vendor_path_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "20260831T000000Z-test"
            source = root / "work/setup/baseline/source"
            source.mkdir(parents=True)
            (source / "Cargo.lock").write_text(
                'version = 3\n\n[[package]]\nname = "forged"\n'
                'source = "git+https://example.invalid/dep#' + "a" * 40 + '"\n',
                encoding="ascii",
            )
            with self.assertRaisesRegex(
                self.scc.SccContractError, "independently sealed canonical archives"
            ):
                self.scc._prepare_dependency_closure(
                    root, source, {}, arm="baseline"
                )

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
