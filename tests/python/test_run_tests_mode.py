from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PACKAGE_GATE = "python3 scripts/check_github_install.py"
AUDIT_PREFIX = "cargo test -p fereg-plugin --features ffi --lib --profile ci"
EXPECTED = [
    (".", "python3 -m unittest discover -s tests/python"),
    (".", PACKAGE_GATE),
    (".", "python3 scripts/check_fma_rounding_vectors.py"),
    (".", "python3 scripts/check_fe_boundary_response_oracle.py"),
    (".", "python3 scripts/check_fe_cancellation_oracle.py"),
    (".", "cargo fmt --all -- --check"),
    ("fuzz", "cargo fmt -- --check"),
    ("fuzz", "cargo check --locked --quiet"),
    (".", "cargo clippy --workspace --all-targets --no-default-features --profile ci -- -D warnings"),
    (".", "cargo clippy -p fereg-plugin --all-targets --features ffi --profile ci -- -D warnings"),
    (".", "cargo test --workspace --all-targets --no-default-features --profile ci"),
    (".", AUDIT_PREFIX + " -- --test-threads=1"),
    (".", AUDIT_PREFIX + " -- --ignored --list"),
    (".", AUDIT_PREFIX + " allocation::first -- --ignored --exact --test-threads=1 --show-output"),
    (".", AUDIT_PREFIX + " allocation::second -- --ignored --exact --test-threads=1 --show-output"),
    (".", "bash scripts/run_portable_static_checks.sh"),
    (".", "bash scripts/check_sizes.sh"),
    (".", "run_mock_bridge_test.sh"),
    (".", "bash tests/mock_spi/run_numeric_batch_guard_test.sh"),
    (".", "uname -s"),
    (".", "run_mock_linux_plugin_audit.sh"),
]


class RunTestsModeTests(unittest.TestCase):
    def setUp(self):
        task_output = ROOT / "target/tmp/solver-routing"
        task_output.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=task_output)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for directory in ("bin", "scripts", "fuzz", "tests/mock_spi"):
            (self.root / directory).mkdir(parents=True)
        self.runner = self.root / "scripts/run_tests.sh"
        shutil.copyfile(ROOT / "scripts/run_tests.sh", self.runner)
        (self.root / "scripts/check_sizes.sh").touch()
        self.log = self.root / "commands.jsonl"
        stub = f"#!{sys.executable}\n" + '''import json, os, pathlib, shlex, sys
name = pathlib.Path(sys.argv[0]).name
command = shlex.join([name, *sys.argv[1:]])
cwd = str(pathlib.Path.cwd().relative_to(os.environ["MOCK_ROOT"]))
with open(os.environ["MOCK_LOG"], "a") as output:
    output.write(json.dumps([cwd, command]) + "\\n")
if command == os.environ.get("MOCK_FAIL"):
    sys.exit(37)
if name == "cargo" and sys.argv[-3:] == ["--", "--ignored", "--list"]:
    print("allocation::first: test\\nallocation::second: test\\n2 tests, 0 benchmarks")
elif name == "uname":
    print(os.environ["MOCK_OS"])
'''
        for name in ("bin/python3", "bin/cargo", "bin/bash", "bin/uname",
                     "tests/mock_spi/run_mock_bridge_test.sh",
                     "tests/mock_spi/run_mock_linux_plugin_audit.sh"):
            path = self.root / name
            path.write_text(stub)
            path.chmod(0o755)

    def run_checks(self, *args, fail=None, platform="Linux"):
        self.log.unlink(missing_ok=True)
        env = dict(os.environ, PATH=f"{self.root / 'bin'}:/usr/bin:/bin",
                   FEREG_TEST_PROFILE="ci", MOCK_ROOT=str(self.root),
                   MOCK_LOG=str(self.log), MOCK_OS=platform, MOCK_FAIL=fail or "")
        result = subprocess.run(["/bin/bash", str(self.runner), *args],
                                cwd=self.root, env=env, text=True,
                                capture_output=True, timeout=30)
        commands = []
        if self.log.exists():
            commands = [tuple(json.loads(line)) for line in self.log.read_text().splitlines()]
        return result, commands

    def test_default_runs_the_strict_package_gate_and_all_checks_in_order(self):
        result, commands = self.run_checks()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands, EXPECTED)
        self.assertNotIn("SOURCE-ONLY", result.stdout + result.stderr)

    def test_source_only_omits_exactly_the_package_gate_and_labels_the_gap(self):
        result, commands = self.run_checks("--source-only")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands, [item for item in EXPECTED if item[1] != PACKAGE_GATE])
        self.assertIn("SOURCE-ONLY: NOT PERFORMED", result.stdout)
        self.assertIn("installed-package binding, inventory, hash, ABI and Mata", result.stdout)
        self.assertIn("does not qualify a package for release or promotion", result.stdout)
        self.assertTrue(result.stdout.endswith(
            "SOURCE-ONLY: source integration PASS; installed-package admission NOT PERFORMED; "
            "release/promotion blocked.\n"))

    def test_unknown_repeated_and_combined_arguments_fail_before_checks(self):
        for args in (("--help",), ("",), ("--source-only=yes",),
                     ("--source-only --other",), ("--source-only", "--source-only"),
                     ("--source-only", "--other"), ("--other", "--source-only")):
            with self.subTest(args=args):
                result, commands = self.run_checks(*args)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(commands, [])
                self.assertIn("Usage:", result.stderr)
                self.assertNotIn("SOURCE-ONLY", result.stdout)

    def test_default_package_failure_cannot_fall_back_to_source_only(self):
        result, commands = self.run_checks(fail=PACKAGE_GATE)
        self.assertEqual(result.returncode, 37)
        self.assertEqual(commands, EXPECTED[:2])
        self.assertNotIn("SOURCE-ONLY", result.stdout + result.stderr)

    def test_source_only_still_propagates_math_and_command_failures(self):
        expected = [item for item in EXPECTED if item[1] != PACKAGE_GATE]
        for failure in ("python3 scripts/check_fe_boundary_response_oracle.py",
                        "cargo check --locked --quiet",
                        AUDIT_PREFIX + " allocation::second -- --ignored --exact --test-threads=1 --show-output",
                        "bash scripts/run_portable_static_checks.sh",
                        "run_mock_linux_plugin_audit.sh"):
            with self.subTest(failure=failure):
                result, commands = self.run_checks("--source-only", fail=failure)
                self.assertEqual(result.returncode, 37, result.stderr)
                stop = next(i for i, item in enumerate(expected) if item[1] == failure)
                self.assertEqual(commands, expected[:stop + 1])
                self.assertNotIn("source integration PASS", result.stdout)

    def test_source_only_preserves_the_non_linux_audit_skip(self):
        result, commands = self.run_checks("--source-only", platform="Darwin")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(commands, [item for item in EXPECTED[:-1] if item[1] != PACKAGE_GATE])
        self.assertIn("SKIP: Linux mock-plugin audit requires a GNU ELF linker.", result.stdout)


if __name__ == "__main__":
    unittest.main()
