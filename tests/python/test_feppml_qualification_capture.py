from __future__ import annotations

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT / "scripts/run_feppml_qualification.py"
QUAL_PATH = ROOT / "scripts/feppml_qualification.py"
MANIFEST_PATH = ROOT / "benchmarks/manifests/feppml_qualification_v1.json"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


QUAL = load_module("feppml_qualification_capture_test_qual", QUAL_PATH)
RUNNER = load_module("feppml_qualification_capture_test_runner", RUNNER_PATH)


class FeppmlQualificationCaptureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = QUAL.load_manifest(MANIFEST_PATH)
        cls.manifest_sha = QUAL.sha256_file(MANIFEST_PATH)

    def _spec(self, program: Path, plan: Path) -> dict:
        interpreter = Path(sys.executable).resolve()
        program = program.resolve()
        plan = plan.resolve()
        identity = {
            "interpreter_path": str(interpreter),
            "interpreter_sha256": QUAL.sha256_file(interpreter),
            "interpreter_version": subprocess.check_output(
                [str(interpreter), "--version"],
                stderr=subprocess.STDOUT,
                text=True,
            ).strip(),
            "program_path": str(program),
            "program_sha256": QUAL.sha256_file(program),
            "plan_path": str(plan),
            "plan_sha256": QUAL.sha256_file(plan),
        }
        return {
            "schema_version": 1,
            "spec_type": "feppml-qualification-capture-v1",
            "qualification_manifest": {
                "id": self.manifest["manifest_id"],
                "sha256": self.manifest_sha,
            },
            "preflight_program": identity,
            "arm_program": copy.deepcopy(identity),
            "evidence_header": {
                "candidate": {},
                "comparator": {},
                "oracles": [],
                "source_audit": {},
                "platform_artifacts": [],
            },
            "environment": {
                "node_id": "node",
                "stata_version": "19.0 MP",
                "stata_flavor": "MP",
                "scheduler_job_id": "local-1",
                "comparator_processor_limit": self.manifest["capture_contract"][
                    "comparator_processor_limit"
                ],
                "os": "Linux 5.14",
                "cpu": "Intel Xeon Gold",
                "rustc_version": "rustc 1.89.0",
            },
            "cell_ids": self.manifest["matrix_contract"]["expected_cell_ids"],
            "complete_sentinel": "FEPPML_CAPTURE_SPEC_COMPLETE",
        }

    def test_capture_spec_binds_program_bytes_and_closed_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            program = root / "capture"
            program.write_text("raise SystemExit(0)\n", encoding="utf-8")
            plan = root / "plan.json"
            plan.write_text("{}\n", encoding="ascii")
            spec_path = root / "spec.json"
            spec_path.write_text(json.dumps(self._spec(program, plan)), encoding="utf-8")
            loaded = RUNNER.load_spec(spec_path, self.manifest, self.manifest_sha)
            self.assertEqual(loaded["cell_ids"], self.manifest["matrix_contract"]["expected_cell_ids"])
            self.assertEqual(
                RUNNER.program_argv(loaded["preflight_program"]),
                [
                    str(Path(sys.executable).resolve()),
                    "-I",
                    "-B",
                    "-S",
                    str(program.resolve()),
                    "--plan",
                    str(plan.resolve()),
                ],
            )

            changed = json.loads(spec_path.read_text(encoding="utf-8"))
            changed["cell_ids"].pop()
            spec_path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(RUNNER.CaptureError, "cell inventory"):
                RUNNER.load_spec(spec_path, self.manifest, self.manifest_sha)

            spec_path.write_text(json.dumps(self._spec(program, plan)), encoding="utf-8")
            program.write_text("raise SystemExit(1)\n", encoding="utf-8")
            with self.assertRaisesRegex(RUNNER.CaptureError, "program bytes"):
                RUNNER.load_spec(spec_path, self.manifest, self.manifest_sha)

    def test_capture_spec_rejects_prefix_extra_and_plan_mutation(self) -> None:
        for injected in (
            {"command": ["/usr/bin/env", "python3"]},
            {"extra_arguments": ["--unsealed-plan", "/tmp/other.json"]},
        ):
            with self.subTest(injected=next(iter(injected))), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                program = root / "capture.py"
                program.write_text("raise SystemExit(0)\n", encoding="ascii")
                plan = root / "plan.json"
                plan.write_text("{}\n", encoding="ascii")
                record = self._spec(program, plan)
                record["preflight_program"].update(injected)
                spec_path = root / "spec.json"
                spec_path.write_text(json.dumps(record), encoding="ascii")
                with self.assertRaisesRegex(RUNNER.CaptureError, "fields are not closed"):
                    RUNNER.load_spec(spec_path, self.manifest, self.manifest_sha)

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            program = root / "capture.py"
            program.write_text("raise SystemExit(0)\n", encoding="ascii")
            plan = root / "plan.json"
            plan.write_text("{}\n", encoding="ascii")
            spec_path = root / "spec.json"
            spec_path.write_text(json.dumps(self._spec(program, plan)), encoding="ascii")
            plan.write_text('{"changed":true}\n', encoding="ascii")
            with self.assertRaisesRegex(RUNNER.CaptureError, "plan bytes"):
                RUNNER.load_spec(spec_path, self.manifest, self.manifest_sha)

    def test_capture_child_uses_isolated_python_and_sealed_supervisor(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            program = root / "capture.py"
            program.write_text(
                "import argparse, json, os\n"
                "from pathlib import Path\n"
                "p=argparse.ArgumentParser(); p.add_argument('--plan'); "
                "p.add_argument('--probe', action='store_true'); "
                "p.add_argument('--output'); a=p.parse_args()\n"
                "Path(a.output).write_text(json.dumps(dict(os.environ), "
                "sort_keys=True)+'\\n', encoding='ascii')\n",
                encoding="ascii",
            )
            plan = root / "plan.json"
            plan.write_text("{}\n", encoding="ascii")
            interpreter = Path(sys.executable).resolve()
            identity = {
                "interpreter_path": str(interpreter),
                "interpreter_sha256": QUAL.sha256_file(interpreter),
                "interpreter_version": subprocess.check_output(
                    [str(interpreter), "-I", "-B", "-S", "--version"],
                    stderr=subprocess.STDOUT,
                    text=True,
                ).strip(),
                "program_path": str(program.resolve()),
                "program_sha256": QUAL.sha256_file(program),
                "plan_path": str(plan.resolve()),
                "plan_sha256": QUAL.sha256_file(plan),
            }
            output = root / "action/output.json"
            log_prefix = root / "action/capture"
            with mock.patch.dict(
                os.environ,
                {
                    "PYTHONPATH": "/tmp/hostile",
                    "PYTHONHOME": "/tmp/hostile-home",
                    "LD_PRELOAD": "/tmp/hostile.so",
                    "FEREG_INJECT": "1",
                },
                clear=False,
            ):
                RUNNER._run_program(
                    identity,
                    ["--probe"],
                    output=output,
                    log_prefix=log_prefix,
                    timeout_seconds=10,
                )
            child_environment = json.loads(output.read_text(encoding="ascii"))
            for forbidden in (
                "PYTHONPATH", "PYTHONHOME", "LD_PRELOAD", "FEREG_INJECT"
            ):
                self.assertNotIn(forbidden, child_environment)
            RUNNER.verify_program_supervisor(
                identity,
                ["--probe"],
                output=output,
                log_prefix=log_prefix,
            )
            supervisor = log_prefix.with_suffix(".supervisor.json")
            document = json.loads(supervisor.read_text(encoding="ascii"))
            document["environment"]["PYTHONPATH"] = "/tmp/hostile"
            supervisor.chmod(0o644)
            supervisor.write_text(json.dumps(document), encoding="ascii")
            with self.assertRaisesRegex(RUNNER.CaptureError, "supervisor invocation"):
                RUNNER.verify_program_supervisor(
                    identity,
                    ["--probe"],
                    output=output,
                    log_prefix=log_prefix,
                )

    def test_capture_spec_rejects_interpreter_digest_or_version_mutation(self) -> None:
        for field, value, message in (
            ("interpreter_sha256", "0" * 64, "interpreter bytes"),
            ("interpreter_version", "Python attacker-controlled", "interpreter version"),
        ):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                program = root / "capture.py"
                program.write_text("raise SystemExit(0)\n", encoding="ascii")
                plan = root / "plan.json"
                plan.write_text("{}\n", encoding="ascii")
                record = self._spec(program, plan)
                record["arm_program"][field] = value
                spec_path = root / "spec.json"
                spec_path.write_text(json.dumps(record), encoding="ascii")
                with self.assertRaisesRegex(RUNNER.CaptureError, message):
                    RUNNER.load_spec(spec_path, self.manifest, self.manifest_sha)

    def _preflight(self) -> dict:
        order = [["candidate", "comparator"]]
        order.extend(
            ["candidate", "comparator"]
            if pair == "AB"
            else ["comparator", "candidate"]
            for pair in self.manifest["capture_contract"]["warm_pair_order"]
        )
        cells = [
            {
                "cell_id": cell_id,
                "timing": {
                    "candidate": {"cold_seconds": 1.0, "warm_seconds": [1.0] * 9},
                    "comparator": {"cold_seconds": 1.0, "warm_seconds": [1.0] * 9},
                },
                "execution": {"arm_order": order},
            }
            for cell_id in self.manifest["matrix_contract"]["expected_cell_ids"]
        ]
        return {
            "schema_version": 1,
            "evidence_type": "feppml-science-preflight-v1",
            "qualification_manifest_sha256": self.manifest_sha,
            "timing_placeholder": True,
            "timed_arm_processes_started": 0,
            "benchmark_evidence": {"cells": cells},
            "complete_sentinel": "FEPPML_SCIENCE_PREFLIGHT_COMPLETE",
        }

    def test_science_preflight_must_finish_before_timing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "preflight.json"
            path.write_text(json.dumps(self._preflight()), encoding="utf-8")
            failed = types.SimpleNamespace(
                evaluate_science_evidence=lambda *_args, **_kwargs: ["oracle mismatch"]
            )
            with self.assertRaisesRegex(RUNNER.CaptureError, "before timing"):
                RUNNER._load_scientific_preflight(
                    path,
                    self.manifest,
                    self.manifest_sha,
                    {},
                    "a" * 64,
                    evaluator=failed,
                )
            passed = types.SimpleNamespace(
                evaluate_science_evidence=lambda *_args, **_kwargs: []
            )
            benchmark = RUNNER._load_scientific_preflight(
                path,
                self.manifest,
                self.manifest_sha,
                {},
                "a" * 64,
                evaluator=passed,
            )
            self.assertEqual(len(benchmark["cells"]), 33)

            wrong_order = self._preflight()
            wrong_order["benchmark_evidence"]["cells"][0]["execution"][
                "arm_order"
            ][1] = ["comparator", "candidate"]
            path.write_text(json.dumps(wrong_order), encoding="utf-8")
            with self.assertRaisesRegex(RUNNER.CaptureError, "pair order"):
                RUNNER._load_scientific_preflight(
                    path,
                    self.manifest,
                    self.manifest_sha,
                    {},
                    "a" * 64,
                    evaluator=passed,
                )

            record = self._preflight()
            record["timed_arm_processes_started"] = 1
            path.write_text(json.dumps(record), encoding="utf-8")
            with self.assertRaisesRegex(RUNNER.CaptureError, "order attestation"):
                RUNNER._load_scientific_preflight(
                    path,
                    self.manifest,
                    self.manifest_sha,
                    {},
                    "a" * 64,
                    evaluator=passed,
                )

    def test_real_science_only_evaluator_ignores_timing_but_blocks_science(self) -> None:
        from tests.python.test_ppml_memory_performance import (
            PpmlMemoryPerformanceContractTests as DevelopmentFixture,
        )

        DevelopmentFixture.setUpClass()
        fixture = DevelopmentFixture()
        benchmark = fixture._evidence()
        placeholder_order = [["candidate", "comparator"]]
        placeholder_order.extend(
            ["candidate", "comparator"]
            if pair == "AB"
            else ["comparator", "candidate"]
            for pair in self.manifest["capture_contract"]["warm_pair_order"]
        )
        for cell in benchmark["cells"]:
            cell["timing"] = {
                arm: {"cold_seconds": 1.0, "warm_seconds": [1.0] * 9}
                for arm in ("candidate", "comparator")
            }
            # Deliberately incomplete for the full performance contract.  The
            # preflight validator must inspect only the frozen arm-order
            # placeholder and then use the real science-only evaluator.
            cell["execution"] = {"arm_order": placeholder_order}
        record = {
            "schema_version": 1,
            "evidence_type": "feppml-science-preflight-v1",
            "qualification_manifest_sha256": self.manifest_sha,
            "timing_placeholder": True,
            "timed_arm_processes_started": 0,
            "benchmark_evidence": benchmark,
            "complete_sentinel": "FEPPML_SCIENCE_PREFLIGHT_COMPLETE",
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "preflight.json"
            path.write_text(json.dumps(record), encoding="utf-8")
            loaded = RUNNER._load_scientific_preflight(
                path,
                self.manifest,
                self.manifest_sha,
                DevelopmentFixture.manifest,
                DevelopmentFixture.manifest_sha256,
            )
            self.assertEqual(len(loaded["cells"]), 33)
            record["benchmark_evidence"]["cells"][0]["scientific"]["candidate"][
                "coefficients"
            ][0] += 0.25
            path.write_text(json.dumps(record), encoding="utf-8")
            with self.assertRaisesRegex(RUNNER.CaptureError, "scientific preflight failed"):
                RUNNER._load_scientific_preflight(
                    path,
                    self.manifest,
                    self.manifest_sha,
                    DevelopmentFixture.manifest,
                    DevelopmentFixture.manifest_sha256,
                )

    def test_arm_record_requires_fresh_licensed_identity_and_exact_artifact(self) -> None:
        environment = {
            "node_id": "node",
            "stata_version": "19.0 MP",
            "stata_flavor": "MP",
            "scheduler_job_id": "job",
            "comparator_processor_limit": 4,
            "os": "Linux 5.14",
            "cpu": "Intel Xeon Gold",
            "rustc_version": "rustc 1.89.0",
        }
        record = {
            "schema_version": 1,
            "evidence_type": "feppml-qualification-arm-v1",
            "cell_id": "cell",
            "arm": "candidate",
            "pair_index": 0,
            "temperature": "cold",
            "seconds": 1.0,
            "peak_rss_bytes": 1000,
            "process_id": 22,
            "node_id": "node",
            "stata_version": "19.0 MP",
            "artifact_sha256": "a" * 64,
            "fresh_process": True,
            "effective_threads": 8,
            "stata_processors": 8,
            "complete_sentinel": "FEPPML_ARM_COMPLETE",
        }
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "arm.json"
            path.write_text(json.dumps(record), encoding="utf-8")
            measurement, effective, _ = RUNNER._arm_measurement(
                path,
                cell_id="cell",
                arm="candidate",
                pair_index=0,
                temperature="cold",
                requested_threads=8,
                artifact_sha256="a" * 64,
                environment=environment,
            )
            self.assertTrue(measurement["fresh_process"])
            self.assertEqual(effective, 8)
            record["fresh_process"] = False
            path.write_text(json.dumps(record), encoding="utf-8")
            with self.assertRaisesRegex(RUNNER.CaptureError, "fresh process"):
                RUNNER._arm_measurement(
                    path,
                    cell_id="cell",
                    arm="candidate",
                    pair_index=0,
                    temperature="cold",
                    requested_threads=8,
                    artifact_sha256="a" * 64,
                    environment=environment,
                )


if __name__ == "__main__":
    unittest.main()
