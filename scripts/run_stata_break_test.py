#!/usr/bin/env python3
"""Drive the licensed hidden eclass Break fixture through an interactive PTY."""
from __future__ import annotations

import argparse
import fcntl
import os
import pty
import select
import subprocess
import tempfile
import termios
import time
from pathlib import Path

from run_stata_tests import sha256_file, validate_candidate_plugin, validate_fixture_log

ARMED = "FEREG_BREAK_ARMED"


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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--plugin-sha256", required=True)
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")

    root = Path(__file__).resolve().parents[1]
    fixture = root / "tests/licensed_stata/estimate_ols_eclass_break.do"
    bound_plugin = validate_candidate_plugin(args.plugin, args.plugin_sha256)
    temporary = tempfile.TemporaryDirectory(prefix="fereg-stata-break-")
    run_dir = Path(temporary.name)
    master, slave = pty.openpty()

    def attach_controlling_terminal() -> None:
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    process = subprocess.Popen(
        [args.stata],
        cwd=run_dir,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        close_fds=True,
        preexec_fn=attach_controlling_terminal,
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
                f"do {stata_quote(fixture)}",
                "",
            ]
        )
        os.write(master, "\n".join(commands).encode("utf-8"))
        transcript += read_until(master, process, ARMED, args.timeout)
        os.write(master, b"\x03")
        expected = "END_FIXTURE|estimate_ols_eclass_break|rc=0"
        transcript += read_until(master, process, expected, args.timeout)
        error = validate_fixture_log(fixture, transcript)
        if error:
            raise RuntimeError(f"Break fixture failed: {error}\n{transcript}")
        if sha256_file(bound_plugin) != args.plugin_sha256:
            raise RuntimeError("candidate plugin bytes changed during the Break fixture")
        print(f"PASS: {expected}")
    finally:
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
