from __future__ import annotations

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
import zipfile

from scripts import feppml_qualification as qualification
from scripts import bundle_windows_feppml_results as results_bundle
from scripts import feppml_platform_load_receipt as platform_receipt
from scripts import prepare_windows_feppml_controller as controller
from scripts import prepare_windows_feppml_target as target
from scripts import seal_windows_feppml_platform_manifest as source_manifest


ROOT = Path(__file__).resolve().parents[2]


def _git(repo: Path, *arguments: str, environment: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    if completed.returncode:
        raise AssertionError(completed.stderr)
    return completed.stdout.strip()


def _initialize(repo: Path) -> None:
    repo.mkdir(parents=True)
    _git(repo, "init", "-q")
    _git(repo, "config", "user.name", "Windows evidence test")
    _git(repo, "config", "user.email", "windows-evidence@example.invalid")


def _commit(repo: Path, when: str, message: str) -> str:
    _git(repo, "add", "-A")
    environment = dict(os.environ)
    environment.update({"GIT_AUTHOR_DATE": when, "GIT_COMMITTER_DATE": when})
    _git(repo, "commit", "-qm", message, environment=environment)
    return _git(repo, "rev-parse", "HEAD")


class WindowsCiDriverContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.driver = (ROOT / "windows-ci.do").read_text(encoding="utf-8")
        cls.lower = cls.driver.lower()
        cls.build = (ROOT / "scripts/build_windows.ps1").read_text(
            encoding="utf-8"
        )
        cls.capture_helper = (
            ROOT / "scripts/prepare_windows_feppml_platform_capture.ps1"
        ).read_text(encoding="utf-8")
        cls.sealer = (
            ROOT / "scripts/seal_windows_feppml_platform_manifest.py"
        ).read_text(encoding="utf-8")

    def test_materialized_target_owns_build_artifact_and_public_package(self) -> None:
        materialize = self.lower.index("prepare_windows_feppml_target.py")
        build = self.lower.index("-expectedsourcecommit")
        stage = self.lower.index(
            "file open `package_handle' using `\"`target_root'/stata/fereg.pkg\"'"
        )
        self.assertLess(materialize, build)
        self.assertLess(build, stage)
        for marker in (
            "input/feppml-windows-preparation.json",
            "input/feppml-windows-target-source.tar",
            "target/windows-ci-target-source",
            "-sourceroot `\"`target_root'\"'",
            "-expectedsourcecommit `\"`target_commit'\"'",
            "-expectedbuilddate `\"`target_build_date'\"'",
            "`\"`target_root'/dist/fereg_windows.plugin\"'",
            "`\"`target_root'/stata/fereg.pkg\"'",
        ):
            self.assertIn(marker, self.lower)
        self.assertNotIn("`\"`root'/stata/fereg.pkg\"'", self.lower)
        self.assertIn('if `"`target_mode\'"\' == "candidate"', self.driver)
        self.assertIn('if `"`target_mode\'"\' == "f0-baseline"', self.driver)
        self.assertIn("capture which `private_file'", self.driver)

    def test_exact_capture_and_generated_manifest_precede_sole_pass(self) -> None:
        capture_program = (
            ROOT / "tests/stata/installation/feppml_platform_load_capture.do"
        ).read_text(encoding="utf-8")
        capture_call = self.lower.index("capture noisily do `\"`platform_capture_program'")
        verify_call = self.lower.index("-capture `\"`platform_capture'\"'")
        seal_call = self.lower.index("--run-id `\"`controller_run_id'\"'")
        status = self.driver.index('file write `status_handle\' "WINDOWS_CI=PASS"')
        self.assertLess(capture_call, verify_call)
        self.assertLess(verify_call, seal_call)
        self.assertLess(seal_call, status)
        self.assertEqual(self.driver.count("WINDOWS_CI=PASS"), 1)
        self.assertTrue(self.driver.rstrip().endswith("file close `status_handle'"))
        for marker in (
            "WINDOWS_CI_RUN_ID",
            "feppml-windows-platform-source-manifest.json",
            "-TargetReceipt",
            "source_archive_sha256=",
            "build_date_utc=",
            "harness_commit=",
            "FEPPML_PLATFORM_CAPTURE_SHA256=",
        ):
            self.assertIn(marker.lower(), self.lower)
        self.assertIn("plugin call __feppml_platform_load_probe, version", capture_program)
        self.assertEqual(capture_program.count("plugin call"), 1)
        self.assertIn('"mock_only" (char(34)) ":false', capture_program)
        self.assertIn('"scientific_claim" (char(34)) ":false', capture_program)

    def test_controller_helpers_fail_closed_on_target_and_build_identity(self) -> None:
        for marker in (
            "$TargetReceipt",
            "exact clean controller",
            "target_archive_sha256=",
            "build_date_utc=",
            "$PluginAscii.Contains($EmbeddedCommit)",
            "licensed-load capture fields are not closed",
        ):
            self.assertIn(marker, self.capture_helper)
        for marker in (
            "ExpectedSourceCommit",
            "ExpectedBuildDate",
            "ExpectedMode",
            "materialized Windows builds require an exact full target commit",
            "pre-existing embedded commit differs from the exact target",
            "pre-existing build date differs from the exact target commit",
            "cannot derive an exact Windows source commit",
        ):
            self.assertIn(marker, self.build)
        self.assertNotIn('FEREG_GIT_COMMIT = "unknown"', self.build)
        self.assertIn('"run_id": run_id', self.sealer)
        self.assertIn('"mode": match.group(1)', self.sealer)
        self.assertIn('"build_date_utc": match.group(7)', self.sealer)
        self.assertIn("artifact/fereg_windows.plugin", (
            ROOT / "scripts/bundle_windows_feppml_results.py"
        ).read_text(encoding="utf-8"))
        loader = (
            ROOT / "tests/mock_spi/actual_plugin_loader_test_windows.c"
        ).read_text(encoding="utf-8")
        runner = (
            ROOT / "tests/mock_spi/run_actual_windows_plugin_test.ps1"
        ).read_text(encoding="utf-8")
        self.assertIn("int candidate_mode = 1", loader)
        self.assertIn("if (!candidate_mode) return 0", loader)
        self.assertIn("check_estimator_action_routing(call, candidate_mode)", loader)
        self.assertIn('[ValidateSet("candidate", "f0-baseline")]', runner)
        self.assertIn('[string] $Mode = "candidate"', runner)

    def test_bounded_mode_specific_fixtures_precede_pass(self) -> None:
        fixtures = (
            "tests/stata/installation/windows_ci_smoke.do",
            "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do",
            "tests/stata/parity_ppmlhdfe/feppml_fail_closed.do",
        )
        status = self.driver.index('file write `status_handle\' "WINDOWS_CI=PASS"')
        self.assertTrue(all(self.lower.rindex(path) < status for path in fixtures))
        hidden = self.lower.rindex(fixtures[1])
        candidate_branch = self.lower.rfind('target_mode\'"\' == "candidate"', 0, hidden)
        self.assertGreater(candidate_branch, 0)
        smoke = (ROOT / fixtures[0]).read_text(encoding="utf-8").lower()
        self.assertIn("doctor threads(2)", smoke)
        self.assertIn("selftest threads(2)", smoke)
        self.assertIn("fereg y c.x", smoke)

    def test_driver_contains_no_remote_or_license_discovery(self) -> None:
        for token in (
            "aws ", "ssh ", "curl ", "invoke-webrequest", "license serial",
            ".stcodes", "stata.lic", "query user",
        ):
            self.assertNotIn(token, self.lower)
        self.assertIsNone(re.search(r"https?://", self.driver))
        ignored = (ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()
        self.assertIn("/windows-ci.status", ignored)
        self.assertIn("/feppml-platform-load-capture.json", ignored)
        self.assertIn("/feppml-windows-platform-source-manifest.json", ignored)


class WindowsTwoSourcePreparationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="windows-two-source-test-")
        self.base = Path(self.temporary.name)
        self.target_repo = self.base / "target-repo"
        self.harness_repo = self.base / "harness-repo"
        _initialize(self.target_repo)
        (self.target_repo / "target.txt").write_text("historical target\n", encoding="ascii")
        self.target_commit = _commit(
            self.target_repo, "2020-02-03T04:05:06Z", "historical target"
        )
        _initialize(self.harness_repo)
        for relative in target.CONTRACT_PATHS:
            source = ROOT / relative
            destination = self.harness_repo / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
        self.harness_commit = _commit(
            self.harness_repo, "2026-08-31T12:00:00Z", "trusted harness"
        )
        self.controller_repo = self.base / "controller-repo"
        self.prepared = controller.prepare(
            output_repo=self.controller_repo,
            target_repo=self.target_repo,
            target_commit=self.target_commit,
            harness_repo=self.harness_repo,
            harness_commit=self.harness_commit,
            mode="f0-baseline",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @property
    def preparation(self) -> Path:
        return self.controller_repo / "input/feppml-windows-preparation.json"

    @property
    def archive(self) -> Path:
        return self.controller_repo / "input/feppml-windows-target-source.tar"

    def _materialize(self, suffix: str = "one") -> tuple[Path, Path]:
        output = self.controller_repo / f"target/materialized-{suffix}"
        receipt = self.controller_repo / f"target/target-{suffix}.receipt"
        target.materialize(self.preparation, self.archive, output, receipt)
        return output, receipt

    def test_old_target_and_new_harness_remain_exact_and_distinct(self) -> None:
        envelope = json.loads(self.preparation.read_text(encoding="ascii"))
        payload = envelope["payload"]
        self.assertEqual(envelope["seal"], qualification.seal_payload(payload)["seal"])
        self.assertEqual(payload["target"]["commit"], self.target_commit)
        self.assertEqual(payload["target"]["tree"], _git(self.target_repo, "rev-parse", "HEAD^{tree}"))
        self.assertEqual(payload["target"]["build_date_utc"], "2020-02-03")
        self.assertEqual(payload["harness"]["commit"], self.harness_commit)
        self.assertNotEqual(payload["target"]["commit"], payload["harness"]["commit"])
        self.assertEqual(set(payload["contract_files"]), set(target.CONTRACT_PATHS))
        self.assertEqual(_git(self.controller_repo, "status", "--porcelain"), "")
        output, receipt = self._materialize()
        self.assertEqual((output / "target.txt").read_text(encoding="ascii"), "historical target\n")
        text = receipt.read_text(encoding="ascii")
        self.assertIn(f"target_commit={self.target_commit}\n", text)
        self.assertIn("build_date_utc=2020-02-03\n", text)
        self.assertIn(f"harness_commit={self.harness_commit}\n", text)
        self.assertTrue(text.endswith("FEPPML_WINDOWS_TARGET_MATERIALIZED\n"))

    def test_materializer_rejects_changed_controller_contract_file(self) -> None:
        path = self.controller_repo / "windows-ci.do"
        path.write_text(path.read_text(encoding="utf-8") + "* changed\n", encoding="utf-8")
        with self.assertRaisesRegex(target.TargetError, "controller contract file changed"):
            target.materialize(
                self.preparation,
                self.archive,
                self.controller_repo / "target/materialized-tampered-controller",
                self.controller_repo / "target/tampered-controller.receipt",
            )

    def test_materializer_rejects_changed_target_archive(self) -> None:
        self.archive.chmod(0o644)
        self.archive.write_bytes(self.archive.read_bytes() + b"changed")
        with self.assertRaisesRegex(target.TargetError, "target archive differs"):
            target.materialize(
                self.preparation,
                self.archive,
                self.controller_repo / "target/materialized-tampered-target",
                self.controller_repo / "target/tampered-target.receipt",
            )

    def test_f0_mode_rejects_target_equal_to_harness(self) -> None:
        with self.assertRaisesRegex(controller.PreparationError, "must be distinct"):
            controller.prepare(
                output_repo=self.base / "invalid-controller",
                target_repo=self.harness_repo,
                target_commit=self.harness_commit,
                harness_repo=self.harness_repo,
                harness_commit=self.harness_commit,
                mode="f0-baseline",
            )

    def test_candidate_mode_rejects_target_different_from_harness(self) -> None:
        with self.assertRaisesRegex(controller.PreparationError, "must equal"):
            controller.prepare(
                output_repo=self.base / "invalid-candidate-controller",
                target_repo=self.target_repo,
                target_commit=self.target_commit,
                harness_repo=self.harness_repo,
                harness_commit=self.harness_commit,
                mode="candidate",
            )

    def test_candidate_mode_accepts_exact_target_harness_identity(self) -> None:
        candidate_controller = self.base / "candidate-controller"
        prepared = controller.prepare(
            output_repo=candidate_controller,
            target_repo=self.harness_repo,
            target_commit=self.harness_commit,
            harness_repo=self.harness_repo,
            harness_commit=self.harness_commit,
            mode="candidate",
        )
        output = candidate_controller / "target/materialized"
        receipt = candidate_controller / "target/target.receipt"
        target.materialize(
            candidate_controller / "input/feppml-windows-preparation.json",
            candidate_controller / "input/feppml-windows-target-source.tar",
            output,
            receipt,
        )
        self.assertEqual(prepared["target"]["commit"], self.harness_commit)
        self.assertEqual(prepared["target"]["tree"], prepared["harness"]["tree"])
        self.assertTrue(receipt.read_text(encoding="ascii").startswith("mode=candidate\n"))

    def test_platform_verifier_rejects_modified_base_claiming_reviewed_harness(self) -> None:
        changed = self.controller_repo / "windows-ci.do"
        changed.write_text(
            changed.read_text(encoding="utf-8") + "\n* malicious controller change\n",
            encoding="utf-8",
        )
        preparation = json.loads(self.preparation.read_text(encoding="ascii"))
        payload = preparation["payload"]
        payload["contract_files"]["windows-ci.do"] = {
            "sha256": qualification.sha256_file(changed),
            "size_bytes": changed.stat().st_size,
        }
        self.preparation.chmod(0o644)
        self.preparation.write_text(
            json.dumps(
                qualification.seal_payload(payload),
                sort_keys=True,
                indent=2,
                ensure_ascii=True,
            )
            + "\n",
            encoding="ascii",
        )
        malicious_commit = _commit(
            self.controller_repo,
            "2026-08-31T12:10:00Z",
            "modified synthetic controller",
        )
        malicious_tree = _git(self.controller_repo, "rev-parse", "HEAD^{tree}")
        source_manifest_path = self.controller_repo / "windows-ci-source.json"
        source_manifest_path.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "project": "fereg",
                    "source_revision": f"commit-{malicious_commit}",
                    "dirty": False,
                },
                sort_keys=True,
            ),
            encoding="ascii",
        )
        malicious_archive = self.base / "malicious-controller.zip"
        _git(
            self.controller_repo,
            "archive",
            "--format=zip",
            f"--output={malicious_archive}",
            malicious_commit,
        )
        with zipfile.ZipFile(
            malicious_archive, "a", compression=zipfile.ZIP_STORED
        ) as archive:
            archive.write(source_manifest_path, "windows-ci-source.json")
        with self.assertRaisesRegex(
            platform_receipt.PlatformReceiptError, "reviewed harness tree"
        ):
            platform_receipt._windows_controller_archive(
                malicious_archive, malicious_commit, malicious_tree
            )

    def test_source_manifest_binds_run_target_harness_artifact_and_load(self) -> None:
        _output, target_receipt = self._materialize("manifest")
        controller_manifest = self.controller_repo / "windows-ci-source.json"
        controller_manifest.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "project": "fereg",
                    "source_revision": f"commit-{self.prepared['controller_commit']}",
                    "dirty": False,
                },
                sort_keys=True,
            ),
            encoding="ascii",
        )
        artifact = self.controller_repo / "target/fereg_windows.plugin"
        artifact.write_bytes(
            b"PE fixture "
            + self.target_commit[:12].encode("ascii")
            + b" 2020-02-03"
        )
        capture_program = (
            self.controller_repo
            / "tests/stata/installation/feppml_platform_load_capture.do"
        )
        artifact_sha = qualification.sha256_file(artifact)
        capture = self.controller_repo / "target/capture.json"
        capture.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "evidence_type": "feppml-licensed-stata-load-v1",
                    "platform": "windows-x86_64",
                    "artifact_sha256": artifact_sha,
                    "artifact_size_bytes": artifact.stat().st_size,
                    "stata_version": "19.0",
                    "stata_flavor": "MP",
                    "license_status": "licensed",
                    "plugin_load_status": "success",
                    "plugin_load_rc": 0,
                    "loaded_plugin_sha256": artifact_sha,
                    "capture_program_sha256": qualification.sha256_file(capture_program),
                    "mock_only": False,
                    "scientific_claim": False,
                    "source_commit": self.target_commit,
                    "embedded_git_commit": self.target_commit[:12],
                    "complete_sentinel": "FEPPML_LICENSED_STATA_LOAD_COMPLETE",
                },
                sort_keys=True,
            ),
            encoding="ascii",
        )
        output = self.controller_repo / "target/source-manifest.json"
        run_id = "win-20260831T120000Z-deadbeef"
        payload = source_manifest.seal(
            artifact=artifact,
            capture=capture,
            capture_program=capture_program,
            controller_source_manifest=controller_manifest,
            target_receipt=target_receipt,
            run_id=run_id,
            output=output,
        )
        document = json.loads(output.read_text(encoding="ascii"))
        self.assertEqual(document["seal"], qualification.seal_payload(payload)["seal"])
        self.assertEqual(payload["run_id"], run_id)
        self.assertEqual(payload["mode"], "f0-baseline")
        self.assertEqual(payload["source_commit"], self.target_commit)
        self.assertEqual(payload["build_date_utc"], "2020-02-03")
        self.assertEqual(payload["controller_commit"], self.prepared["controller_commit"])
        self.assertEqual(payload["harness_commit"], self.harness_commit)
        self.assertEqual(payload["artifact_sha256"], artifact_sha)
        self.assertTrue(payload["licensed_stata_load"])
        self.assertFalse(payload["scientific_claim"])

        checksum = self.controller_repo / "target/plugin.sha256"
        checksum.write_text(f"{artifact_sha}  fereg_windows.plugin\n", encoding="ascii")
        built = self.controller_repo / "target/built.verified"
        staged = self.controller_repo / "target/staged.verified"
        for path in (built, staged):
            path.write_text(f"WINDOWS_PLUGIN_SHA256={artifact_sha}", encoding="ascii")
        capture_verification = self.controller_repo / "target/capture.verified"
        capture_verification.write_text(
            f"FEPPML_PLATFORM_CAPTURE_SHA256={qualification.sha256_file(capture)}",
            encoding="ascii",
        )
        capture_inputs = self.controller_repo / "target/capture.inputs"
        capture_inputs.write_text(
            "\n".join(
                (
                    f"controller_commit={payload['controller_commit']}",
                    f"mode={payload['mode']}",
                    f"source_commit={payload['source_commit']}",
                    f"source_tree={payload['source_tree']}",
                    f"source_archive_sha256={payload['source_archive_sha256']}",
                    f"source_archive_size_bytes={payload['source_archive_size_bytes']}",
                    f"embedded_git_commit={payload['embedded_git_commit']}",
                    f"build_date_utc={payload['build_date_utc']}",
                    f"harness_commit={payload['harness_commit']}",
                    f"harness_tree={payload['harness_tree']}",
                    f"artifact_sha256={payload['artifact_sha256']}",
                    f"artifact_size_bytes={payload['artifact_size_bytes']}",
                    f"capture_program_sha256={payload['capture_program_sha256']}",
                    "FEPPML_PLATFORM_CAPTURE_INPUTS_COMPLETE",
                )
            ),
            encoding="ascii",
        )
        bundle_one = self.controller_repo / "target/results-one.zip"
        bundle_two = self.controller_repo / "target/results-two.zip"
        bundle_arguments = {
            "artifact": artifact,
            "checksum": checksum,
            "capture": capture,
            "target_receipt": target_receipt,
            "source_manifest": output,
            "built_verification": built,
            "staged_verification": staged,
            "capture_inputs": capture_inputs,
            "capture_verification": capture_verification,
        }
        first = results_bundle.bundle(output=bundle_one, **bundle_arguments)
        second = results_bundle.bundle(output=bundle_two, **bundle_arguments)
        self.assertEqual(first["sha256"], second["sha256"])
        self.assertEqual(bundle_one.read_bytes(), bundle_two.read_bytes())
        with zipfile.ZipFile(bundle_one) as archive:
            self.assertEqual(
                archive.namelist(),
                [
                    "artifact/fereg_windows.plugin",
                    "evidence/capture.json",
                    "evidence/source-manifest.json",
                ],
            )
            self.assertEqual(
                archive.read("artifact/fereg_windows.plugin"), artifact.read_bytes()
            )

        controller_archive = self.base / "exact-controller-source.zip"
        _git(
            self.controller_repo,
            "archive",
            "--format=zip",
            f"--output={controller_archive}",
            self.prepared["controller_commit"],
        )
        with zipfile.ZipFile(controller_archive, "a", compression=zipfile.ZIP_STORED) as archive:
            archive.write(controller_manifest, "windows-ci-source.json")
        run_receipt = self.base / "controller-receipt.json"
        receipt_payload = {
            "schema_version": 1,
            "run_id": run_id,
            "project": "fereg",
            "profile": "stata-do",
            "source_revision": f"commit-{self.prepared['controller_commit']}",
            "source_sha256": qualification.sha256_file(controller_archive),
            "windows_target": "x86_64",
            "execution_identity": "SYSTEM",
            "status": "PASS",
            "error_code": None,
            "checks": {
                "archive_paths": True,
                "execution_identity": True,
                "project_test": True,
                "shutdown_backstop": True,
                "source_hash": True,
                "source_manifest": True,
                "stata": True,
                "results_hash": True,
            },
            "completed_at_utc": "2026-08-31T12:30:00Z",
            "results_sha256": first["sha256"],
            "results_size_bytes": first["size_bytes"],
        }
        run_receipt.write_text(
            json.dumps(receipt_payload, sort_keys=True), encoding="ascii"
        )
        controller_output = self.base / "controller-output.txt"
        controller_output.write_text(
            "\n".join(
                (
                    f"RUN_ID={run_id}",
                    "PROJECT=fereg",
                    "PROFILE=stata-do",
                    f"SOURCE_REVISION=commit-{self.prepared['controller_commit']}",
                    f"SOURCE_SHA256={receipt_payload['source_sha256']}",
                    f"RECEIPT_SHA256={qualification.sha256_file(run_receipt)}",
                    "WINDOWS_STATUS=PASS",
                    "INSTANCE_STOPPED=true",
                    "TRANSIENT_OBJECTS_DELETED=true",
                    "LOCK_RELEASED=true",
                    f"RECEIPT_PATH=/private/tmp/windows-ci-results/{run_id}/receipt.json",
                    f"RESULTS_SHA256={first['sha256']}",
                    f"RESULTS_SIZE_BYTES={first['size_bytes']}",
                    f"RESULTS_PATH=/private/tmp/windows-ci-results/{run_id}/results.zip",
                    "WINDOWS_CI_RUN=PASS",
                )
            )
            + "\n",
            encoding="ascii",
        )
        platform_load_receipt = self.base / "platform-load.json"
        platform_record = self.base / "platform-record.json"
        created = platform_receipt.create(
            platform="windows-x86_64",
            artifact=artifact,
            capture_path=capture,
            receipt=platform_load_receipt,
            record=platform_record,
            source_repo=self.target_repo,
            source_commit=self.target_commit,
            source_archive=self.archive,
            controller_repo=self.controller_repo,
            controller_commit=self.prepared["controller_commit"],
            capture_program=capture_program,
            windows_controller_receipt=run_receipt,
            windows_controller_output=controller_output,
            windows_controller_archive=controller_archive,
            windows_results_archive=bundle_one,
            windows_source_manifest=output,
        )
        verified = platform_receipt.verify(platform_load_receipt, platform_record)
        verified_loaded = platform_receipt.verify_evidence(
            json.loads(platform_record.read_text(encoding="ascii"))
        )
        self.assertEqual(verified, verified_loaded)
        self.assertEqual(created["source_commit"], verified["source_commit"])
        self.assertEqual(verified["source_commit"], self.target_commit)
        self.assertEqual(
            verified["controller_commit"], self.prepared["controller_commit"]
        )
        self.assertNotEqual(verified["source_commit"], verified["controller_commit"])

        extra_controller_archive = self.base / "controller-source-extra.zip"
        shutil.copy2(controller_archive, extra_controller_archive)
        with zipfile.ZipFile(extra_controller_archive, "a") as archive:
            archive.writestr("unrelated.txt", "not in the controller tree")
        with self.assertRaisesRegex(
            platform_receipt.PlatformReceiptError, "differs from its exact tree"
        ):
            platform_receipt._windows_controller_archive(
                extra_controller_archive,
                self.prepared["controller_commit"],
                self.prepared["controller_tree"],
            )
        with self.assertRaisesRegex(source_manifest.ManifestError, "identity is invalid"):
            source_manifest.seal(
                artifact=artifact,
                capture=capture,
                capture_program=capture_program,
                controller_source_manifest=controller_manifest,
                target_receipt=target_receipt,
                run_id="made-up-run",
                output=self.controller_repo / "target/invalid-manifest.json",
            )


if __name__ == "__main__":
    unittest.main()
