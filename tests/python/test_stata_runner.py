from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.run_stata_tests import (
    expected_plugin_filename,
    expected_sentinel,
    run_fixture,
    sha256_file,
    validate_fixture_log,
)


class StataRunnerSentinelTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = Path("tests/stata/installation/installation_smoke.do")

    def test_accepts_exact_terminal_success_sentinel(self):
        sentinel = expected_sentinel(self.fixture)
        self.assertIsNone(validate_fixture_log(self.fixture, f"noise\n{sentinel}\n"))

    def test_rejects_missing_sentinel_even_when_log_looks_successful(self):
        error = validate_fixture_log(self.fixture, "installation_smoke.do: PASS\n")
        self.assertIn("missing", error or "")

    def test_rejects_nonzero_sentinel(self):
        error = validate_fixture_log(self.fixture, "END_FIXTURE|installation_smoke|rc=9\n")
        self.assertIn("expected", error or "")

    def test_rejects_success_followed_by_failure_sentinel(self):
        log = "END_FIXTURE|installation_smoke|rc=0\nEND_FIXTURE|installation_smoke|rc=9\n"
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("exactly one", error or "")

    def test_rejects_wrong_sentinel_followed_by_expected(self):
        log = "END_FIXTURE|other_fixture|rc=0\nEND_FIXTURE|installation_smoke|rc=0\n"
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("exactly one", error or "")

    def test_rejects_expected_sentinel_followed_by_malformed(self):
        log = "END_FIXTURE|installation_smoke|rc=0\nEND_FIXTURE|installation_smoke|success\n"
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("exactly one", error or "")

    def test_command_echo_does_not_count_as_sentinel_output(self):
        log = '. di as result "END_FIXTURE|installation_smoke|rc=0"\n'
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("missing", error or "")

    def test_rejects_singularity_warning_even_with_success_sentinel(self):
        sentinel = expected_sentinel(self.fixture)
        log = (
            "warning: variance matrix is nonsymmetric or highly singular.\n"
            f"{sentinel}\n"
        )
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("forbidden", error or "")

    def test_zero_process_status_cannot_mask_missing_sentinel(self):
        def fake_runner(args, *, cwd, **kwargs):
            Path(cwd, "run.log").write_text("assertion is false\nr(9);\n", encoding="utf-8")
            return subprocess.CompletedProcess(args, 0)

        root = Path(__file__).resolve().parents[2]
        result = run_fixture("stata", root / self.fixture, root, runner=fake_runner)
        self.assertEqual(result.process_returncode, 0)
        self.assertIn("missing", result.error or "")

    def test_zero_process_status_cannot_mask_nonzero_sentinel(self):
        def fake_runner(args, *, cwd, **kwargs):
            Path(cwd, "run.log").write_text(
                "END_FIXTURE|installation_smoke|rc=9\n", encoding="utf-8"
            )
            return subprocess.CompletedProcess(args, 0)

        root = Path(__file__).resolve().parents[2]
        result = run_fixture("stata", root / self.fixture, root, runner=fake_runner)
        self.assertEqual(result.process_returncode, 0)
        self.assertIn("expected", result.error or "")

    def test_explicit_candidate_binding_never_falls_back_to_dist(self):
        observed_wrapper = ""

        def fake_runner(args, *, cwd, **kwargs):
            nonlocal observed_wrapper
            observed_wrapper = Path(cwd, "run.do").read_text(encoding="utf-8")
            Path(cwd, "run.log").write_text(
                expected_sentinel(self.fixture) + "\n", encoding="utf-8"
            )
            return subprocess.CompletedProcess(args, 0)

        root = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as raw:
            plugin = Path(raw) / expected_plugin_filename()
            plugin.write_bytes(b"exact staged candidate")
            result = run_fixture(
                "stata",
                root / self.fixture,
                root,
                runner=fake_runner,
                plugin=plugin,
                plugin_sha256=sha256_file(plugin),
            )
            self.assertIsNone(result.error)
            self.assertIn(str(plugin.resolve()), observed_wrapper)
            self.assertNotIn(str((root / "dist").resolve()), observed_wrapper)
            self.assertIn("__fereg_runner_provenance", observed_wrapper)

    def test_missing_or_wrong_candidate_binding_fails_before_stata(self):
        root = Path(__file__).resolve().parents[2]
        missing = Path(tempfile.gettempdir()) / expected_plugin_filename()
        with self.assertRaises(ValueError):
            run_fixture(
                "stata",
                root / self.fixture,
                root,
                plugin=missing,
                plugin_sha256="0" * 64,
            )
        with tempfile.TemporaryDirectory() as raw:
            plugin = Path(raw) / expected_plugin_filename()
            plugin.write_bytes(b"wrong binding")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                run_fixture(
                    "stata",
                    root / self.fixture,
                    root,
                    plugin=plugin,
                    plugin_sha256="0" * 64,
                )

    def test_staged_package_binding_excludes_source_ado_and_dist(self):
        observed_wrapper = ""

        def fake_runner(args, *, cwd, **kwargs):
            nonlocal observed_wrapper
            observed_wrapper = Path(cwd, "run.do").read_text(encoding="utf-8")
            Path(cwd, "run.log").write_text(
                expected_sentinel(self.fixture) + "\n", encoding="utf-8"
            )
            return subprocess.CompletedProcess(args, 0)

        root = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as raw:
            package = Path(raw) / "fereg-0.5.0"
            package.mkdir()
            (package / "fereg.pkg").write_text("v 3\n", encoding="utf-8")
            (package / "stata.toc").write_text("v 3\n", encoding="utf-8")
            plugin = package / expected_plugin_filename()
            plugin.write_bytes(b"exact staged package plugin")
            result = run_fixture(
                "stata",
                root / self.fixture,
                root,
                runner=fake_runner,
                plugin=plugin,
                plugin_sha256=sha256_file(plugin),
                ado_root=package,
            )
            self.assertIsNone(result.error)
            self.assertIn(f"adopath ++ \"{package.resolve()}\"", observed_wrapper)
            self.assertNotIn(f"adopath ++ \"{(root / 'stata').resolve()}\"", observed_wrapper)
            self.assertNotIn(f"adopath ++ \"{(root / 'dist').resolve()}\"", observed_wrapper)
            self.assertIn("global FEREG_STAGED_PACKAGE_ROOT", observed_wrapper)
            self.assertIn("global FEREG_STAGED_PLUGIN_NAME", observed_wrapper)

    def test_staged_package_rejects_plugin_outside_ado_root(self):
        root = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            package = temporary / "package"
            package.mkdir()
            (package / "fereg.pkg").write_text("v 3\n", encoding="utf-8")
            (package / "stata.toc").write_text("v 3\n", encoding="utf-8")
            plugin = temporary / expected_plugin_filename()
            plugin.write_bytes(b"outside")
            with self.assertRaisesRegex(ValueError, "outside"):
                run_fixture(
                    "stata",
                    root / self.fixture,
                    root,
                    plugin=plugin,
                    plugin_sha256=sha256_file(plugin),
                    ado_root=package,
                )


if __name__ == "__main__":
    unittest.main()
