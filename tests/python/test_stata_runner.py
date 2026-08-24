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
        self.fixture = Path("tests/stata/installation/phase01_smoke.do")

    def test_accepts_exact_terminal_success_sentinel(self):
        sentinel = expected_sentinel(self.fixture)
        self.assertIsNone(validate_fixture_log(self.fixture, f"noise\n{sentinel}\n"))

    def test_rejects_missing_sentinel_even_when_log_looks_successful(self):
        error = validate_fixture_log(self.fixture, "phase01_smoke.do: PASS\n")
        self.assertIn("missing", error or "")

    def test_rejects_nonzero_sentinel(self):
        error = validate_fixture_log(self.fixture, "END_FIXTURE|phase01_smoke|rc=9\n")
        self.assertIn("expected", error or "")

    def test_rejects_success_followed_by_failure_sentinel(self):
        log = "END_FIXTURE|phase01_smoke|rc=0\nEND_FIXTURE|phase01_smoke|rc=9\n"
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("exactly one", error or "")

    def test_rejects_wrong_sentinel_followed_by_expected(self):
        log = "END_FIXTURE|other_fixture|rc=0\nEND_FIXTURE|phase01_smoke|rc=0\n"
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("exactly one", error or "")

    def test_rejects_expected_sentinel_followed_by_malformed(self):
        log = "END_FIXTURE|phase01_smoke|rc=0\nEND_FIXTURE|phase01_smoke|success\n"
        error = validate_fixture_log(self.fixture, log)
        self.assertIn("exactly one", error or "")

    def test_command_echo_does_not_count_as_sentinel_output(self):
        log = '. di as result "END_FIXTURE|phase01_smoke|rc=0"\n'
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
                "END_FIXTURE|phase01_smoke|rc=9\n", encoding="utf-8"
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


if __name__ == "__main__":
    unittest.main()
