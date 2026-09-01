from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
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
