#!/usr/bin/env python3
"""Drive the licensed hidden eclass Break fixture through an interactive PTY."""
from __future__ import annotations

import argparse
import fcntl
import os
import pty
import select
import secrets
import subprocess
import tempfile
import termios
import time
from pathlib import Path

from run_stata_tests import sha256_file, validate_candidate_plugin, validate_fixture_log

BREAK_FIXTURES = {
    "ols": ("estimate_ols_eclass_break.do", ("FEREG_NATIVE_BREAK_ARMED", "FEREG_BREAK_ARMED")),
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


def read_until(master: int, process: subprocess.Popen[bytes], needle: str, timeout: float) -> str:
    deadline = time.monotonic() + timeout
    chunks: list[bytes] = []
    encoded = needle.encode("utf-8")
    while time.monotonic() < deadline:
        remaining = max(0.0, deadline - time.monotonic())
        ready, _, _ = select.select([master], [], [], min(0.25, remaining))
        if ready:
            chunk = os.read(master, 65_536)
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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--plugin-sha256", required=True)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--fixture-kind", choices=sorted(BREAK_FIXTURES), default="ols")
    parser.add_argument("--public-scope", action="store_true",
                        help="exercise feppml's outer scope with the private injection adapter")
    args = parser.parse_args()
    if args.public_scope and args.fixture_kind != "ppml":
        parser.error("--public-scope requires --fixture-kind ppml")
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")

    root = Path(__file__).resolve().parents[1]
    fixture_name, armed_markers = BREAK_FIXTURES[args.fixture_kind]
    fixture = root / "tests/licensed_stata" / fixture_name
    bound_plugin = validate_candidate_plugin(args.plugin, args.plugin_sha256)
    temporary = tempfile.TemporaryDirectory(prefix="fereg-stata-break-")
    run_dir = Path(temporary.name)
    native_marker_token = secrets.token_hex(16)
    # Native markers: (file prefix, file content) per armed sentinel.
    native_markers = {
        "FEPPML_NATIVE_BREAK_ARMED": ("feppml", b"FEPPML_NATIVE_BREAK_ARMED\n"),
        "FEREG_NATIVE_BREAK_ARMED": ("fereg", b"FEREG_NATIVE_BREAK_ARMED\n"),
    }
    master, slave = pty.openpty()

    def attach_controlling_terminal() -> None:
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    process_environment = os.environ.copy()
    process_environment["TMPDIR"] = str(run_dir)
    process_environment["FEREG_PPML_BREAK_MARKER_TOKEN"] = native_marker_token
    process_environment["FEREG_BREAK_MARKER_TOKEN"] = native_marker_token
    process = subprocess.Popen(
        [args.stata],
        cwd=run_dir,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
        preexec_fn=attach_controlling_terminal,
        env=process_environment,
    )
    os.close(slave)
    transcript = ""
    try:
        transcript += read_until(master, process, ". ", args.timeout)
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
                "plugin call __fereg_break_provenance, version",
                f"do {stata_quote(fixture)}" + (" public" if args.public_scope else ""),
                "",
            ]
        )
        os.write(master, "\n".join(commands).encode("utf-8"))
        for armed in armed_markers:
            if armed in native_markers:
                prefix, content = native_markers[armed]
                native_marker_path = run_dir / f"{prefix}-native-break-{native_marker_token}.marker"
                transcript += read_until_marker_file(
                    master,
                    process,
                    native_marker_path,
                    content,
                    args.timeout,
                )
                native_marker_path.unlink()
            else:
                transcript += read_until(master, process, armed, args.timeout)
            if armed == "FEPPML_PREDICT_BREAK_ARMED":
                time.sleep(0.05)
            os.write(master, b"\x03")
        expected = f"END_FIXTURE|{fixture.stem}|rc=0"
        transcript += read_until(master, process, expected, args.timeout)
        error = validate_fixture_log(fixture, transcript)
        if error:
            raise RuntimeError(f"Break fixture failed: {error}\n{transcript}")
        if sha256_file(bound_plugin) != args.plugin_sha256:
            raise RuntimeError("candidate plugin bytes changed during the Break fixture")
        if args.public_scope:
            if "FEPPML_PUBLIC_SCOPE_VERIFIED" not in transcript:
                raise RuntimeError("public-scope Break fixture did not verify its boundary")
            print("BOUNDARY: feppml public scope")
        print(f"PASS: {expected}")
    finally:
        native_marker_path.unlink(missing_ok=True)
        os.close(master)
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        temporary.cleanup()


if __name__ == "__main__":
    main()
