from __future__ import annotations

import hashlib
import tempfile
import unittest
import zipfile
from pathlib import Path

import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import stage_release  # noqa: E402


class ReleaseStagingTests(unittest.TestCase):
    def make_plugins(self, directory: Path) -> None:
        for index, name in enumerate(stage_release.EXPECTED_PLUGINS, start=1):
            plugin = directory / name
            plugin.write_bytes((f"mock-native-{index}\n").encode())
            digest = hashlib.sha256(plugin.read_bytes()).hexdigest()
            (directory / f"{name}.sha256").write_text(
                f"{digest}  {name}\n", encoding="ascii"
            )

    def test_release_zip_contains_one_plugin_per_supported_platform(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            plugins = root / "plugins"
            plugins.mkdir()
            self.make_plugins(plugins)
            output = root / "release.zip"
            stage_release.create(ROOT, plugins, output)
            with zipfile.ZipFile(output) as archive:
                names = set(archive.namelist())
            prefix = f"{stage_release.PACKAGE_NAME}/"
            for name in stage_release.EXPECTED_PLUGINS:
                self.assertIn(prefix + name, names)
            self.assertIn(prefix + "stfixest.ado", names)
            self.assertIn(prefix + "stfixest.pkg", names)
            self.assertIn(prefix + "MANIFEST.sha256", names)
            self.assertIn(prefix + "RELEASE_MANIFEST.json", names)

    def test_checksum_mismatch_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            plugins = Path(temporary)
            self.make_plugins(plugins)
            target = plugins / stage_release.EXPECTED_PLUGINS[0]
            target.write_bytes(b"tampered")
            with self.assertRaises(RuntimeError):
                stage_release.verify_plugins(plugins)


if __name__ == "__main__":
    unittest.main()
