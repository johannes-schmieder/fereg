import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import check_github_install
import stage_release


class GitHubInstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "dist").mkdir()
        (self.root / "stata").mkdir()
        directives = ["v 3"]
        for name in stage_release.PRODUCTION_STATA_FILES:
            if name in {"fereg.pkg", "stata.toc"}:
                continue
            shutil.copyfile(ROOT / "stata" / name, self.root / "stata" / name)
            directives.append(f"{'F' if name.endswith('.json') else 'f'} stata/{name}")
        plugin = "fereg_macos.plugin"
        digest = hashlib.sha256(b"test plugin").hexdigest()
        (self.root / "dist" / plugin).write_bytes(b"test plugin")
        (self.root / "dist" / (plugin + ".sha256")).write_text(f"{digest}  {plugin}\n")
        metadata = {"source_sha": "a" * 40, "runtime_sha256": "runtime",
                    "plugins": [{"path": plugin, "sha256": digest}]}
        (self.root / "dist/fereg-native.json").write_text(json.dumps(metadata))
        for name in ("fereg-LICENSE.txt", "fereg-THIRD-PARTY-NOTICES.txt", "fereg-SOURCE.txt"):
            (self.root / "dist" / name).write_text("test metadata\n")
        for path in sorted((self.root / "dist").iterdir()):
            directives.append(f"{'f' if path.suffix == '.plugin' else 'F'} dist/{path.name}")
        self.package = self.root / "fereg.pkg"
        self.package.write_text("\n".join(directives) + "\n")
        (self.root / "stata.toc").write_text("v 3\np fereg test package\n")
        identity = patch.object(check_github_install, "runtime_identity", return_value="runtime")
        identity.start()
        self.addCleanup(identity.stop)

    def test_complete_package(self):
        check_github_install.check(self.root)

    def test_stale_or_modified_plugin_is_rejected(self):
        with patch.object(check_github_install, "runtime_identity", return_value="changed"):
            with self.assertRaisesRegex(RuntimeError, "runtime changed"):
                check_github_install.check(self.root)
        (self.root / "dist/fereg_macos.plugin").write_bytes(b"modified plugin")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            check_github_install.check(self.root)

    def test_missing_file_and_ancillary_metadata_are_rejected(self):
        original = self.package.read_text()
        self.package.write_text(original.replace("F dist/fereg-SOURCE.txt", "f dist/fereg-SOURCE.txt"))
        with self.assertRaisesRegex(RuntimeError, "uppercase F"):
            check_github_install.check(self.root)
        self.package.write_text(original)
        (self.root / "stata/feppml.ado").unlink()
        with self.assertRaisesRegex(RuntimeError, "missing or unexpected"):
            check_github_install.check(self.root)

    def test_omitted_command_is_rejected(self):
        self.package.write_text(self.package.read_text().replace("f stata/feppml.ado\n", ""))
        with self.assertRaisesRegex(RuntimeError, "incomplete or duplicated"):
            check_github_install.check(self.root)
