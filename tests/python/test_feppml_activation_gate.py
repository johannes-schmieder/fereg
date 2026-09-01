from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

from scripts import feppml_activation_gate as gate
from scripts import feppml_native_qualification as native
from scripts import feppml_phase12_local as local_runner
from ci import run_stata_ci as stata_ci


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(root), *args], text=True
    ).strip()


def commit(root: Path, message: str) -> None:
    subprocess.run(["git", "-C", str(root), "add", "."], check=True)
    subprocess.run(
        [
            "git", "-C", str(root), "-c", "user.name=Gate Test", "-c",
            "user.email=gate@example.invalid", "commit", "-qm", message,
        ],
        check=True,
    )


def preactivation_blob(relative: str) -> bytes:
    """Return the frozen preactivation control-plane blob in either state."""

    candidates = [(Path(gate.ROOT) / relative).read_bytes()]
    for revision in ("HEAD", "HEAD^"):
        completed = subprocess.run(
            ["git", "-C", str(gate.ROOT), "show", f"{revision}:{relative}"],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
        )
        if completed.returncode == 0:
            candidates.append(completed.stdout)
    for candidate in candidates:
        if relative == "scripts/stage_release.py":
            if gate.sha256_bytes(candidate) == gate.FROZEN_PREACTIVATION_STAGER_SHA256:
                return candidate
        elif relative == ".github/workflows/stata-ci.yml":
            if (
                candidate.count(b"          - ppml-fail-closed\n") == 1
                and b"          - ppml-scientific\n" not in candidate
            ):
                return candidate
        else:  # pragma: no cover - helper is intentionally closed to two files
            raise AssertionError(relative)
    raise AssertionError(f"frozen preactivation blob is unavailable: {relative}")


class ActivationGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def make_source(
        self, *, state: str = "preactivation_stub", profiles: dict | None = None
    ) -> tuple[Path, dict[str, str]]:
        root = self.root / f"source-{len(list(self.root.glob('source-*')))}"
        for directory in (
            "ci", "scripts", "stata", "tests/stata/parser",
            "tests/stata/installation", "tests/stata/documentation",
        ):
            (root / directory).mkdir(parents=True, exist_ok=True)
        if state == "preactivation_stub":
            wrapper = (
                "program define feppml, eclass sortpreserve\n"
                "    version 18.0\n    exit 498\nend\n"
            )
            package = "v 3\nf feppml.ado\n"
            stager = preactivation_blob("scripts/stage_release.py").decode("utf-8")
            default_profiles = {
                "profiles": {
                    "ppml-fail-closed": {
                        "driver": "fereg-fixtures", "build_plugin": False,
                        "fixtures": ["tests/stata/parser/one.do"],
                        "required_outputs": [],
                    },
                    "syntax": {
                        "driver": "fereg-fixtures", "build_plugin": False,
                        "fixtures": ["tests/stata/parser/one.do"],
                        "required_outputs": [],
                    },
                }
            }
        else:
            wrapper = (
                "program define feppml, eclass sortpreserve\n"
                "    version 18.0\n    _fereg_ppml `0'\nend\n"
            )
            private = gate.PRIVATE_PPML_ADO_ORDER
            package = "v 3\nf feppml.ado\n" + "".join(
                f"f {name}\n" for name in private
            )
            stager = "PRODUCTION_STATA_FILES = " + repr(("feppml.ado", *private)) + "\n"
            default_profiles = {
                "profiles": {
                    "ppml-scientific": {
                        "driver": "fereg-fixtures", "build_plugin": True,
                        "fixtures": sorted(gate.PUBLIC_PPML_FIXTURES),
                        "required_outputs": [],
                    }
                }
            }
        (root / "stata/feppml.ado").write_text(wrapper, encoding="utf-8")
        (root / "stata/fereg.pkg").write_text(package, encoding="utf-8")
        (root / "scripts/stage_release.py").write_text(stager, encoding="utf-8")
        (root / "ci/stata_profiles.json").write_text(
            json.dumps(profiles or default_profiles, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (root / "tests/stata/parser/one.do").write_text(
            'display "END_FIXTURE|one|rc=0"\n', encoding="utf-8"
        )
        for name in gate.PRIVATE_PPML_ADO_ORDER:
            (root / "stata" / name).write_text(f"*! {name}\n", encoding="utf-8")
        for relative in (
            "scripts/feppml_activation_gate.py", "scripts/feppml_phase12_local.py",
            "scripts/run_stata_tests.py", "ci/run_stata_ci.py",
        ):
            source = Path(gate.ROOT) / relative
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(source.read_bytes())
        (root / "scripts/check_repository.py").write_text(
            "#!/usr/bin/env python3\nprint('repository check pass')\n",
            encoding="utf-8",
        )
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        commit(root, "source")
        return root, {
            "commit": git(root, "rev-parse", "HEAD^{commit}"),
            "tree": git(root, "rev-parse", "HEAD^{tree}"),
        }

    def artifacts(
        self, source_root: Path, source: dict[str, str]
    ) -> tuple[list[dict], list[dict], Path]:
        artifact_root = self.root / f"artifacts-{source['commit'][:8]}"
        artifact_root.mkdir()
        archive = subprocess.check_output(
            ["git", "-C", str(source_root), "archive", "--format=tar", source["commit"]]
        )
        specs = [
            ("source_archive", "source", "source.tar", archive),
            ("plugin", "macos-universal", "fereg_macos.plugin", b"mac-"),
            ("plugin", "linux-x86_64", "fereg_linux.plugin", b"linux-"),
            ("plugin", "windows-x86_64", "fereg_windows.plugin", b"windows-"),
            ("release_archive", "all", "fereg-0.5.0.zip", b"release-"),
        ]
        identities: list[dict] = []
        records: list[dict] = []
        for role, platform_name, filename, prefix in specs:
            data = prefix if role == "source_archive" else prefix + source["commit"][:12].encode()
            path = artifact_root / filename
            path.write_bytes(data)
            identity = {
                "role": role, "platform": platform_name, "filename": filename,
                "sha256": hashlib.sha256(data).hexdigest(), "size_bytes": len(data),
                "embedded_git_commit": source["commit"][:12],
            }
            identities.append(identity)
            records.append({**identity, "path": str(path)})
        identities.sort(key=lambda item: (item["role"], item["platform"]))
        records.sort(key=lambda item: (item["role"], item["platform"]))
        index = artifact_root / "index.json"
        index.write_text(
            json.dumps({"schema_version": 1, "source": source, "artifacts": records}),
            encoding="utf-8",
        )
        return identities, records, index

    def test_local_generic_producer_result_and_check_close_end_to_end(self) -> None:
        source_root, source = self.make_source()
        _identities, _records, artifact_index = self.artifacts(source_root, source)
        result = self.root / "repository-result.json"
        summary = self.root / "repository-summary.json"
        producer = source_root / "scripts/feppml_phase12_local.py"
        expected_summary = {
            "schema_version": 1,
            "result_type": "feppml-phase12-local-command-summary-v1",
            "check_id": "repository", "source": source,
            "commands": [[
                str(Path(gate.sys.executable).resolve()),
                str(source_root.resolve() / "scripts/check_repository.py"),
            ]],
            "status": "pass",
            "complete_sentinel": gate.result_sentinel("repository"),
        }
        expected_bytes = gate.canonical_json_bytes(expected_summary)
        gate.run_closed_argv_result(
            check_id="repository", source_root=source_root,
            source_sha=source["commit"], public_state="preactivation_stub",
            artifact_index=artifact_index, producer_id="feppml-phase12-local",
            producer_version="1", producer_entrypoint=producer,
            environment_keys=["PATH"], toolchain=local_runner.toolchain("repository"),
            profile_name="phase12-local:repository",
            declared_outputs=[
                ("summary", str(summary), hashlib.sha256(expected_bytes).hexdigest())
            ],
            timeout_seconds=30,
            argv=[
                str(Path(gate.sys.executable).resolve()), str(producer.resolve()), "repository",
                "--output", str(summary.resolve()),
                "--evidence-dir", str(
                    summary.resolve().with_name(f"{summary.name}.raw")
                ),
            ],
            output=result,
        )
        check = self.root / "repository-check.json"
        gate.make_check_receipt(
            check_id="repository", source_root=source_root,
            source_sha=source["commit"], public_state="preactivation_stub",
            artifact_index=artifact_index, producer_id="feppml-activation-gate",
            producer_version="1",
            producer_entrypoint=source_root / "scripts/feppml_activation_gate.py",
            upstream=[("producer_result", result)], output=check,
        )
        payload, _digest, _size = gate.verify_check_receipt(
            check, source_root=source_root
        )
        self.assertEqual(payload["check_id"], "repository")
        command_log = summary.resolve().with_name(
            f"{summary.name}.raw"
        ) / "command-000.stdout.log"
        original_log = command_log.read_bytes()
        command_log.chmod(0o644)
        command_log.write_bytes(original_log + b"forged\n")
        with self.assertRaisesRegex(gate.GateError, "changed"):
            gate.verify_check_receipt(check, source_root=source_root)
        command_log.write_bytes(original_log)
        summary.chmod(0o644)
        summary.write_text("{}\n", encoding="utf-8")
        with self.assertRaisesRegex(gate.GateError, "changed"):
            gate.verify_check_receipt(check, source_root=source_root)

    def test_stata_source_materialization_ignores_dirty_worktree(self) -> None:
        source_root, source = self.make_source()
        fixture = source_root / "tests/stata/parser/one.do"
        committed = fixture.read_bytes()
        fixture.write_text("display \"dirty bypass\"\n", encoding="utf-8")
        destination = self.root / "materialized"
        destination.mkdir()
        archive = self.root / "materialized-source.tar"
        with mock.patch.dict(os.environ, {}, clear=False):
            os.environ.pop("FEREG_TRACKED_FILES_MANIFEST", None)
            record = stata_ci.materialize_exact_commit(
                source_root, source["commit"], destination, archive
            )
        self.assertEqual(
            (destination / "tests/stata/parser/one.do").read_bytes(), committed
        )
        self.assertEqual(record["source"], source)
        self.assertEqual(
            record["build_date_utc"],
            gate._commit_build_date(source_root, source["commit"]),
        )
        with mock.patch.dict(
            os.environ, {"FEREG_TRACKED_FILES_MANIFEST": "/tmp/forged"}
        ):
            with self.assertRaisesRegex(RuntimeError, "not allowed"):
                stata_ci.materialize_exact_commit(
                    source_root, source["commit"], self.root / "rejected",
                    self.root / "rejected.tar",
                )

    def make_stata_evidence(
        self, source_root: Path, source: dict[str, str]
    ) -> tuple[Path, Path]:
        evidence = self.root / "stata-evidence"
        (evidence / "fixture-logs").mkdir(parents=True)
        archive = subprocess.check_output(
            ["git", "-C", str(source_root), "archive", "--format=tar", source["commit"]]
        )
        (evidence / "source.tar").write_bytes(archive)
        (evidence / "source.json").write_text(
            json.dumps({
                "schema_version": 1,
                "record_type": "fereg-exact-source-archive-v1",
                "source": source,
                "build_date_utc": gate._commit_build_date(
                    source_root, source["commit"]
                ),
                "archive": {
                    "filename": "source.tar",
                    "sha256": hashlib.sha256(archive).hexdigest(),
                    "size_bytes": len(archive),
                },
            }),
            encoding="utf-8",
        )
        fixture = "tests/stata/parser/one.do"
        stable = fixture.replace("/", "__")
        (evidence / "fixture-logs" / f"{stable}.log").write_text(
            "END_FIXTURE|one|rc=0\n", encoding="utf-8"
        )
        status = {
            "schema_version": "1", "completed": "1", "stata_rc": "0",
            "stata_version": "18.0", "stata_edition": "MP", "stata_os": "MacOSX",
            "stata_machine_type": "arm64", "stata_processors": "8",
        }
        (evidence / "fixture-logs" / f"{stable}.status").write_text(
            "".join(f"{key}={value}\n" for key, value in status.items()),
            encoding="utf-8",
        )
        row = {
            "fixture": fixture, "status": "success", "failure_kind": None,
            "failure_detail": None, "process_rc": 0, "stata_rc": 0,
        }
        common = {
            "schema_version": 1, "completed": True, "status": "success",
            "failure_kind": None, "failure_detail": None, "stata_rc": 0,
            "stata_version": "18.0", "stata_edition": "MP", "stata_os": "MacOSX",
            "stata_machine_type": "arm64", "stata_processors": 8,
            "tests_passed": 1, "tests_failed": 0, "results": [row],
        }
        (evidence / "fixture-summary.json").write_text(
            json.dumps(common), encoding="utf-8"
        )
        profile = {
            **common, "profile": "syntax", "plugin_sha256": None,
            "build_rc": None, "break_receipts": [],
        }
        profile_path = evidence / "profile-summary.json"
        profile_path.write_text(json.dumps(profile), encoding="utf-8")
        process = {
            "schema_version": 1, "started_at": "start",
            "command_executable": str(Path(gate.sys.executable).resolve()),
            "launch_error": None,
            "timed_out": False, "process_rc": 0, "pid": 42,
            "completed_at": "end", "duration_seconds": 1.0,
        }
        process_cwd = Path("/private/tmp/fereg-stata-ci-focused")
        staged_root = process_cwd / "repo"
        process["cwd"] = str(process_cwd)
        process["argv"] = [
            str(Path(gate.sys.executable).resolve()),
            str(staged_root / "ci/run_fixture_profile.py"),
            "--profile-config", str(staged_root / "ci/stata_profiles.json"),
            "--profile", "syntax", "--repo-root", str(staged_root),
            "--run-root", str(process_cwd),
            "--summary-json", str(process_cwd / "profile-summary.json"),
            "--stata", "/Applications/Stata", "--tested-sha", source["commit"],
        ]
        exact_env = {
            "FEREG_BUILD_DATE": gate._commit_build_date(
                source_root, source["commit"]
            ),
            "FEREG_GIT_COMMIT": source["commit"][:12],
            "PATH": "/usr/bin:/bin",
        }
        process["environment"] = [
            {
                "name": name,
                "present": name in exact_env,
                "value_sha256": (
                    hashlib.sha256(exact_env[name].encode()).hexdigest()
                    if name in exact_env else None
                ),
            }
            for name in gate.STATA_PROCESS_ENVIRONMENT
        ]
        (evidence / "process.json").write_text(json.dumps(process), encoding="utf-8")
        (evidence / "process.stdout.log").write_text(
            "PASS: END_FIXTURE|one|rc=0\n", encoding="utf-8"
        )
        (evidence / "process.stderr.log").write_bytes(b"")
        ci = {
            "schema_version": 1, "tested_sha": source["commit"],
            "repository": "local/fereg", "ref": "local", "run_id": "local",
            "run_attempt": "1", "profile": "syntax", "driver": "fereg-fixtures",
            "status": "success", "stata_status": "success", "rust_status": "not_run",
            "rust_rc": None, "failure_kind": None, "failure_detail": None,
            "process_rc": 0, "stata_rc": 0, "stata_version": "18.0",
            "stata_edition": "MP", "stata_bundle_version": "18.0",
            "stata_executable": "/Applications/Stata", "platform": "MacOSX; arm64",
            "stata_processors": 8, "runner_name": "focused", "started_at": "start",
            "completed_at": "end", "duration_seconds": 1.0, "tests_passed": 1,
            "tests_failed": 0, "plugin_sha256": None, "required_outputs": [],
        }
        ci_path = evidence / "receipt.json"
        ci_path.write_text(json.dumps(ci), encoding="utf-8")
        return ci_path, profile_path

    def test_stata_raw_evidence_seals_and_log_mutation_fails(self) -> None:
        profiles = {
            "profiles": {
                "ppml-fail-closed": {
                    "driver": "fereg-fixtures", "build_plugin": False,
                    "fixtures": ["tests/stata/parser/one.do"], "required_outputs": [],
                },
                "syntax": {
                    "driver": "fereg-fixtures", "build_plugin": False,
                    "fixtures": ["tests/stata/parser/one.do"], "required_outputs": [],
                },
            }
        }
        source_root, source = self.make_source(profiles=profiles)
        _identities, _records, artifact_index = self.artifacts(source_root, source)
        ci, summary = self.make_stata_evidence(source_root, source)
        result = self.root / "stata-result.json"
        gate.seal_stata_profile_result(
            check_id="stata_syntax", source_root=source_root,
            source_sha=source["commit"], public_state="preactivation_stub",
            artifact_index=artifact_index, ci_receipt=ci, profile_summary=summary,
            producer_entrypoint=source_root / "ci/run_stata_ci.py", output=result,
        )
        check = self.root / "stata-check.json"
        gate.make_check_receipt(
            check_id="stata_syntax", source_root=source_root,
            source_sha=source["commit"], public_state="preactivation_stub",
            artifact_index=artifact_index, producer_id="feppml-activation-gate",
            producer_version="1",
            producer_entrypoint=source_root / "scripts/feppml_activation_gate.py",
            upstream=[("producer_result", result)], output=check,
        )
        gate.verify_check_receipt(check, source_root=source_root)
        log = ci.parent / "fixture-logs/tests__stata__parser__one.do.log"
        log.write_text("END_FIXTURE|wrong|rc=0\n", encoding="utf-8")
        with self.assertRaisesRegex(gate.GateError, "changed|sentinel"):
            gate.verify_check_receipt(check, source_root=source_root)

    def test_staged_requires_both_installation_fixtures_when_activated(self) -> None:
        source = {"commit": "a" * 40, "tree": "b" * 40}
        summary = {
            "schema_version": 1, "completed": True, "status": "success",
            "source_sha": source["commit"], "archive_sha256": "c" * 64,
            "archive_bytes": 10, "manifest_sha256": "d" * 64,
            "plugin": "fereg_macos.plugin", "plugin_sha256": "e" * 64,
            "plugin_bytes": 10, "stata_version": "18", "stata_edition": "MP",
            "stata_executable": "/Applications/Stata",
            "stata_executable_sha256": "f" * 64,
            "stata_executable_size_bytes": 1,
            "stata_os": "MacOSX", "stata_machine_type": "arm64",
            "stata_processors": 8, "tests_passed": 1, "tests_failed": 0,
            "results": [{
                "fixture": "tests/stata/installation/feppml_installation_smoke.do",
                "status": "success", "failure_kind": None, "failure_detail": None,
                "process_rc": 0, "stata_rc": 0,
            }],
        }
        path = self.root / "staged-summary.json"
        path.write_text(json.dumps(summary), encoding="utf-8")
        archive = self.root / "archive.zip"
        archive.write_bytes(b"archive")
        artifacts = [{
            "role": "release_archive", "platform": "all", "filename": "archive.zip",
            "sha256": hashlib.sha256(b"archive").hexdigest(), "size_bytes": 7,
            "embedded_git_commit": source["commit"][:12],
        }]
        with self.assertRaisesRegex(gate.GateError, "inventory|summary"):
            gate._validate_staged_evidence_files(
                source_root=self.root, source=source, public_state="activated",
                artifacts=artifacts, summary_path=path, archive_path=archive,
            )

    def test_f0_verifier_is_invoked_and_artifact_cross_bound(self) -> None:
        source = {"commit": "a" * 40, "tree": "b" * 40}
        artifact = {
            "role": "plugin", "platform": "linux-x86_64", "filename": "x",
            "sha256": "c" * 64, "size_bytes": 12,
            "embedded_git_commit": source["commit"][:12],
        }
        native_artifact = {
            "role": "plugin", "platform": "macos-universal", "filename": "y",
            "sha256": "e" * 64, "size_bytes": 13,
            "embedded_git_commit": source["commit"][:12],
        }
        document = {
            "payload": {
                "evidence": {
                    "candidate": {
                        "source": source,
                        "artifact": {
                            "platform": "linux-x86_64", "sha256": "d" * 64,
                            "size_bytes": 12,
                            "embedded_git_commit": source["commit"][:12],
                        },
                    }
                },
                "evaluation": {
                    "status": "pass", "stage": "complete",
                    "performance_evaluated": True,
                    "behavior": {
                        "candidate_source": source,
                        "candidate_artifact": {
                            "platform": "macos-universal",
                            "sha256": native_artifact["sha256"],
                            "size_bytes": native_artifact["size_bytes"],
                            "embedded_git_commit": source["commit"][:12],
                        },
                    },
                },
            }
        }
        verifier = mock.Mock(return_value=document)
        with mock.patch.object(
            gate, "_source_module", return_value=SimpleNamespace(verify_receipt=verifier)
        ):
            with self.assertRaisesRegex(gate.GateError, "incomplete|mixed"):
                gate._validate_upstream_outcome(
                    "fereg_f0",
                    {"f0_qualification": ({}, self.root / "f0.json", "fereg-f0-regression-v1")},
                    source, "preactivation_stub", [artifact, native_artifact], self.root,
                )
        verifier.assert_called_once()

    def test_f0_cross_binds_distinct_linux_and_licensed_host_artifacts(self) -> None:
        source = {"commit": "a" * 40, "tree": "b" * 40}
        artifacts = [
            {
                "role": "plugin", "platform": "linux-x86_64", "filename": "linux",
                "sha256": "c" * 64, "size_bytes": 12,
                "embedded_git_commit": source["commit"][:12],
            },
            {
                "role": "plugin", "platform": "macos-universal", "filename": "mac",
                "sha256": "d" * 64, "size_bytes": 13,
                "embedded_git_commit": source["commit"][:12],
            },
        ]
        behavior_source = dict(source)
        document = {
            "payload": {
                "evidence": {
                    "candidate": {
                        "source": source,
                        "artifact": {
                            "platform": "linux-x86_64",
                            "sha256": artifacts[0]["sha256"],
                            "size_bytes": artifacts[0]["size_bytes"],
                            "embedded_git_commit": source["commit"][:12],
                        },
                    }
                },
                "evaluation": {
                    "status": "pass",
                    "stage": "complete",
                    "performance_evaluated": True,
                    "behavior": {
                        "candidate_source": behavior_source,
                        "candidate_artifact": {
                            "platform": "macos-universal",
                            "sha256": artifacts[1]["sha256"],
                            "size_bytes": artifacts[1]["size_bytes"],
                            "embedded_git_commit": source["commit"][:12],
                        },
                    },
                },
            }
        }
        verifier = mock.Mock(return_value=document)
        with mock.patch.object(
            gate,
            "_source_module",
            return_value=SimpleNamespace(verify_receipt=verifier),
        ):
            gate._validate_upstream_outcome(
                "fereg_f0",
                {
                    "f0_qualification": (
                        {}, self.root / "f0.json", "fereg-f0-regression-v1"
                    )
                },
                source,
                "preactivation_stub",
                artifacts,
                self.root,
            )
            mismatched = copy.deepcopy(document)
            mismatched["payload"]["evaluation"]["behavior"][
                "candidate_source"
            ]["tree"] = "f" * 40
            verifier.return_value = mismatched
            with self.assertRaisesRegex(gate.GateError, "incomplete|mixed"):
                gate._validate_upstream_outcome(
                    "fereg_f0",
                    {
                        "f0_qualification": (
                            {}, self.root / "f0.json", "fereg-f0-regression-v1"
                        )
                    },
                    source,
                    "preactivation_stub",
                    artifacts,
                    self.root,
                )

    def test_scc_f0_verifier_receives_exact_committed_manifest(self) -> None:
        source = {"commit": "a" * 40, "tree": "b" * 40}
        artifact = {
            "role": "plugin", "platform": "linux-x86_64", "filename": "x",
            "sha256": "c" * 64, "size_bytes": 12,
            "embedded_git_commit": source["commit"][:12],
        }
        close = self.root / "run/receipts/final.json"
        close.parent.mkdir(parents=True)
        close.write_text("{}", encoding="utf-8")
        verifier = mock.Mock(return_value={
            "candidate_source_commit": source["commit"],
            "candidate": {
                "source": source,
                "artifact": {
                    "platform": "linux-x86_64", "sha256": artifact["sha256"],
                    "size_bytes": artifact["size_bytes"],
                },
            },
        })
        with mock.patch.object(
            gate, "_source_module", return_value=SimpleNamespace(verify_closed_run=verifier)
        ):
            gate._validate_upstream_outcome(
                "scc_fereg_f0", {"scc_close": ({}, close, "x")}, source,
                "preactivation_stub", [artifact, {
                    "role": "source_archive", "platform": "source", "filename": "source.tar",
                    "sha256": "d" * 64, "size_bytes": 1,
                    "embedded_git_commit": source["commit"][:12],
                }], self.root,
            )
        self.assertEqual(
            verifier.call_args.args[1],
            self.root / "benchmarks/manifests/fereg_f0_regression_v1.json",
        )

    def test_native_receipt_without_per_step_commands_is_rejected(self) -> None:
        path = self.root / "native.json"
        payload = {
            "schema_version": 1, "kind": native.MACOS_BUILD_TYPE,
            "source": {"commit": "a" * 40, "tree": "b" * 40},
            "producer": {}, "artifact": {}, "files": {}, "exit_code": 0,
            "build_binary_audit_actual_loader": "success",
            "complete_sentinel": native.MACOS_SENTINEL,
        }
        path.write_bytes(native.canonical_json_bytes(native.seal_payload(payload)))
        with self.assertRaisesRegex(native.NativeQualificationError, "fields are not closed"):
            native.verify_macos_build(path, self.root)

    def test_native_build_date_is_commit_derived_not_wall_clock(self) -> None:
        source_root, source = self.make_source()
        with mock.patch.dict(os.environ, {"TZ": "Pacific/Kiritimati"}):
            east = native._commit_build_date(source_root, source["commit"])
        with mock.patch.dict(os.environ, {"TZ": "Pacific/Pago_Pago"}):
            west = native._commit_build_date(source_root, source["commit"])
        self.assertEqual(east, west)
        self.assertEqual(
            east, gate._commit_build_date(source_root, source["commit"])
        )

    def test_shared_fixture_mutation_is_forbidden_in_activation_diff(self) -> None:
        root = self.root / "lineage"
        fixture = next(iter(gate.PUBLIC_PPML_FIXTURES))
        path = root / fixture
        path.parent.mkdir(parents=True)
        path.write_text("qualified\n", encoding="utf-8")
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        commit(root, "parent")
        parent = git(root, "rev-parse", "HEAD")
        path.write_text("END_FIXTURE|trivial|rc=0\n", encoding="utf-8")
        commit(root, "mutate fixture")
        active = git(root, "rev-parse", "HEAD")
        with self.assertRaisesRegex(gate.GateError, "forbidden path"):
            gate._changed_paths(root, parent, active)

    def test_activation_workflow_change_is_exact_single_choice_swap(self) -> None:
        workflow_path = ".github/workflows/stata-ci.yml"
        self.assertIn(workflow_path, gate.ACTIVATION_ALLOWED_PATHS)
        self.assertIn(workflow_path, gate.ACTIVATION_REQUIRED_CHANGED_PATHS)
        parent = preactivation_blob(workflow_path)
        self.assertEqual(parent.count(b"          - ppml-hidden\n"), 1)
        self.assertEqual(parent.count(b"          - ppml-fail-closed\n"), 1)
        self.assertNotIn(b"          - ppml-scientific\n", parent)
        active = parent.replace(
            b"          - ppml-fail-closed\n",
            b"          - ppml-scientific\n",
        )
        gate._require_exact_activation_workflow(parent, active)
        self.assertEqual(active.count(b"          - ppml-hidden\n"), 1)
        with self.assertRaisesRegex(gate.GateError, "exact"):
            gate._require_exact_activation_workflow(
                parent, active.replace(b"          - quick\n", b"")
            )

    def test_activation_release_stager_is_one_exact_transform(self) -> None:
        parent = preactivation_blob("scripts/stage_release.py")
        active = gate._expected_activated_stager(parent)
        gate._require_exact_activation_stager(parent, active)
        self.assertEqual(active.count(b"PRODUCTION_STATA_FILES ="), 1)
        self.assertEqual(active.count(b"SUPPORT_CAVEATS ="), 1)
        self.assertIn(b'"ols_result_wire": "1.4"', active)
        self.assertIn(b'"ppml_request_wire": "1.1"', active)
        self.assertIn(b'"ppml_result_wire": "1.1"', active)
        mutations = (
            active + b'\nPRODUCTION_STATA_FILES += ("developer.ado",)\n',
            active.replace(b'VERSION = "0.5.0"', b'VERSION = "0.5.1"'),
            active.replace(b'"ols_result_wire": "1.4"', b'"ols_result_wire": "1.5"'),
            active.replace(b"def copy_release_files(", b"def copy_release_files_changed("),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation), self.assertRaisesRegex(
                gate.GateError, "exact reviewed"
            ):
                gate._require_exact_activation_stager(parent, mutation)

    def test_preactivation_stager_is_frozen_beyond_first_inventory(self) -> None:
        parent = preactivation_blob("scripts/stage_release.py")
        self.assertEqual(
            gate.sha256_bytes(parent), gate.FROZEN_PREACTIVATION_STAGER_SHA256
        )
        mutation = parent.replace(
            b"def copy_release_files(\n", b"def copy_release_files_changed(\n", 1
        )
        self.assertEqual(
            gate._production_stata_files(parent, "parent"),
            gate._production_stata_files(mutation, "mutation"),
        )
        source_root, _source = self.make_source()
        stager = source_root / "scripts/stage_release.py"
        stager.write_bytes(mutation)
        commit(source_root, "mutate stager")
        mutated = {
            "commit": git(source_root, "rev-parse", "HEAD"),
            "tree": git(source_root, "rev-parse", "HEAD^{tree}"),
        }
        with self.assertRaisesRegex(gate.GateError, "frozen F0 bytes"):
            gate.audit_source(source_root, mutated, "preactivation_stub")

    def test_self_sealed_preactivation_aggregate_is_not_authentication(self) -> None:
        prior = {
            "public_state": "preactivation_stub",
            "source": {"commit": "b" * 40, "tree": "c" * 40},
        }
        receipt = self.root / "prior.json"
        receipt.write_text("{}\n", encoding="utf-8")
        index = self.root / "prior-index.json"
        index.write_text("{}\n", encoding="utf-8")
        parent_root = self.root / "parent-root"
        (parent_root / ".git").mkdir(parents=True)
        raw = receipt.read_bytes()
        record = {
            "receipt_path": str(receipt),
            "receipt_sha256": hashlib.sha256(raw).hexdigest(),
            "receipt_size_bytes": len(raw),
            "receipt_payload_sha256": hashlib.sha256(
                gate.canonical_json_bytes(prior)
            ).hexdigest(),
            "index_path": str(index),
            "index_sha256": gate.sha256_file(index),
            "index_size_bytes": index.stat().st_size,
            "source_root": str(parent_root),
        }

        def fake_git(_root: Path, args: list[str]) -> str:
            if args[:2] == ["rev-list", "--parents"]:
                return f"{'a'*40} {'b'*40}"
            if args[:2] == ["rev-parse", f"{'b'*40}^{{tree}}"]:
                return "c" * 40
            raise AssertionError(args)

        with (
            mock.patch.object(
                gate, "verify_aggregate_receipt",
                side_effect=gate.GateError(
                    "aggregate receipt cannot be reproduced from authoritative inputs"
                ),
            ),
            mock.patch.object(gate, "_git", side_effect=fake_git),
        ):
            with self.assertRaisesRegex(gate.GateError, "cannot be reproduced"):
                gate.audit_activation_lineage(
                    self.root, {"commit": "a" * 40, "tree": "d" * 40},
                    "activated", record, index_base=self.root,
                )

    def test_authoritative_aggregate_verify_recomputes_index(self) -> None:
        source_root = self.root / "verify-source"
        (source_root / ".git").mkdir(parents=True)
        index = self.root / "verify-index.json"
        index.write_text("{}\n", encoding="utf-8")
        receipt = self.root / "verify-receipt.json"
        receipt.write_text("{}\n", encoding="utf-8")
        payload = {
            "input_index_sha256": gate.sha256_file(index),
            "status": "pass",
        }
        with (
            mock.patch.object(gate, "_parse_aggregate_receipt", return_value=payload),
            mock.patch.object(gate, "evaluate", return_value={**payload, "forged": False}),
        ):
            with self.assertRaisesRegex(gate.GateError, "cannot be reproduced"):
                gate.verify_aggregate_receipt(receipt, index, source_root)

    def test_schema_rejects_boolean_only_native_and_wrong_raw_encoding(self) -> None:
        source = {"commit": "a" * 40, "tree": "b" * 40}
        artifact = {
            "role": "plugin", "platform": "linux-x86_64", "filename": "x",
            "sha256": "c" * 64, "size_bytes": 1,
            "embedded_git_commit": source["commit"][:12],
        }
        payload = {
            "schema_version": 1, "receipt_type": gate.CHECK_RECEIPT_TYPE,
            "check_id": "native_build", "source": source,
            "public_state": "preactivation_stub", "artifacts": [artifact],
            "claims": list(gate.COMMON_CHECK_CLAIMS["native_build"]),
            "producer": {"id": "x", "version": "1", "entrypoint_sha256": "d" * 64},
            "upstream": [], "status": "pass", "waivers": [],
            "complete_sentinel": gate.CHECK_SENTINEL,
        }
        with self.assertRaisesRegex(gate.GateError, "upstream inventory"):
            gate.validate_check_payload(payload)
        payload["check_id"] = "feppml_qualification"
        payload["claims"] = list(gate.COMMON_CHECK_CLAIMS["feppml_qualification"])
        payload["upstream"] = [
            {
                "role": "qualification_evidence",
                "receipt_type": "feppml-qualification-v1-evidence",
                "encoding": "sealed_json", "path": "/tmp/x",
                "file_sha256": "e" * 64, "payload_sha256": "f" * 64,
                "size_bytes": 1,
            },
            {
                "role": "qualification_receipt",
                "receipt_type": "feppml_qualification_evaluation",
                "encoding": "sealed_json", "path": "/tmp/y",
                "file_sha256": "1" * 64, "payload_sha256": "2" * 64,
                "size_bytes": 1,
            },
        ]
        with self.assertRaisesRegex(gate.GateError, "wrong encoding"):
            gate.validate_check_payload(payload)


if __name__ == "__main__":
    unittest.main()
