from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
import zipfile
from unittest.mock import patch
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
from scripts import package_release, stage_release, stage_local_install  # noqa: E402


class ReleaseStagingTests(unittest.TestCase):
    PRODUCTION_STATA_FILES = (
        "fereg.ado", "_fereg_display.ado", "fereg_run.ado", "_fereg_estimate_public.ado",
        "_fereg_estimate_ols.ado", "_fereg_load.ado", "_fereg_factor_sample.ado", "_fereg_indepvars.ado", "_fereg_memory_baseline.ado", "_fereg_memory_validate.ado", "_fereg_input_host.ado",
        "feppml.ado", "_fereg_ppml.ado", "_fereg_estimate_ppml.ado", "_fereg_ppml_result_mata.ado",
        "_fereg_ppml_result_mata.json", "_fereg_ppml_validation_error.mo",
        "_fereg_ppml_validate_result.mo", "_fereg_ppml_post_scalars.mo",
        "feppml_p.ado", "fereg_parse.ado", "fereg_p.ado", "fereg_estat.ado",
        "fereg.sthlp", "feppml.sthlp", "fereg_parse.sthlp",
        "fereg_postestimation.sthlp", "fereg.pkg", "stata.toc",
    )

    def test_posted_release_label_matches_the_release_tag(self) -> None:
        # e(fereg_version) and the selftest's r(fereg_version) name the release.
        label = stage_release.RELEASE_TAG.removeprefix("v")
        for name in ("fereg.ado", "_fereg_estimate_public.ado", "_fereg_estimate_ppml.ado"):
            text = (ROOT / "stata" / name).read_text()
            self.assertIn(f'fereg_version "{label}"', text, name)

    def test_runtime_release_inventory_is_exact(self) -> None:
        self.assertEqual(
            stage_release.PRODUCTION_STATA_FILES,
            self.PRODUCTION_STATA_FILES,
        )
        self.assertEqual(stage_release.VERSION, "0.5.2")
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
            self.assertIn(prefix + "CMG-MIT.txt", names)
            # Check the installer layout even when testing an uncommitted edit.
            from scripts.run_staged_stata_tests import _safe_members
            with zipfile.ZipFile(output) as archive:
                _safe_members(archive)
                self.assertEqual(
                    archive.read(prefix + "CMG-MIT.txt"),
                    (ROOT / "licenses/CMG-MIT.txt").read_bytes(),
                )
            self.assertIn("CMG-MIT.txt", {row["path"] for row in manifest["files"]})
            self.assertNotIn(prefix + "_fereg_estimate_ols_test.ado", names)
            self.assertNotIn(prefix + "_fereg_ols_hidden_numeric_v1.ado", names)
            private_ppml_files = (
                "_fereg_ppml.ado",
                "_fereg_estimate_ppml.ado",
                "feppml_p.ado",
            )
            for private_ppml in private_ppml_files:
                self.assertIn(prefix + private_ppml, names)
            self.assertEqual(len(manifest['plugins']), 3)
            self.assertTrue(all(p['source_sha'] == manifest['source_sha'] for p in manifest['plugins']))
            self.assertIn(prefix + '_fereg_factor_sample.ado', names)
            self.assertNotIn(prefix + 'LEGACY_COMPARATOR.json', names)
            self.assertFalse(any('fereg_legacy_' in name for name in names))
            self.assertEqual(manifest["version"], "0.5.2")
            self.assertEqual(manifest["phase"], "stable")
            self.assertEqual(manifest["action_protocol"], "1.7")
            self.assertEqual(manifest["ols_result_wire"], "1.5")
            self.assertEqual(manifest["model_wire"], "2.1")
            self.assertEqual(manifest["ppml_request_wire"], "1.8")
            self.assertEqual(manifest["ppml_result_wire"], "1.5")
            caveats = " ".join(manifest["support_caveats"]).lower()
            self.assertNotRegex(caveats, r"r\(498\)|fail[- ]closed|ols[- ]only")

    def test_reused_native_build_keeps_its_source_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.make_plugins(root)
            audit = self.make_audit(root)
            data = json.loads(audit.read_text())
            native_sha = "a" * 40
            output = root / "release.zip"
            stage_release.create(ROOT, root, audit, output, allow_dirty=True,
                                 plugin_sources={"fereg_windows.plugin": native_sha})
            with zipfile.ZipFile(output) as archive:
                manifest = json.loads(archive.read(
                    f"{stage_release.PACKAGE_NAME}/RELEASE_MANIFEST.json"))
            self.assertEqual(manifest["source_sha"], stage_release.source_identity(ROOT)[0])
            self.assertEqual({p["path"]: p["source_sha"] for p in manifest["plugins"]},
                             {name: native_sha if name == "fereg_windows.plugin" else manifest["source_sha"]
                              for name in stage_release.EXPECTED_PLUGINS})
            with self.assertRaisesRegex(RuntimeError, "full lowercase Git commit SHA"):
                stage_release.create(ROOT, root, audit, output, allow_dirty=True,
                                     plugin_sources={"fereg_windows.plugin": "short"})
            data["cargo_lock_sha256"] = "0" * 64
            audit.write_text(json.dumps(data))
            with self.assertRaisesRegex(RuntimeError, "Cargo.lock"):
                stage_release.create(ROOT, root, audit, output, allow_dirty=True,
                                     plugin_sources={"fereg_windows.plugin": native_sha})

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
            self.assertIn(prefix + "licenses/CMG-MIT.txt", names)
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
            self.assertEqual(source_manifest["version"], "0.5.2")

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
                self.assertNotIn("f fereg_legacy_windows.plugin", package)
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

    def test_explicit_macos_beta_has_only_its_installable_files_and_mit_metadata(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plugins = root / "plugins"
            plugins.mkdir()
            self.make_plugins(plugins)
            audit = self.make_audit(root)
            output = root / "macos.zip"
            stage_release.create(ROOT, plugins, audit, output, allow_dirty=True,
                                 platforms=("macos",))
            with zipfile.ZipFile(output) as archive:
                prefix = stage_release.PACKAGE_NAME + "/"
                manifest = json.loads(archive.read(prefix + "RELEASE_MANIFEST.json"))
                package = archive.read(prefix + "fereg.pkg").decode()
                sbom = json.loads(archive.read(prefix + "SBOM.spdx.json"))
                for line in package.splitlines():
                    if line.startswith(("f ", "F ")):
                        self.assertIn(prefix + line[2:], archive.namelist())
                for name in ("fereg-LICENSE.txt", "fereg-THIRD-PARTY-NOTICES.txt",
                             "fereg-SOURCE.txt", "_fereg_ppml_result_mata.json",
                             "fereg_macos.plugin.sha256"):
                    self.assertIn(f"F {name}", package.splitlines())
                self.assertNotIn("f fereg_linux.plugin", package)
                self.assertNotIn("f fereg_windows.plugin", package)
                self.assertNotIn("F fereg_linux.plugin.sha256", package)
                self.assertNotIn("F fereg_windows.plugin.sha256", package)
                self.assertEqual(manifest["included_platforms"], ["macos"])
                self.assertEqual(manifest["missing_platforms"], ["linux", "windows"])
                self.assertNotIn("review_only", manifest)
                self.assertEqual(sbom["packages"][0]["licenseDeclared"], "MIT")

    def test_local_install_is_complete_and_rejects_reuse_or_bad_checksum(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plugins = root / "plugins"
            plugins.mkdir()
            self.make_plugins(plugins)
            output = root / "local"
            stage_local_install.stage(ROOT, plugins, output, "macos")
            for line in (output / "fereg.pkg").read_text().splitlines():
                if line.startswith(("f ", "F ")):
                    self.assertTrue((output / line[2:]).is_file(), line)
            self.assertFalse((output / "fereg_linux.plugin").exists())
            with self.assertRaisesRegex(RuntimeError, "already exists"):
                stage_local_install.stage(ROOT, plugins, output, "macos")
            (plugins / "fereg_macos.plugin").write_bytes(b"tampered")
            with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                stage_local_install.stage(ROOT, plugins, root / "rejected", "macos")
            self.assertFalse((root / "rejected").exists())

    def test_dependency_sources_preserve_licenses_and_relocate_config(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            vendor = root / "dependencies"
            crate = vendor / "sample-1.0.0"
            crate.mkdir(parents=True)
            (crate / "Cargo.toml").write_text('[package]\nname="sample"\nversion="1.0.0"\n')
            (crate / ".cargo-checksum.json").write_text('{}\n')
            # r-efi records its license grant in AUTHORS rather than LICENSE.
            (crate / "AUTHORS").write_text("retained dependency license\n")
            cmg = vendor / "cmg-0.2.0"
            cmg.mkdir()
            cmg_files = {
                "Cargo.toml": '[package]\nname="cmg"\nversion="0.2.0"\nlicense="MIT"\n',
                "Cargo.toml.orig": '[package]\nname="cmg"\nversion="0.2.0"\nlicense="MIT"\n',
                "LICENSE": (ROOT / "licenses/CMG-MIT.txt").read_text(),
                "lib.rs": "pub fn unchanged() {}\n",
            }
            for name, text in cmg_files.items():
                (cmg / name).write_text(text)
            (cmg / ".cargo-checksum.json").write_text(json.dumps({
                "package": None,
                "files": {name: hashlib.sha256(text.encode()).hexdigest()
                          for name, text in cmg_files.items()},
            }))
            configuration = root / "config.toml"
            configuration.write_text('[source.crates-io]\nreplace-with="vendored-sources"\n'
                                     '[source.vendored-sources]\ndirectory='
                                     + json.dumps(str(vendor)) + '\n')
            staging = root / "source"
            staging.mkdir()
            (staging / "licenses").mkdir()
            cmg_mit = (ROOT / "licenses/CMG-MIT.txt").read_text()
            (staging / "licenses/CMG-MIT.txt").write_text(cmg_mit)
            rayon = staging / "vendor/rayon-core"
            rayon.mkdir(parents=True)
            (rayon / "LICENSE-MIT").write_text("local fork notice\n")
            repository = "https://github.com/johannes-schmieder/CMG"
            revision = "a" * 40
            source_key = f"git+{repository}?rev={revision}"
            with configuration.open("a") as handle:
                handle.write(f'\n[source."{source_key}"]\ngit="{repository}"\nrev="{revision}"\n'
                             'replace-with="vendored-sources"\n')
            (staging / "crates/fereg-core").mkdir(parents=True)
            (staging / "crates/fereg-core/Cargo.toml").write_text(
                f'[dependencies.cmg]\ngit="{repository}"\nrev="{revision}"\n')
            (staging / "Cargo.lock").write_text(
                f'[[package]]\nname="cmg"\nversion="0.2.0"\nsource="{source_key}#{revision}"\n')
            provenance = {"repository": repository, "revision": revision, "version": "0.2.0",
                          "files": {p.name: stage_release.digest(p) for p in cmg.iterdir()}}
            (staging / "licenses/CMG-SOURCE.json").write_text(json.dumps(provenance))
            package_release.copy_dependency_sources(staging, vendor, configuration)
            self.assertEqual((staging / "vendor/dependencies/sample-1.0.0/AUTHORS").read_text(),
                             "retained dependency license\n")
            config = (staging / ".cargo/config.toml").read_text()
            self.assertIn('directory="vendor/dependencies"', config)
            self.assertNotIn(str(root), config)
            notices = (staging / "THIRD_PARTY_LICENSES.txt").read_text()
            self.assertIn("retained dependency license", notices)
            self.assertIn("local fork notice", notices)
            packaged_cmg = staging / "vendor/dependencies/cmg-0.2.0"
            self.assertEqual((packaged_cmg / "LICENSE").read_text(), cmg_mit)
            self.assertEqual((packaged_cmg / "Cargo.toml").read_text(), cmg_files["Cargo.toml"])
            self.assertEqual((packaged_cmg / "Cargo.toml.orig").read_text(), cmg_files["Cargo.toml.orig"])
            self.assertEqual((packaged_cmg / "lib.rs").read_text(), cmg_files["lib.rs"])
            self.assertEqual((cmg / "LICENSE").read_text(), cmg_files["LICENSE"])
            checksums = json.loads((packaged_cmg / ".cargo-checksum.json").read_text())
            for name, expected in checksums["files"].items():
                self.assertEqual(stage_release.digest(packaged_cmg / name), expected)
            self.assertIn(cmg_mit.rstrip(), notices)
            self.assertNotIn("previous license text", notices)
            sources = package_release.tomllib.loads(configuration.read_text())["source"]
            for path, changed in [(cmg / "lib.rs", "private patch"),
                                  (cmg / "Cargo.toml", cmg_files["Cargo.toml"].replace("MIT", "GPL-3.0-only")),
                                  (staging / "Cargo.lock", (staging / "Cargo.lock").read_text().replace(revision, "b" * 40))]:
                original = path.read_text()
                path.write_text(changed)
                with self.assertRaisesRegex(RuntimeError, "CMG"):
                    package_release.verify_cmg_source(staging, vendor, sources)
                path.write_text(original)
            (cmg / "extra.rs").write_text("unexpected")
            with self.assertRaisesRegex(RuntimeError, "CMG"):
                package_release.verify_cmg_source(staging, vendor, sources)
            (cmg / "extra.rs").unlink()
            with self.assertRaisesRegex(RuntimeError, "does not match"):
                package_release.copy_dependency_sources(staging, root, configuration)

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
