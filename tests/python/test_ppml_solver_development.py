from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ppml_solver_development.py"
MANIFEST = ROOT / "benchmarks/manifests/feppml_solver_development_v2.json"
DRIVER = ROOT / "tests/stata/development/feppml_solver_development.do"

SPEC = importlib.util.spec_from_file_location("ppml_solver_development", SCRIPT)
assert SPEC and SPEC.loader
DEVELOPMENT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEVELOPMENT)


def result_row(scope: dict[str, object], arm: str, repetition: int) -> dict[str, object]:
    solver = arm in DEVELOPMENT.SOLVERS
    return {
        **scope,
        "scope_id": scope["id"],
        "implementation": "candidate" if solver else "reference",
        "arm_type": "solver" if solver else "comparator",
        "arm": arm,
        "repetition": repetition,
        "rc": 0,
        "seconds": 1.0,
        "sample_n": 100.0,
        "ll": -10.0,
        "deviance": 2.0,
        "converged": 1.0,
        "rank": 1.0,
        "df_a": 18.0,
        "df_resid": 81.0,
        "inner_iterations": 4.0,
        "route_code": float(DEVELOPMENT.ROUTE_CODES[arm]) if solver else None,
        "fallback_count": 0.0 if solver else None,
        "effective_threads": float(scope["threads"]) if solver else None,
        "b_names": ["x1"],
        "b_values": [0.25],
        "v_names": ["x1"],
        "v_values": [0.01],
        "certificate_values": [1.0] * 16 if solver else [],
        "runtime_values": [0.0] * 108 if solver else [],
    }


def run_document() -> dict[str, object]:
    scope = {
        "id": "unit-scope", "n": 100, "topology": "balanced_2fe", "controls": 1,
        "difficulty": "easy", "threads": 7, "seed": 11,
    }
    records = [
        result_row(scope, arm, repetition)
        for repetition in (1, 2)
        for arm in (*DEVELOPMENT.SOLVERS, *DEVELOPMENT.COMPARATORS)
    ]
    return {
        "schema_version": 2,
        "kind": "feppml_solver_development_run",
        "qualification_eligible": False,
        "auto_calibration": False,
        "repetitions": 2,
        "include_oracle": False,
        "sources": {"candidate": {"commit": "a" * 40}},
        "scopes": [scope],
        "records": records,
        "processes": [],
    }


class PpmlSolverDevelopmentTests(unittest.TestCase):
    def test_manifest_has_only_public_explicit_arms_and_no_auto_policy(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        self.assertFalse(manifest["qualification_eligible"])
        self.assertFalse(manifest["auto_calibration"])
        self.assertEqual([row["id"] for row in manifest["solvers"]], ["map", "lsmr", "cmg"])
        self.assertEqual([row["id"] for row in manifest["comparators"]], ["ppmlhdfe", "ppmlhdfe_lsmr"])
        serialized = json.dumps(manifest).lower()
        self.assertNotIn("recommendation", serialized)
        self.assertNotIn("candidate_routes", serialized)

    def test_local_profiles_are_bounded_and_campaign_is_16_bundled_tasks(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        self.assertEqual(len(DEVELOPMENT.profile_scopes(manifest, "local_science")), 8)
        self.assertEqual(len(DEVELOPMENT.profile_scopes(manifest, "local_smoke")), 3)
        tasks = DEVELOPMENT.campaign_tasks(manifest)
        self.assertEqual(len(tasks), 16)
        self.assertEqual(sum(len(task["scopes"]) for task in tasks), 56)
        self.assertEqual([len(task["scopes"]) for task in tasks[:8]], [3] * 8)
        self.assertEqual([len(task["scopes"]) for task in tasks[8:]], [4] * 8)
        self.assertTrue(all(2 <= len(task["scopes"]) <= 4 for task in tasks))
        self.assertEqual(
            manifest["scc_campaign"]["scheduler"],
            {
                "project": "welfgr", "parallel_environment": "omp", "slots": 28,
                "m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4",
                "mem_per_core": "4G", "h_rt": "02:00:00",
            },
        )

    def test_manifest_rejects_any_auto_calibration_role(self) -> None:
        manifest = DEVELOPMENT.load_manifest()
        manifest["auto_calibration"] = True
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.validate_manifest(manifest)

    def test_qhost_class_is_exact(self) -> None:
        source = "HOSTNAME ARCH NCPU\nhl:m_core=28\nhl:cpu_arch=broadwell\nhl:cpu_type=E5-2680v4\n"
        self.assertEqual(
            DEVELOPMENT.parse_qhost_class(source),
            {"m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4"},
        )
        with self.assertRaises(DEVELOPMENT.DevelopmentError):
            DEVELOPMENT.parse_qhost_class(source.replace("m_core=28", "m_core=32"))

    def test_analysis_checks_every_repetition_and_emits_no_auto_recommendation(self) -> None:
        document = run_document()
        analysis = DEVELOPMENT.analyze_document(document)
        self.assertTrue(analysis["science_pass"])
        self.assertTrue(analysis["route_pass"])
        self.assertEqual(analysis["solver_summary"][0]["ppmlhdfe_lsmr_median_seconds"], 1.0)
        self.assertNotIn("route_recommendation", analysis)
        self.assertNotIn("recommendation", json.dumps(analysis).lower())

        changed = copy.deepcopy(document)
        target = next(row for row in changed["records"] if row["arm"] == "lsmr" and row["repetition"] == 2)
        target["b_values"] = [0.5]
        failed = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(failed["science_pass"])
        self.assertTrue(any("lsmr:r2:map_coefficients" in item for item in failed["science_failures"]))

    def test_analysis_rejects_fallback_and_missing_second_comparator(self) -> None:
        changed = run_document()
        target = next(row for row in changed["records"] if row["arm"] == "cmg" and row["repetition"] == 2)
        target["fallback_count"] = 1.0
        changed["records"] = [
            row for row in changed["records"]
            if not (row["arm"] == "ppmlhdfe_lsmr" and row["repetition"] == 2)
        ]
        analysis = DEVELOPMENT.analyze_document(changed)
        self.assertFalse(analysis["science_pass"])
        self.assertFalse(analysis["route_pass"])
        self.assertTrue(any("ppmlhdfe_lsmr:r2:missing" in item for item in analysis["science_failures"]))
        self.assertTrue(any("cmg:r2:fallback" in item for item in analysis["route_failures"]))
        cmg = next(row for row in analysis["solver_summary"] if row["solver"] == "cmg")
        self.assertFalse(cmg["performance_eligible"])

    def test_control_covariance_uses_the_full_matrix_stride(self) -> None:
        row = {
            "v_names": ["x1", "x2", "_cons"],
            "v_values": [1.0, 2.0, 3.0, 2.0, 4.0, 5.0, 3.0, 5.0, 6.0],
        }
        self.assertEqual(DEVELOPMENT._control_covariance(row, 2), [1.0, 2.0, 2.0, 4.0])
        self.assertEqual(DEVELOPMENT._optional_vector("1 . .a 4"), [1.0, None, None, 4.0])

    def test_exclusive_json_refuses_to_overwrite_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            DEVELOPMENT.exclusive_json(path, {"qualification_eligible": False})
            with self.assertRaises(FileExistsError):
                DEVELOPMENT.exclusive_json(path, {"qualification_eligible": False})
            self.assertFalse(json.loads(path.read_text())["qualification_eligible"])

    def test_stata_status_requires_terminal_sentinel_and_ignores_echoed_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            console = Path(temporary) / "arm.log"
            console.write_text("")
            self.assertEqual(DEVELOPMENT.stata_stage_status(console, 0, "PASS"), (999, "missing-success-sentinel"))
            console.write_text('. display as error "FEREG_PPML_SOLVER_RUN_FAIL|arm=map|rc=603"\nPASS\n')
            self.assertEqual(DEVELOPMENT.stata_stage_status(console, 0, "PASS"), (0, "success-sentinel"))
            console.with_suffix(".stata.log").write_text("FEREG_PPML_SOLVER_RUN_FAIL|arm=map|rc=430\n")
            self.assertEqual(DEVELOPMENT.stata_stage_status(console, 0, "PASS"), (430, "failure-sentinel"))

    def test_driver_uses_public_solver_surface_and_separate_comparators(self) -> None:
        source = DRIVER.read_text(encoding="utf-8")
        self.assertIn("feppml y `control_list'", source)
        self.assertIn("solver(`arm')", source)
        self.assertNotIn("_fereg_estimate_ppml", source)
        self.assertNotIn("fesolver(", source)
        self.assertNotIn("set processors `threads'", source)
        self.assertIn("ppmlhdfe_lsmr", source)
        self.assertIn("accel(lsmr)", source)
        self.assertIn("FEREG_PPML_SOLVER_REFERENCE_ROUTE_PASS", source)
        self.assertIn("e(feppml_certificate)", source)
        self.assertIn("e(feppml_linear_route)", source)
        self.assertIn("data_meta", source)

    def test_scc_scripts_encode_exact_array_and_full_lifecycle(self) -> None:
        array = (ROOT / "benchmarks/scc/feppml_solver_dev_job.sh").read_text()
        for expected in (
            "#$ -pe omp 28", "#$ -l m_core=28", "#$ -l cpu_arch=broadwell",
            "#$ -l cpu_type=E5-2680v4", "#$ -t 1-16", "run-task",
        ):
            self.assertIn(expected, array)
        for name, marker in (
            ("feppml_solver_dev_setup_job.sh", "record-setup"),
            ("feppml_solver_dev_collector_job.sh", "merge --run-root"),
            ("feppml_solver_dev_submit.sh", "record-submission"),
            ("feppml_solver_dev_close.sh", "qacct -j"),
        ):
            self.assertIn(marker, (ROOT / "benchmarks/scc" / name).read_text())
        submit = (ROOT / "benchmarks/scc/feppml_solver_dev_submit.sh").read_text()
        self.assertIn('--local-science "$local_science" --local-smoke "$local_smoke"', submit)

    def test_qacct_acceptance_requires_complete_array_and_success(self) -> None:
        required = DEVELOPMENT.load_manifest()["receipt_contract"]["qacct_required"]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "qacct.txt"
            blocks = []
            for task in (1, 2):
                blocks.append("\n".join([
                    "jobnumber 123", f"taskid {task}", "owner johannes", "project welfgr",
                    "hostname node", "slots 28", "failed 0", "exit_status 0",
                    "ru_wallclock 1.0", "cpu 1.0", "maxvmem 1G",
                ]))
            path.write_text("\n==============================================================\n".join(blocks))
            rows = DEVELOPMENT._accept_qacct(path, "123", [1, 2], 28, required)
            self.assertEqual(len(rows), 2)
            path.write_text(path.read_text().replace("exit_status 0", "exit_status 1", 1))
            with self.assertRaises(DEVELOPMENT.DevelopmentError):
                DEVELOPMENT._accept_qacct(path, "123", [1, 2], 28, required)


if __name__ == "__main__":
    unittest.main()
