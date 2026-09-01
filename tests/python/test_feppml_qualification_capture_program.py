from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
PROGRAM = ROOT / "scripts/feppml_qualification_capture.py"
SPEC = importlib.util.spec_from_file_location("feppml_real_capture_test", PROGRAM)
assert SPEC is not None and SPEC.loader is not None
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class FeppmlRealCaptureProgramTests(unittest.TestCase):
    def test_preflight_placeholder_uses_cold_then_warm_frozen_cadence(self) -> None:
        self.assertEqual(
            [list(order) for order in CAPTURE.QUALIFICATION_ARM_ORDER],
            [
                ["candidate", "comparator"],
                ["candidate", "comparator"],
                ["comparator", "candidate"],
                ["candidate", "comparator"],
                ["comparator", "candidate"],
                ["candidate", "comparator"],
                ["comparator", "candidate"],
                ["candidate", "comparator"],
                ["comparator", "candidate"],
                ["candidate", "comparator"],
            ],
        )

    def _plan(self, root: Path, stata_name: str = "stata-mp") -> Path:
        stata = root / stata_name
        stata.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        stata.chmod(0o755)
        development = root / "development.json"
        development.write_text("{}\n", encoding="ascii")
        data = root / "cell.dta"
        data.write_bytes(b"immutable-data")
        do_file = root / "capture.do"
        do_file.write_text("version 19\n", encoding="ascii")
        candidate = root / "fereg_linux.plugin"
        comparator = root / "ppmlhdfe.bundle"
        candidate.write_bytes(b"candidate")
        comparator.write_bytes(b"comparator")
        immutable = lambda path: {"path": str(path), "sha256": digest(path)}
        closure = {
            "schema_version": 1,
            "closure_type": "feppml-capture-plan-closure-v1",
            "materialization_request_sha256": "a" * 64,
            "recipe_manifest_sha256": "b" * 64,
            "recipe_inventory_sha256": "c" * 64,
            "dataset_inventory_sha256": "d" * 64,
            "oracle_fixture_inventory_sha256": "e" * 64,
            "candidate_source_tree": "f" * 40,
            "candidate_plugin_sha256": "1" * 64,
            "candidate_bundle_sha256": "2" * 64,
            "comparator_source_inventory_sha256": "3" * 64,
            "comparator_bundle_sha256": "4" * 64,
            "tool_inventory_sha256": "5" * 64,
            "expected_inventory_sha256": "6" * 64,
            "stata_executable_sha256": digest(stata),
            "complete_sentinel": "FEPPML_CAPTURE_PLAN_CLOSURE_COMPLETE",
        }
        arm = lambda artifact: {
            "science_do_file": immutable(do_file),
            "timing_do_file": immutable(do_file),
            "artifact": immutable(artifact),
            "science_output": "result.json",
            "expected_status_map": [{"name": "_cons", "status_code": 1}],
            "expected_output": "fit",
            "probe_rows": 2,
            "expected_cluster_dimensions": 0,
            "expected_saved_channels": 0,
            "expected_source_provenance": {
                "matrices": ["e(b)"],
                "scalars": ["e(N)"],
                "variables": ["e(sample)"],
                "receipt_complete": artifact == candidate,
            },
        }
        plan = {
            "schema_version": 1,
            "plan_type": "feppml-qualification-capture-plan-v1",
            "qualification_manifest_sha256": "a" * 64,
            "development_manifest": immutable(development),
            "stata": {"executable": str(stata), "sha256": digest(stata), "batch_arguments": ["-q", "-b", "do"], "comparator_processor_limit": 16},
            "identity": {
                "source_commit": "1" * 40,
                "source_tree": "f" * 40,
                "cargo_lock_sha256": "2" * 64,
                "references_lock_sha256": "3" * 64,
                "manifest_sha256": digest(development),
                "generator_sha256": "4" * 64,
                "reference_commits": {"ppmlhdfe": "5" * 40},
            },
            "cells": [{
                "cell_id": "cell",
                "requested_threads": 1,
                "data": immutable(data),
                "candidate": arm(candidate),
                "comparator": arm(comparator),
                "oracle_fixture": None,
            }],
            "constrained_memory": [],
            "closure": closure,
            "complete_sentinel": "FEPPML_CAPTURE_PLAN_COMPLETE",
        }
        path = root / "plan.json"
        path.write_text(json.dumps(plan), encoding="ascii")
        return path

    def test_plan_rejects_non_stata_executable_and_changed_dataset(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "basename"):
                CAPTURE.load_plan(self._plan(root, "python-fake"))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = self._plan(root)
            (root / "cell.dta").write_bytes(b"changed")
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "bytes"):
                CAPTURE.load_plan(plan)

    def test_stata_flavor_is_derived_from_the_sealed_executable(self) -> None:
        for executable, expected in (
            ("stata-mp", "MP"),
            ("stata-se", "SE"),
            ("stata", "BE"),
        ):
            with self.subTest(executable=executable), tempfile.TemporaryDirectory() as temporary:
                loaded = CAPTURE.load_plan(self._plan(Path(temporary), executable))
                self.assertEqual(CAPTURE.stata_flavor(loaded), expected)

    def test_portable_plan_replay_preserves_recorded_argv_but_reopens_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = root / "original" / "20260831T120000Z-feppml-q"
            original.mkdir(parents=True)
            plan_path = self._plan(original)
            relocated = root / "collected" / original.name
            shutil.copytree(original, relocated)
            shutil.rmtree(original)

            def resolver(path: Path) -> Path:
                return relocated / path.relative_to(original)

            loaded = CAPTURE.load_plan(
                relocated / plan_path.name,
                path_resolver=resolver,
                verify_stata_executable=False,
            )
            self.assertEqual(loaded["cells"][0]["data"]["path"], original / "cell.dta")
            self.assertEqual(CAPTURE.stata_flavor(loaded), "MP")
            (relocated / "cell.dta").write_bytes(b"substituted")
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "bytes"):
                CAPTURE.load_plan(
                    relocated / plan_path.name,
                    path_resolver=resolver,
                    verify_stata_executable=False,
                )

    def test_licensed_child_environment_is_complete_scrubbed_and_replayable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = CAPTURE.load_plan(self._plan(root))
            work = root / "child"
            receipt = work / "core-preflight.json"
            hostile = {
                "FEREG_PPML_ROUTE": "injected",
                "PYTHONPATH": "/tmp/shadow",
                "LD_PRELOAD": "/tmp/injected.so",
                "DYLD_INSERT_LIBRARIES": "/tmp/injected.dylib",
                "RAYON_NUM_THREADS": "999",
            }
            with mock.patch.dict(os.environ, hostile, clear=False):
                environment = CAPTURE._sealed_child_environment(
                    plan,
                    work,
                    {
                        "FEREG_MEMORY_LIMIT_BYTES": "4000000000",
                        "FEREG_PPML_PREFLIGHT_RECEIPT": str(receipt),
                    },
                )
            self.assertEqual(
                set(environment),
                {
                    "HOME", "TMPDIR", "STATATMP", "PATH", "LANG", "LC_ALL",
                    "TZ", "FEREG_MEMORY_LIMIT_BYTES",
                    "FEREG_PPML_PREFLIGHT_RECEIPT",
                },
            )
            self.assertEqual(environment["HOME"], str(work / "home"))
            self.assertEqual(environment["TMPDIR"], str(work / "tmp"))
            self.assertEqual(environment["LC_ALL"], "C")
            self.assertNotIn("PYTHONPATH", environment)
            self.assertNotIn("LD_PRELOAD", environment)
            self.assertNotIn("FEREG_PPML_ROUTE", environment)

    def test_hostile_pythonpath_cannot_shadow_sealed_capture_dependencies(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            shadow = Path(temporary) / "shadow/scripts"
            shadow.mkdir(parents=True)
            (shadow / "__init__.py").write_text("", encoding="ascii")
            for name in (
                "normalize_feppml_stata_raw.py",
                "parse_feppml_stata_protocol.py",
            ):
                (shadow / name).write_text(
                    "raise RuntimeError('PYTHONPATH SHADOW EXECUTED')\n",
                    encoding="ascii",
                )
            completed = subprocess.run(
                [sys.executable, str(PROGRAM), "--help"],
                env={**os.environ, "PYTHONPATH": str(Path(temporary) / "shadow")},
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertNotIn("PYTHONPATH SHADOW EXECUTED", completed.stderr)

    def test_independent_oracle_environment_blocks_sitecustomize_and_module_shadow(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shadow = root / "shadow"
            shadow.mkdir()
            for filename in (
                "sitecustomize.py",
                "ppml_explicit_dummy_oracle.py",
                "ppml_multiway_cluster_oracle.py",
            ):
                (shadow / filename).write_text(
                    "raise RuntimeError('ORACLE SHADOW EXECUTED')\n",
                    encoding="ascii",
                )
            plan_root = root / "plan-root"
            plan_root.mkdir()
            plan = CAPTURE.load_plan(self._plan(plan_root))
            oracle_root = root / "oracle"
            environment = CAPTURE._sealed_oracle_environment(plan, oracle_root)
            self.assertEqual(
                environment,
                {
                    "HOME": str(oracle_root / "independent-home"),
                    "TMPDIR": str(oracle_root / "independent-tmp"),
                    "PATH": "/usr/bin:/bin",
                    "LANG": "C",
                    "LC_ALL": "C",
                    "TZ": "UTC",
                },
            )
            hostile = {
                "PYTHONPATH": str(shadow),
                "PYTHONHOME": str(shadow),
                "LD_PRELOAD": str(shadow / "injected.so"),
                "DYLD_INSERT_LIBRARIES": str(shadow / "injected.dylib"),
            }
            oracle = ROOT / "scripts/feppml_explicit_dummy_oracle.py"
            with mock.patch.dict(os.environ, hostile, clear=False):
                completed = subprocess.run(
                    [
                        str(Path(sys.executable).resolve()),
                        "-I",
                        "-B",
                        "-S",
                        str(oracle),
                        "--help",
                    ],
                    env=environment,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    check=False,
                )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertNotIn("ORACLE SHADOW EXECUTED", completed.stderr)

    def test_plan_rejects_any_stata_batch_argv_prefix_or_suffix(self) -> None:
        for arguments in (
            ["/usr/bin/env", "-q", "-b", "do"],
            ["-q", "-b", "do", "injected.do"],
        ):
            with self.subTest(arguments=arguments), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                plan_path = self._plan(root)
                plan = json.loads(plan_path.read_text(encoding="ascii"))
                plan["stata"]["batch_arguments"] = arguments
                plan_path.write_text(json.dumps(plan), encoding="ascii")
                with self.assertRaisesRegex(
                    CAPTURE.CaptureFailure, "exact sealed batch argv"
                ):
                    CAPTURE.load_plan(plan_path)

    def _plan_with_oracle(self, root: Path, command: list[str]) -> Path:
        plan_path = self._plan(root)
        plan = json.loads(plan_path.read_text(encoding="ascii"))
        canonical_export = root / "oracle.csv"
        canonical_export.write_text("obs_id,y\n1,1\n", encoding="ascii")
        model = root / "oracle-model.json"
        model.write_text("{}\n", encoding="ascii")
        generator = root / "oracle-generator.py"
        generator.write_text("raise SystemExit(0)\n", encoding="ascii")
        program = root / "oracle.py"
        program.write_text("raise SystemExit(0)\n", encoding="ascii")
        immutable = lambda path: {"path": str(path), "sha256": digest(path)}
        plan["cells"][0]["oracle_fixture"] = {
            "fixture_id": "cell__explicit_v1",
            "data": immutable(root / "cell.dta"),
            "canonical_export": immutable(canonical_export),
            "model": immutable(model),
            "generator": immutable(generator),
            "row_count": 1,
            "independent": {
                "command": command,
                "program": immutable(program),
                "dependencies": [],
                "output": "oracle.json",
            },
        }
        plan_path.write_text(json.dumps(plan), encoding="ascii")
        return plan_path

    def test_independent_oracle_rejects_prefix_and_extra_argv(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            program = root / "oracle.py"
            exact = [
                str(Path(sys.executable).resolve()), "-I", "-B", "-S",
                str(program.resolve()),
            ]
            CAPTURE.load_plan(self._plan_with_oracle(root, exact))

        for command in (
            ["/usr/bin/env", str(Path(sys.executable).resolve()), "oracle.py"],
            [str(Path(sys.executable).resolve()), "oracle.py", "--unsealed"],
        ):
            with self.subTest(command=command), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                absolute_command = [
                    str((root / value).resolve()) if value == "oracle.py" else value
                    for value in command
                ]
                plan_path = self._plan_with_oracle(root, absolute_command)
                with self.assertRaisesRegex(
                    CAPTURE.CaptureFailure, "exact sealed Python/program argv"
                ):
                    CAPTURE.load_plan(plan_path)

    def test_portable_oracle_replay_binds_recorded_cwd_not_copy_location(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "collected/oracle-fixture"
            root.mkdir(parents=True)
            output = root / "oracle.json"
            output.write_text('{"complete":true}\n', encoding="ascii")
            (root / "independent.stdout").write_bytes(b"")
            (root / "independent.stderr").write_bytes(b"")
            command = ["/sealed/python", "/sealed/oracle.py", "--output", "/sealed/oracle.json"]
            child_environment = {
                "HOME": "/sealed/home",
                "TMPDIR": "/sealed/tmp",
                "PATH": "/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
                "TZ": "UTC",
            }
            recorded_root = Path(
                "/projectnb/welfgr/fereg/runs/20260831T120000Z-q/"
                "work/preflight/licensed-science/cell/oracle-fixture"
            )
            files = [
                {
                    "path": path.relative_to(root).as_posix(),
                    "sha256": digest(path),
                    "size_bytes": path.stat().st_size,
                }
                for path in (
                    root / "independent.stdout",
                    root / "independent.stderr",
                    output,
                )
            ]
            supervisor = {
                "schema_version": 1,
                "receipt_type": "feppml-independent-oracle-supervisor-v1",
                "command": command,
                "cwd": str(recorded_root),
                "return_code": 0,
                "child_environment": child_environment,
                "files": files,
                "complete_sentinel": (
                    "FEPPML_INDEPENDENT_ORACLE_SUPERVISOR_COMPLETE"
                ),
            }
            supervisor_path = root / "independent-supervisor.json"
            supervisor_path.write_text(json.dumps(supervisor), encoding="ascii")
            self.assertEqual(
                CAPTURE._replay_independent_oracle(
                    root,
                    command,
                    output,
                    logical_root=recorded_root,
                    child_environment=child_environment,
                ),
                {"complete": True},
            )
            supervisor["cwd"] = str(root.resolve())
            supervisor_path.write_text(json.dumps(supervisor), encoding="ascii")
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "supervisor"):
                CAPTURE._replay_independent_oracle(
                    root,
                    command,
                    output,
                    logical_root=recorded_root,
                    child_environment=child_environment,
                )

    def test_child_without_ordered_markers_and_rss_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plan = CAPTURE.load_plan(self._plan(root))
            cell = plan["cells"][0]
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "markers or RSS"):
                CAPTURE._stata_child(
                    plan, cell, "candidate", root / "work",
                    requested_threads=1, pair_index=0, temperature="cold",
                )

    def test_source_contract_uses_direct_argv_and_external_rss(self) -> None:
        source = PROGRAM.read_text(encoding="utf-8")
        for token in ("subprocess.Popen(", "command, cwd=work", "VmRSS:", 'Path(f\"/proc/{pid}/status\")', '"READY"', '"START"', '"DONE"'):
            self.assertIn(token, source)
        self.assertNotIn("child_environment = dict(os.environ)", source)
        self.assertIn("_sealed_sibling(", source)
        self.assertNotIn("shell=True", source)
        self.assertIn("transport and provenance boundary", source)

    def test_science_record_binds_both_source_inventories_exactly(self) -> None:
        cell = {
            "cell_id": "cell",
            "data": {"sha256": "a" * 64},
            "candidate": {"expected_source_provenance": {
                "matrices": ["e(b)"], "scalars": ["e(N)"],
                "variables": ["e(sample)"], "receipt_complete": True,
            }},
            "comparator": {"expected_source_provenance": {
                "matrices": ["e(b)"], "scalars": ["e(df)"],
                "variables": ["e(sample)"], "receipt_complete": False,
            }},
        }
        candidate = {
            "scientific": {}, "diagnostics": {}, "memory": {},
            "slowdown_explanation": None, "stata": {"licensed": True},
            "source_provenance": copy.deepcopy(cell["candidate"]["expected_source_provenance"]),
            "complete_sentinel": "FEPPML_STATA_SCIENCE_COMPLETE",
        }
        comparator = {
            "scientific": {}, "stata": {"licensed": True},
            "source_provenance": copy.deepcopy(cell["comparator"]["expected_source_provenance"]),
            "complete_sentinel": "FEPPML_STATA_SCIENCE_COMPLETE",
        }
        CAPTURE._science_cell_record(cell, candidate, comparator, None)
        for arm, record in (("candidate", candidate), ("comparator", comparator)):
            mutated = copy.deepcopy(record)
            mutated["source_provenance"]["variables"].append("__q_fe999")
            with self.subTest(arm=arm), self.assertRaisesRegex(
                CAPTURE.CaptureFailure, "source inventory|native e\\(\\) receipt"
            ):
                CAPTURE._science_cell_record(
                    cell,
                    mutated if arm == "candidate" else candidate,
                    mutated if arm == "comparator" else comparator,
                    None,
                )
        cell["oracle_fixture"] = {
            "fixture_id": "cell__explicit_v1",
            "data": {"sha256": "1" * 64},
            "canonical_export": {"sha256": "2" * 64},
            "model": {"sha256": "3" * 64},
            "generator": {"sha256": "4" * 64},
            "row_count": 8,
        }
        CAPTURE._oracle_result(cell, candidate, comparator, {"oracle": True})
        for arm, field in (
            ("candidate", "source_provenance"),
            ("comparator", "stata"),
            ("candidate", "complete_sentinel"),
        ):
            left, right = copy.deepcopy(candidate), copy.deepcopy(comparator)
            target = left if arm == "candidate" else right
            if field == "source_provenance":
                target[field]["variables"].append("forged")
            elif field == "stata":
                target[field]["licensed"] = False
            else:
                target[field] = "FORGED"
            with self.subTest(oracle_arm=arm, field=field), self.assertRaises(CAPTURE.CaptureFailure):
                CAPTURE._oracle_result(cell, left, right, {"oracle": True})

    def _memory_preflight(self, root: Path) -> tuple[Path, dict[str, object]]:
        limit = 4 * 1024 * 1024 * 1024
        headroom = max(512 * 1024 * 1024, limit // 5)
        record: dict[str, object] = {
            "schema_version": 1,
            "receipt_type": "feppml-memory-preflight-v1",
            "requested": "fast",
            "decision": "rejected",
            "selected_route": None,
            "fast_peak_bytes": limit,
            "low_peak_bytes": 1_000_000_000,
            "available_bytes": limit,
            "embedding_resident_bytes": 1024,
            "safety_headroom_bytes": headroom,
            "usable_bytes": limit - headroom,
            "planner_allocator_event_growth": 0,
            "planner_allocator_current_growth_bytes": 0,
            "planner_allocator_peak_growth_bytes": 0,
            "allocation_bytes": 0,
            "workspace_allocation_started": False,
            "embedded_git_commit": "1" * 12,
            "complete_sentinel": "FEPPML_MEMORY_PREFLIGHT_COMPLETE",
        }
        path = root / "core-preflight.json"
        path.write_text(json.dumps(record), encoding="ascii")
        return path, record

    def test_core_memory_preflight_receipt_is_exact_and_before_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path, record = self._memory_preflight(root)
            self.assertEqual(
                CAPTURE._preflight_receipt(
                    path,
                    requested="fast",
                    memory_limit_bytes=4 * 1024 * 1024 * 1024,
                    source_commit="1" * 40,
                ),
                record,
            )

            record["workspace_allocation_started"] = True
            path.write_text(json.dumps(record), encoding="ascii")
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "identity"):
                CAPTURE._preflight_receipt(
                    path,
                    requested="fast",
                    memory_limit_bytes=4 * 1024 * 1024 * 1024,
                    source_commit="1" * 40,
                )

    def test_nonzero_rc_cannot_replace_missing_or_invalid_core_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path, record = self._memory_preflight(root)
            record["allocation_bytes"] = 1
            path.write_text(json.dumps(record), encoding="ascii")
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "arithmetic"):
                CAPTURE._preflight_receipt(
                    path,
                    requested="fast",
                    memory_limit_bytes=4 * 1024 * 1024 * 1024,
                    source_commit="1" * 40,
                )

            path.unlink()
            with self.assertRaisesRegex(CAPTURE.CaptureFailure, "cannot read"):
                CAPTURE._preflight_receipt(
                    path,
                    requested="fast",
                    memory_limit_bytes=4 * 1024 * 1024 * 1024,
                    source_commit="1" * 40,
                )


if __name__ == "__main__":
    unittest.main()
