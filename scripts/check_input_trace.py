#!/usr/bin/env python3
"""Check a parallel-input oracle's solver trace.

The fixture writes `phase=case|label=<case>|<mode>` before each estimation;
the plugin then appends `phase=input_transport|mode=parallel` when the numeric
window ran and one `phase=input_digest` line per loaded action. Within a case,
every digest must be identical (serial, forced-serial and parallel readers).
Modes `serial`, `forced` and `alias` must not use the window; `parallel` must,
unless the case is listed with --serial-ok (inputs below the work policy).
--require-serial verifies an unqualified host: no run may open a numeric window.
"""
from __future__ import annotations

import argparse
import collections
import sys
from pathlib import Path


def fields(line: str) -> dict[str, str]:
    """Split `|key=value` fields; a bare part continues the previous value
    (case labels are written as `label=<case>|<mode>`)."""
    result: dict[str, str] = {}
    key = None
    for part in line.strip().split("|")[1:]:
        if "=" in part:
            key, _, value = part.partition("=")
            result[key] = value
        elif key is not None:
            result[key] += "|" + part
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path)
    parser.add_argument("--serial-ok", action="append", default=[])
    parser.add_argument("--require-serial", action="store_true",
                        help="require the serial fallback in every run")
    args = parser.parse_args()
    runs: list[dict[str, object]] = []
    unassigned_windows = 0
    for line in args.trace.read_text(encoding="utf-8").splitlines():
        if not line.startswith("FEREG_SOLVER_TRACE|"):
            continue
        record = fields(line)
        phase = record.get("phase")
        if phase == "case":
            case, _, mode = record["label"].partition("|")
            runs.append({"case": case, "mode": mode, "digests": [], "parallel": 0})
        elif runs and phase == "input_digest":
            runs[-1]["digests"].append(record["digest"])
        elif phase == "input_transport" and record.get("mode") == "parallel":
            if runs:
                runs[-1]["parallel"] += 1
            else:
                unassigned_windows += 1
    if not runs:
        print("no fixture cases in trace", file=sys.stderr)
        return 1
    failures = []
    if args.require_serial and unassigned_windows:
        failures.append("the numeric window ran before the first case")
    digests: dict[str, set[str]] = collections.defaultdict(set)
    parallel_runs = 0
    for run in runs:
        label = f"{run['case']}|{run['mode']}"
        if not run["digests"]:
            failures.append(f"{label}: no input digest")
        digests[run["case"]].update(run["digests"])
        if (args.require_serial or run["mode"] in ("serial", "forced", "alias")) and run["parallel"]:
            failures.append(f"{label}: the numeric window ran")
        if run["mode"] == "parallel":
            if run["parallel"]:
                parallel_runs += 1
            elif not args.require_serial and run["case"] not in args.serial_ok:
                failures.append(f"{label}: the numeric window did not run")
    for case, observed in sorted(digests.items()):
        if len(observed) != 1:
            failures.append(f"{case}: {len(observed)} distinct input digests {sorted(observed)}")
    print(f"cases={len(digests)} runs={len(runs)} parallel_runs={parallel_runs}")
    for failure in failures:
        print(f"FAIL {failure}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
