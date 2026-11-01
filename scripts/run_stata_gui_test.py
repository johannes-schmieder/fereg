#!/usr/bin/env python3
"""Run a fixture with the macOS Stata GUI executable (not the console binary).

The GUI application binary runs the fixture in its own batch mode (`-e`), so
`c(console)` is empty and fereg sees the GUI interface. The wrapper keeps its
own log and status file and exits Stata. With `--break-markers N`, each native
Break marker is answered with a real GUI Break through Stata's AppleScript
`UtilSetStataBreak`; that needs a one-time macOS Automation approval, and an
unanswered request fails the run instead of hanging.
"""
from __future__ import annotations

import argparse
import os
import secrets
import signal
import subprocess
import sys
import time
from pathlib import Path


def stata_string(path: Path) -> str:
    text = str(path.resolve())
    if '"' in text:
        raise ValueError(f"unsupported path: {text}")
    return f'"{text}"'


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", type=Path, default=Path("/Applications/Stata/StataMP.app"))
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--break-markers", type=int, default=0)
    parser.add_argument("--break-signal", action="store_true",
                        help="deliver Break as SIGINT to the GUI process instead of AppleScript")
    parser.add_argument("--trace", action="store_true", help="enable the solver trace (trace.txt)")
    parser.add_argument("--timeout", type=float, default=1800)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    log, status, trace = work / "gui.log", work / "gui.status", work / "trace.txt"
    for path in (log, status, trace):
        path.unlink(missing_ok=True)
    token = secrets.token_hex(16)
    sysdirs = work / "sysdirs"
    for name in ("personal", "plus", "oldplace", "site"):
        (sysdirs / name).mkdir(parents=True, exist_ok=True)
    lines = [
        "version 15.0", "clear all", "set more off",
        f"log using {stata_string(log)}, replace text name(fereg_gui)",
        *[f"sysdir set {name} {stata_string(sysdirs / name.lower())}"
          for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE")],
        f"adopath ++ {stata_string(root / 'stata')}",
        f"adopath ++ {stata_string(args.fixture.parent)}",
        f"adopath ++ {stata_string(args.plugin.parent)}",
        'display "GUI_HOST|console=`c(console)\'|mode=`c(mode)\'|version=`c(stata_version)\'"',
        f"capture noisily do {stata_string(args.fixture)}",
        "local fixture_rc = _rc",
        "capture log close fereg_gui",
        "tempname handle",
        f"file open `handle' using {stata_string(status)}, write text replace",
        "file write `handle' \"rc=`fixture_rc'\" _n",
        "file close `handle'",
        "exit, clear STATA",
    ]
    wrapper = work / "gui_wrapper.do"
    wrapper.write_text("\n".join(lines) + "\n")
    environment = dict(os.environ, TMPDIR=str(work) + "/",
                       FEREG_BREAK_MARKER_TOKEN=token, FEREG_PPML_BREAK_MARKER_TOKEN=token)
    if args.trace:
        environment.update(FEREG_SOLVER_TRACE="1", FEREG_SOLVER_TRACE_FILE=str(trace))
    binary = args.app / "Contents/MacOS" / args.app.stem
    process = subprocess.Popen([str(binary), "-e", "do", str(wrapper)], cwd=work, env=environment,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    breaks = 0
    deadline = time.monotonic() + args.timeout
    try:
        while process.poll() is None and not status.exists():
            for prefix in ("fereg", "feppml"):
                marker = work / f"{prefix}-native-break-{token}.marker"
                if breaks < args.break_markers and marker.exists():
                    if args.break_signal:
                        process.send_signal(signal.SIGINT)
                    else:
                        subprocess.run(["osascript", "-e",
                                        f'tell application "{args.app}" to UtilSetStataBreak'],
                                       check=True, timeout=20, capture_output=True)
                    # Release an input worker only after Break delivery.
                    marker.unlink()
                    breaks += 1
                    print(f"BREAK_SENT|{prefix}|{breaks}", flush=True)
            if time.monotonic() > deadline:
                raise RuntimeError("GUI fixture timed out")
            time.sleep(0.2)
        process.wait(timeout=120)
    finally:
        if process.poll() is None:
            process.terminate()
    result = status.read_text().strip() if status.exists() else "missing"
    transcript = log.read_text(errors="replace") if log.exists() else ""
    host = next((line for line in transcript.splitlines() if line.startswith("GUI_HOST|")), "")
    print(host)
    print(f"STATUS|{result}|breaks={breaks}")
    sentinel = f"END_FIXTURE|{args.fixture.stem}|rc=0"
    ok = (result == "rc=0" and sentinel in transcript and "console=|" in host
          and breaks == args.break_markers)
    print(("PASS: " if ok else "FAIL: ") + sentinel)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
