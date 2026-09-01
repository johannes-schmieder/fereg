from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
PROGRAM = ROOT / "benchmarks/scc/feppml_linux_platform_scc.py"
SPEC = importlib.util.spec_from_file_location(
    "feppml_linux_platform_scc_test", PROGRAM
)
assert SPEC is not None and SPEC.loader is not None
SCC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCC)


class FeppmlLinuxPlatformSccTests(unittest.TestCase):
    def test_direct_isolated_cli_bootstraps_repository_imports(self):
        completed = subprocess.run(
            [sys.executable, "-I", "-B", "-S", str(PROGRAM), "--help"],
            cwd="/",
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("Closed SCC bootstrap", completed.stdout)

    def repository(
        self, root: Path, name: str, paths: tuple[str, ...]
    ) -> tuple[Path, str]:
        repo = root / name
        for relative in paths:
            source = ROOT / relative
            destination = repo / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source.read_bytes())
            if relative.endswith((".py", ".sh")):
                destination.chmod(0o755)
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.email", "test@example.invalid"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.name", "SCC Test"],
            check=True,
        )
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(repo), "commit", "-qm", "bootstrap"], check=True
        )
        commit = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()
        return repo, commit

    def prepared(
        self, root: Path
    ) -> tuple[Path, Path, str, Path, str, Path, object]:
        repo, commit = self.repository(root, "source", SCC.SOURCE_IDENTITY_PATHS)
        harness_repo, harness_commit = self.repository(
            root, "harness", SCC.HARNESS_PATHS
        )
        project = root / "project"
        run_root = project / "runs/20260831T180000Z-feppml-linux-platform"
        SCC.prepare(
            argparse.Namespace(
                run_root=run_root,
                candidate_repo=repo,
                candidate_commit=commit,
                harness_repo=harness_repo,
                harness_commit=harness_commit,
            )
        )
        patcher = mock.patch.object(SCC, "PROJECT_ROOT", project.resolve())
        patcher.start()
        self.addCleanup(patcher.stop)
        SCC.record_deployment(
            argparse.Namespace(
                run_root=run_root,
                preparation_sha256=SCC.digest_file(
                    run_root / "input/preparation.json"
                ),
            )
        )
        SCC.record_submission(
            argparse.Namespace(run_root=run_root, job_id="7387001")
        )
        return (
            repo,
            harness_repo,
            commit,
            run_root,
            harness_commit,
            project,
            patcher,
        )

    def qacct_text(
        self,
        submission: dict[str, object],
        *,
        overrides: dict[str, str] | None = None,
    ) -> str:
        ledger = submission["ledger"]
        assert isinstance(ledger, dict)
        values = {
            "qname": "batch.q",
            "hostname": "node-a",
            "owner": "johannes",
            "project": "welfgr",
            "jobname": str(ledger["job_name"]),
            "jobnumber": str(submission["job_id"]),
            "taskid": "undefined",
            "qsub_time": "Sun Aug 31 18:00:00 2026",
            "start_time": "Sun Aug 31 18:01:00 2026",
            "end_time": "Sun Aug 31 18:02:00 2026",
            "granted_pe": "omp",
            "slots": "4",
            "failed": "0",
            "exit_status": "0",
            "ru_wallclock": "60.0",
            "cpu": "45.5",
            "maxvmem": "1.5G",
            "cwd": str(ledger["submit_cwd"]),
            "scriptfile": str(ledger["scriptfile"]),
            "submit_cmd_line": shlex.join(ledger["argv"]),
        }
        values.update(overrides or {})
        body = "\n".join(f"{key:<15} {value}" for key, value in values.items())
        return "=" * 62 + "\n" + body + "\n"

    def complete_fake_job(self, run_root: Path) -> dict[str, object]:
        preparation = SCC.verify_input(run_root)
        source = preparation["source"]
        controller = preparation["harness_source"]
        artifact = run_root / "output/fereg_linux.plugin"
        artifact.write_bytes(
            b"ELF licensed-load fixture "
            + source["embedded_git_commit"].encode("ascii")
            + b" "
            + source["build_date_utc"].encode("ascii")
        )
        build_log = run_root / "logs/build.log"
        build_log.write_text(
            "build=PASS\nbinary_audit=PASS\nactual_plugin_loader=PASS\n",
            encoding="ascii",
        )
        capture_program = (
            run_root
            / "input/harness/tests/stata/installation/feppml_platform_load_capture.do"
        )
        capture_path = run_root / "output/capture.json"
        capture = {
            "schema_version": 1,
            "evidence_type": "feppml-licensed-stata-load-v1",
            "platform": "linux-x86_64",
            "artifact_sha256": SCC.digest_file(artifact),
            "artifact_size_bytes": artifact.stat().st_size,
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "license_status": "licensed",
            "plugin_load_status": "success",
            "plugin_load_rc": 0,
            "loaded_plugin_sha256": SCC.digest_file(artifact),
            "capture_program_sha256": SCC.digest_file(capture_program),
            "mock_only": False,
            "scientific_claim": False,
            "source_commit": source["commit"],
            "embedded_git_commit": source["embedded_git_commit"],
            "complete_sentinel": "FEPPML_LICENSED_STATA_LOAD_COMPLETE",
        }
        capture_path.write_text(
            json.dumps(capture, sort_keys=True, indent=2) + "\n", encoding="ascii"
        )
        attestation = run_root / "output/native-attestation.json"
        sidecars = SCC.platform_receipts.native_sidecar_paths(attestation)
        sidecars["status"].write_text(
            "schema_version=1\ncompleted=1\nstata_rc=0\n"
            "stata_version=19.0\nstata_flavor=MP\nstata_os=Unix\n",
            encoding="ascii",
        )
        sidecars["stata_log"].write_text(
            "licensed Stata batch transcript\n"
            f"fereg.git_commit={source['embedded_git_commit']}\n"
            "FEPPML_LICENSED_STATA_LOAD_COMPLETE\n",
            encoding="utf-8",
        )
        producer = run_root / "input/harness/scripts/capture_feppml_platform_load.py"
        stata_bytes = b"licensed-stata-test-executable"
        stata_sha = hashlib.sha256(stata_bytes).hexdigest()
        runtime_root = Path("/private/tmp/feppml-native-test-run")
        runtime_stata = Path("/Applications/Stata/StataMP")
        runtime_program = runtime_root / capture_program.name
        runtime_artifact = runtime_root / artifact.name
        runtime_capture = runtime_root / "capture.json"
        runtime_status = runtime_root / "capture.status"
        runtime_isolated = runtime_root / "sysdirs"
        runtime_wrapper = runtime_root / "run.do"
        sidecars["wrapper"].write_text(
            SCC.platform_receipts.native_wrapper_text(
                capture_program=runtime_program,
                artifact=runtime_artifact,
                platform="linux-x86_64",
                artifact_sha256=SCC.digest_file(artifact),
                artifact_size_bytes=artifact.stat().st_size,
                capture_program_sha256=SCC.digest_file(capture_program),
                source_commit=source["commit"],
                embedded_git_commit=source["embedded_git_commit"],
                raw_capture=runtime_capture,
                status=runtime_status,
                isolated_root=runtime_isolated,
            ),
            encoding="utf-8",
        )
        invocation = {
            "schema_version": 1,
            "kind": "feppml-native-capture-invocation-v1",
            "platform": "linux-x86_64",
            "source_commit": source["commit"],
            "controller_commit": controller["commit"],
            "artifact_sha256": SCC.digest_file(artifact),
            "capture_program_sha256": SCC.digest_file(capture_program),
            "producer_sha256": SCC.digest_file(producer),
            "stata_executable_sha256": stata_sha,
            "stata_executable_size_bytes": len(stata_bytes),
            "process_returncode": 0,
            "wrapper_sha256": SCC.digest_file(sidecars["wrapper"]),
            "cwd": str(runtime_root),
            "stata_executable": str(runtime_stata),
            "capture_program": str(runtime_program),
            "artifact": str(runtime_artifact),
            "raw_capture": str(runtime_capture),
            "status": str(runtime_status),
            "isolated_root": str(runtime_isolated),
            "wrapper": str(runtime_wrapper),
            "argv": [
                str(runtime_stata), "-q", "-b", "do", str(runtime_wrapper)
            ],
            "complete_sentinel": "FEPPML_NATIVE_CAPTURE_INVOCATION_COMPLETE",
        }
        sidecars["invocation"].write_text(
            json.dumps(invocation, sort_keys=True, indent=2) + "\n",
            encoding="ascii",
        )
        attestation_payload = {
            "schema_version": 1,
            "kind": SCC.platform_receipts.NATIVE_ATTESTATION_KIND,
            "platform": "linux-x86_64",
            **SCC._platform_source_identity(source),
            **SCC._platform_controller_identity(controller),
            "artifact_sha256": SCC.digest_file(artifact),
            "artifact_size_bytes": artifact.stat().st_size,
            "capture_sha256": SCC.digest_file(capture_path),
            "capture_program_sha256": SCC.digest_file(capture_program),
            "producer_sha256": SCC.digest_file(producer),
            "producer_size_bytes": producer.stat().st_size,
            "stata_executable_sha256": stata_sha,
            "stata_executable_size_bytes": len(stata_bytes),
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "process_returncode": 0,
            "stata_rc": 0,
            "status_sha256": SCC.digest_file(sidecars["status"]),
            "status_size_bytes": sidecars["status"].stat().st_size,
            "raw_log_sha256": SCC.digest_file(sidecars["stata_log"]),
            "raw_log_size_bytes": sidecars["stata_log"].stat().st_size,
            "wrapper_sha256": SCC.digest_file(sidecars["wrapper"]),
            "wrapper_size_bytes": sidecars["wrapper"].stat().st_size,
            "invocation_sha256": SCC.digest_file(sidecars["invocation"]),
            "invocation_size_bytes": sidecars["invocation"].stat().st_size,
            "terminal_sentinel_count": 1,
            "embedded_commit_line_count": 1,
            "licensed_stata_load": True,
            "mock_only": False,
            "scientific_claim": False,
            "complete_sentinel": "FEPPML_NATIVE_CAPTURE_EXECUTION_COMPLETE",
        }
        SCC.write_sealed(attestation, attestation_payload)
        native = SCC._native_capture_record(
            root=run_root,
            source=source,
            controller=controller,
            artifact=artifact,
            capture_path=capture_path,
            capture=capture,
            capture_program=capture_program,
            attestation=attestation,
        )
        job = {
            "schema_version": 1,
            "kind": "feppml_linux_platform_bootstrap_job",
            "run_id": run_root.name,
            "job_id": "7387001",
            "job_name": SCC._job_name(run_root.name),
            "platform": "linux-x86_64",
            "source": dict(source),
            "harness_source": dict(controller),
            "build_date_utc": source["build_date_utc"],
            "rustc": SCC.EXPECTED_RUSTC,
            "cargo": SCC.EXPECTED_CARGO,
            "artifact": SCC._file_record(artifact, run_root),
            "build_log": SCC._file_record(build_log, run_root),
            "capture": SCC._file_record(capture_path, run_root),
            "native_capture": native,
            "capture_program_sha256": capture["capture_program_sha256"],
            "stata_version": "19.0",
            "stata_flavor": "MP",
            "licensed_stata_load": True,
            "mock_only": False,
            "scientific_claim": False,
            "build_binary_audit_actual_loader": "success",
            "complete_sentinel": "FEPPML_LINUX_PLATFORM_JOB_COMPLETE",
        }
        job_receipt = run_root / "receipts/job.json"
        SCC.write_sealed(job_receipt, job)
        SCC._write(
            run_root / "JOB.SUCCESS",
            (
                "FEPPML_LINUX_PLATFORM_JOB_COMPLETE|"
                f"{SCC.digest_file(job_receipt)}\n"
            ).encode("ascii"),
        )
        return job

    def test_prepare_preserves_distinct_exact_target_and_harness_git_objects(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo, harness_repo, commit, run_root, harness_commit, _project, _ = (
                self.prepared(root)
            )
            preparation = SCC.verify_input(run_root)
            self.assertNotEqual(commit, harness_commit)
            self.assertEqual(preparation["source"]["commit"], commit)
            self.assertEqual(preparation["harness_source"]["commit"], harness_commit)
            self.assertTrue((run_root / "input/source.bundle").is_file())
            self.assertTrue((run_root / "input/controller.bundle").is_file())
            for name, expected in (("source", commit), ("controller", harness_commit)):
                checkout = root / f"checkout-{name}"
                subprocess.run(
                    [
                        "git", "clone", "-q", "--no-checkout",
                        str(run_root / f"input/{name}.bundle"), str(checkout),
                    ],
                    check=True,
                )
                resolved = subprocess.check_output(
                    ["git", "-C", str(checkout), "rev-parse", f"{expected}^{{commit}}"],
                    text=True,
                ).strip()
                self.assertEqual(resolved, expected)
            self.assertFalse(
                (repo / "benchmarks/scc/feppml_linux_platform_scc.py").exists()
            )
            self.assertTrue(
                (harness_repo / "benchmarks/scc/feppml_linux_platform_scc.py").is_file()
            )

    def test_prepared_inputs_are_immutable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            *_, run_root, _harness_commit, _project, _ = self.prepared(Path(temporary))
            source_bundle = run_root / "input/source.bundle"
            source_bundle.chmod(0o644)
            source_bundle.write_bytes(source_bundle.read_bytes() + b"mutation")
            with self.assertRaisesRegex(SCC.BootstrapError, "changed"):
                SCC.verify_input(run_root)

    def test_formal_build_date_is_commit_derived_not_wall_clock(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo, _harness_repo, commit, run_root, _harness_commit, _project, _ = (
                self.prepared(Path(temporary))
            )
            source = SCC.verify_input(run_root)["source"]
            self.assertEqual(
                source["build_date_utc"], SCC._commit_build_date_utc(repo, commit)
            )
            with mock.patch.dict(
                SCC.os.environ,
                {"TZ": "Pacific/Kiritimati", "FEREG_BUILD_DATE": "2099-12-31"},
                clear=False,
            ):
                east = SCC._formal_build_environment(source, slots="4")
            with mock.patch.dict(
                SCC.os.environ,
                {"TZ": "Pacific/Pago_Pago", "FEREG_BUILD_DATE": "1970-01-01"},
                clear=False,
            ):
                west = SCC._formal_build_environment(source, slots="4")
            self.assertEqual(east["FEREG_BUILD_DATE"], source["build_date_utc"])
            self.assertEqual(west["FEREG_BUILD_DATE"], source["build_date_utc"])
            self.assertEqual(east["FEREG_GIT_COMMIT"], commit[:12])
            self.assertEqual(west["FEREG_GIT_COMMIT"], commit[:12])

    def test_qacct_is_exact_scheduler_identity_not_just_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            *_, run_root, _harness_commit, _project, _ = self.prepared(Path(temporary))
            submission = dict(SCC.verify_submission(run_root))
            path = run_root / "receipts/qacct.txt"
            path.write_text(self.qacct_text(submission), encoding="ascii")
            record = SCC.parse_qacct(path, submission, run_root)
            self.assertEqual(record["owner"], "johannes")
            self.assertEqual(record["job_name"], SCC._job_name(run_root.name))
            self.assertEqual(record["granted_pe"], "omp")
            for field, value, message in (
                ("owner", "someone_else", "exact successful scalar job"),
                ("project", "unrelated", "exact successful scalar job"),
                ("jobnumber", "7387999", "exact successful scalar job"),
                ("jobname", "unrelated_success", "exact successful scalar job"),
                ("slots", "8", "slot count differs"),
                ("cwd", "/projectnb/welfgr/unrelated", "cwd differs"),
                ("scriptfile", "/tmp/unrelated.sh", "scriptfile differs"),
                ("submit_cmd_line", "qsub unrelated.sh", "command differs"),
                ("end_time", "Sun Aug 31 17:59:00 2026", "not ordered"),
            ):
                path.write_text(
                    self.qacct_text(submission, overrides={field: value}),
                    encoding="ascii",
                )
                with self.assertRaisesRegex(SCC.BootstrapError, message):
                    SCC.parse_qacct(path, submission, run_root)
            path.write_text(
                self.qacct_text(submission) + self.qacct_text(submission),
                encoding="ascii",
            )
            with self.assertRaisesRegex(SCC.BootstrapError, "exactly one"):
                SCC.parse_qacct(path, submission, run_root)

    def test_actual_submit_command_is_generated_from_the_sealed_ledger(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            *_, run_root, _harness_commit, _project, _ = self.prepared(Path(temporary))
            expected = SCC._submission_ledger(run_root)["argv"]
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(
                    SCC.main(
                        ["print-submission-command", "--run-root", str(run_root)]
                    ),
                    0,
                )
            self.assertEqual(shlex.split(output.getvalue().strip()), expected)
            submit = (
                ROOT / "benchmarks/scc/feppml_linux_platform_submit.sh"
            ).read_text(encoding="utf-8")
            self.assertIn("print-submission-command", submit)
            self.assertIn("$QSUB_COMMAND", submit)
            self.assertNotIn("qsub -terse", submit)

    def test_close_and_authoritative_verifier_cross_bind_native_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo, harness_repo, commit, run_root, harness_commit, _project, _ = (
                self.prepared(root)
            )
            job = self.complete_fake_job(run_root)
            submission = dict(SCC.verify_submission(run_root))
            qacct = run_root / "receipts/qacct.txt"
            qacct.write_text(self.qacct_text(submission), encoding="ascii")
            SCC.close(
                argparse.Namespace(
                    run_root=run_root,
                    candidate_repo=repo,
                    harness_repo=harness_repo,
                )
            )
            verified = SCC.verify_closed_run(
                run_root,
                candidate_repo=repo,
                harness_repo=harness_repo,
                expected_source_commit=commit,
                expected_harness_commit=harness_commit,
            )
            self.assertEqual(verified["native_capture"], job["native_capture"])
            self.assertEqual(
                verified["build_date_utc"], verified["source"]["build_date_utc"]
            )
            self.assertEqual(
                verified["submission_ledger"]["job_name"],
                SCC._job_name(run_root.name),
            )
            self.assertNotEqual(
                verified["source"]["commit"], verified["harness_source"]["commit"]
            )
            portable = root / "portable-copy" / run_root.name
            shutil.copytree(run_root, portable)
            portable_verified = SCC.verify_closed_run(portable)
            self.assertEqual(
                portable_verified["close_sha256"], verified["close_sha256"]
            )
            stata_log = run_root / job["native_capture"]["stata_log"]["path"]
            stata_log.chmod(0o644)
            stata_log.write_text("hand-authored pass\n", encoding="ascii")
            with self.assertRaises(
                (SCC.BootstrapError, SCC.platform_receipts.PlatformReceiptError)
            ):
                SCC.verify_closed_run(run_root)

    def test_shell_lifecycle_is_guarded_and_collects_all_native_sidecars(self) -> None:
        scripts = [
            ROOT / f"benchmarks/scc/feppml_linux_platform_{name}.sh"
            for name in ("deploy", "job", "submit", "collect")
        ]
        subprocess.run(["bash", "-n", *map(str, scripts)], check=True)
        combined = "\n".join(path.read_text(encoding="utf-8") for path in scripts)
        for token in (
            "-P welfgr", "qacct -j", "seq 1 30", "verify-job",
            "fereg-rust-1.85.1", "stata-mp/19", "--progress",
            "native-attestation.json.status", "native-attestation.json.stata.log",
            "native-attestation.json.wrapper.do",
            "native-attestation.json.invocation.json", "--harness-repo",
        ):
            self.assertIn(token, combined)
        self.assertNotIn("--delete", combined)
        self.assertNotIn("--info=progress2", combined)


if __name__ == "__main__":
    unittest.main()
