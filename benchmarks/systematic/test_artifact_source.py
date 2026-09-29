import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from artifact_source import REQUIRED_INPUTS, package_files, sha256, snapshot, verify_source
import provenance


class ArtifactSourceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for name in (*REQUIRED_INPUTS, "vendor/runtime/src/lib.rs", "vendor/runtime/build.rs"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        files = package_files(self.root)
        self.source = dict(files=files, snapshot=snapshot(files))

    def test_complete_inventory_accepts_and_binds_optional_explicit_inputs(self):
        path = self.root / "tests/fixture.do"
        path.parent.mkdir()
        path.write_text("fixture")
        self.source["files"]["tests/fixture.do"] = sha256(path)
        self.source["snapshot"] = snapshot(self.source["files"])
        self.assertEqual(verify_source(self.root, self.source), self.source["snapshot"])
        path.write_text("changed fixture")
        with self.assertRaisesRegex(ValueError, "source input changed: tests/fixture.do"):
            verify_source(self.root, self.source)

    def test_omitted_bridge_build_config_or_vendor_fails_even_with_consistent_snapshot(self):
        for name in ("plugin/stata_bridge.c", "plugin/exports_linux.map", ".cargo/config.toml",
                     "rust-toolchain.toml", "scripts/build_linux.sh", "vendor/runtime/build.rs"):
            with self.subTest(name=name):
                source = copy.deepcopy(self.source)
                del source["files"][name]
                source["snapshot"] = snapshot(source["files"])
                with self.assertRaisesRegex(ValueError, "inventory omits package/build inputs"):
                    verify_source(self.root, source)

    def test_modified_content_cannot_keep_the_validated_snapshot(self):
        path = self.root / "plugin/stata_bridge.c"
        path.write_text("new bridge")
        with self.assertRaisesRegex(ValueError, "source input changed"):
            verify_source(self.root, self.source)
        self.source["files"]["plugin/stata_bridge.c"] = sha256(path)
        with self.assertRaisesRegex(ValueError, "snapshot does not bind"):
            verify_source(self.root, self.source)

    def test_new_source_and_missing_required_source_fail(self):
        path = self.root / "plugin/new_bridge.h"
        path.write_text("new header")
        with self.assertRaisesRegex(ValueError, "inventory omits"):
            verify_source(self.root, self.source)
        path.unlink()
        (self.root / "plugin/stata_bridge.h").unlink()
        with self.assertRaisesRegex(ValueError, "missing required package inputs"):
            verify_source(self.root, self.source)

    def test_unregistered_build_products_do_not_change_identity(self):
        for name in ("vendor/runtime/target/debug/lib.rlib", "legacy/fereg-core/target/test.bin",
                     "stata/__pycache__/cached.pyc", "crates/.DS_Store", "vendor/runtime/.git/HEAD",
                     "target/lib.rlib", "dist/fereg_linux.plugin"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("generated")
        self.assertEqual(package_files(self.root), self.source["files"])
        verify_source(self.root, self.source)

    def test_symlinks_and_external_inventory_paths_fail(self):
        path = self.root / "plugin/alias.h"
        path.symlink_to(self.root / "plugin/stata_bridge.h")
        with self.assertRaisesRegex(ValueError, "symlink"):
            verify_source(self.root, self.source)
        path.unlink()
        for name in ("../escape", "/absolute", "plugin/../escape"):
            with self.subTest(name=name):
                source = copy.deepcopy(self.source)
                source["files"][name] = "0" * 64
                source["snapshot"] = snapshot(source["files"])
                with self.assertRaisesRegex(ValueError, "invalid source input path"):
                    verify_source(self.root, source)

    def test_frozen_runtime_binds_the_new_verifier(self):
        controller = self.root / "controller"
        controller.mkdir()
        for name in provenance.MEASUREMENT_FILES:
            (controller / name).write_text(name)
        for name in ("input/candidate.tar.gz", "candidate/dist/fereg_linux.plugin",
                     "env/python-freeze.txt", "env/R-packages.csv"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        (self.root / "input/manifest.json").write_text(json.dumps(dict(candidate_commit="candidate")))
        (self.root / "input/candidate.json").write_text(json.dumps(dict(
            commit="candidate", sha256=sha256(self.root / "input/candidate.tar.gz"))))
        with patch.object(provenance, "HERE", controller):
            identity = provenance.freeze(self.root)
            self.assertIn("artifact_source.py", identity["controller"])
            self.assertEqual(provenance.verify(self.root), identity["fingerprint"])
            (controller / "artifact_source.py").write_text("changed verifier")
            with self.assertRaisesRegex(ValueError, "measurement controller changed: artifact_source.py"):
                provenance.verify(self.root)


if __name__ == "__main__":
    unittest.main()
