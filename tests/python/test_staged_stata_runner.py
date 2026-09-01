from __future__ import annotations

import hashlib
import tempfile
import unittest
import zipfile
from pathlib import Path

from scripts import run_staged_stata_tests, stage_release


class StagedStataRunnerTests(unittest.TestCase):
    def make_staging(self, root: Path, *, dirty: bool = False) -> Path:
        staging = root / stage_release.PACKAGE_NAME
        staging.mkdir()
        (staging / "fereg.pkg").write_text("v 3\nf fereg.ado\n", encoding="utf-8")
        (staging / "stata.toc").write_text("v 3\n", encoding="utf-8")
        (staging / "fereg.ado").write_text("program define fereg\nend\n", encoding="utf-8")
        for index, name in enumerate(stage_release.EXPECTED_PLUGINS, start=1):
            plugin = staging / name
            plugin.write_bytes(f"native-{index}".encode("ascii"))
            digest = hashlib.sha256(plugin.read_bytes()).hexdigest()
            (staging / f"{name}.sha256").write_text(
                f"{digest}  {name}\n", encoding="ascii"
            )
        stage_release.write_manifest(staging, "a" * 40, dirty)
        return staging

    def archive(self, staging: Path, output: Path) -> str:
        stage_release.write_zip(staging, output)
        return stage_release.digest(output)

    def test_extracts_and_validates_one_flat_clean_release(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            staging = self.make_staging(root)
            archive = root / "release.zip"
            digest = self.archive(staging, archive)
            extracted = run_staged_stata_tests.extract_staged_release(
                archive, digest, root / "extracted"
            )
            self.assertEqual(extracted.source_sha, "a" * 40)
            self.assertEqual(extracted.archive_sha256, digest)
            self.assertEqual(extracted.archive_bytes, archive.stat().st_size)
            self.assertEqual(
                extracted.plugin.name,
                run_staged_stata_tests.expected_plugin_filename(),
            )
            self.assertEqual(
                extracted.plugin_sha256,
                stage_release.digest(extracted.plugin),
            )
            self.assertEqual(extracted.plugin_bytes, extracted.plugin.stat().st_size)
            self.assertEqual(
                extracted.manifest_sha256,
                stage_release.digest(extracted.package_root / "RELEASE_MANIFEST.json"),
            )

    def test_rejects_archive_sha_mismatch_before_extraction(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            staging = self.make_staging(root)
            archive = root / "release.zip"
            self.archive(staging, archive)
            with self.assertRaisesRegex(
                run_staged_stata_tests.StagedReleaseError, "SHA-256"
            ):
                run_staged_stata_tests.extract_staged_release(
                    archive, "0" * 64, root / "extracted"
                )

    def test_rejects_path_traversal_and_multiple_roots(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            archive = root / "unsafe.zip"
            with zipfile.ZipFile(archive, "w") as output:
                output.writestr("fereg-0.5.0/../escape", b"bad")
                output.writestr("other/file", b"bad")
            digest = stage_release.digest(archive)
            with self.assertRaisesRegex(
                run_staged_stata_tests.StagedReleaseError, "flat package"
            ):
                run_staged_stata_tests.extract_staged_release(
                    archive, digest, root / "extracted"
                )

    def test_rejects_file_changed_after_manifest_was_written(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            staging = self.make_staging(root)
            (staging / "fereg.pkg").write_text("tampered\n", encoding="utf-8")
            archive = root / "tampered.zip"
            digest = self.archive(staging, archive)
            with self.assertRaisesRegex(
                run_staged_stata_tests.StagedReleaseError,
                "fails its record",
            ):
                run_staged_stata_tests.extract_staged_release(
                    archive, digest, root / "extracted"
                )

    def test_rejects_dirty_source_manifest_for_licensed_installation(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            staging = self.make_staging(root, dirty=True)
            archive = root / "dirty.zip"
            digest = self.archive(staging, archive)
            with self.assertRaisesRegex(
                run_staged_stata_tests.StagedReleaseError,
                "clean source",
            ):
                run_staged_stata_tests.extract_staged_release(
                    archive, digest, root / "extracted"
                )

    def test_rejects_unsealed_extra_member(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            staging = self.make_staging(root)
            (staging / "undeclared.ado").write_text("bad\n", encoding="utf-8")
            archive = root / "extra.zip"
            stage_release.write_zip(staging, archive)
            digest = stage_release.digest(archive)
            with self.assertRaisesRegex(
                run_staged_stata_tests.StagedReleaseError,
                "sealed inventory",
            ):
                run_staged_stata_tests.extract_staged_release(
                    archive, digest, root / "extracted"
                )

    def test_dormant_feppml_smoke_requires_exact_extracted_paths(self):
        fixture = (
            Path(__file__).resolve().parents[2]
            / "tests/stata/installation/feppml_installation_smoke.do"
        ).read_text(encoding="utf-8")
        for token in (
            "$FEREG_STAGED_PACKAGE_ROOT",
            "$FEREG_STAGED_PLUGIN_NAME",
            "_fereg_ppml.ado",
            "_fereg_estimate_ppml.ado",
            "feppml_p.ado",
            "fereg.pkg stata.toc `staged_plugin'",
            "resolved outside the extracted release",
        ):
            self.assertIn(token, fixture)
        self.assertNotIn("adopath", fixture.lower())


if __name__ == "__main__":
    unittest.main()
