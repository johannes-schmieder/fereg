from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import run_stata_break_test as runner
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

    def test_public_ols_is_marker_only_and_keeps_public_dispatchers(self):
        args = self.arguments("--fixture-kind", "ols", "--public-scope")
        fixture = Path("tests/licensed_stata/estimate_ols_public_break.do")
        self.assertEqual(fixture_command(fixture, args), f"do {stata_quote(fixture)}")
        source = Path("stata/_fereg_estimate_ols.ado")
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "adapter.do"
            self.assertEqual(runner.public_ols_adapter(source, target), runner.sha256_file(source))
            original = source.read_text().replace(
                "program define _fereg_estimate_ols, eclass",
                "program define __fereg_break_transaction, eclass", 1)
            generated = target.read_text()
            self.assertTrue(generated.startswith(original))
            forwarder = generated[len(original):]
            self.assertIn("breakmarkertoken(`token')", forwarder)
            self.assertNotIn("inject", forwarder)
            self.assertNotIn("program define fereg", forwarder)
            self.assertNotIn("program define _fereg_estimate_public", forwarder)
            with self.assertRaises(FileExistsError):
                runner.public_ols_adapter(source, target)
            bad = Path(directory) / "bad.ado"
            bad.write_text("program define something_else, eclass\nend\n")
            with self.assertRaisesRegex(RuntimeError, "anchor changed"):
                runner.public_ols_adapter(bad, Path(directory) / "bad.do")
        text = fixture.read_text()
        self.assertIn("capture noisily fereg y c.x1", text)
        self.assertIn("solver(auto) preconditioner(auto)", text)
        self.assertNotIn("inject(", text)
        self.assertNotIn("clear all", text)
        for extra in (["--public-scope", "--ols-solver", "cmgschur"],
                      ["--fixture-kind", "ols-builder", "--public-scope"]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                self.arguments(*extra)

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

    def test_actual_builder_fixture_and_readiness_guard(self):
        fixture = Path("tests/licensed_stata/estimate_ols_builder_break.do")
        for workers in (2, 4):
            args = self.arguments("--fixture-kind", "ols-builder", "--builder-workers", str(workers))
            self.assertEqual(fixture_command(fixture, args), f"do {stata_quote(fixture)} {workers}")
        event = ("FEREG_SOLVER_TRACE|phase=routing_builder|event=builder_break_ready|"
                 "task=0|ordinal=2")
        self.assertEqual(builder_trace_events(event + "\n" + event), [event, event])
        first = event.replace("ordinal=2", "ordinal=0")
        self.assertEqual(builder_trace_events(first + "\n" + first), [first, first])
        for bad in ("", event, "\n".join([event] * 3),
                    event.replace("ordinal=2", "ordinal=oops"),
                    event.replace("ordinal=2", "ordinal=-1"),
                    event.replace("task=0", "task=1"),
                    event.replace("task=0", "task=-1"),
                    event.replace("phase=routing_builder", "phase=routing_overlap"),
                    event + "|issued=1024|charged=32"):
            with self.assertRaises(RuntimeError):
                builder_trace_events(bad)
        for extra in (["--builder-workers", "2"],
                      ["--fixture-kind", "ols-builder", "--ols-solver", "cmgschur"],
                      ["--fixture-kind", "ols-builder", "--builder-workers", "1"]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                self.arguments(*extra)

    def test_merged_append_records_cannot_pass_exact_event_check(self):
        launch = "FEREG_SOLVER_TRACE|phase=auto_schur_route|checkpoint=4|samples=1"
        ready = ("FEREG_SOLVER_TRACE|phase=routing_builder|event=builder_break_ready|"
                 "task=0|ordinal=1")
        with self.assertRaisesRegex(RuntimeError, "invalid synchronous builder"):
            builder_trace_events(launch + ready + "\n\n" + ready + "\n")
        self.assertEqual(builder_trace_events(launch + "\n" + ready + "\n" + ready + "\n"),
                         [ready, ready])
        for extra in (ready + "|ordinal=1", ready + "|extra=1", ready.rsplit("|", 1)[0],
                      "prefix" + ready):
            with self.subTest(extra=extra), self.assertRaises(RuntimeError):
                builder_trace_events(ready + "\n" + ready + "\n" + extra)

    def test_failure_capture_distinguishes_raw_empty_missing_and_unreadable(self):
        for state in ("present", "empty", "missing", "unreadable"):
            with self.subTest(state=state), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                trace = root / "builder.trace"
                raw = b"malformed\xff\n" if state == "present" else b""
                if state in ("present", "empty"):
                    trace.write_bytes(raw)
                elif state == "unreadable":
                    trace.mkdir()
                prefix = root / "saved"
                failure = RuntimeError("expected two synchronous builder Break readiness records")
                errors = runner.preserve_evidence(prefix, runner.read_trace(trace), b"PTY\xff", {"plugin_sha256": "a" * 64},
                                                   failure, [])
                record = json.loads((root / "saved.json").read_text())
                self.assertEqual(record["status"], "failed")
                self.assertEqual(record["trace_status"], state)
                self.assertIn(str(failure), record["error"])
                self.assertEqual(record["plugin_sha256"], "a" * 64)
                self.assertEqual((root / "saved.pty").read_bytes(), b"PTY\xff")
                self.assertEqual(bool(errors), state == "unreadable")
                if state in ("present", "empty"):
                    self.assertEqual((root / "saved.trace").read_bytes(), raw)
                else:
                    self.assertFalse((root / "saved.trace").exists())

    def test_capture_failure_cannot_prevent_child_reaping_or_temporary_cleanup(self):
        process = mock.Mock()
        process.poll.return_value = None
        process.wait.side_effect = [runner.subprocess.TimeoutExpired("stata", 5), 0]
        temporary = mock.Mock()
        failure = RuntimeError("original validation failure")
        with mock.patch.object(runner.select, "select", return_value=([], [], [])), \
                mock.patch.object(runner.os, "close") as close, \
                mock.patch.object(runner, "preserve_evidence", side_effect=OSError("disk full")):
            errors = runner.finish_attempt(process, 17, temporary, Path("prefix"), Path("trace"),
                                           bytearray(b"PTY"), {}, failure)
        process.terminate.assert_called_once()
        process.kill.assert_called_once()
        self.assertEqual(process.wait.call_count, 2)
        temporary.cleanup.assert_called_once()
        close.assert_called_once_with(17)
        self.assertEqual(errors, ["capture evidence: disk full"])
        self.assertEqual(str(failure), "original validation failure")

    def test_cleanup_errors_and_bounded_drain_are_reflected_in_saved_status(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            temporary = mock.Mock()
            temporary.cleanup.side_effect = OSError("cleanup refused")
            with mock.patch.object(runner.select, "select", return_value=([17], [], [])), \
                    mock.patch.object(runner.os, "read", return_value=b"x"), \
                    mock.patch.object(runner.os, "close", side_effect=OSError("close refused")):
                errors = runner.finish_attempt(None, 17, temporary, root / "saved", root / "missing",
                                               bytearray(), {}, None, ["slave close refused"])
            record = json.loads((root / "saved.json").read_text())
            self.assertEqual(record["status"], "failed")
            self.assertEqual(record["cleanup_errors"], errors)
            self.assertEqual(len(errors), 4)
            self.assertEqual((root / "saved.pty").read_bytes(), b"x" * 64)

    def test_popen_failure_retains_original_error_and_closes_resources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            args = self.arguments("--evidence-prefix", str(root / "saved"))
            real_close = runner.os.close
            with mock.patch.object(runner, "parse_arguments", return_value=args), \
                    mock.patch.object(runner, "validate_candidate_plugin", return_value=root / "plugin"), \
                    mock.patch.object(runner.pty, "openpty", return_value=(17, 18)), \
                    mock.patch.object(runner.os, "close", side_effect=lambda fd:
                                      None if fd in (17, 18) else real_close(fd)) as close, \
                    mock.patch.object(runner.select, "select", return_value=([], [], [])), \
                    mock.patch.object(runner.subprocess, "Popen", side_effect=OSError("cannot launch")), \
                    contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()) as output:
                with self.assertRaisesRegex(OSError, "cannot launch"):
                    runner.main()
            self.assertEqual(close.call_args_list[:2], [mock.call(18), mock.call(17)])
            record = json.loads((root / "saved.json").read_text())
            self.assertEqual(record["status"], "failed")
            self.assertEqual(record["trace_status"], "missing")
            self.assertIn("cannot launch", record["error"])
            self.assertNotIn("PASS:", output.getvalue())

    def test_native_marker_attempt_preserves_trace_on_pass_and_validation_failure(self):
        event = ("FEREG_SOLVER_TRACE|phase=routing_builder|event=builder_break_ready|"
                 "task=0|ordinal=1\n")
        for valid in (False, True):
            with self.subTest(valid=valid), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                plugin = root / "plugin"
                plugin.write_bytes(b"exact fake binary")
                args = self.arguments("--fixture-kind", "ols-builder", "--builder-workers", "4",
                                      "--evidence-prefix", str(root / "saved"))
                args.plugin_sha256 = runner.sha256_file(plugin)
                process = mock.Mock(pid=1234, returncode=0)
                process.poll.return_value = 0
                process.wait.return_value = 0

                def launch(*_args, **kwargs):
                    Path(kwargs["env"]["FEREG_SOLVER_TRACE_FILE"]).write_text(event * (2 if valid else 1))
                    return process

                def read_marker(_master, _process, path, expected, _timeout, capture):
                    path.write_bytes(expected)
                    capture.extend(b"native marker observed\n")
                    return "native marker observed\n"

                def read_until(_master, _process, needle, _timeout, capture):
                    text = ". " if needle == ". " else (
                        "FEREG_BUILDER_BREAK_RESET_VERIFIED\n" + needle + "\n")
                    capture.extend(text.encode())
                    return text

                real_close = runner.os.close
                with mock.patch.object(runner, "parse_arguments", return_value=args), \
                        mock.patch.object(runner, "validate_candidate_plugin", return_value=plugin), \
                        mock.patch.object(runner.pty, "openpty", return_value=(17, 18)), \
                        mock.patch.object(runner.os, "close", side_effect=lambda fd:
                                          None if fd in (17, 18) else real_close(fd)), \
                        mock.patch.object(runner.os, "write"), \
                        mock.patch.object(runner.select, "select", return_value=([], [], [])), \
                        mock.patch.object(runner.subprocess, "Popen", side_effect=launch), \
                        mock.patch.object(runner, "read_until", side_effect=read_until), \
                        mock.patch.object(runner, "read_until_marker_file", side_effect=read_marker) as marker, \
                        contextlib.redirect_stderr(io.StringIO()), \
                        contextlib.redirect_stdout(io.StringIO()) as output:
                    if valid:
                        runner.main()
                    else:
                        with self.assertRaisesRegex(RuntimeError, "expected two"):
                            runner.main()
                self.assertEqual(marker.call_count, 2)
                record = json.loads((root / "saved.json").read_text())
                self.assertEqual(record["status"], "passed" if valid else "failed")
                self.assertEqual(record["plugin_sha256"], args.plugin_sha256)
                self.assertEqual(record["process_returncode"], 0)
                self.assertEqual((root / "saved.trace").read_text(), event * (2 if valid else 1))
                self.assertEqual("PASS:" in output.getvalue(), valid)
                process.wait.assert_called_once_with(timeout=5)

    def test_timeout_retains_partial_pty_bytes(self):
        process = mock.Mock()
        capture = bytearray()
        with mock.patch.object(runner.time, "monotonic", side_effect=[0, 0, 0, 2]), \
                mock.patch.object(runner.select, "select", return_value=([17], [], [])), \
                mock.patch.object(runner.os, "read", return_value=b"partial output"):
            with self.assertRaisesRegex(RuntimeError, "partial output"):
                runner.read_until(17, process, "missing marker", 1, capture)
        self.assertEqual(capture, b"partial output")


if __name__ == "__main__":
    unittest.main()
