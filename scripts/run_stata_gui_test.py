#!/usr/bin/env python3
"""Run a fixture in the macOS Stata GUI (not the console binary).

The GUI application is launched directly so it inherits the test environment
(solver trace, Break token, TMPDIR), commands are queued through Stata's
AppleScript interface, and `--break-markers N` delivers a real Break with
`UtilSetStataBreak` each time the native marker file appears. The fixture's
log and END_FIXTURE sentinel decide success, as in the console runners.
"""
from __future__ import annotations

import argparse
import os
import secrets
import subprocess
import sys
import time
from pathlib import Path


def stata_string(path: Path) -> str:
    text = str(path.resolve())
    if '"' in text:
        raise ValueError(f"unsupported path: {text}")
    return f'"{text}"'


def osascript(app: Path, command: str) -> str:
    script = f'tell application "{app}" to {command}'
    return subprocess.run(["osascript", "-e", script], check=True,
                          capture_output=True, text=True).stdout.strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", type=Path, default=Path("/Applications/Stata/StataMP.app"))
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--break-markers", type=int, default=0)
    parser.add_argument("--timeout", type=float, default=1800)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    log = work / "gui.log"
    status = work / "gui.status"
    trace = work / "trace.txt"
    for path in (log, status, trace):
        path.unlink(missing_ok=True)
    token = secrets.token_hex(16)
    sysdirs = work / "sysdirs"
    wrapper = work / "gui_wrapper.do"
    lines = [
        "version 15.0", "clear all", "set more off",
        f"log using {stata_string(log)}, replace text name(fereg_gui)",
        *[f"sysdir set {name} {stata_string(sysdirs / name.lower())}"
          for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE")],
        f"adopath ++ {stata_string(root / 'stata')}",
        f"adopath ++ {stata_string(args.fixture.parent)}",
        f"adopath ++ {stata_string(args.plugin.parent)}",
        f'display "GUI_HOST|console=`c(console)\'|version=`c(stata_version)\'|edition=`c(edition_real)\'"',
        f"capture noisily do {stata_string(args.fixture)}",
        "local fixture_rc = _rc",
        "log close fereg_gui",
        "tempname handle",
        f"file open `handle' using {stata_string(status)}, write text replace",
        "file write `handle' \"rc=`fixture_rc'\" _n",
        "file close `handle'",
    ]
    for name in ("personal", "plus", "oldplace", "site"):
        (sysdirs / name).mkdir(parents=True, exist_ok=True)
    wrapper.write_text("\n".join(lines) + "\n")
    environment = dict(os.environ, TMPDIR=str(work) + "/", FEREG_SOLVER_TRACE="1",
                       FEREG_SOLVER_TRACE_FILE=str(trace),
                       FEREG_BREAK_MARKER_TOKEN=token, FEREG_PPML_BREAK_MARKER_TOKEN=token)
    binary = args.app / "Contents/MacOS" / args.app.stem
    process = subprocess.Popen([str(binary)], env=environment,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 120
        while True:
            try:
                if osascript(args.app, "UtilIsStataFree") == "true":
                    break
            except subprocess.CalledProcessError:
                pass
            if time.monotonic() > deadline:
                raise RuntimeError("Stata GUI did not become ready")
            time.sleep(1)
        osascript(args.app, f'DoCommandAsync "do {stata_string(wrapper)[1:-1]}"')
        breaks = 0
        deadline = time.monotonic() + args.timeout
        while not status.exists():
            for prefix in ("fereg", "feppml"):
                marker = work / f"{prefix}-native-break-{token}.marker"
                if breaks < args.break_markers and marker.exists():
                    marker.unlink()
                    osascript(args.app, "UtilSetStataBreak")
                    breaks += 1
                    print(f"BREAK_SENT|{prefix}|{breaks}")
            if time.monotonic() > deadline:
                raise RuntimeError("GUI fixture timed out")
            time.sleep(0.2)
        result = status.read_text().strip()
        transcript = log.read_text(errors="replace")
        host = next((line for line in transcript.splitlines() if line.startswith("GUI_HOST|")), "")
        print(host)
        print(f"STATUS|{result}|breaks={breaks}")
        sentinel = f"END_FIXTURE|{args.fixture.stem}|rc=0"
        ok = result == "rc=0" and sentinel in transcript and breaks == args.break_markers
        print(("PASS: " if ok else "FAIL: ") + sentinel)
        return 0 if ok else 1
    finally:
        try:
            osascript(args.app, 'DoCommandAsync "exit, clear STATA"')
        except subprocess.CalledProcessError:
            pass
        try:
            process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            process.terminate()


if __name__ == "__main__":
    sys.exit(main())
