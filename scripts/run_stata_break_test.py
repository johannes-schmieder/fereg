#!/usr/bin/env python3
"""Drive the licensed hidden eclass Break fixture through an interactive PTY."""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import pty
import select
import secrets
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

try:
    from .run_stata_tests import sha256_file, validate_candidate_plugin, validate_fixture_log
except ImportError:
    from run_stata_tests import sha256_file, validate_candidate_plugin, validate_fixture_log

BREAK_FIXTURES = {
    "ols": ("estimate_ols_eclass_break.do", ("FEREG_NATIVE_BREAK_ARMED", "FEREG_BREAK_ARMED")),
    "ols-builder": ("estimate_ols_builder_break.do", ("FEREG_NATIVE_BREAK_ARMED", "FEREG_NATIVE_BREAK_ARMED")),
    "ppml": (
        "estimate_ppml_eclass_break.do",
        (
            "FEPPML_NATIVE_BREAK_ARMED",
            "FEPPML_BREAK_ARMED",
            "FEPPML_NATIVE_BREAK_ARMED",
            "FEPPML_BREAK_ARMED",
            "FEPPML_PREDICT_BREAK_ARMED",
        ),
    ),
}


def stata_quote(path: Path) -> str:
    text = str(path.resolve()).replace("\\", "/")
    if '"' in text or "\n" in text or "\r" in text:
        raise ValueError(f"Stata path contains an unsupported character: {text}")
    return f'"{text}"'


def read_until(master: int, process: subprocess.Popen[bytes], needle: str, timeout: float,
               capture: bytearray | None = None) -> str:
    deadline = time.monotonic() + timeout
    chunks: list[bytes] = []
    encoded = needle.encode("utf-8")
    while time.monotonic() < deadline:
        remaining = max(0.0, deadline - time.monotonic())
        ready, _, _ = select.select([master], [], [], min(0.25, remaining))
        if ready:
            chunk = os.read(master, 65_536)
            if capture is not None:
                capture.extend(chunk)
            if not chunk:
                break
            chunks.append(chunk)
            if encoded in b"".join(chunks):
                return b"".join(chunks).decode("utf-8", errors="replace")
        elif process.poll() is not None:
            break
    transcript = b"".join(chunks).decode("utf-8", errors="replace")
    raise RuntimeError(
        f"Stata did not emit {needle!r} before timeout/process exit; transcript:\n{transcript}"
    )


def read_until_marker_file(
    master: int,
    process: subprocess.Popen[bytes],
    marker_path: Path,
    expected: bytes,
    timeout: float,
    capture: bytearray | None = None,
) -> str:
    deadline = time.monotonic() + timeout
    chunks: list[bytes] = []
    while time.monotonic() < deadline:
        if marker_path.exists():
            contents = marker_path.read_bytes()
            if contents != expected:
                raise RuntimeError(
                    f"native Break marker {marker_path} had invalid contents {contents!r}"
                )
            return b"".join(chunks).decode("utf-8", errors="replace")
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            chunk = os.read(master, 65_536)
            if capture is not None:
                capture.extend(chunk)
            if not chunk:
                break
            chunks.append(chunk)
        elif process.poll() is not None:
            break
    transcript = b"".join(chunks).decode("utf-8", errors="replace")
    raise RuntimeError(
        f"Stata did not create native Break marker {marker_path} before "
        f"timeout/process exit; transcript:\n{transcript}"
    )


def bounded_integer(text: str, maximum: int) -> int:
    if not text.isascii() or not text.isdecimal() or int(text) > maximum:
        raise argparse.ArgumentTypeError(f"must be decimal digits in 0..{maximum}")
    return int(text)


def parse_arguments(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--plugin-sha256", required=True)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--evidence-prefix", type=Path,
                        help="write .json, .pty and any raw .trace here on success or failure; "
                             "defaults to a unique prefix under target/tmp/stata-break")
    parser.add_argument("--fixture-kind", choices=sorted(BREAK_FIXTURES), default="ols")
    parser.add_argument("--ols-solver", choices=("auto", "cmgschur"), default="auto",
                        help="OLS route exercised by both native and publication Break checks")
    parser.add_argument("--builder-workers", type=int, choices=(2, 4),
                        help="worker count for the actual optional-builder Break fixture (default 4)")
    parser.add_argument("--cmg-samples", type=lambda value: bounded_integer(value, 2**32 - 1),
                        help="optional CMG-Schur tree count; positive counts use a degree-30 fixture")
    parser.add_argument("--cmg-seed", type=lambda value: bounded_integer(value, 2**53 - 1),
                        help="optional CMG-Schur seed; requires positive --cmg-samples")
    parser.add_argument("--public-scope", action="store_true",
                        help="exercise public OLS marker-only Break or feppml's outer injection scope")
    args = parser.parse_args(argv)
    if args.public_scope and args.fixture_kind not in ("ols", "ppml"):
        parser.error("--public-scope requires --fixture-kind ols or ppml")
    if args.public_scope and args.fixture_kind == "ols" and args.ols_solver != "auto":
        parser.error("public OLS Break requires --ols-solver auto")
    if args.builder_workers is not None and args.fixture_kind != "ols-builder":
        parser.error("--builder-workers requires --fixture-kind ols-builder")
    if args.ols_solver != "auto" and args.fixture_kind != "ols":
        parser.error("--ols-solver requires --fixture-kind ols")
    if args.cmg_samples is not None or args.cmg_seed is not None:
        if args.fixture_kind != "ols" or args.ols_solver != "cmgschur":
            parser.error("--cmg-samples and --cmg-seed require --fixture-kind ols --ols-solver cmgschur")
    if args.cmg_seed is not None and not args.cmg_samples:
        parser.error("--cmg-seed requires positive --cmg-samples")
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    return args


def fixture_command(fixture: Path, args: argparse.Namespace) -> str:
    arguments: list[str] = []
    if args.fixture_kind == "ols-builder":
        arguments.append(str(args.builder_workers or 4))
    elif args.fixture_kind == "ols" and not args.public_scope:
        arguments.append(args.ols_solver)
        if args.cmg_samples is not None:
            arguments.append(str(args.cmg_samples))
            if args.cmg_seed is not None:
                arguments.append(str(args.cmg_seed))
    elif args.public_scope and args.fixture_kind == "ppml":
        arguments.append("public")
    suffix = " " + " ".join(arguments) if arguments else ""
    return f"do {stata_quote(fixture)}{suffix}"


def public_ols_adapter(source: Path, target: Path) -> str:
    """Keep the public dispatchers and real transaction; add only a test token."""
    text = source.read_text()
    original = "program define _fereg_estimate_ols, eclass"
    renamed = "program define __fereg_break_transaction, eclass"
    if text.count(original) != 1 or renamed in text:
        raise RuntimeError("public OLS Break transaction anchor changed")
    with target.open("x") as output:
        output.write(text.replace(original, renamed, 1))
        output.write('''
program define _fereg_estimate_ols, eclass
    version 18.0
    local marker_option
    if "$FEREG_PUBLIC_BREAK_ARMED" == "1" {
        local token : environment FEREG_BREAK_MARKER_TOKEN
        assert strlen("`token'") == 32
        local marker_option "breakmarkertoken(`token')"
    }
    __fereg_break_transaction `0' `marker_option'
end
''')
    return sha256_file(source)


def builder_trace_events(trace: str) -> list[str]:
    """Return the synchronous builder's readiness records.

    Each record is emitted at the builder's first cancellation poll after
    executed construction work: one task (0) and its zero-based poll ordinal.
    """
    events = []
    for line in trace.splitlines():
        if "event=builder_break_ready" not in line:
            continue
        try:
            if not line.startswith("FEREG_SOLVER_TRACE|phase=routing_builder|event=builder_break_ready|"):
                raise ValueError("fragmented builder record")
            pairs = [part.split("=", 1) for part in line.split("|")[1:]]
            fields = dict(pairs)
            if len(pairs) != len(fields) or set(fields) != {"phase", "event", "task", "ordinal"}:
                raise ValueError("invalid builder record fields")
            if fields["task"] != "0" or not fields["ordinal"].isdigit():
                raise ValueError("invalid builder readiness point")
        except (ValueError, KeyError) as error:
            raise RuntimeError("invalid synchronous builder Break trace") from error
        events.append(line)
    if len(events) != 2:
        raise RuntimeError("expected two synchronous builder Break readiness records")
    return events


def read_trace(trace_path: Path) -> tuple[str, bytes | None, str | None]:
    try:
        raw = trace_path.read_bytes()
        return ("present" if raw else "empty", raw, None)
    except FileNotFoundError:
        return ("missing", None, None)
    except OSError as error:
        return ("unreadable", None, f"read trace: {error}")


def preserve_evidence(prefix: Path, trace: tuple[str, bytes | None, str | None], transcript: bytes,
                      metadata: dict, failure: BaseException | None,
                      cleanup_errors: list[str]) -> list[str]:
    """Best-effort capture; a capture error must never replace the original error."""
    errors: list[str] = []
    record = dict(metadata, status="failed" if failure or cleanup_errors else "passed",
                  error=None if failure is None else f"{type(failure).__name__}: {failure}",
                  cleanup_errors=list(cleanup_errors))
    files = {".pty": transcript}
    record["trace_status"], raw_trace, trace_error = trace
    if raw_trace is not None:
        files[".trace"] = raw_trace
    if trace_error is not None:
        errors.append(trace_error)
    record["files"] = {
        suffix: {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        for suffix, data in files.items()
    }
    try:
        prefix.parent.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        return [*errors, f"create evidence directory: {error}"]
    for suffix, data in files.items():
        try:
            with Path(str(prefix) + suffix).open("xb") as output:
                output.write(data)
        except OSError as error:
            errors.append(f"write {suffix}: {error}")
    record["capture_errors"] = list(errors)
    if errors:
        record["status"] = "failed"
    try:
        with Path(str(prefix) + ".json").open("x") as output:
            json.dump(record, output, indent=2, sort_keys=True)
            output.write("\n")
    except OSError as error:
        errors.append(f"write .json: {error}")
    return errors


def finish_attempt(process, master: int, temporary, prefix: Path, trace_path: Path,
                   capture: bytearray, metadata: dict,
                   failure: BaseException | None, initial_errors: list[str] | None = None) -> list[str]:
    """Reap before reading terminal evidence, and always release temporary state."""
    errors = list(initial_errors or [])
    trace = ("unreadable", None, "terminal trace capture did not complete")
    try:
        if process is not None:
            try:
                if process.poll() is None:
                    process.terminate()
            except Exception as error:
                errors.append(f"child terminate: {error}")
            try:
                process.wait(timeout=5)
            except Exception as error:
                if not isinstance(error, subprocess.TimeoutExpired):
                    errors.append(f"child wait: {error}")
                try:
                    process.kill()
                except Exception as kill_error:
                    errors.append(f"child kill: {kill_error}")
                try:
                    process.wait(timeout=5)
                except Exception as wait_error:
                    errors.append(f"child final wait: {wait_error}")
            metadata["process_returncode"] = process.returncode
        # Drain already available PTY bytes after reaping. A closed slave gives EIO.
        # Bound reads even if a surviving descendant keeps its slave open.
        try:
            for _ in range(64):
                if not select.select([master], [], [], 0)[0]:
                    break
                chunk = os.read(master, 65_536)
                if not chunk:
                    break
                capture.extend(chunk)
            else:
                errors.append("terminal PTY drain reached its 64-read limit; capture may be incomplete")
        except OSError:
            pass
        trace = read_trace(trace_path)
    finally:
        try:
            os.close(master)
        except OSError as error:
            errors.append(f"close PTY: {error}")
        try:
            temporary.cleanup()
        except Exception as error:
            errors.append(f"temporary cleanup: {error}")
    metadata["finished_unix"] = time.time()
    try:
        errors.extend(preserve_evidence(prefix, trace, bytes(capture), metadata, failure, errors))
    except Exception as error:
        errors.append(f"capture evidence: {error}")
    return errors


def main() -> None:
    args = parse_arguments()

    root = Path(__file__).resolve().parents[1]
    fixture_name, armed_markers = BREAK_FIXTURES[args.fixture_kind]
    public_ols = args.public_scope and args.fixture_kind == "ols"
    if public_ols:
        fixture_name = "estimate_ols_public_break.do"
        armed_markers = ("FEREG_NATIVE_BREAK_ARMED",)
    fixture = root / "tests/licensed_stata" / fixture_name
    fixture_sha256 = sha256_file(fixture)
    runner_sha256 = sha256_file(Path(__file__))
    bound_plugin = validate_candidate_plugin(args.plugin, args.plugin_sha256)
    prefix = args.evidence_prefix or root / "target/tmp/stata-break" / secrets.token_hex(8)
    if any(Path(str(prefix) + suffix).exists() for suffix in (".json", ".pty", ".trace")):
        raise RuntimeError(f"evidence prefix already exists: {prefix}")
    prefix.parent.mkdir(parents=True, exist_ok=True)
    temporary = tempfile.TemporaryDirectory(prefix=".stata-break-", dir=prefix.parent)
    run_dir = Path(temporary.name)
    native_marker_token = secrets.token_hex(16)
    # Native markers: (file prefix, file content) per armed sentinel.
    native_markers = {
        "FEPPML_NATIVE_BREAK_ARMED": ("feppml", b"FEPPML_NATIVE_BREAK_ARMED\n"),
        "FEREG_NATIVE_BREAK_ARMED": ("fereg", b"FEREG_NATIVE_BREAK_ARMED\n"),
    }
    try:
        master, slave = pty.openpty()
    except BaseException:
        temporary.cleanup()
        raise

    def attach_controlling_terminal() -> None:
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    process_environment = os.environ.copy()
    process_environment["TMPDIR"] = str(run_dir)
    process_environment["FEREG_PPML_BREAK_MARKER_TOKEN"] = native_marker_token
    process_environment["FEREG_BREAK_MARKER_TOKEN"] = native_marker_token
    trace_path = run_dir / "builder.trace"
    if args.fixture_kind == "ols-builder":
        process_environment["FEREG_SOLVER_TRACE"] = "1"
        process_environment["FEREG_SOLVER_TRACE_FILE"] = str(trace_path)
    process = None
    failure = None
    capture = bytearray()
    metadata = {"plugin": str(bound_plugin), "plugin_sha256": args.plugin_sha256,
                "fixture": str(fixture), "fixture_sha256": fixture_sha256,
                "runner_sha256": runner_sha256,
                "invocation": {key: str(value) if isinstance(value, Path) else value
                               for key, value in vars(args).items()},
                "started_unix": time.time()}
    transcript = ""
    try:
        process = subprocess.Popen(
            [args.stata], cwd=run_dir, stdin=slave, stdout=slave, stderr=slave,
            close_fds=True, preexec_fn=attach_controlling_terminal, env=process_environment,
        )
        metadata["process_pid"] = process.pid
        os.close(slave)
        slave = None
        transcript += read_until(master, process, ". ", args.timeout, capture)
        isolated = run_dir / "sysdirs"
        commands = ["version 18.0", "clear all", "set more off"]
        for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
            target = isolated / name.lower()
            target.mkdir(parents=True)
            commands.append(f"sysdir set {name} {stata_quote(target)}")
        commands.extend(
            [
                f"adopath ++ {stata_quote(root / 'stata')}",
                f"adopath ++ {stata_quote(bound_plugin.parent)}",
                "capture program __fereg_break_provenance, plugin "
                f"using({stata_quote(bound_plugin)})",
                "local provenance_load_rc = _rc",
                "if `provenance_load_rc' != 0 & `provenance_load_rc' != 110 "
                "exit `provenance_load_rc'",
                'plugin call __fereg_break_provenance, selftest "ado=0.5.0" '
                '"protocol_major=1" "protocol_minor=6" '
                '"stata=`c(stata_version)\'" "threads=1" "verbose=1"',
            ]
        )
        if public_ols:
            adapter = run_dir / "public-ols-break-adapter.do"
            metadata["transaction_ado_sha256"] = public_ols_adapter(
                root / "stata/_fereg_estimate_ols.ado", adapter)
            metadata["test_adapter_sha256"] = sha256_file(adapter)
            commands.append(f"do {stata_quote(adapter)}")
        commands.extend([fixture_command(fixture, args), ""])
        os.write(master, "\n".join(commands).encode("utf-8"))
        for armed in armed_markers:
            if armed in native_markers:
                marker_prefix, content = native_markers[armed]
                native_marker_path = run_dir / f"{marker_prefix}-native-break-{native_marker_token}.marker"
                transcript += read_until_marker_file(
                    master,
                    process,
                    native_marker_path,
                    content,
                    args.timeout,
                    capture,
                )
                native_marker_path.unlink()
            else:
                transcript += read_until(master, process, armed, args.timeout, capture)
            if armed == "FEPPML_PREDICT_BREAK_ARMED":
                time.sleep(0.05)
            os.write(master, b"\x03")
        expected = f"END_FIXTURE|{fixture.stem}|rc=0"
        transcript += read_until(master, process, expected, args.timeout, capture)
        error = validate_fixture_log(fixture, transcript)
        if error:
            raise RuntimeError(f"Break fixture failed: {error}\n{transcript}")
        if sha256_file(bound_plugin) != args.plugin_sha256:
            raise RuntimeError("candidate plugin bytes changed during the Break fixture")
        if args.public_scope:
            sentinel = "FEREG_PUBLIC_SCOPE_VERIFIED" if public_ols else "FEPPML_PUBLIC_SCOPE_VERIFIED"
            if sentinel not in transcript:
                raise RuntimeError("public-scope Break fixture did not verify its boundary")
            print("BOUNDARY: " + ("fereg public OLS marker-only scope" if public_ols else "feppml public scope"))
            if public_ols:
                print("NATIVE: cancellation at the pre-preparation wait checkpoint; no inject preset")
        if args.fixture_kind == "ols-builder":
            if "FEREG_SOLVER_TRACE_IO_ERROR|" in transcript:
                raise RuntimeError("solver trace I/O failed during builder Break fixture")
            if "FEREG_BUILDER_BREAK_RESET_VERIFIED" not in transcript:
                raise RuntimeError("builder Break fixture did not verify rollback/reset")
            for event in builder_trace_events(trace_path.read_text() if trace_path.exists() else ""):
                print("BUILDER: " + event)
            print("TRACE_SHA256: " + sha256_file(trace_path))
            print(f"BOUNDARY: actual optional builder, workers={args.builder_workers or 4}, two cancellations and follow-up fits")
        if args.fixture_kind == "ols":
            print(f"BOUNDARY: OLS solver({args.ols_solver})")
            if args.cmg_samples:
                if "FEREG_SAMPLED_BREAK_VERIFIED" not in transcript:
                    raise RuntimeError("sampled Break fixture did not verify its boundary")
                print(f"SAMPLING: count={args.cmg_samples} seed={args.cmg_seed or 0} degree=30")
                print("NATIVE: cancellation at the pre-preparation wait checkpoint")
                print("PUBLICATION: completed sampled solve interrupted after the plugin")
    except BaseException as error:
        failure = error
        raise
    finally:
        errors = []
        if slave is not None:
            try:
                os.close(slave)
            except OSError as error:
                errors.append(f"close slave PTY: {error}")
        errors = finish_attempt(process, master, temporary, prefix, trace_path,
                                capture, metadata, failure, errors)
        print(f"EVIDENCE: {prefix}", file=sys.stderr)
        for error in errors:
            print(f"EVIDENCE/CLEANUP ERROR: {error}", file=sys.stderr)
        if errors and failure is None:
            raise RuntimeError("Break evidence capture or child cleanup failed")
    print(f"PASS: {expected}")


if __name__ == "__main__":
    main()
