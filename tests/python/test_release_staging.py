from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
from scripts import package_release, stage_release  # noqa: E402


class ReleaseStagingTests(unittest.TestCase):
    PRODUCTION_STATA_FILES = (
        "fereg.ado", "fereg_run.ado", "_fereg_estimate_public.ado",
        "_fereg_ols.ado", "_fereg_estimate_ols.ado", "_fereg_load.ado", "_fereg_memory_baseline.ado", "_fereg_memory_validate.ado",
        "feppml.ado", "_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "_fereg_ppml_result_mata.ado",
        "_fereg_ppml_result_mata.json", "_fereg_ppml_validation_error.mo",
        "_fereg_ppml_validate_result.mo", "_fereg_ppml_post_scalars.mo",
        "feppml_p.ado", "fereg_parse.ado", "fereg_p.ado", "fereg_estat.ado",
        "fereg.sthlp", "feppml.sthlp", "fereg_parse.sthlp",
        "fereg_postestimation.sthlp", "fereg.pkg", "stata.toc",
    )

    def test_runtime_release_inventory_is_exact(self) -> None:
        self.assertEqual(
            stage_release.PRODUCTION_STATA_FILES,
            self.PRODUCTION_STATA_FILES,
        )
        self.assertEqual(stage_release.VERSION, "0.5.0")
    def test_ppml_objects_are_bound_to_source_and_oldest_compiler(self):
        stage_release.validate_ppml_mata_objects(ROOT / "stata")
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            names = [name for name in self.PRODUCTION_STATA_FILES
                     if name.startswith("_fereg_ppml_") and name != "_fereg_ppml.ado"]
            for name in names:
                (target / name).write_bytes((ROOT / "stata" / name).read_bytes())
            for changed in ("_fereg_ppml_result_mata.ado", "_fereg_ppml_validate_result.mo"):
                path = target / changed
                original = path.read_bytes()
                path.write_bytes(original + b"changed")
                with self.assertRaises(RuntimeError):
                    stage_release.validate_ppml_mata_objects(target)
                path.write_bytes(original)
            path = target / "_fereg_ppml_result_mata.json"
            receipt = json.loads(path.read_text())
            receipt["compiler_release"] = "19"
            path.write_text(json.dumps(receipt))
            with self.assertRaises(RuntimeError):
                stage_release.validate_ppml_mata_objects(target)

    def make_plugins(self, directory: Path) -> None:
        for index, name in enumerate(stage_release.EXPECTED_PLUGINS, start=1):
            plugin = directory / name
            plugin.write_bytes((f"mock-native-{index}\n").encode())
            digest = hashlib.sha256(plugin.read_bytes()).hexdigest()
            (directory / f"{name}.sha256").write_text(
                f"{digest}  {name}\n", encoding="ascii"
            )

    def make_audit(self, directory: Path) -> Path:
        source_sha, _ = stage_release.source_identity(ROOT)
        path = directory / "audit.json"
        path.write_text(
            json.dumps(
                {
                    "schema": 1,
                    "status": "pass",
                    "source_sha": source_sha,
                    "cargo_lock_sha256": stage_release.digest(ROOT / "Cargo.lock"),
                    "advisory_count": 0,
                    "toolchain": "1.85.1",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        return path

    def test_release_zip_is_audited_and_excludes_developer_adapters(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plugins = root / "plugins"
            plugins.mkdir()
            self.make_plugins(plugins)
            audit = self.make_audit(root)
            output = root / "release.zip"
            second_output = root / "release-second.zip"
            stage_release.create(
                ROOT, plugins, audit, output, allow_dirty=True
            )
            stage_release.create(
                ROOT, plugins, audit, second_output, allow_dirty=True
            )
            self.assertEqual(output.read_bytes(), second_output.read_bytes())
            with zipfile.ZipFile(output) as archive:
                names = set(archive.namelist())
                manifest = json.loads(
                    archive.read(
                        f"{stage_release.PACKAGE_NAME}/RELEASE_MANIFEST.json"
                    )
                )
            prefix = f"{stage_release.PACKAGE_NAME}/"
            for name in stage_release.EXPECTED_PLUGINS:
                self.assertIn(prefix + name, names)
                self.assertIn(prefix + name + ".sha256", names)
            self.assertIn(prefix + "fereg.ado", names)
            self.assertIn(prefix + "fereg.pkg", names)
            self.assertIn(prefix + "MANIFEST.sha256", names)
            self.assertIn(prefix + "RELEASE_MANIFEST.json", names)
            self.assertIn(prefix + "SBOM.spdx.json", names)
            self.assertIn(prefix + "DEPENDENCY_AUDIT.json", names)
            self.assertNotIn(prefix + "_fereg_estimate_ols_test.ado", names)
            self.assertNotIn(prefix + "_fereg_ols_hidden_numeric_v1.ado", names)
            private_ppml_files = (
                "_fereg_ppml.ado",
                "_fereg_estimate_ppml.ado",
                "feppml_p.ado",
            )
            for private_ppml in private_ppml_files:
                self.assertIn(prefix + private_ppml, names)
            self.assertEqual(manifest["version"], "0.5.0")
            self.assertEqual(manifest["phase"], "beta")
            self.assertEqual(manifest["action_protocol"], "1.6")
            self.assertEqual(manifest["ols_result_wire"], "1.5")
            self.assertEqual(manifest["model_wire"], "2.1")
            self.assertEqual(manifest["ppml_request_wire"], "1.7")
            self.assertEqual(manifest["ppml_result_wire"], "1.4")
            caveats = " ".join(manifest["support_caveats"]).lower()
            self.assertNotRegex(caveats, r"r\(498\)|fail[- ]closed|ols[- ]only")

    def test_source_zip_uses_tracked_inventory_without_mutating_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            audit = self.make_audit(root)
            output = root / "source.zip"
            second_output = root / "source-second.zip"
            root_manifest = ROOT / "MANIFEST.sha256"
            root_source_manifest = ROOT / "SOURCE_MANIFEST.json"
            before = (root_manifest.exists(), root_source_manifest.exists())
            package_release.create(ROOT, audit, output, allow_dirty=True)
            package_release.create(
                ROOT, audit, second_output, allow_dirty=True
            )
            self.assertEqual(output.read_bytes(), second_output.read_bytes())
            after = (root_manifest.exists(), root_source_manifest.exists())
            self.assertEqual(before, after)
            with zipfile.ZipFile(output) as archive:
                names = set(archive.namelist())
                source_manifest = json.loads(
                    archive.read(
                        f"{package_release.PACKAGE_NAME}/SOURCE_MANIFEST.json"
                    )
                )
            prefix = f"{package_release.PACKAGE_NAME}/"
            self.assertIn(prefix + "Cargo.lock", names)
            self.assertIn(prefix + "SBOM.spdx.json", names)
            self.assertIn(prefix + "DEPENDENCY_AUDIT.json", names)
            self.assertNotIn(
                prefix + "stata/_fereg_estimate_ols_test.ado", names
            )
            self.assertFalse(any(name.startswith(prefix + ".ci/") for name in names))
            self.assertFalse(
                any(name.startswith(prefix + "build_evidence/") for name in names)
            )
            self.assertFalse(
                any(
                    name.startswith(prefix + "benchmarks/reports/captured/")
                    for name in names
                )
            )
            self.assertFalse(any(name.endswith(".pdf") for name in names))
            self.assertEqual(source_manifest["version"], "0.5.0")

    def test_checksum_mismatch_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            plugins = Path(temporary)
            self.make_plugins(plugins)
            target = plugins / stage_release.EXPECTED_PLUGINS[0]
            target.write_bytes(b"tampered")
            with self.assertRaises(RuntimeError):
                stage_release.verify_plugins(plugins)

    def test_review_subset_is_reproducible_and_cannot_replace_full_release(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plugins = root / "plugins"
            plugins.mkdir()
            self.make_plugins(plugins)
            (plugins / "fereg_windows.plugin").unlink()
            (plugins / "fereg_windows.plugin.sha256").unlink()
            audit = self.make_audit(root)
            with self.assertRaisesRegex(RuntimeError, "missing or empty"):
                stage_release.create(ROOT, plugins, audit, root / "release.zip", allow_dirty=True)
            for name, order in (("first.zip", ("macos", "linux")),
                                ("second.zip", ("linux", "macos"))):
                stage_release.create(ROOT, plugins, audit, root / name, allow_dirty=True,
                                     review_platforms=order)
            self.assertEqual((root / "first.zip").read_bytes(), (root / "second.zip").read_bytes())
            with zipfile.ZipFile(root / "first.zip") as archive:
                prefix = stage_release.PACKAGE_NAME + "-review/"
                manifest = json.loads(archive.read(prefix + "RELEASE_MANIFEST.json"))
                package = archive.read(prefix + "fereg.pkg").decode()
                self.assertNotIn(prefix + "fereg_windows.plugin", archive.namelist())
                self.assertNotIn("f fereg_windows.plugin", package)
                for platform in ("macos", "linux"):
                    self.assertIn(f"f fereg_{platform}.plugin", package)
                self.assertEqual(manifest['schema'], 3)
                self.assertIs(manifest['review_only'], True)
                self.assertIs(manifest['publication_eligible'], False)
                self.assertEqual(manifest['missing_platforms'], ['windows'])

    def test_invalid_review_selections_fail_closed(self):
        for selection in ((), ('macos', 'macos'), ('plan9',)):
            with self.assertRaises(RuntimeError):
                stage_release.selected_plugins(selection)

    def test_dependency_audit_must_match_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            audit = self.make_audit(root)
            payload = json.loads(audit.read_text())
            payload["source_sha"] = "0" * 40
            audit.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaises(RuntimeError):
                stage_release.validate_dependency_audit(
                    ROOT, audit, stage_release.source_identity(ROOT)[0]
                )

    def test_output_inside_source_is_rejected(self):
        with self.assertRaises(RuntimeError):
            stage_release.require_external_output(
                ROOT, ROOT / "release-artifacts" / "fereg.zip"
            )


if __name__ == "__main__":
    unittest.main()
