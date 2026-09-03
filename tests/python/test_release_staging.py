from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
from scripts import package_release, stage_release  # noqa: E402
from scripts.feppml_public_state import ACTIVATED, public_state  # noqa: E402


class ReleaseStagingTests(unittest.TestCase):
    PREACTIVATION_STATA_FILES = (
        "fereg.ado", "fereg_run.ado", "_fereg_estimate_public.ado",
        "_fereg_ols.ado", "_fereg_estimate_ols.ado", "_fereg_load.ado",
        "feppml.ado", "fereg_parse.ado", "fereg_p.ado", "fereg_estat.ado",
        "fereg.sthlp", "feppml.sthlp", "fereg_parse.sthlp",
        "fereg_postestimation.sthlp", "fereg.pkg", "stata.toc",
    )

    def test_runtime_release_inventory_is_exact_for_public_state(self) -> None:
        expected = list(self.PREACTIVATION_STATA_FILES)
        if public_state(ROOT) == ACTIVATED:
            insertion = expected.index("feppml.ado") + 1
            expected[insertion:insertion] = [
                "_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "feppml_p.ado"
            ]
        self.assertEqual(stage_release.PRODUCTION_STATA_FILES, tuple(expected))
        self.assertEqual(stage_release.VERSION, "0.5.0")
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
                if public_state(ROOT) == ACTIVATED:
                    self.assertIn(prefix + private_ppml, names)
                else:
                    self.assertNotIn(prefix + private_ppml, names)
            self.assertEqual(manifest["version"], "0.5.0")
            self.assertEqual(manifest["phase"], "beta")
            self.assertEqual(manifest["action_protocol"], "1.5")
            self.assertEqual(manifest["ols_result_wire"], "1.4")
            self.assertEqual(manifest["model_wire"], "2.1")
            if public_state(ROOT) == ACTIVATED:
                self.assertEqual(manifest["ppml_request_wire"], "1.5")
                self.assertEqual(manifest["ppml_result_wire"], "1.2")
                caveats = " ".join(manifest["support_caveats"]).lower()
                self.assertNotRegex(caveats, r"r\(498\)|fail[- ]closed|ols[- ]only")
            else:
                self.assertNotIn("ppml_request_wire", manifest)
                self.assertNotIn("ppml_result_wire", manifest)
                self.assertEqual(len(manifest["support_caveats"]), 3)

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
