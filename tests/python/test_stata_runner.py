from __future__ import annotations

import json
import platform
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.run_stata_tests import (
    NOT_RUN,
    REQUIREMENTS,
    expected_plugin_filename,
    expected_sentinel,
    fixture_requirements,
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
            self.assertIn("plugin call __fereg_runner_provenance, selftest", observed_wrapper)
            self.assertIn('"ado=0.6.0" "protocol_major=1" "protocol_minor=7"', observed_wrapper)
            self.assertIn('"threads=1" "verbose=1"', observed_wrapper)
            self.assertNotIn("__fereg_runner_provenance, version", observed_wrapper)

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
            package = Path(raw) / "fereg-0.6.0"
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


class FixtureRequirementTests(unittest.TestCase):
    root = Path(__file__).resolve().parents[2]

    def write_fixture(self, directory: Path, name: str, requirement: str) -> Path:
        path = directory / f"{name}.do"
        path.write_text(
            f"version 18.0\n* fixture-requires: {requirement}\n"
            f'display "END_FIXTURE|{name}|rc=0"\n',
            encoding="utf-8",
        )
        return path

    def test_unknown_and_repeated_requirements_fail_clearly(self):
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "fixture.do"
            for text in ("* fixture-requires: network\n",
                         "* fixture-requires: windows\n* fixture-requires: windows\n"):
                path.write_text(text, encoding="utf-8")
                with self.subTest(text=text), self.assertRaisesRegex(ValueError, "requirement"):
                    fixture_requirements(path)

    def test_repository_fixtures_declare_their_inputs(self):
        declared = {
            "tests/stata/installation/staged_plugin_binary.do": ("staged-package",),
            "tests/stata/installation/feppml_installation_smoke.do": ("staged-package",),
            "tests/stata/installation/windows_ci_smoke.do": ("windows",),
            "tests/stata/development/parallel_input_oracle.do": ("input-trace",),
        }
        for path in sorted((self.root / "tests/stata").rglob("*.do")):
            relative = path.relative_to(self.root).as_posix()
            with self.subTest(fixture=relative):
                self.assertEqual(fixture_requirements(path), declared.get(relative, ()))

    def test_an_unmet_input_is_not_run_and_never_launches_stata(self):
        def refusing_runner(*args, **kwargs):
            raise AssertionError("an unmet fixture must not start Stata")

        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw).resolve()
            staged = self.write_fixture(directory, "needs_package", "staged-package")
            result = run_fixture("stata", staged, directory, runner=refusing_runner)
            self.assertEqual(result.failure_kind, NOT_RUN)
            self.assertEqual(result.error, f"not run: needs {REQUIREMENTS['staged-package']}")
            self.assertIsNone(result.stata_rc)
            if platform.system() != "Windows":
                windows = self.write_fixture(directory, "needs_windows", "windows")
                result = run_fixture("stata", windows, directory, runner=refusing_runner)
                self.assertEqual(result.failure_kind, NOT_RUN)
                self.assertIn("windows_ci_runtime.do", result.error or "")

    def test_input_trace_is_provided_and_decides_the_verdict(self):
        name = "trace_fixture"
        observed: dict[str, str] = {}

        def tracing_runner(trace_lines):
            def runner(args, *, cwd, env, **kwargs):
                observed.update(env or {})
                Path(cwd, "run.log").write_text(
                    f"END_FIXTURE|{name}|rc=0\n", encoding="utf-8")
                Path(cwd, "fixture.status").write_text(
                    "completed=1\nstata_rc=0\n", encoding="utf-8")
                Path(env["FEREG_SOLVER_TRACE_FILE"]).write_text(
                    "".join(f"FEREG_SOLVER_TRACE|{line}\n" for line in trace_lines),
                    encoding="utf-8")
                return subprocess.CompletedProcess(args, 0)
            return runner

        agreeing = ["phase=case|label=ols_a|serial", "phase=input_digest|digest=d1",
                    "phase=case|label=ols_a|parallel", "phase=input_transport|mode=parallel",
                    "phase=input_digest|digest=d1"]
        disagreeing = [*agreeing[:-1], "phase=input_digest|digest=d2"]
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw).resolve()
            (directory / "scripts").mkdir()
            shutil.copy2(self.root / "scripts/check_input_trace.py", directory / "scripts")
            fixture = self.write_fixture(directory, name, "input-trace")
            logs = directory / "logs"
            result = run_fixture("stata", fixture, directory, runner=tracing_runner(agreeing),
                                 log_dir=logs, require_status=True)
            self.assertIsNone(result.error)
            self.assertEqual(observed["FEREG_SOLVER_TRACE"], "1")
            self.assertIn("cases=1 runs=2 parallel_runs=1",
                          (logs / f"{name}.do.trace-check.txt").read_text(encoding="utf-8"))
            invocation = json.loads((logs / f"{name}.do.invocation.json").read_text())
            self.assertEqual(set(invocation["environment"]),
                             {"FEREG_SOLVER_TRACE", "FEREG_SOLVER_TRACE_FILE"})
            result = run_fixture("stata", fixture, directory,
                                 runner=tracing_runner(disagreeing), require_status=True)
            self.assertEqual(result.failure_kind, "input_trace")
            self.assertIn("distinct input digests", result.error or "")


if __name__ == "__main__":
    unittest.main()
