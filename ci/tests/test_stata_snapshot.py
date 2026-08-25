from __future__ import annotations

from pathlib import Path
import sys
import tempfile
import unittest


CI_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_DIR))

import run_stata_ci  # noqa: E402
import supervise_stata  # noqa: E402


class SnapshotManifestTests(unittest.TestCase):
    def test_non_macos_lock_uses_runtime_temporary_directory(self) -> None:
        if sys.platform == "darwin":
            self.skipTest("Linux/SCC default exercised off macOS")
        self.assertEqual(
            supervise_stata.default_lock_file().parent,
            Path(tempfile.gettempdir()),
        )

    def test_shell_wrapper_does_not_pin_a_machine_python_path(self) -> None:
        wrapper = (CI_DIR / "run_stata_ci.sh").read_text(encoding="utf-8")
        self.assertNotIn("/usr/bin/python", wrapper)
        self.assertIn("FEREG_PYTHON_BIN", wrapper)

    def test_declared_files_are_loaded_without_git_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "ci").mkdir()
            (root / "ci" / "fixture.do").write_text(
                "display 1\n", encoding="utf-8"
            )
            (root / "README.md").write_text("fereg\n", encoding="utf-8")
            manifest = root / "source-files.txt"
            manifest.write_text(
                "README.md\nci/fixture.do\n", encoding="utf-8"
            )

            self.assertEqual(
                run_stata_ci.tracked_files(root, manifest),
                [Path("README.md"), Path("ci/fixture.do")],
            )

    def test_declared_files_reject_escape_from_source_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            root = parent / "source"
            root.mkdir()
            outside = parent / "outside.txt"
            outside.write_text("outside\n", encoding="utf-8")
            manifest = root / "source-files.txt"
            manifest.write_text("../outside.txt\n", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "unsafe path"):
                run_stata_ci.tracked_files(root, manifest)

    def test_declared_files_require_every_listed_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "source-files.txt"
            manifest.write_text("missing.txt\n", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "missing declared"):
                run_stata_ci.tracked_files(root, manifest)


if __name__ == "__main__":
    unittest.main()
