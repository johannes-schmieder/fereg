from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/check_python_environment.py"
MODULES = ("numpy", "scipy", "pandas")


class PythonEnvironmentPreflightTests(unittest.TestCase):
    def run_preflight(self, sources: dict[str, str]) -> subprocess.CompletedProcess[str]:
        # Shadow modules on PYTHONPATH decide what this interpreter can import.
        with tempfile.TemporaryDirectory() as raw:
            for name, source in sources.items():
                (Path(raw) / f"{name}.py").write_text(source, encoding="utf-8")
            environment = dict(os.environ, PYTHONPATH=raw)
            return subprocess.run([sys.executable, str(SCRIPT)], env=environment,
                                  capture_output=True, text=True, timeout=60, check=False)

    def test_a_missing_scientific_module_fails_and_names_the_interpreter(self):
        for missing in MODULES:
            with self.subTest(missing=missing):
                sources = {name: "__version__ = 'shadow'\n" for name in MODULES}
                sources[missing] = "raise ImportError('blocked for this test')\n"
                result = self.run_preflight(sources)
                self.assertEqual(result.returncode, 3)
                self.assertEqual(result.stdout, "")
                self.assertIn(f"PYTHON ENVIRONMENT: FAIL for {sys.executable}", result.stderr)
                self.assertIn(f"{missing} is not importable (blocked for this test)", result.stderr)
                self.assertIn("never counted as passed", result.stderr)
                self.assertIn("FEREG_PYTHON_BIN", result.stderr)

    def test_a_complete_environment_reports_its_versions(self):
        result = self.run_preflight({name: f"__version__ = '{name}-shadow'\n" for name in MODULES})
        if sys.version_info[:2] < (3, 11):
            self.assertEqual(result.returncode, 3)
            self.assertIn("Python 3.11 or newer is required", result.stderr)
            return
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"Python environment: PASS ({sys.executable}", result.stdout)
        for name in MODULES:
            self.assertIn(f"{name} {name}-shadow", result.stdout)


if __name__ == "__main__":
    unittest.main()
