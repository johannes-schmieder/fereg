from __future__ import annotations

import contextlib
import io
from pathlib import Path
import unittest

from scripts.run_stata_break_test import builder_trace_events, fixture_command, parse_arguments, stata_quote


class StataBreakOptionsTests(unittest.TestCase):
    def arguments(self, *extra: str):
        return parse_arguments([
            "--stata", "unused-stata",
            "--plugin", "unused.plugin",
            "--plugin-sha256", "0" * 64,
            *extra,
        ])

    def test_defaults_preserve_existing_fixture_commands(self):
        fixture = Path("tests/licensed_stata/estimate_ols_eclass_break.do")
        self.assertEqual(fixture_command(fixture, self.arguments()),
                         f"do {stata_quote(fixture)} auto")
        self.assertEqual(fixture_command(fixture, self.arguments("--ols-solver", "cmgschur")),
                         f"do {stata_quote(fixture)} cmgschur")
        ppml = Path("tests/licensed_stata/estimate_ppml_eclass_break.do")
        self.assertEqual(fixture_command(ppml, self.arguments("--fixture-kind", "ppml")),
                         f"do {stata_quote(ppml)}")
        self.assertEqual(fixture_command(ppml, self.arguments("--fixture-kind", "ppml", "--public-scope")),
                         f"do {stata_quote(ppml)} public")

    def test_sampling_integers_reach_fixture_without_float_rounding(self):
        fixture = Path("tests/licensed_stata/estimate_ols_eclass_break.do")
        for count, seed in [(0, None), (2, None), (2, 0), (2**32 - 1, 2**53 - 1)]:
            with self.subTest(count=count, seed=seed):
                extra = ["--ols-solver", "cmgschur", "--cmg-samples", str(count)]
                if seed is not None:
                    extra.extend(["--cmg-seed", str(seed)])
                args = self.arguments(*extra)
                suffix = f" cmgschur {count}" + (f" {seed}" if seed is not None else "")
                self.assertEqual(fixture_command(fixture, args), f"do {stata_quote(fixture)}{suffix}")

    def test_invalid_sampling_requests_fail_before_stata(self):
        requests = [
            ["--cmg-samples", "0"],
            ["--fixture-kind", "ppml", "--cmg-samples", "2"],
            ["--ols-solver", "cmgschur", "--cmg-seed", "0"],
            ["--ols-solver", "cmgschur", "--cmg-samples", "0", "--cmg-seed", "1"],
        ]
        for value in ["-1", "1.5", "4294967296", "NaN"]:
            requests.append(["--ols-solver", "cmgschur", "--cmg-samples", value])
        for value in ["-1", "1.5", "9007199254740990.5", "9007199254740992", "NaN"]:
            requests.append(["--ols-solver", "cmgschur", "--cmg-samples", "2", "--cmg-seed", value])
        for extra in requests:
            with self.subTest(extra=extra), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    self.arguments(*extra)
                self.assertEqual(raised.exception.code, 2)

    def test_actual_builder_fixture_and_positive_work_guard(self):
        fixture = Path("tests/licensed_stata/estimate_ols_builder_break.do")
        for workers in (2, 4):
            args = self.arguments("--fixture-kind", "ols-builder", "--builder-workers", str(workers))
            self.assertEqual(fixture_command(fixture, args), f"do {stata_quote(fixture)} {workers}")
        event = ("FEREG_SOLVER_TRACE|phase=routing_overlap|event=builder_break_ready|"
                 "task=0|issued=1024|charged=32|ordinal=2")
        self.assertEqual(builder_trace_events(event + "\n" + event), [event, event])
        for bad in ("", event, event.replace("charged=32", "charged=0"),
                    event.replace("charged=32", "charged=2048"), event.replace("ordinal=2", "ordinal=0")):
            with self.assertRaises(RuntimeError):
                builder_trace_events(bad)
        for extra in (["--builder-workers", "2"],
                      ["--fixture-kind", "ols-builder", "--ols-solver", "cmgschur"],
                      ["--fixture-kind", "ols-builder", "--builder-workers", "1"]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                self.arguments(*extra)


if __name__ == "__main__":
    unittest.main()
