import copy
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"
MODULE_PATH = ROOT / "scripts/fereg_f0_regression.py"
SPEC = importlib.util.spec_from_file_location("fereg_f0_regression", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
F0 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F0)


class F0RegressionContractTests(unittest.TestCase):
    def test_controller_cli_rejects_direct_nonisolated_api(self):
        with self.assertRaisesRegex(SystemExit, "Python -I -B -S"):
            F0.main([])

    @classmethod
    def setUpClass(cls):
        cls.manifest = F0.load_manifest(MANIFEST)
        cls.manifest_sha256 = F0.sha256_file(MANIFEST)

    def _artifact(self, manifest=None, platform="macos-universal"):
        active = manifest or self.manifest
        record = next(
            item
            for item in active["baseline_candidate"]["artifacts"]
            if item["platform"] == platform
        )
        return {
            field: record[field]
            for field in F0.ARTIFACT_FIELDS
        }

    @staticmethod
    def _candidate_identity(platform="macos-universal"):
        commit = "1" * 40
        return {
            "source": {
                "commit": commit,
                "tree": "2" * 40,
                "cargo_lock_sha256": "3" * 64,
                "references_lock_sha256": "4" * 64,
            },
            "artifact": {
                "platform": platform,
                "sha256": "5" * 64,
                "size_bytes": 9_000_000,
                "embedded_git_commit": commit[:12],
            },
        }

    @staticmethod
    def _scientific():
        return {
            "stata_rc": 0,
            "sample_identity_sha256": "6" * 64,
            "singleton_identity_sha256": "7" * 64,
            "coefficient_names": ["x1", "x2"],
            "coefficient_status": ["kept", "kept"],
            "numeric_hex_sha256": "b" * 64,
            "coefficients": [1.25, -0.5],
            "vce_shape": [2, 2],
            "vce": [0.04, 0.001, 0.001, 0.09],
            "dof": {"N": 10_000_000, "df_m": 2, "df_r": 9_000_000},
            "fit": {"rss": 100.0, "r2": 0.75},
            "outputs": {},
            "solver": {
                "route": "map",
                "iterations": 4,
                "identity_sha256": "8" * 64,
            },
            "certificates": {
                "converged": True,
                "values": {
                    "normal_equation_linf": 1e-12,
                    "fe_projection_linf": 2e-12,
                    "vce_symmetry_linf": 0.0,
                    "reconstruction_linf": 3e-12,
                },
                "digests": {"result": "9" * 64},
            },
        }

    @staticmethod
    def _pairs(ratios=None, *, baseline_rss=1_000_000_000, candidate_rss=None):
        ratios = ratios or [1.0] * 9
        if candidate_rss is None:
            candidate_rss = baseline_rss
        orders = ["AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB"]
        return [
            {
                "index": index,
                "order": orders[index - 1],
                "baseline": {"seconds": 10.0, "peak_rss_bytes": baseline_rss},
                "candidate": {
                    "seconds": 10.0 * ratios[index - 1],
                    "peak_rss_bytes": candidate_rss,
                },
            }
            for index in range(1, 10)
        ]

    @staticmethod
    def _memory(constrained=None):
        return {
            "native_allocation_bytes": 100_000_000,
            "allocation_classes": {
                "observations": 4,
                "observation_columns": 7,
                "fe_levels": 2,
                "clusters": 0,
            },
            "repeated_call_retained_bytes": 0,
            "constrained_memory": constrained,
        }

    def _evidence(self, manifest=None, profile="local_screen"):
        active = manifest or self.manifest
        manifest_hash = (
            self.manifest_sha256
            if active is self.manifest
            else hashlib.sha256(F0.canonical_json_bytes(active)).hexdigest()
        )
        platform = next(
            item["platforms"][0]
            for item in active["profiles"]
            if item["id"] == profile
        )
        cell_ids = F0.EXPECTED_CELLS_BY_PROFILE[profile]
        constrained_contract = {
            "limit_bytes": 500_000_000,
            "low": {"status": "success", "rc": 0},
            "fast": {"status": "preflight_failure", "rc": 909},
        }
        cells = []
        for cell_id in cell_ids:
            scientific = self._scientific()
            if "save_fe" in cell_id:
                scientific["outputs"] = {
                    "saved_effects_sha256": "a" * 64,
                    "saved_variables": ["__hdfe1", "__hdfe2"],
                }
            constrained = (
                constrained_contract
                if cell_id == active["memory_rules"]["constrained_cell"]
                else None
            )
            cells.append(
                {
                    "cell_id": cell_id,
                    "environment": {
                        "host_id": "same-node-cell-host",
                        "os": "macOS" if platform == "macos-universal" else "Linux",
                        "arch": "universal" if platform == "macos-universal" else "x86_64",
                        "cpu": "test-cpu",
                        "stata_version": "19.0.115",
                        "stata_flavor": "MP",
                        "toolchain": "rustc-1.85.1",
                        "physical_memory_bytes": 64_000_000_000,
                    },
                    "scientific": {
                        "baseline": copy.deepcopy(scientific),
                        "candidate": copy.deepcopy(scientific),
                    },
                    "pairs": self._pairs(),
                    "confirmation_pairs": None,
                    "memory": {
                        "baseline": self._memory(copy.deepcopy(constrained)),
                        "candidate": self._memory(copy.deepcopy(constrained)),
                    },
                }
            )
        return {
            "schema_version": 1,
            "evidence_type": "fereg-f0-regression-v1-evidence",
            "profile": profile,
            "manifest": {"id": active["manifest_id"], "sha256": manifest_hash},
            "baseline": {
                "source": copy.deepcopy(active["baseline_candidate"]["source"]),
                "artifact": self._artifact(active, platform),
            },
            "candidate": self._candidate_identity(platform),
            "behavior": None,
            "cells": cells,
        }

    def _synthetic_frozen_manifest(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "frozen"
        manifest["sealing"]["state"] = "sealed"
        manifest["sealing"]["missing_required_evidence"] = []
        manifest["sealing"]["controller_source"] = {
            "commit": "a" * 40,
            "tree": "b" * 40,
        }
        for index, artifact in enumerate(manifest["baseline_candidate"]["artifacts"]):
            artifact["status"] = "qualified"
            if artifact["sha256"] is None:
                artifact["sha256"] = format(index + 10, "064x")
                artifact["size_bytes"] = 9_000_000 + index
                artifact["embedded_git_commit"] = manifest["baseline_candidate"]["source"]["commit"][:12]
        return manifest

    @staticmethod
    def _write_json(path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n",
            encoding="ascii",
        )

    @classmethod
    def _write_sealed(cls, path, payload, *, style="f0"):
        if style == "f0":
            document = F0.seal_payload(payload)
        else:
            encoded = json.dumps(
                payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True
            ).encode("ascii")
            if style == "qualification":
                encoded += b"\n"
                canonicalization = "json-sort-keys-compact-ascii-newline-v1"
            elif style == "scc":
                canonicalization = "json-sort-keys-compact-ascii-v1"
            elif style == "behavior":
                canonicalization = None
            else:
                raise AssertionError(style)
            seal = {
                "algorithm": "sha256",
                "payload_sha256": hashlib.sha256(encoded).hexdigest(),
            }
            if canonicalization is not None:
                seal["canonicalization"] = canonicalization
            document = {"payload": payload, "seal": seal}
        cls._write_json(path, document)

    @staticmethod
    def _qacct(job_id, task_id=None):
        rows = [
            "==============================================================",
            "qname shared.q",
            "hostname test-host",
            "project welfgr",
            f"jobnumber {job_id}",
        ]
        if task_id is not None:
            rows.append(f"taskid {task_id}")
        rows.extend(
            [
                "slots 4" if task_id is None else "slots 16",
                "failed 0",
                "exit_status 0",
                "ru_wallclock 12.0",
                "cpu 10.0",
                "maxvmem 2.0G",
            ]
        )
        return "\n".join(rows) + "\n"

    def _platform_load(
        self,
        root,
        platform,
        source,
        artifact,
        archive_digit,
        *,
        archive_sha256=None,
        archive_size_bytes=12345,
    ):
        platform_root = root / "evidence" / platform
        program = root / "tests/stata/installation/feppml_platform_load_capture.do"
        capture = {
            "schema_version": 1,
            "evidence_type": "feppml-licensed-stata-load-v1",
            "platform": platform,
            "artifact_sha256": artifact["sha256"],
            "artifact_size_bytes": artifact["size_bytes"],
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "license_status": "licensed",
            "plugin_load_status": "success",
            "plugin_load_rc": 0,
            "loaded_plugin_sha256": artifact["sha256"],
            "capture_program_sha256": F0.sha256_file(program),
            "mock_only": False,
            "scientific_claim": False,
            "source_commit": source["commit"],
            "embedded_git_commit": artifact["embedded_git_commit"],
            "complete_sentinel": "FEPPML_LICENSED_STATA_LOAD_COMPLETE",
        }
        capture_path = platform_root / "capture.json"
        self._write_json(capture_path, capture)
        receipt = {
            "schema_version": 1,
            "kind": "feppml_platform_licensed_stata_load",
            "platform": platform,
            "artifact_sha256": artifact["sha256"],
            "artifact_size_bytes": artifact["size_bytes"],
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "capture_program_sha256": capture["capture_program_sha256"],
            "capture_output_sha256": F0.sha256_file(capture_path),
            "source_commit": source["commit"],
            "source_tree": source["tree"],
            "source_archive_sha256": archive_sha256 or archive_digit * 64,
            "source_archive_size_bytes": archive_size_bytes,
            "embedded_git_commit": artifact["embedded_git_commit"],
            "licensed_stata_load": True,
            "mock_only": False,
            "scientific_claim": False,
            "receipt_scope": "artifact_and_licensed_stata_load_only",
            "complete_sentinel": "FEPPML_PLATFORM_LOAD_RECEIPT_COMPLETE",
        }
        receipt_path = platform_root / "platform-load.json"
        self._write_sealed(receipt_path, receipt, style="qualification")
        capture_binding = F0.file_binding(capture_path, root)
        receipt_binding = F0.file_binding(receipt_path, root, sealed=True)
        record = {
            "platform": platform,
            "source_commit": source["commit"],
            "source_tree": source["tree"],
            "source_archive_sha256": receipt["source_archive_sha256"],
            "source_archive_size_bytes": receipt["source_archive_size_bytes"],
            "embedded_git_commit": artifact["embedded_git_commit"],
            "artifact": {
                "path": f"/remote/output/{platform}.plugin",
                "sha256": artifact["sha256"],
                "size_bytes": artifact["size_bytes"],
            },
            "capture": {
                "path": "/remote/output/capture.json",
                "sha256": capture_binding["sha256"],
                "size_bytes": capture_binding["size_bytes"],
            },
            "receipt": {
                "path": "/remote/receipts/platform-load.json",
                "sha256": receipt_binding["sha256"],
                "size_bytes": receipt_binding["size_bytes"],
                "payload_sha256": receipt_binding["payload_sha256"],
            },
            "load_status": "success",
            "licensed_stata_load": True,
            "mock_only": False,
            "scientific_claim": False,
            "receipt_scope": "artifact_and_licensed_stata_load_only",
        }
        record_path = platform_root / "platform-record.json"
        self._write_json(record_path, record)
        return {
            "capture": capture_binding,
            "receipt": receipt_binding,
            "platform_record": F0.file_binding(record_path, root),
        }, receipt

    def _artifact_sealed_fixture(self, root):
        receipt_target = root / "build_evidence/local/f0-2dee5d8/receipt.json"
        receipt_target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / "build_evidence/local/f0-2dee5d8/receipt.json", receipt_target)
        behavior_manifest = root / "benchmarks/manifests/fereg_f0_behavior_v1.json"
        behavior_manifest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / "benchmarks/manifests/fereg_f0_behavior_v1.json", behavior_manifest)
        behavior_contract = json.loads(behavior_manifest.read_text(encoding="utf-8"))
        for resource in [*behavior_contract["fixtures"], *behavior_contract["resources"]]:
            destination = root / resource["path"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / resource["path"], destination)
        capture_program = root / "tests/stata/installation/feppml_platform_load_capture.do"
        capture_program.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(
            ROOT / "tests/stata/installation/feppml_platform_load_capture.do",
            capture_program,
        )

        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "artifacts_sealed"
        manifest["sealing"]["state"] = "artifacts_sealed"
        manifest["sealing"]["missing_required_evidence"] = list(
            F0.ARTIFACT_SEALED_MISSING_EVIDENCE
        )
        source = manifest["baseline_candidate"]["source"]
        linux_root = root / "evidence/linux-x86_64"
        linux_root.mkdir(parents=True, exist_ok=True)
        source_archive_path = linux_root / "source.tar"
        source_archive_path.write_bytes(
            subprocess.check_output(
                ["git", "-C", str(ROOT), "archive", "--format=tar", source["commit"]]
            )
        )
        linux_artifact_path = linux_root / "fereg_linux.plugin"
        linux_artifact_path.write_bytes(
            b"synthetic unit fixture plugin\0" + source["commit"][:12].encode("ascii")
        )
        for index, item in enumerate(manifest["baseline_candidate"]["artifacts"]):
            item["status"] = "qualified"
            if item["platform"] == "linux-x86_64":
                item["sha256"] = F0.sha256_file(linux_artifact_path)
                item["size_bytes"] = linux_artifact_path.stat().st_size
                item["embedded_git_commit"] = source["commit"][:12]
            elif item["sha256"] is None:
                item["sha256"] = ("b" if item["platform"] == "linux-x86_64" else "c") * 64
                item["size_bytes"] = 8_000_000 + index
                item["embedded_git_commit"] = source["commit"][:12]
        artifacts = {
            item["platform"]: F0._artifact_identity(item)
            for item in manifest["baseline_candidate"]["artifacts"]
        }

        record = {"sha256": "e" * 64, "size_bytes": 1}
        package_members = behavior_contract["package_contract"]["baseline_members"]
        staged_members = [
            "fereg.pkg",
            "stata.toc",
            *[
                name
                for name in package_members
                if not name.endswith(".plugin") or name == "fereg_macos.plugin"
            ],
        ]
        staged_files = {name: dict(record) for name in staged_members}
        package_snapshot = {
            "platform": "macos-universal",
            "package": {
                "format_version": 3,
                "package_name": "fereg",
                "package_version": "0.5.0",
                "description": "fixture package",
                "members": package_members,
                "file": dict(record),
            },
            "toc": {
                "format_version": 3,
                "package_name": "fereg",
                "package_version": "0.5.0",
                "description": "fixture package",
                "file": dict(record),
            },
            "staged_members": staged_members,
            "staged_files": staged_files,
            "byte_identical_fereg_files": {
                name: dict(staged_files[name])
                for name in behavior_contract["package_contract"][
                    "byte_identical_fereg_members"
                ]
            },
            "help_examples": [
                {"id": example, "sha256": "f" * 64}
                for example in behavior_contract["package_contract"]["help_example_ids"]
            ],
        }
        behavior_payload = {
            "schema_version": 1,
            "receipt_type": F0.DIRECT_BEHAVIOR_OBSERVATION_TYPE,
            "status": "pass",
            "role": "baseline",
            "phase": "baseline",
            "contract": {
                "id": "fereg_f0_behavior_v1",
                "sha256": F0.sha256_file(behavior_manifest),
            },
            "f0_manifest": {
                "id": "fereg_f0_regression_v1",
                "sha256": "d" * 64,
                "status": "candidate_unsealed",
                "source": source,
                "artifact": artifacts["macos-universal"],
            },
            "source": source,
            "artifact": artifacts["macos-universal"],
            "stata_environment": {
                "stata_version": "19.0",
                "stata_edition": "MP",
                "stata_os": "MacOSX",
                "stata_machine_type": "Macintosh (Apple Silicon)",
                "stata_processors": 8,
            },
            "package": package_snapshot,
            "fixture_inventory": [
                {
                    "id": fixture["id"],
                    "path": fixture["path"],
                    "sha256": fixture["sha256"],
                    "categories": fixture["categories"],
                }
                for fixture in behavior_contract["fixtures"]
            ],
            "fixture_outcomes": [
                {
                    "id": fixture["id"],
                    "process_rc": 0,
                    "stata_rc": 0,
                    "terminal_sentinel": f"END_FIXTURE|{fixture['id']}|rc=0",
                    "status": "pass",
                }
                for fixture in behavior_contract["fixtures"]
            ],
            "complete_sentinel": "FEREG_F0_DIRECT_COMMAND_COMPLETE",
        }
        behavior_path = root / "build_evidence/local/f0-2dee5d8/direct-command-baseline.json"
        self._write_sealed(behavior_path, behavior_payload, style="behavior")

        macos, _ = self._platform_load(
            root, "macos-universal", source, artifacts["macos-universal"], "1"
        )
        linux_shared, linux_receipt = self._platform_load(
            root, "linux-x86_64", source, artifacts["linux-x86_64"], "2"
        )
        linux_root = root / "evidence/linux-x86_64"
        platform_record_path = linux_root / "platform-record.json"
        self._write_json(platform_record_path, {"record": "bound-by-bootstrap"})
        qacct_path = linux_root / "qacct.txt"
        qacct_path.write_text(self._qacct("700"), encoding="utf-8")
        receipt_binding = linux_shared["receipt"]
        platform_record_binding = F0.file_binding(platform_record_path, root)
        qacct_record = F0._parse_qacct(qacct_path.read_text(), "fixture")[0]
        close_payload = {
            "schema_version": 1,
            "kind": "feppml_linux_platform_bootstrap_close",
            "run_id": "20260831T120000Z-platform",
            "source": {
                "commit": source["commit"],
                "tree": source["tree"],
                "archive_sha256": linux_receipt["source_archive_sha256"],
                "archive_size_bytes": linux_receipt["source_archive_size_bytes"],
                "embedded_git_commit": source["commit"][:12],
            },
            "job_id": "700",
            "qacct": {
                "job_id": "700",
                **{
                    key: qacct_record[key]
                    for key in (
                        "qname", "hostname", "slots", "failed", "exit_status",
                        "ru_wallclock", "cpu", "maxvmem",
                    )
                },
            },
            "qacct_sha256": F0.sha256_file(qacct_path),
            "platform_record": {
                "path": "output/platform-record.json",
                "sha256": platform_record_binding["sha256"],
                "size_bytes": platform_record_binding["size_bytes"],
            },
            "platform_receipt": {
                "path": "receipts/platform-load.json",
                "sha256": receipt_binding["sha256"],
                "size_bytes": receipt_binding["size_bytes"],
            },
            "platform_receipt_payload_sha256": receipt_binding["payload_sha256"],
            "licensed_stata_load": True,
            "mock_only": False,
            "scientific_claim": False,
            "terminal_sentinel": "FEPPML_LINUX_PLATFORM_BOOTSTRAP_COMPLETE",
        }
        close_path = linux_root / "linux-platform-close.json"
        self._write_sealed(close_path, close_payload, style="qualification")
        sentinel_path = linux_root / "RUN.SUCCESS"
        sentinel_path.write_text(
            f"FEPPML_LINUX_PLATFORM_BOOTSTRAP_COMPLETE|{F0.sha256_file(close_path)}\n",
            encoding="ascii",
        )
        linux = {
            **linux_shared,
            "platform_record": platform_record_binding,
            "bootstrap_close": F0.file_binding(close_path, root, sealed=True),
            "qacct": F0.file_binding(qacct_path, root),
            "success_sentinel": F0.file_binding(sentinel_path, root),
        }

        windows_shared, windows_receipt = self._platform_load(
            root, "windows-x86_64", source, artifacts["windows-x86_64"], "3"
        )
        controller = {
            "schema_version": 1,
            "run_id": "win-test",
            "project": "fereg",
            "profile": "stata-do",
            "source_revision": f"commit-{source['commit']}",
            "source_sha256": windows_receipt["source_archive_sha256"],
            "windows_target": "x86_64",
            "execution_identity": "SYSTEM",
            "status": "PASS",
            "error_code": None,
            "checks": {"source_hash": True, "stata": True, "project_test": True},
            "completed_at_utc": "2026-08-31T12:00:00Z",
        }
        controller_path = root / "evidence/windows-x86_64/controller.json"
        self._write_json(controller_path, controller)
        windows = {
            "policy": "licensed_stata_load",
            "controller_receipt": F0.file_binding(controller_path, root),
            **windows_shared,
        }
        local_binding = F0.file_binding(receipt_target, root)
        artifact_payload = {
            "schema_version": 1,
            "record_type": F0.ARTIFACT_SEAL_TYPE,
            "baseline_candidate": manifest["baseline_candidate"],
            "evidence": {
                "local_full_receipt": local_binding,
                "direct_behavior_receipt": F0.file_binding(
                    behavior_path, root, sealed=True
                ),
                "macos_load": macos,
                "linux": linux,
                "windows": windows,
            },
            "terminal_sentinel": "FEREG_F0_ARTIFACT_SEAL_COMPLETE",
        }
        artifact_path = root / "build_evidence/f0-seal/artifact-seal.json"
        self._write_sealed(artifact_path, artifact_payload)
        manifest["sealing"]["artifact_seal"] = F0.file_binding(
            artifact_path, root, sealed=True
        )
        return manifest

    def _frozen_seal_fixture(self, root):
        diagnostic = self._artifact_sealed_fixture(root)
        diagnostic_path = root / "build_evidence/f0-seal/artifacts-sealed-manifest.json"
        self._write_json(diagnostic_path, diagnostic)
        diagnostic_binding = F0.file_binding(diagnostic_path, root)
        manifest_identity = {
            "id": diagnostic["manifest_id"],
            "sha256": diagnostic_binding["sha256"],
        }
        run_id = "20260831T130000Z-f0-baseline-evidence"
        scc_root = root / "build_evidence/f0-seal/scc"
        jobs = {"setup": "801", "smoke": "802", "array": "803", "collector": "804"}
        linux = next(
            F0._artifact_identity(item)
            for item in diagnostic["baseline_candidate"]["artifacts"]
            if item["platform"] == "linux-x86_64"
        )
        candidate = self._candidate_identity("linux-x86_64")
        arms = {
            "baseline": {
                "source": diagnostic["baseline_candidate"]["source"],
                "artifact": linux,
            },
            "candidate": candidate,
        }
        preparation = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-preparation-v1",
            "run_id": run_id,
            "manifest": manifest_identity,
        }
        preparation_path = scc_root / "preparation.json"
        self._write_sealed(preparation_path, preparation, style="scc")
        preparation_binding = F0.file_binding(preparation_path, root, sealed=True)
        setup = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-setup-v1",
            "run_id": run_id,
            "manifest": manifest_identity,
            "preparation_receipt_sha256": preparation_binding["sha256"],
            "arms": arms,
            "sge": {"job_id": jobs["setup"]},
        }
        setup_path = scc_root / "setup.json"
        self._write_sealed(setup_path, setup, style="scc")
        setup_binding = F0.file_binding(setup_path, root, sealed=True)
        submission = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-submission-v1",
            "run_id": run_id,
            "manifest": manifest_identity,
            "preparation_receipt_sha256": preparation_binding["sha256"],
            "jobs": jobs,
        }
        submission_path = scc_root / "submission.json"
        self._write_sealed(submission_path, submission, style="scc")
        submission_binding = F0.file_binding(submission_path, root, sealed=True)

        evidence = self._evidence(diagnostic, "scc_governing")
        evidence["manifest"] = manifest_identity
        evidence["baseline"] = arms["baseline"]
        evidence["candidate"] = arms["candidate"]
        task_bindings = []
        cell_bindings = []
        for task_id, cell in enumerate(evidence["cells"], 1):
            cell_id = cell["cell_id"]
            cell_path = scc_root / f"cells/{cell_id}.json"
            self._write_json(cell_path, cell)
            cell_binding = F0.file_binding(cell_path, root)
            cell_bindings.append(cell_binding)
            task = {
                "schema_version": 1,
                "receipt_type": "fereg-f0-scc-task-v1",
                "run_id": run_id,
                "task_id": task_id,
                "cell_id": cell_id,
                "manifest": manifest_identity,
                "setup_receipt_sha256": setup_binding["sha256"],
                "arms": arms,
                "environment": cell["environment"],
                "cell_sha256": cell_binding["sha256"],
                "sge": {
                    "job_id": jobs["smoke"] if task_id == 1 else jobs["array"],
                    "task_id": str(task_id),
                },
                "terminal_sentinel": f"FEREG_F0_TASK_SUCCESS|{cell_id}",
            }
            task_path = scc_root / f"tasks/task-{task_id:03d}.json"
            self._write_sealed(task_path, task, style="scc")
            task_bindings.append(F0.file_binding(task_path, root, sealed=True))
        evidence_path = scc_root / "evidence.json"
        self._write_json(evidence_path, evidence)
        evidence_binding = F0.file_binding(evidence_path, root)
        qualification_document = F0.build_receipt(
            diagnostic,
            evidence,
            manifest_sha256=diagnostic_binding["sha256"],
            root=root,
        )
        qualification_path = scc_root / "qualification.json"
        qualification_path.write_bytes(F0.canonical_json_bytes(qualification_document))
        qualification_binding = F0.file_binding(
            qualification_path, root, sealed=True
        )
        collector = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-collector-v1",
            "run_id": run_id,
            "manifest": manifest_identity,
            "evidence_sha256": evidence_binding["sha256"],
            "qualification_sha256": qualification_binding["sha256"],
            "qualification_status": "blocked",
            "qualification_reasons": ["baseline_candidate_unsealed"],
            "sge": {"job_id": jobs["collector"]},
            "terminal_sentinel": "FEREG_F0_COLLECTOR_BASELINE_EVIDENCE_COMPLETE",
        }
        collector_path = scc_root / "collector.json"
        self._write_sealed(collector_path, collector, style="scc")
        collector_binding = F0.file_binding(collector_path, root, sealed=True)

        qacct_bindings = {}
        accounting = {}
        for stage in ("setup", "smoke", "array", "collector"):
            if stage == "smoke":
                text = self._qacct(jobs[stage], 1)
                records = 1
            elif stage == "array":
                text = "".join(
                    self._qacct(jobs[stage], task_id)
                    for task_id in range(2, len(F0.SCC_GOVERNING_CELL_IDS) + 1)
                )
                records = len(F0.SCC_GOVERNING_CELL_IDS) - 1
            else:
                text = self._qacct(jobs[stage])
                records = 1
            path = scc_root / f"qacct/{stage}.txt"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
            binding = F0.file_binding(path, root)
            qacct_bindings[stage] = binding
            accounting[stage] = {
                "job_id": jobs[stage],
                "records": records,
                "sha256": binding["sha256"],
            }
        run = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-scc-baseline-evidence-v1",
            "status": "complete_nonqualifying",
            "run_id": run_id,
            "manifest": manifest_identity,
            "jobs": jobs,
            "accounting": accounting,
            "cells": list(F0.SCC_GOVERNING_CELL_IDS),
            "evidence_sha256": evidence_binding["sha256"],
            "qualification_sha256": qualification_binding["sha256"],
            "setup_receipt_sha256": setup_binding["sha256"],
            "submission_receipt_sha256": submission_binding["sha256"],
            "collector_receipt_sha256": collector_binding["sha256"],
            "terminal_sentinel": "FEREG_F0_SCC_BASELINE_EVIDENCE_COMPLETE",
        }
        run_path = scc_root / "baseline-evidence.json"
        self._write_sealed(run_path, run, style="scc")
        scc = {
            "preparation": preparation_binding,
            "setup": setup_binding,
            "submission": submission_binding,
            "task_receipts": task_bindings,
            "cell_evidence": cell_bindings,
            "evidence": evidence_binding,
            "qualification": qualification_binding,
            "collector": collector_binding,
            "run": F0.file_binding(run_path, root, sealed=True),
            "qacct": qacct_bindings,
        }
        frozen = copy.deepcopy(diagnostic)
        frozen["status"] = "frozen"
        frozen["sealing"]["state"] = "sealed"
        frozen["sealing"]["missing_required_evidence"] = []
        qualification_payload = {
            "schema_version": 1,
            "record_type": F0.QUALIFICATION_SEAL_TYPE,
            "baseline_candidate": frozen["baseline_candidate"],
            "artifact_seal": frozen["sealing"]["artifact_seal"],
            "diagnostic_manifest": diagnostic_binding,
            "scc": scc,
            "terminal_sentinel": "FEREG_F0_QUALIFICATION_SEAL_COMPLETE",
        }
        seal_path = root / "build_evidence/f0-seal/qualification-seal.json"
        self._write_sealed(seal_path, qualification_payload)
        frozen["sealing"]["qualification_seal"] = F0.file_binding(
            seal_path, root, sealed=True
        )
        return frozen, evidence, qacct_bindings

    # The lifecycle now delegates platform, behavior, and SCC receipt schemas
    # to their authoritative verifiers.  These compact fixtures exercise the
    # F0 transition and byte bindings without inventing parallel receipt
    # shapes in this test module.
    def _artifact_sealed_fixture(self, root):
        receipt_target = root / "build_evidence/local/f0-2dee5d8/receipt.json"
        receipt_target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(
            ROOT / "build_evidence/local/f0-2dee5d8/receipt.json",
            receipt_target,
        )
        manifest = copy.deepcopy(self.manifest)
        manifest["status"] = "artifacts_sealed"
        manifest["sealing"]["state"] = "artifacts_sealed"
        manifest["sealing"]["missing_required_evidence"] = list(
            F0.ARTIFACT_SEALED_MISSING_EVIDENCE
        )
        controller_source = {"commit": "a" * 40, "tree": "b" * 40}
        manifest["sealing"]["controller_source"] = controller_source
        for index, artifact in enumerate(manifest["baseline_candidate"]["artifacts"]):
            artifact["status"] = "qualified"
            if artifact["sha256"] is None:
                artifact["sha256"] = format(index + 10, "064x")
                artifact["size_bytes"] = 9_000_000 + index
                artifact["embedded_git_commit"] = manifest["baseline_candidate"][
                    "source"
                ]["commit"][:12]
        payload = {
            "schema_version": 1,
            "record_type": F0.ARTIFACT_SEAL_TYPE,
            "baseline_candidate": manifest["baseline_candidate"],
            "controller_source": controller_source,
            "target_source_archive": {"sha256": "d" * 64, "size_bytes": 1},
            "evidence": {
                "local_full_receipt": F0.file_binding(receipt_target, root),
                "direct_behavior": {},
                "macos": {},
                "linux": {},
                "windows": {},
            },
            "terminal_sentinel": "FEREG_F0_ARTIFACT_SEAL_COMPLETE",
        }
        artifact_path = root / "build_evidence/f0-seal/artifact-seal.json"
        self._write_sealed(artifact_path, payload)
        manifest["sealing"]["artifact_seal"] = F0.file_binding(
            artifact_path, root, sealed=True
        )
        return manifest

    @staticmethod
    def _artifact_verifier_patches():
        return mock.patch.multiple(
            F0,
            _verify_baseline_behavior_evidence=lambda *args, **kwargs: {},
            _verify_portable_platform_evidence=lambda *args, **kwargs: {},
            _verify_linux_platform_evidence=lambda *args, **kwargs: {},
        )

    def _frozen_seal_fixture(self, root):
        diagnostic = self._artifact_sealed_fixture(root)
        diagnostic_path = (
            root / "benchmarks/manifests/fereg_f0_artifacts_sealed_test.json"
        )
        self._write_json(diagnostic_path, diagnostic)
        diagnostic_binding = F0.file_binding(diagnostic_path, root)
        run_root = root / "build_evidence/f0-seal/diagnostic-run"
        run_root.mkdir(parents=True)
        linux = next(
            F0._artifact_identity(item)
            for item in diagnostic["baseline_candidate"]["artifacts"]
            if item["platform"] == "linux-x86_64"
        )
        arm = {
            "source": diagnostic["baseline_candidate"]["source"],
            "artifact": linux,
        }
        closure = {
            "schema_version": 1,
            "status": "complete_nonqualifying",
            "run_id": "20260831T130000Z-f0-baseline-evidence",
            "manifest": {
                "id": diagnostic["manifest_id"],
                "sha256": diagnostic_binding["sha256"],
            },
            "baseline": arm,
            "candidate": arm,
            "terminal_sentinel": "FEREG_F0_DIAGNOSTIC_RUN_VERIFIED",
        }
        frozen = copy.deepcopy(diagnostic)
        frozen["status"] = "frozen"
        frozen["sealing"]["state"] = "sealed"
        frozen["sealing"]["missing_required_evidence"] = []
        qualification_payload = {
            "schema_version": 1,
            "record_type": F0.QUALIFICATION_SEAL_TYPE,
            "baseline_candidate": frozen["baseline_candidate"],
            "artifact_seal": frozen["sealing"]["artifact_seal"],
            "diagnostic_manifest": diagnostic_binding,
            "diagnostic_run_root": run_root.relative_to(root).as_posix(),
            "diagnostic_closure": closure,
            "terminal_sentinel": "FEREG_F0_QUALIFICATION_SEAL_COMPLETE",
        }
        seal_path = root / "build_evidence/f0-seal/qualification-seal.json"
        self._write_sealed(seal_path, qualification_payload)
        frozen["sealing"]["qualification_seal"] = F0.file_binding(
            seal_path, root, sealed=True
        )
        return frozen, closure, seal_path

    def test_manifest_binds_audited_candidate_but_remains_unsealed(self):
        baseline = self.manifest["baseline_candidate"]
        self.assertEqual(
            baseline["source"]["commit"],
            "2dee5d8c4cd99b51bbaca600348f66f325b58ab4",
        )
        self.assertEqual(
            baseline["source"]["tree"],
            "cc10b50077f162cab8be10702a9627b99ee5eea2",
        )
        macos = next(item for item in baseline["artifacts"] if item["platform"] == "macos-universal")
        self.assertEqual(
            macos["sha256"],
            "633fcd1a0b91617ea5c1aff10f17facf0990bd6ca72893cb81f0c100dfeb2298",
        )
        self.assertEqual(self.manifest["status"], "candidate_unsealed")
        self.assertTrue(self.manifest["sealing"]["missing_required_evidence"])
        local_receipt = self.manifest["sealing"]["observed_local_evidence"][0]
        self.assertEqual(
            local_receipt["receipt_sha256"],
            "5813d66caa6f6b1d1a4369e56c9d90a435f6973f2b62fcfc169b3e91ba5a4b71",
        )
        self.assertEqual(
            local_receipt["raw_receipt_sha256"],
            "4c18f1b7561aed1ac5e5488c37c4d6a6c1d981990dadbe4bc1fb68eb37d39da5",
        )
        tracked_receipt = ROOT / local_receipt["receipt_path"]
        self.assertEqual(F0.sha256_file(tracked_receipt), local_receipt["receipt_sha256"])
        receipt_payload = json.loads(tracked_receipt.read_text(encoding="utf-8"))
        self.assertEqual(receipt_payload["stata_executable"], "platform-default-stata-mp")

    def test_closed_local_and_scc_inventory(self):
        self.assertEqual(len(F0.LOCAL_SCREEN_CELL_IDS), 46)
        self.assertEqual(len(F0.SCC_GOVERNING_CELL_IDS), 42)
        mutation = copy.deepcopy(self.manifest)
        mutation["cells"][0]["id"] = "invented"
        with self.assertRaisesRegex(F0.ContractError, "inventory or order"):
            F0.validate_manifest(mutation)
        evidence = self._evidence()
        evidence["cells"].pop()
        with self.assertRaisesRegex(F0.ContractError, "inventory or order"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )

    def test_green_unsealed_evidence_is_blocked_not_falsely_qualified(self):
        result = F0.evaluate_evidence(
            self.manifest,
            self._evidence(),
            manifest_sha256=self.manifest_sha256,
        )
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reasons"], ["baseline_candidate_unsealed"])
        self.assertEqual(result["scientific_policy"], "extraction_exact")
        self.assertAlmostEqual(result["aggregate_primary_geometric_mean_ratio"], 1.0)

    def test_frozen_synthetic_manifest_is_rejected_without_byte_bound_seals(self):
        manifest = self._synthetic_frozen_manifest()
        with self.assertRaisesRegex(F0.ContractError, "artifact seal"):
            F0.validate_manifest(manifest)

    def test_artifact_sealed_state_requires_all_exact_platform_and_behavior_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self._artifact_sealed_fixture(root)
            with self._artifact_verifier_patches():
                F0.validate_manifest(manifest, root=root)
            capture = root / manifest["sealing"]["artifact_seal"]["path"]
            capture.write_bytes(capture.read_bytes() + b"tamper")
            with self._artifact_verifier_patches():
                with self.assertRaisesRegex(F0.ContractError, "missing or changed"):
                    F0.validate_manifest(manifest, root=root)

    def test_sealed_manifest_can_use_a_separate_closed_evidence_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            original = workspace / "original"
            original.mkdir()
            manifest = self._artifact_sealed_fixture(original)
            seal_root = workspace / "copied-seal"
            shutil.copytree(original, seal_root)
            with self._artifact_verifier_patches():
                F0.validate_manifest(
                    manifest, root=ROOT, evidence_root=seal_root
                )
            artifact_seal = seal_root / manifest["sealing"]["artifact_seal"]["path"]
            artifact_seal.write_bytes(artifact_seal.read_bytes() + b"tamper")
            with self._artifact_verifier_patches():
                with self.assertRaisesRegex(F0.ContractError, "missing or changed"):
                    F0.validate_manifest(
                        manifest, root=ROOT, evidence_root=seal_root
                    )

    def test_windows_f0_controller_binds_reviewed_harness_not_synthetic_commit(self):
        controller = {"commit": "a" * 40, "tree": "b" * 40}
        verified = {
            "controller_commit": "c" * 40,
            "controller_tree": "d" * 40,
            "platform_provenance": {
                "harness_commit": controller["commit"],
                "harness_tree": controller["tree"],
            },
        }
        self.assertTrue(
            F0._platform_controller_matches(
                "windows-x86_64", verified, controller
            )
        )
        verified["platform_provenance"]["harness_commit"] = "e" * 40
        self.assertFalse(
            F0._platform_controller_matches(
                "windows-x86_64", verified, controller
            )
        )

    def test_behavior_uses_its_native_artifact_but_rejects_source_mismatch(self):
        manifest = self._synthetic_frozen_manifest()
        baseline_source = manifest["baseline_candidate"]["source"]
        candidate_source = self._candidate_identity("linux-x86_64")["source"]
        baseline_artifact = self._artifact(manifest, "macos-universal")
        candidate_artifact = {
            "platform": "macos-universal",
            "sha256": "d" * 64,
            "size_bytes": 9_100_000,
            "embedded_git_commit": candidate_source["commit"][:12],
        }
        environment = {
            "stata_version": "19.0",
            "stata_edition": "MP",
            "stata_os": "MacOSX",
            "stata_machine_type": "MacAppleSilicon",
            "stata_processors": 8,
        }
        baseline = {
            "phase": "baseline",
            "source": copy.deepcopy(baseline_source),
            "artifact": copy.deepcopy(baseline_artifact),
            "stata_environment": copy.deepcopy(environment),
        }
        candidate = {
            "phase": "preactivation",
            "source": copy.deepcopy(candidate_source),
            "artifact": copy.deepcopy(candidate_artifact),
            "stata_environment": copy.deepcopy(environment),
        }
        comparison = {"status": "pass", "phase": "preactivation"}
        evidence = {
            "baseline_observation": {"path": "baseline.json"},
            "baseline_bundle": {"path": "baseline.tar", "sha256": "1" * 64},
            "candidate_observation": {"path": "candidate.json"},
            "candidate_bundle": {"path": "candidate.tar", "sha256": "2" * 64},
            "comparison": {
                "path": "comparison.json",
                "payload_sha256": "3" * 64,
            },
            "verified_comparison": comparison,
        }

        def validate_with(candidate_observation):
            verifier = SimpleNamespace(
                verify_comparison=lambda *args, **kwargs: comparison,
                verify_observation=mock.Mock(
                    side_effect=[baseline, candidate_observation]
                ),
            )
            with mock.patch.object(
                F0,
                "_validate_file_binding",
                side_effect=lambda value, *args, **kwargs: (
                    Path("/retained") / value["path"],
                    {},
                ),
            ), mock.patch.object(
                F0,
                "_load_behavior_contract",
                return_value=(verifier, {}, "4" * 64),
            ):
                return F0._validate_candidate_behavior_evidence(
                    evidence,
                    ROOT,
                    ROOT,
                    baseline_source,
                    candidate_source,
                    manifest["baseline_candidate"]["artifacts"],
                )

        observed = validate_with(candidate)
        self.assertEqual(observed["baseline_artifact"], baseline_artifact)
        self.assertEqual(observed["candidate_artifact"], candidate_artifact)
        self.assertNotEqual(
            observed["candidate_artifact"]["platform"], "linux-x86_64"
        )

        wrong_source = copy.deepcopy(candidate)
        wrong_source["source"]["tree"] = "e" * 40
        with self.assertRaisesRegex(
            F0.ContractError, "candidate substitutes tested source"
        ):
            validate_with(wrong_source)

    def test_frozen_state_requires_complete_bound_scc_science_and_qacct_chain(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, closure, seal_path = self._frozen_seal_fixture(root)
            verifier = SimpleNamespace(
                verify_diagnostic_run=lambda *args, **kwargs: dict(
                    kwargs.get("expected_closure") or closure
                )
            )
            with self._artifact_verifier_patches(), mock.patch.object(
                F0, "_load_repository_module", return_value=verifier
            ):
                F0.validate_manifest(manifest, root=root)
            seal_path.write_bytes(seal_path.read_bytes() + b"tamper")
            with self._artifact_verifier_patches(), mock.patch.object(
                F0, "_load_repository_module", return_value=verifier
            ):
                with self.assertRaisesRegex(F0.ContractError, "missing or changed"):
                    F0.validate_manifest(manifest, root=root)

    def test_extraction_policy_is_exact_and_registered_fallback_requires_adr(self):
        baseline = self._scientific()
        candidate = copy.deepcopy(baseline)
        candidate["coefficients"][0] += 1e-12
        self.assertIn("coefficients", F0.compare_scientific(self.manifest, baseline, candidate))

        tolerant = copy.deepcopy(self.manifest)
        tolerant["scientific_rules"]["active_policy"] = "registered_tolerance"
        tolerant["scientific_rules"]["registered_tolerance_activation"] = {
            "authorized": True,
            "adr": "ADR-test-only",
        }
        F0.validate_manifest(tolerant)
        self.assertEqual(F0.compare_scientific(tolerant, baseline, candidate), [])
        unauthorized = copy.deepcopy(tolerant)
        unauthorized["scientific_rules"]["registered_tolerance_activation"] = {
            "authorized": False,
            "adr": None,
        }
        with self.assertRaisesRegex(F0.ContractError, "accepted ADR"):
            F0.validate_manifest(unauthorized)

    def test_scientific_failure_preempts_malformed_timing(self):
        evidence = self._evidence()
        evidence["cells"][0]["scientific"]["candidate"]["coefficients"][0] += 0.01
        evidence["cells"][0]["pairs"] = [{"not": "timing"}]
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["stage"], "scientific")
        self.assertFalse(result["performance_evaluated"])
        self.assertTrue(any(reason.endswith(":coefficients") for reason in result["reasons"]))

    def test_nine_pair_alternation_is_binding(self):
        evidence = self._evidence()
        evidence["cells"][0]["pairs"][1]["order"] = "AB"
        with self.assertRaisesRegex(F0.ContractError, "AB/BA cadence"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        evidence = self._evidence()
        evidence["cells"][0]["pairs"].pop()
        with self.assertRaisesRegex(F0.ContractError, "nine warm pairs"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )

    def test_student_t_log_ratio_ci_detects_reproducible_slowdown(self):
        evidence = self._evidence()
        cell = evidence["cells"][0]
        cell["pairs"] = self._pairs([1.001] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["status"], "blocked")
        self.assertIn(
            f"confirmation_required:{F0.LOCAL_SCREEN_CELL_IDS[0]}",
            result["reasons"],
        )
        metric = result["cells"][0]["initial"]
        self.assertAlmostEqual(metric["confidence_interval_ratio"][0], 1.001)
        cell["confirmation_pairs"] = self._pairs([1.001] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertEqual(result["status"], "fail")
        self.assertIn(
            f"confirmed_reproducible_paired_slowdown:{cell['cell_id']}",
            result["reasons"],
        )

    def test_aggregate_102_gate_is_equal_weighted_across_primary_cells(self):
        evidence = self._evidence()
        for cell in evidence["cells"]:
            if "save_fe" not in cell["cell_id"]:
                cell["pairs"] = self._pairs([1.03] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn("aggregate_geometric_mean_ratio_above_1.02", result["reasons"])
        self.assertAlmostEqual(result["aggregate_primary_geometric_mean_ratio"], 1.03)

    def test_individual_105_gate_requires_and_uses_confirmation(self):
        evidence = self._evidence()
        cell = evidence["cells"][0]
        noisy_over_105 = [0.5, 2.0, 0.5, 2.0, 0.5, 2.0, 0.5, 2.0, 2.0]
        cell["pairs"] = self._pairs(noisy_over_105)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(f"confirmation_required:{cell['cell_id']}", result["reasons"])
        cell["confirmation_pairs"] = self._pairs([1.0] * 9)
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertNotIn(f"confirmation_required:{cell['cell_id']}", result["reasons"])
        self.assertEqual(result["status"], "blocked")

    def test_rss_allocation_class_allocation_bytes_and_leak_gates(self):
        evidence = self._evidence()
        cell = evidence["cells"][0]
        cell["pairs"] = self._pairs(candidate_rss=1_070_000_000)
        candidate_memory = cell["memory"]["candidate"]
        candidate_memory["allocation_classes"]["observations"] += 1
        candidate_memory["native_allocation_bytes"] += 1_048_577
        candidate_memory["repeated_call_retained_bytes"] = 1
        result = F0.evaluate_evidence(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        self.assertIn(f"peak_rss_regression:{cell['cell_id']}", result["reasons"])
        self.assertIn(f"data_sized_allocation_class_regression:{cell['cell_id']}", result["reasons"])
        self.assertIn(f"native_allocation_regression:{cell['cell_id']}", result["reasons"])
        self.assertIn(f"repeated_call_leak:{cell['cell_id']}", result["reasons"])

    def test_constrained_memory_requires_low_success_and_fast_r909(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self._artifact_sealed_fixture(root)
            evidence = self._evidence(manifest, "scc_governing")
            evidence["candidate"] = copy.deepcopy(evidence["baseline"])
            evidence["behavior"] = None
            with self._artifact_verifier_patches(), mock.patch.object(
                F0,
                "_validate_candidate_behavior_evidence",
                return_value={"status": "pass"},
            ):
                result = F0.evaluate_evidence(manifest, evidence, root=root)
            self.assertEqual(result["status"], "blocked")
            constrained = next(
                cell
                for cell in evidence["cells"]
                if cell["cell_id"] == "constrained_memory_t1_low"
            )
            constrained["memory"]["candidate"]["constrained_memory"]["fast"]["rc"] = 0
            with self._artifact_verifier_patches(), mock.patch.object(
                F0,
                "_validate_candidate_behavior_evidence",
                return_value={"status": "pass"},
            ):
                result = F0.evaluate_evidence(manifest, evidence, root=root)
            self.assertIn(
                "constrained_memory_contract:constrained_memory_t1_low",
                result["reasons"],
            )

    def test_identity_substitution_and_manifest_tampering_are_rejected(self):
        evidence = self._evidence()
        evidence["manifest"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(F0.ContractError, "manifest identity"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        evidence = self._evidence()
        evidence["baseline"]["source"]["tree"] = "0" * 40
        with self.assertRaisesRegex(F0.ContractError, "baseline source"):
            F0.evaluate_evidence(
                self.manifest, evidence, manifest_sha256=self.manifest_sha256
            )
        mutation = copy.deepcopy(self.manifest)
        mutation["performance_rules"]["aggregate_geometric_mean_ratio_max"] = 1.2
        with self.assertRaisesRegex(F0.ContractError, "not frozen"):
            F0.validate_manifest(mutation)

    def test_receipt_seal_and_external_identity_reject_tampering_and_resealing(self):
        evidence = self._evidence()
        document = F0.build_receipt(
            self.manifest, evidence, manifest_sha256=self.manifest_sha256
        )
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "receipt.json"
            identity = Path(directory) / "identity.json"
            F0.write_receipt(receipt, document, identity_path=identity)
            verified = F0.verify_receipt(receipt, MANIFEST, identity_path=identity)
            self.assertEqual(verified["payload"]["evaluation"]["status"], "blocked")

            tampered = copy.deepcopy(document)
            tampered["payload"]["evaluation"]["status"] = "pass"
            receipt.write_bytes(F0.canonical_json_bytes(tampered))
            with self.assertRaisesRegex(F0.ContractError, "seal mismatch"):
                F0.verify_receipt(receipt, MANIFEST, identity_path=identity)

            resealed = F0.build_receipt(
                self.manifest,
                evidence,
                manifest_sha256=self.manifest_sha256,
            )
            resealed["payload"]["evidence"]["candidate"]["artifact"]["size_bytes"] += 1
            resealed = F0.seal_payload(resealed["payload"])
            receipt.write_bytes(F0.canonical_json_bytes(resealed))
            with self.assertRaises(F0.ContractError):
                F0.verify_receipt(receipt, MANIFEST, identity_path=identity)

    def test_source_identity_binds_commit_tree_and_lockfiles_and_rejects_dirty(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q", str(repository)], check=True)
            subprocess.run(["git", "-C", str(repository), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(repository), "config", "user.name", "Test"], check=True)
            (repository / "Cargo.lock").write_text("cargo\n", encoding="utf-8")
            (repository / "references.lock.toml").write_text("references\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repository), "add", "Cargo.lock", "references.lock.toml"], check=True)
            subprocess.run(["git", "-C", str(repository), "commit", "-qm", "fixture"], check=True)
            identity = F0.source_identity(repository)
            self.assertEqual(F0.verify_source_identity(identity, repository), identity)
            (repository / "Cargo.lock").write_text("dirty\n", encoding="utf-8")
            with self.assertRaisesRegex(F0.ContractError, "tracked modifications"):
                F0.verify_source_identity(identity, repository)


if __name__ == "__main__":
    unittest.main()
