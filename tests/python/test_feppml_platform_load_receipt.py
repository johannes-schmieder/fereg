from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

from scripts import capture_feppml_platform_load as capture_runner
from scripts import feppml_platform_load_receipt as platform


def git(repo: Path, *arguments: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(repo), *arguments], text=True
    ).strip()


def make_repo(repo: Path, files: dict[str, bytes]) -> tuple[str, str]:
    for relative, value in files.items():
        path = repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.email", "test@example.invalid"],
        check=True,
    )
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.name", "Platform Test"],
        check=True,
    )
    subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True)
    subprocess.run(["git", "-C", str(repo), "commit", "-qm", "fixture"], check=True)
    return git(repo, "rev-parse", "HEAD"), git(repo, "rev-parse", "HEAD^{tree}")


def canonical_archive(repo: Path, commit: str, output: Path) -> Path:
    output.write_bytes(
        subprocess.check_output(
            ["git", "-C", str(repo), "archive", "--format=tar", commit]
        )
    )
    return output


class FeppmlPlatformLoadReceiptTests(unittest.TestCase):
    def isolated_cli(
        self, *arguments: str, cwd: Path, python: str | None = None
    ) -> subprocess.CompletedProcess[str]:
        hostile = cwd / "hostile"
        hostile.mkdir(exist_ok=True)
        (hostile / "feppml_qualification.py").write_text(
            "raise RuntimeError('hostile qualification import executed')\n",
            encoding="ascii",
        )
        (hostile / "prepare_windows_feppml_target.py").write_text(
            "raise RuntimeError('hostile Windows target import executed')\n",
            encoding="ascii",
        )
        environment = dict(os.environ)
        environment["PYTHONPATH"] = str(hostile)
        return subprocess.run(
            [
                python or sys.executable,
                "-I",
                "-B",
                "-S",
                str(Path(platform.__file__).resolve()),
                *arguments,
            ],
            cwd=cwd,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def native_fixture(
        self, root: Path, *, platform_name: str, same_controller: bool = False
    ) -> dict[str, object]:
        target_repo = root / "target"
        controller_files = {
                "scripts/capture_feppml_platform_load.py": Path(
                    capture_runner.__file__
                ).read_bytes(),
                "tests/stata/installation/feppml_platform_load_capture.do": (
                    capture_runner.CAPTURE_PROGRAM.read_bytes()
                ),
        }
        target_files = {"target.txt": b"old target\n"}
        if same_controller:
            target_files.update(controller_files)
        target_commit, target_tree = make_repo(target_repo, target_files)
        if same_controller:
            controller_repo = target_repo
            controller_commit, controller_tree = target_commit, target_tree
        else:
            controller_repo = root / "controller"
            controller_commit, controller_tree = make_repo(
                controller_repo, controller_files
            )
            self.assertNotEqual(target_commit, controller_commit)
        archive = canonical_archive(target_repo, target_commit, root / "target.tar")
        build_date = platform._source_identity(
            target_repo, target_commit, archive
        )[0]["build_date_utc"]
        artifact_name = {
            "linux-x86_64": "fereg_linux.plugin",
            "macos-universal": "fereg_macos.plugin",
        }[platform_name]
        artifact = root / artifact_name
        artifact.write_bytes(
            b"plugin\0" + target_commit[:12].encode("ascii")
            + b"\0" + str(build_date).encode("ascii")
        )
        program = (
            controller_repo
            / "tests/stata/installation/feppml_platform_load_capture.do"
        )
        stata = root / "stata-mp"
        stata.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
        stata.chmod(0o755)
        capture = root / "capture.json"
        attestation = root / "native-attestation.json"

        def fake_stata(command, *, cwd, **_kwargs):
            run_root = Path(cwd)
            (run_root / "capture.status").write_text(
                "schema_version=1\ncompleted=1\nstata_rc=0\n"
                "stata_version=19.0\nstata_flavor=MP\nstata_os=Unix\n",
                encoding="ascii",
            )
            (run_root / "run.log").write_text(
                "bounded licensed process\n"
                f"fereg.git_commit={target_commit[:12]}\n"
                "FEPPML_LICENSED_STATA_LOAD_COMPLETE\n",
                encoding="ascii",
            )
            capture_value = {
                "schema_version": 1,
                "evidence_type": platform.CAPTURE_TYPE,
                "platform": platform_name,
                "artifact_sha256": platform.qualification.sha256_file(artifact),
                "artifact_size_bytes": artifact.stat().st_size,
                "stata_version": "19.0",
                "stata_flavor": "MP",
                "license_status": "licensed",
                "plugin_load_status": "success",
                "plugin_load_rc": 0,
                "loaded_plugin_sha256": platform.qualification.sha256_file(artifact),
                "capture_program_sha256": platform.qualification.sha256_file(program),
                "mock_only": False,
                "scientific_claim": False,
                "source_commit": target_commit,
                "embedded_git_commit": target_commit[:12],
                "complete_sentinel": "FEPPML_LICENSED_STATA_LOAD_COMPLETE",
            }
            (run_root / "capture.json").write_text(
                json.dumps(capture_value), encoding="ascii"
            )
            return subprocess.CompletedProcess(command, 0)

        with mock.patch.object(capture_runner, "CAPTURE_PROGRAM", program):
            capture_runner.capture(
                platform=platform_name,
                artifact=artifact,
                stata=str(stata),
                output=capture,
                source_repo=target_repo,
                source_commit=target_commit,
                source_tree=target_tree,
                controller_repo=controller_repo,
                controller_commit=controller_commit,
                controller_tree=controller_tree,
                source_archive=archive,
                attestation=attestation,
                runner=fake_stata,
                system="Linux" if platform_name == "linux-x86_64" else "Darwin",
            )
        return {
            "target_repo": target_repo,
            "target_commit": target_commit,
            "target_tree": target_tree,
            "controller_repo": controller_repo,
            "controller_commit": controller_commit,
            "controller_tree": controller_tree,
            "build_date_utc": build_date,
            "archive": archive,
            "artifact": artifact,
            "program": program,
            "producer": controller_repo / platform.NATIVE_PRODUCER_RELATIVE,
            "stata": stata,
            "capture": capture,
            "attestation": attestation,
        }

    def create_native(
        self, root: Path, *, platform_name: str, same_controller: bool = False
    ) -> dict[str, object]:
        fixture = self.native_fixture(
            root, platform_name=platform_name, same_controller=same_controller
        )
        receipt = root / "receipt.json"
        record = root / "record.json"
        platform.create(
            platform=platform_name,
            artifact=fixture["artifact"],
            capture_path=fixture["capture"],
            receipt=receipt,
            record=record,
            source_repo=fixture["target_repo"],
            source_commit=fixture["target_commit"],
            source_archive=fixture["archive"],
            controller_repo=fixture["controller_repo"],
            controller_commit=fixture["controller_commit"],
            capture_program=fixture["program"],
            native_attestation=fixture["attestation"],
            native_stata_executable=(
                fixture["stata"] if platform_name == "macos-universal" else None
            ),
        )
        fixture.update({"receipt": receipt, "record": record})
        return fixture

    def test_native_create_verify_and_loaded_record_are_source_controller_exact(self) -> None:
        for platform_name in ("macos-universal", "linux-x86_64"):
            with self.subTest(platform=platform_name), tempfile.TemporaryDirectory() as temporary:
                fixture = self.create_native(Path(temporary), platform_name=platform_name)
                payload = platform.verify(fixture["receipt"], fixture["record"])
                loaded = json.loads(Path(fixture["record"]).read_text(encoding="ascii"))
                self.assertEqual(platform.verify_evidence(loaded), payload)
                self.assertEqual(payload["source_commit"], fixture["target_commit"])
                self.assertEqual(payload["controller_commit"], fixture["controller_commit"])
                self.assertNotEqual(payload["source_commit"], payload["controller_commit"])
                self.assertEqual(payload["qualification_profile"], "f0-baseline")
                self.assertEqual(loaded["record_type"], platform.RECORD_TYPE)
                self.assertEqual(
                    payload["platform_provenance"]["record_type"],
                    "feppml-native-capture-controller-v1",
                )

    def test_native_candidate_profile_requires_exact_target_controller_lineage(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = self.create_native(
                Path(temporary),
                platform_name="macos-universal",
                same_controller=True,
            )
            payload = platform.verify(fixture["receipt"], fixture["record"])
            self.assertEqual(payload["qualification_profile"], "candidate")
            self.assertEqual(payload["source_commit"], payload["controller_commit"])
            self.assertEqual(payload["source_tree"], payload["controller_tree"])

    def test_native_portable_verification_reopens_every_copied_byte(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = self.create_native(root, platform_name="linux-x86_64")
            copied = root / "copied"
            copied.mkdir()
            paths: dict[str, Path] = {}
            for name in ("artifact", "capture", "archive", "program", "producer", "receipt", "record"):
                source = Path(fixture[name])
                destination = copied / source.name
                shutil.copyfile(source, destination)
                paths[name] = destination
            original_attestation = Path(fixture["attestation"])
            copied_attestation = copied / original_attestation.name
            shutil.copyfile(original_attestation, copied_attestation)
            for field, original in platform.native_sidecar_paths(original_attestation).items():
                shutil.copyfile(
                    original,
                    platform.native_sidecar_paths(copied_attestation)[field],
                )
            payload = platform.verify_portable(
                receipt=paths["receipt"],
                record=paths["record"],
                artifact=paths["artifact"],
                capture_path=paths["capture"],
                source_archive=paths["archive"],
                capture_program=paths["program"],
                native_attestation=copied_attestation,
                native_producer=paths["producer"],
            )
            self.assertEqual(payload["source_commit"], fixture["target_commit"])
            paths["archive"].write_bytes(paths["archive"].read_bytes() + b"tamper")
            with self.assertRaisesRegex(platform.PlatformReceiptError, "portable evidence"):
                platform.verify_portable(
                    receipt=paths["receipt"], record=paths["record"],
                    artifact=paths["artifact"], capture_path=paths["capture"],
                    source_archive=paths["archive"], capture_program=paths["program"],
                    native_attestation=copied_attestation,
                    native_producer=paths["producer"],
                )

    def test_native_exact_wrapper_rejects_early_exit_even_when_hashes_are_resealed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = self.native_fixture(root, platform_name="linux-x86_64")
            attestation = Path(fixture["attestation"])
            sidecars = platform.native_sidecar_paths(attestation)
            wrapper = sidecars["wrapper"]
            wrapper.chmod(0o644)
            wrapper.write_text("exit 0\n" + wrapper.read_text(encoding="utf-8"), encoding="utf-8")
            invocation_path = sidecars["invocation"]
            invocation_path.chmod(0o644)
            invocation = json.loads(invocation_path.read_text(encoding="ascii"))
            invocation["wrapper_sha256"] = platform.qualification.sha256_file(wrapper)
            invocation_path.write_text(json.dumps(invocation, sort_keys=True, indent=2) + "\n")
            attestation.chmod(0o644)
            document = json.loads(attestation.read_text(encoding="ascii"))
            document["payload"]["wrapper_sha256"] = platform.qualification.sha256_file(wrapper)
            document["payload"]["wrapper_size_bytes"] = wrapper.stat().st_size
            document["payload"]["invocation_sha256"] = platform.qualification.sha256_file(invocation_path)
            document["payload"]["invocation_size_bytes"] = invocation_path.stat().st_size
            attestation.write_text(
                json.dumps(platform.qualification.seal_payload(document["payload"])),
                encoding="ascii",
            )
            source = {
                "source_commit": fixture["target_commit"],
                "source_tree": fixture["target_tree"],
                "source_archive_sha256": platform.qualification.sha256_file(fixture["archive"]),
                "source_archive_size_bytes": Path(fixture["archive"]).stat().st_size,
                "embedded_git_commit": str(fixture["target_commit"])[:12],
                "build_date_utc": fixture["build_date_utc"],
            }
            controller = {
                "controller_commit": fixture["controller_commit"],
                "controller_tree": fixture["controller_tree"],
            }
            with self.assertRaisesRegex(platform.PlatformReceiptError, "invocation|wrapper"):
                platform._native_provenance(
                    attestation=attestation,
                    stata_executable=None,
                    status_file=sidecars["status"], stata_log=sidecars["stata_log"],
                    wrapper=wrapper, invocation=invocation_path,
                    platform="linux-x86_64", source=source, controller=controller,
                    artifact=fixture["artifact"], capture_path=fixture["capture"],
                    capture=json.loads(Path(fixture["capture"]).read_text()),
                    capture_program=fixture["program"], producer=fixture["producer"],
                )

    def test_target_archive_must_be_the_exact_git_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = self.native_fixture(root, platform_name="linux-x86_64")
            archive = Path(fixture["archive"])
            archive.write_bytes(archive.read_bytes() + b"other")
            with self.assertRaisesRegex(platform.PlatformReceiptError, "git archive"):
                platform.create(
                    platform="linux-x86_64", artifact=fixture["artifact"],
                    capture_path=fixture["capture"], receipt=root / "receipt.json",
                    record=root / "record.json", source_repo=fixture["target_repo"],
                    source_commit=fixture["target_commit"], source_archive=archive,
                    controller_repo=fixture["controller_repo"],
                    controller_commit=fixture["controller_commit"],
                    capture_program=fixture["program"],
                    native_attestation=fixture["attestation"],
                )

    def test_build_date_is_commit_derived_and_host_clock_independent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = self.native_fixture(root, platform_name="linux-x86_64")
            observed = []
            original_tz = os.environ.get("TZ")
            try:
                for zone in ("UTC", "Pacific/Honolulu", "Pacific/Kiritimati"):
                    os.environ["TZ"] = zone
                    time.tzset()
                    observed.append(
                        platform._source_identity(
                            fixture["target_repo"],
                            fixture["target_commit"],
                            fixture["archive"],
                        )[0]["build_date_utc"]
                    )
            finally:
                if original_tz is None:
                    os.environ.pop("TZ", None)
                else:
                    os.environ["TZ"] = original_tz
                time.tzset()
            self.assertEqual(len(set(observed)), 1)
            self.assertEqual(observed[0], fixture["build_date_utc"])

    def test_isolated_cli_help_uses_only_reviewed_import_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            completed = self.isolated_cli("--help", cwd=Path(temporary))
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("verify-portable", completed.stdout)
            self.assertNotIn("hostile", completed.stderr)

    def test_isolated_cli_verifies_a_valid_native_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = self.create_native(
                root / "fixture", platform_name="linux-x86_64"
            )
            completed = self.isolated_cli(
                "verify",
                "--receipt",
                str(Path(fixture["receipt"]).resolve()),
                "--record",
                str(Path(fixture["record"]).resolve()),
                cwd=root,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(
                completed.stdout.strip(), "FEPPML_PLATFORM_VERIFY_COMPLETE"
            )
            self.assertNotIn("hostile", completed.stderr)


if __name__ == "__main__":
    unittest.main()
