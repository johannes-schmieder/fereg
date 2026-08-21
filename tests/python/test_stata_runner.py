from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "run_stata_tests", ROOT / "scripts/run_stata_tests.py"
)
assert SPEC is not None and SPEC.loader is not None
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class StataRunnerSentinelTests(unittest.TestCase):
    def test_success_requires_exact_matching_zero_sentinel(self):
        self.assertIsNone(
            RUNNER.validate_fixture_log(
                "ordinary output\nEND_FIXTURE|sample|rc=0\n", "sample"
            )
        )

    def test_missing_sentinel_fails_even_if_log_looks_successful(self):
        failure = RUNNER.validate_fixture_log("sample.do: PASS\n", "sample")
        self.assertIn("missing", failure)

    def test_nonzero_wrong_name_and_duplicates_fail(self):
        self.assertIn(
            "rc=111",
            RUNNER.validate_fixture_log("END_FIXTURE|sample|rc=111\n", "sample"),
        )
        self.assertIn(
            "expected",
            RUNNER.validate_fixture_log("END_FIXTURE|other|rc=0\n", "sample"),
        )
        duplicate = "END_FIXTURE|sample|rc=0\nEND_FIXTURE|sample|rc=0\n"
        self.assertIn("found 2", RUNNER.validate_fixture_log(duplicate, "sample"))


if __name__ == "__main__":
    unittest.main()
