from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("auto_calibration", ROOT / "scripts/ppml_auto_calibration.py")
assert SPEC and SPEC.loader
AUTO = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUTO)


def row(index: int, map_time=1.5, cmg_time=1.5) -> dict:
    return {"scope_id": str(index), "dataset_id": str(index), "reference": 2.0,
            "features": {"effects": 2, "controls": 10, "threads": 7,
                         "observations_per_largest_level": 50, "core_level_ratio": 3,
                         "nonbase_nuisance_columns": 0},
            "times": {"map": map_time, "lsmr": 1.0, "cmg": cmg_time}}


class AutoCalibrationTests(unittest.TestCase):
    def test_manifest_and_closed_inventory(self):
        self.assertFalse(AUTO.manifest()["qualification_eligible"])
        self.assertEqual(len(AUTO.scopes("scc_train")), 146)
        self.assertEqual(len(AUTO.scopes("scc_holdout")), 264)
        for stage in ("scc_train", "scc_holdout"):
            self.assertEqual([s for t in range(1, AUTO.task_count(stage) + 1)
                              for s in AUTO.scopes(stage, t)], AUTO.scopes(stage))

    def test_repartition_preserves_every_previous_scope_and_repetition(self):
        old = json.loads((ROOT / "benchmarks/manifests/feppml_auto_calibration_v2.json").read_text())
        m = AUTO.manifest()
        training = [s for p in old["local_training_profiles"]
                    for s in AUTO.dev.profile_scopes(AUTO.dev.load_manifest(), p)]
        self.assertEqual(AUTO.scopes("scc_train"), training)
        self.assertEqual(AUTO.scopes("scc_holdout"),
                         AUTO.holdout_scopes(old, "local_holdout") + AUTO.holdout_scopes(old, "scc"))
        self.assertEqual(m["training_repetitions"], old["training_repetitions"])
        self.assertEqual(m["holdout_repetitions"], old["holdout_repetitions"])
        self.assertEqual(m["policy"], old["policy"])

    def test_expensive_scopes_are_isolated_and_jobs_bounded(self):
        m = AUTO.manifest()
        self.assertEqual(m["scheduler"]["h_rt"], "12:00:00")
        self.assertEqual(m["local_validation"]["maximum_seconds"], 14400)
        for stage in ("scc_train", "scc_holdout"):
            tasks = AUTO.partition_scopes(AUTO.scopes(stage), m["partition"])
            self.assertTrue(all(1 <= len(t) <= 4 for t in tasks))
            for task in tasks:
                if any(s["topology"] == "many_levels_2fe" and s["controls"] >= 50 for s in task):
                    self.assertEqual(len(task), 1)

    def test_local_budget_rejected_before_any_process_or_output(self):
        from argparse import Namespace
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(AUTO.subprocess, "Popen") as spawn:
            output = Path(temporary) / "local"
            for budget in (0, -1, 14401):
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.run_local_validation(Namespace(budget_seconds=budget, output_dir=output))
            spawn.assert_not_called()
            self.assertFalse(output.exists())

    def test_local_gate_inventory_includes_failed_modelspace_regression(self):
        jobs = AUTO.local_validation_jobs()
        self.assertEqual(len(jobs), 4)
        self.assertIn("checkpoint-sparse_3fe-n16384", jobs[-1][2])

    def test_local_budget_timeout_cannot_publish_success(self):
        from argparse import Namespace
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "local"
            process = mock.Mock(pid=123456)
            process.wait.side_effect = [AUTO.subprocess.TimeoutExpired("local", 1), 0]
            args = Namespace(budget_seconds=1, output_dir=output, candidate_root=ROOT,
                             reference_root=Path(temporary), stata="stata")
            with mock.patch.object(AUTO, "identity", return_value={}), \
                    mock.patch.object(AUTO.subprocess, "Popen", return_value=process), \
                    mock.patch.object(AUTO.os, "killpg") as kill:
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.run_local_validation(args)
                kill.assert_called_once_with(123456, AUTO.signal.SIGTERM)
                self.assertFalse((output / "local-validation.json").exists())

    def test_local_scientific_failure_stops_before_later_gates(self):
        from argparse import Namespace
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "local"
            process = mock.Mock()
            process.wait.return_value = 1
            args = Namespace(budget_seconds=100, output_dir=output, candidate_root=ROOT,
                             reference_root=Path(temporary), stata="stata")
            with mock.patch.object(AUTO, "identity", return_value={}), \
                    mock.patch.object(AUTO.subprocess, "Popen", return_value=process) as spawn:
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.run_local_validation(args)
                self.assertEqual(spawn.call_count, 1)
                self.assertFalse((output / "local-validation.json").exists())

    def test_scheduler_host_and_task_are_bound_to_the_measured_process(self):
        identity = {"job_id": "100", "task_id": 2, "slots": 28, "host": "node-a"}
        accounting = [{"jobnumber": "100", "taskid": "2", "hostname": "node-a.bu.edu"}]
        with mock.patch.object(AUTO.dev, "read_json", return_value={"task_identity": identity}):
            AUTO.verify_task_accounting(Path("unused"), 2, accounting)
            for field, wrong in (("job_id", "101"), ("task_id", 1), ("slots", 7), ("host", "node-b")):
                changed = {"task_identity": {**identity, field: wrong}}
                with mock.patch.object(AUTO.dev, "read_json", return_value=changed):
                    with self.assertRaises(AUTO.dev.DevelopmentError):
                        AUTO.verify_task_accounting(Path("unused"), 2, accounting)

    def test_dependencies_freeze_policy_before_holdouts(self):
        script = (ROOT / "benchmarks/scc/feppml_auto_calibration_submit.sh").read_text()
        self.assertIn('-hold_jid "$setup" -t "1-$train_tasks"', script)
        self.assertIn('-hold_jid "$policy_dependencies"', script)
        self.assertIn('policy_dependencies="$train,$original_train"', script)
        self.assertIn('-hold_jid "$policy" -t "1-$holdout_tasks"', script)
        self.assertIn('-hold_jid "$holdout"', script)
        self.assertNotIn("--local-holdout", script)
        for name in ("job", "setup_job", "policy_job", "collect_job"):
            wrapper = (ROOT / f"benchmarks/scc/feppml_auto_calibration_{name}.sh").read_text()
            self.assertIn("#$ -P welfgr", wrapper)
            self.assertIn("#$ -l h_rt=", wrapper)
        setup = (ROOT / "benchmarks/scc/feppml_auto_calibration_setup_job.sh").read_text()
        self.assertIn('CARGO_BUILD_JOBS="$NSLOTS"', setup)
        self.assertIn('RAYON_NUM_THREADS="$NSLOTS"', setup)

    def test_exact_core_restriction_and_receipt_first_release(self):
        for name in ("job", "placement_job"):
            text = (ROOT / f"benchmarks/scc/feppml_auto_calibration_{name}.sh").read_text()
            self.assertIn("#$ -pe omp 28", text)
            self.assertIn("#$ -l num_proc=28", text)
            self.assertIn("#$ -l m_core=28", text)
        text = (ROOT / "benchmarks/scc/feppml_auto_calibration_submit.sh").read_text()
        self.assertTrue(all("qsub -terse -h " in s for s in text.splitlines() if "$(qsub " in s))
        self.assertLess(text.index('record-submission'), text.index('qrls "$train"'))
        self.assertIn("accept-placement --run-root", text)

    def test_recovery_maps_only_retry_tasks_and_preserves_holdouts(self):
        config = {"recovery": {"retry_tasks": [3, 8, 13]}}
        self.assertEqual([AUTO.scheduled_task(config, "scc_train", i) for i in (1, 2, 3)], [3, 8, 13])
        self.assertEqual(AUTO.scheduled_task(config, "scc_holdout", 3), 3)
        self.assertEqual(AUTO.scheduled_task({}, "scc_train", 7), 7)
        for index in (0, 4):
            with self.assertRaises(AUTO.dev.DevelopmentError):
                AUTO.scheduled_task(config, "scc_train", index)

    def test_recovery_accounting_keeps_physical_scheduler_identity(self):
        identity = {"job_id": "100", "task_id": 2, "slots": 28, "host": "node-a"}
        rows = [{"jobnumber": "100", "taskid": "2", "hostname": "node-a.bu.edu"}]
        with mock.patch.object(AUTO.dev, "read_json", return_value={"task_identity": identity}):
            AUTO.verify_task_accounting(Path("unused"), 8, rows, 2)
            with self.assertRaises(AUTO.dev.DevelopmentError):
                AUTO.verify_task_accounting(Path("unused"), 8, rows, 8)

    def test_only_proven_pre_estimation_placement_failures_are_retryable(self):
        row = {k: "0" for k in AUTO.dev.load_manifest()["receipt_contract"]["qacct_required"]}
        row.update(jobnumber="100", taskid="3", owner="johannes", project="welfgr", slots="28",
                   failed="0", exit_status="1", ru_wallclock="2")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "logs").mkdir()
            (root / "work").mkdir()
            log = root / "logs/feppml_auto_cal.o100.3"
            host = root / "work/qhost-scc_train-003.txt"
            log.write_text("PPML auto calibration failed: calibration requires a 28-core host\n")
            host.write_text("hl:m_core=32\n")
            self.assertEqual(AUTO.validate_placement_history(root, [row], "100", complete=False), [3])
            for key, value in (("failed", "1"), ("exit_status", "2"), ("ru_wallclock", "11"),
                               ("ru_wallclock", "nan"), ("owner", "other"), ("slots", "7")):
                with self.subTest(key=key, value=value), self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.validate_placement_history(root, [{**row, key: value}], "100", complete=False)
            for text in ("hl:m_core=28", "hl:m_core=nan", "hl:m_core=32 hl:m_core=96"):
                host.write_text(text)
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.validate_placement_history(root, [row], "100", complete=False)
            host.write_text("hl:m_core=32")
            log.write_text("calibration task failed science or route checks")
            with self.assertRaises(AUTO.dev.DevelopmentError):
                AUTO.validate_placement_history(root, [row], "100", complete=False)
            log.write_text("PPML auto calibration failed: calibration requires a 28-core host\n")
            (root / "shards/scc_train/task-003").mkdir(parents=True)
            with self.assertRaises(AUTO.dev.DevelopmentError):
                AUTO.validate_placement_history(root, [row], "100", complete=False)

    def test_original_accounting_must_be_complete_before_policy(self):
        row = {k: "0" for k in AUTO.dev.load_manifest()["receipt_contract"]["qacct_required"]}
        row.update(jobnumber="100", taskid="1", owner="johannes", project="welfgr", slots="28",
                   failed="0", exit_status="0", ru_wallclock="2")
        root = Path("unused")
        self.assertEqual(AUTO.validate_placement_history(root, [row], "100", complete=False), [])
        for rows in ([row], [row, row], [{**row, "taskid": "39"}]):
            with self.assertRaises(AUTO.dev.DevelopmentError):
                AUTO.validate_placement_history(root, rows, "100", complete=True)
        rows = [{**row, "taskid": str(i)} for i in range(1, 39)]
        self.assertEqual(AUTO.validate_placement_history(root, rows, "100", complete=True), [])

    def test_controller_separation_rejects_scientific_or_manifest_changes(self):
        original = {"crates/a.rs": "same", "scripts/ppml_auto_calibration.py": "old",
                    "benchmarks/manifests/feppml_auto_calibration_v3.json": "same"}
        candidate = Path("candidate")
        changed = {**original, "scripts/ppml_auto_calibration.py": "new"}
        with mock.patch.object(AUTO.dev, "source_identity", return_value={}), \
                mock.patch.object(AUTO, "source_files", side_effect=lambda r: original if r == candidate else changed):
            AUTO.controller_identity(candidate)
            for name in ("crates/a.rs", "benchmarks/manifests/feppml_auto_calibration_v3.json"):
                changed[name] = "different"
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.controller_identity(candidate)
                changed[name] = "same"

    def test_recovered_policy_replays_every_original_or_retry_task_before_publication(self):
        from argparse import Namespace
        root, original = Path("recovery"), Path("original")
        config = {"recovery": {"original_root": str(original), "retry_tasks": [3, 8]}}
        def read(path):
            return {"source": {"sha256": "same"}} if path.name == "setup.json" else {"jobs": {"policy": "100"}}
        with mock.patch.object(AUTO.dev, "validate_run_root", side_effect=lambda p: p), \
                mock.patch.object(AUTO, "check_controller", return_value=config), \
                mock.patch.object(AUTO, "task_count", return_value=38), \
                mock.patch.object(AUTO, "accept_accounting", return_value=[]), \
                mock.patch.object(AUTO, "placement_history", return_value=([], [3, 8])) as history, \
                mock.patch.object(AUTO.dev, "read_json", side_effect=read), \
                mock.patch.dict(AUTO.os.environ, {"JOB_ID": "100"}), \
                mock.patch.object(AUTO, "verify_task_accounting") as account, \
                mock.patch.object(AUTO, "verify_host_receipt"), \
                mock.patch.object(AUTO, "verify_measurement", return_value={"source": {"sha256": "same"}}) as replay, \
                mock.patch.object(AUTO, "fit_runs") as fit, \
                mock.patch.object(AUTO.dev, "sha256_file", return_value="policy"), \
                mock.patch.object(AUTO.dev, "exclusive_json"), \
                mock.patch.object(AUTO.dev, "exclusive_text") as success:
            AUTO.fit_scc(Namespace(run_root=root))
            history.assert_called_once_with(original, root, [3, 8], complete=True)
            self.assertEqual(replay.call_count, 38)
            self.assertEqual(account.call_args_list[2].args[3], 1)
            self.assertEqual(account.call_args_list[7].args[3], 2)
            runs = fit.call_args.args[0].runs
            self.assertEqual(runs[2], root / "shards/scc_train/task-003")
            self.assertEqual(runs[6], original / "shards/scc_train/task-007")
            fit.reset_mock(); success.reset_mock()
            replay.side_effect = AUTO.dev.DevelopmentError("failed certificate")
            with self.assertRaises(AUTO.dev.DevelopmentError):
                AUTO.fit_scc(Namespace(run_root=root))
            fit.assert_not_called(); success.assert_not_called()

    def test_submitted_configuration_mutation_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "input").mkdir(); (root / "receipts").mkdir()
            config = root / "input/config.json"
            config.write_text('{}')
            (root / "receipts/submission.json").write_text(json.dumps({"config": AUTO.dev.file_receipt(config)}))
            config.write_text('{"changed": true}')
            with self.assertRaisesRegex(AUTO.dev.DevelopmentError, "configuration changed"):
                AUTO.check_controller(root)

    def test_host_resources_are_recorded_without_assuming_cpu_architecture(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "qhost.txt"
            for arch in ("broadwell", "other"):
                path.write_text(f"  hl:m_core=28\n  hl:cpu_arch={arch}\n  hl:cpu_type=test\n")
                receipt = AUTO.host_receipt(path)
                self.assertEqual(receipt["resources"]["cpu_arch"], arch)
                self.assertEqual(receipt["sha256"], AUTO.dev.sha256_file(path))
            for text in ("hl:m_core=28", "hl:m_core=56 hl:cpu_arch=x hl:cpu_type=y",
                         "hl:m_core=28 hl:m_core=56 hl:cpu_arch=x hl:cpu_type=y"):
                path.write_text(text)
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.host_receipt(path)

    def test_fresh_holdout_has_no_training_dataset(self):
        training = {AUTO.dev.dataset_id(s) for s in AUTO.scopes("scc_train")}
        holdout = {AUTO.dev.dataset_id(s) for s in AUTO.holdout_scopes(AUTO.manifest(), "local_holdout")}
        cluster = {AUTO.dev.dataset_id(s) for s in AUTO.holdout_scopes(AUTO.manifest(), "scc")}
        self.assertFalse(training & holdout or training & cluster or holdout & cluster)

    def test_thread_comparisons_share_data(self):
        for stage in ("scc_holdout",):
            grouped = {}
            for scope in AUTO.scopes(stage):
                key = (scope["topology"], scope["n"], scope["controls"], scope["seed"])
                grouped.setdefault(key, set()).add(AUTO.dev.dataset_id(scope))
            self.assertTrue(all(len(values) == 1 for values in grouped.values()))

    def test_task_bounds(self):
        for stage in ("scc_train", "scc_holdout"):
            for task in (0, AUTO.task_count(stage) + 1):
                with self.assertRaises(AUTO.dev.DevelopmentError):
                    AUTO.scopes(stage, task)

    def test_features_preserve_fe_permutation(self):
        scope = {"controls": 50, "threads": 7}
        a = AUTO.features(scope, {"N": 25000, "levels1": 800, "levels2": 65, "levels3": 400})
        b = AUTO.features(scope, {"N": 25000, "levels1": 65, "levels2": 400, "levels3": 800})
        self.assertEqual(a, b)
        self.assertEqual(a["nonbase_nuisance_columns"], 64)
        self.assertNotIn("topology", a)

    def test_default_and_no_winning_region(self):
        rows = [row(i) for i in range(8)]
        policy = AUTO.fit(rows)
        self.assertEqual(policy["regions"], [])
        self.assertEqual(AUTO.select(rows[0], policy), "lsmr")
        rows[0]["features"]["effects"] = 1
        self.assertEqual(AUTO.select(rows[0], policy), "map")

    def test_material_supported_region_is_deterministic(self):
        rows = [row(i, cmg_time=0.7) for i in range(8)]
        policy = AUTO.fit(rows)
        self.assertEqual(policy, AUTO.fit(copy.deepcopy(rows)))
        self.assertEqual(policy["regions"][0]["solver"], "cmg")
        self.assertEqual(AUTO.select(rows[0], policy), "cmg")

    def test_small_gain_does_not_create_region(self):
        self.assertEqual(AUTO.fit([row(i, cmg_time=0.95) for i in range(8)])["regions"], [])

    def test_minimum_is_datasets_not_replicated_threads(self):
        rows = [row(0, cmg_time=0.5) for _ in range(8)]
        self.assertEqual(AUTO.fit(rows)["regions"], [])

    def test_individual_guardrail_rejects_region(self):
        rows = [row(i, cmg_time=0.6) for i in range(8)]
        rows[0]["times"]["cmg"] = 1.11
        self.assertEqual(AUTO.fit(rows)["regions"], [])

    def test_missing_default_fails_closed(self):
        rows = [row(i) for i in range(8)]
        del rows[0]["times"]["lsmr"]
        with self.assertRaises(AUTO.dev.DevelopmentError):
            AUTO.fit(rows)

    def test_failed_selected_route_cannot_silently_change_selection(self):
        rows = [row(i, cmg_time=0.7) for i in range(8)]
        policy = AUTO.fit(rows)
        del rows[0]["times"]["cmg"]
        self.assertEqual(AUTO.select(rows[0], policy), "cmg")
        self.assertFalse(AUTO.validate_policy(rows, policy)["pass"])

    def test_cmg_outside_public_envelope_uses_default(self):
        rows = [row(i, cmg_time=0.7) for i in range(8)]
        policy = AUTO.fit(rows)
        policy["regions"][0]["predicates"] = []
        rows[0]["features"]["nonbase_nuisance_columns"] = 65
        self.assertEqual(AUTO.select(rows[0], policy), "lsmr")

    def test_holdout_regression_fails_without_refitting(self):
        train = [row(i, cmg_time=0.7) for i in range(8)]
        policy = AUTO.fit(train)
        frozen = copy.deepcopy(policy)
        heldout = [row(i, cmg_time=1.2) for i in range(8)]
        self.assertFalse(AUTO.validate_policy(heldout, policy)["pass"])
        self.assertEqual(policy, frozen)

    def test_dataset_weighting_ignores_thread_replication(self):
        rows = [row(0), row(0), row(1)]
        self.assertAlmostEqual(AUTO.dataset_geomean(rows, [2, 2, 0.5]), 1.0)

    def test_science_failure_never_yields_timing_rows(self):
        with self.assertRaises(AUTO.dev.DevelopmentError):
            AUTO.rows_from_run({}, {"science_pass": False})

    def test_memory_is_mandatory_and_bounded(self):
        r = {"implementation": "candidate", "arm": "lsmr", "rc": 0, "scope_id": "a", "runtime_values": [0.0] * 108}
        r["runtime_values"][22] = 1.0
        self.assertTrue(AUTO.memory_check({"records": [r]})["pass"])
        for invalid in (None, float("nan"), -1.0, 1.25001):
            r["runtime_values"][22] = invalid
            self.assertFalse(AUTO.memory_check({"records": [r]})["pass"])


if __name__ == "__main__":
    unittest.main()
