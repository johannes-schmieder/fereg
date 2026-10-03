"""Guard canonical identity independently of network access or native builds."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("legacy_comparator", ROOT / "scripts/legacy_comparator.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ComparatorIdentity(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.manifest = MODULE.baseline()
        self.manifest["assets"] = {}
        self.manifest_path = self.directory / "baseline.json"
        self.name, self.meta_name = MODULE.names("linux")
        (self.directory / self.name).write_bytes(b"test-only native artifact")
        self.metadata = {key: self.manifest[key] for key in
                         ("source_commit", "build_date", "rust_toolchain", "source_files", "source_trees")}
        self.metadata.update(schema=1, platform="linux", filename=self.name,
                             sha256=MODULE.digest(self.directory / self.name),
                             bytes=(self.directory / self.name).stat().st_size)
        self.write_metadata()
        self.manifest_path.write_text(json.dumps(self.manifest))
        replacement = patch.object(MODULE, "MANIFEST", self.manifest_path)
        replacement.start()
        self.addCleanup(replacement.stop)
        quiet = patch("builtins.print")
        quiet.start()
        self.addCleanup(quiet.stop)

    def write_metadata(self):
        (self.directory / self.meta_name).write_text(json.dumps(self.metadata))
        (self.directory / f"{self.name}.sha256").write_text(f"{self.metadata['sha256']}  {self.name}\n")

    def test_unregistered_artifact_cannot_be_used_even_with_self_consistent_hashes(self):
        with self.assertRaisesRegex(ValueError, "no canonical"):
            MODULE.verify(self.directory, "linux")

    def test_wrong_source_cannot_be_registered(self):
        self.metadata["source_commit"] = "0" * 40
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "provenance"):
            MODULE.register(self.directory, "linux")

    def test_registered_identity_detects_tampering_and_cannot_be_replaced(self):
        MODULE.register(self.directory, "linux")
        MODULE.verify(self.directory, "linux")
        (self.directory / self.name).write_bytes(b"replacement")
        with self.assertRaisesRegex(ValueError, "digest"):
            MODULE.verify(self.directory, "linux")
        self.metadata.update(sha256=MODULE.digest(self.directory / self.name), bytes=11)
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "already registered"):
            MODULE.register(self.directory, "linux")
        with self.assertRaisesRegex(ValueError, "canonical"):
            MODULE.verify(self.directory, "linux")

    def test_missing_canonical_registration_never_downloads_or_builds(self):
        with patch.object(MODULE.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "never builds"):
                MODULE.fetch(self.directory / "cache", "linux")
            run.assert_not_called()

    def test_changed_metadata_is_rejected_even_if_binary_is_unchanged(self):
        MODULE.register(self.directory, "linux")
        self.metadata["checks"] = ["forged qualification"]
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "canonical"):
            MODULE.verify(self.directory, "linux")


if __name__ == "__main__":
    unittest.main()
