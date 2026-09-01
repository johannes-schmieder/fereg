from __future__ import annotations

from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
PROGRAM = ROOT / "scripts/capture_feppml_platform_load.py"
SPEC = importlib.util.spec_from_file_location(
    "capture_feppml_platform_load_test", PROGRAM
)
assert SPEC is not None and SPEC.loader is not None
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class CaptureFeppmlPlatformLoadTests(unittest.TestCase):
    def test_direct_isolated_cli_bootstraps_reviewed_imports(self) -> None:
        completed = subprocess.run(
            [sys.executable, "-I", "-B", "-S", str(PROGRAM), "--help"],
            cwd="/",
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("Capture one exact plugin load", completed.stdout)

    @staticmethod
    def cli_arguments(*, stata: str = "/tmp/stata-mp") -> list[str]:
        return [
            "--platform",
            "linux-x86_64",
            "--artifact",
            "/tmp/fereg_linux.plugin",
            "--stata",
            stata,
            "--output",
            "/tmp/capture.json",
            "--source-commit",
            "a" * 40,
        ]

    def test_native_attested_scc_cli_succeeds_with_one_sentinel(self) -> None:
        arguments = [
            *self.cli_arguments(),
            "--source-archive",
            "/tmp/source.tar",
            "--attestation",
            "/tmp/native-attestation.json",
        ]
        stdout = io.StringIO()
        with (
            mock.patch.object(CAPTURE, "capture", return_value={}) as capture,
            mock.patch.object(CAPTURE.platform_receipt, "create") as create,
            redirect_stdout(stdout),
        ):
            self.assertEqual(CAPTURE.main(arguments), 0)

        capture.assert_called_once()
        self.assertEqual(
            capture.call_args.kwargs["source_archive"], Path("/tmp/source.tar")
        )
        self.assertEqual(
            capture.call_args.kwargs["attestation"],
            Path("/tmp/native-attestation.json"),
        )
        create.assert_not_called()
        self.assertEqual(
            stdout.getvalue(), "FEPPML_LICENSED_STATA_LOAD_CAPTURE_COMPLETE\n"
        )

    def test_partial_evidence_argument_combinations_fail_before_capture(self) -> None:
        self.assertEqual(
            CAPTURE._cli_evidence_mode(
                source_archive=None,
                attestation=None,
                receipt=None,
                record=None,
            ),
            "capture-only",
        )
        options = (
            ("--source-archive", "/tmp/source.tar"),
            ("--attestation", "/tmp/native-attestation.json"),
            ("--receipt", "/tmp/receipt.json"),
            ("--record", "/tmp/record.json"),
        )
        valid_masks = {0, 0b0011, 0b1111}
        for mask in range(1 << len(options)):
            if mask in valid_masks:
                continue
            arguments = self.cli_arguments()
            for index, (option, value) in enumerate(options):
                if mask & (1 << index):
                    arguments.extend((option, value))
            with (
                self.subTest(mask=mask),
                mock.patch.object(CAPTURE, "capture") as capture,
                self.assertRaisesRegex(SystemExit, "must be exactly none"),
            ):
                CAPTURE.main(arguments)
            capture.assert_not_called()

    def test_full_portable_sealing_arguments_still_create_receipt_and_record(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_archive = root / "source.tar"
            source_archive.write_bytes(b"source archive")
            stata = root / "stata-mp"
            stata.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
            stata.chmod(0o755)
            attestation = root / "native-attestation.json"
            receipt = root / "receipt.json"
            record = root / "record.json"
            arguments = [
                *self.cli_arguments(stata=str(stata)),
                "--source-archive",
                str(source_archive),
                "--attestation",
                str(attestation),
                "--receipt",
                str(receipt),
                "--record",
                str(record),
            ]
            stdout = io.StringIO()
            with (
                mock.patch.object(CAPTURE, "capture", return_value={}) as capture,
                mock.patch.object(CAPTURE.platform_receipt, "create") as create,
                redirect_stdout(stdout),
            ):
                self.assertEqual(CAPTURE.main(arguments), 0)

            capture.assert_called_once()
            create.assert_called_once()
            self.assertEqual(create.call_args.kwargs["receipt"], receipt)
            self.assertEqual(create.call_args.kwargs["record"], record)
            self.assertEqual(
                create.call_args.kwargs["native_attestation"], attestation
            )
            self.assertEqual(
                stdout.getvalue(), "FEPPML_LICENSED_STATA_LOAD_CAPTURE_COMPLETE\n"
            )

    def source(self, root: Path) -> tuple[Path, Path, str]:
        repo = root / "source"
        program = (
            repo
            / "tests/stata/installation/feppml_platform_load_capture.do"
        )
        program.parent.mkdir(parents=True)
        program.write_bytes(CAPTURE.CAPTURE_PROGRAM.read_bytes())
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.email", "test@example.invalid"],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.name", "Capture Test"],
            check=True,
        )
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(
            ["git", "-C", str(repo), "commit", "-qm", "capture"], check=True
        )
        commit = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()
        return repo, program, commit

    def test_native_runner_binds_exact_source_program_artifact_and_stata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo, program, commit = self.source(root)
            artifact = root / "fereg_linux.plugin"
            artifact.write_bytes(b"plugin-binary-" + commit[:12].encode("ascii"))
            stata = root / "stata-mp"
            stata.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
            stata.chmod(0o755)
            output = root / "evidence/capture.json"
            observed_command: list[str] = []

            def fake_runner(command, *, cwd, **kwargs):
                del kwargs
                observed_command.extend(command)
                run_root = Path(cwd)
                (run_root / "capture.status").write_text(
                    "schema_version=1\n"
                    "completed=1\n"
                    "stata_rc=0\n"
                    "stata_version=19.0\n"
                    "stata_flavor=MP\n"
                    "stata_os=Unix\n",
                    encoding="ascii",
                )
                (run_root / "run.log").write_text(
                    f"fereg.git_commit={commit[:12]}\n"
                    "FEPPML_LICENSED_STATA_LOAD_COMPLETE\n",
                    encoding="ascii",
                )
                value = {
                    "schema_version": 1,
                    "evidence_type": "feppml-licensed-stata-load-v1",
                    "platform": "linux-x86_64",
                    "artifact_sha256": digest(artifact),
                    "artifact_size_bytes": artifact.stat().st_size,
                    "stata_version": "19.0",
                    "stata_flavor": "MP",
                    "license_status": "licensed",
                    "plugin_load_status": "success",
                    "plugin_load_rc": 0,
                    "loaded_plugin_sha256": digest(artifact),
                    "capture_program_sha256": digest(program),
                    "source_commit": commit,
                    "embedded_git_commit": commit[:12],
                    "mock_only": False,
                    "scientific_claim": False,
                    "complete_sentinel": "FEPPML_LICENSED_STATA_LOAD_COMPLETE",
                }
                (run_root / "capture.json").write_text(
                    json.dumps(value), encoding="ascii"
                )
                return subprocess.CompletedProcess(command, 0)

            with mock.patch.object(CAPTURE, "CAPTURE_PROGRAM", program):
                value = CAPTURE.capture(
                    platform="linux-x86_64",
                    artifact=artifact,
                    stata=str(stata),
                    output=output,
                    source_repo=repo,
                    source_commit=commit,
                    runner=fake_runner,
                    system="Linux",
                )
            self.assertEqual(value["source_commit"], commit)
            self.assertEqual(json.loads(output.read_text())["artifact_sha256"], digest(artifact))
            self.assertEqual(observed_command[0], str(stata.resolve()))
            self.assertEqual(observed_command[1:4], ["-q", "-b", "do"])
            self.assertFalse(output.stat().st_mode & 0o222)

    def test_wrong_host_missing_commit_or_existing_output_fails_before_launch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo, program, commit = self.source(root)
            artifact = root / "fereg_linux.plugin"
            artifact.write_bytes(b"no embedded identity")
            stata = root / "stata-mp"
            stata.write_text("#!/bin/sh\n", encoding="ascii")
            stata.chmod(0o755)
            with mock.patch.object(CAPTURE, "CAPTURE_PROGRAM", program):
                with self.assertRaisesRegex(CAPTURE.CaptureError, "native Linux"):
                    CAPTURE.capture(
                        platform="linux-x86_64",
                        artifact=artifact,
                        stata=str(stata),
                        output=root / "capture.json",
                        source_repo=repo,
                        source_commit=commit,
                        system="Darwin",
                    )
                with self.assertRaisesRegex(CAPTURE.CaptureError, "embedded source"):
                    CAPTURE.capture(
                        platform="linux-x86_64",
                        artifact=artifact,
                        stata=str(stata),
                        output=root / "capture.json",
                        source_repo=repo,
                        source_commit=commit,
                        system="Linux",
                    )

    def test_source_uses_direct_process_and_retains_verifiable_run_sidecars(self) -> None:
        source = PROGRAM.read_text(encoding="utf-8")
        self.assertIn("subprocess.run", source)
        self.assertNotIn("shell=True", source)
        self.assertIn('sidecars["stata_log"]', source)
        self.assertIn("must not be committed", source)
        self.assertIn("native_wrapper_text", source)


if __name__ == "__main__":
    unittest.main()
