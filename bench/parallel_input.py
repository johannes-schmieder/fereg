#!/usr/bin/env python3
"""Frozen paired comparison of numeric input loading in licensed Stata.

The replay timing harness replaces Stata's accessor with a mock, so this
comparison runs the public commands in Stata itself. Two arms (incumbent and
candidate), each an ado tree plus its plugin, run the frozen cells of
`parallel_input.do` in fresh Stata processes, in mirrored blocks within one
allocation. Every timed call is kept. `verify` runs each arm once with the
solver trace to show which arm executed the parallel window.

  parallel_input.py run CONFIG      # writes <output>/results.csv, plan.json
  parallel_input.py verify CONFIG   # writes <output>/verify-<arm>.txt
  parallel_input.py report CONFIG   # prints medians, ratios and regressions

CONFIG (JSON): stata, data_dir, output, threads [..], blocks (even), reps, seed,
arms {incumbent: {ado, plugin}, candidate: {ado, plugin}}.
"""
from __future__ import annotations

import csv
import json
import math
import os
import random
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CELLS = tuple(f"c{index}" for index in range(1, 15))
FIELDS = ("arm", "block", "cell", "threads", "rep", "rows", "n", "wall_s", "load_s",
          "native_s", "alloc_peak")


def stata_string(path: Path) -> str:
    text = str(Path(path).resolve())
    if '"' in text:
        raise ValueError(f"unsupported path: {text}")
    return f'"{text}"'


def run_arm(config: dict, arm: str, args: str, environment: dict | None = None, *,
            fixture: str = "parallel_input") -> str:
    spec = config["arms"][arm]
    with tempfile.TemporaryDirectory(prefix="fereg-parallel-input-") as name:
        run = Path(name)
        lines = ["clear all", "set more off"]
        for sysdir in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
            (run / sysdir.lower()).mkdir()
            lines.append(f"sysdir set {sysdir} {stata_string(run / sysdir.lower())}")
        lines += [
            f"adopath ++ {stata_string(Path(spec['ado']))}",
            f"adopath ++ {stata_string(Path(spec['plugin']).parent)}",
            f"do {stata_string(ROOT / 'bench' / (fixture + '.do'))} {args}",
        ]
        (run / "run.do").write_text("\n".join(lines) + "\n")
        subprocess.run([config["stata"], "-q", "-b", "do", "run.do"], cwd=run, check=False,
                       env=environment)
        log = (run / "run.log").read_text(errors="replace")
    if f"END_FIXTURE|{fixture}|rc=0" not in log:
        raise RuntimeError(f"{arm} failed:\n{log[-4000:]}")
    return log


def command_run(config: dict, *, fixture: str = "parallel_input") -> None:
    output = Path(config["output"])
    output.mkdir(parents=True, exist_ok=True)
    results = output / "results.csv"
    if results.exists():
        raise SystemExit(f"{results} exists; a frozen run is never resumed or resampled")
    results.write_text(",".join(FIELDS) + "\n")
    rng = random.Random(config["seed"])
    first = rng.choice(sorted(config["arms"]))
    order = []
    pair = [first, *(arm for arm in config["arms"] if arm != first)]
    for block in range(config["blocks"]):
        rotation = (block // 2) % len(pair)
        rotated = pair[rotation:] + pair[:rotation] if len(pair) > 2 else pair
        order.append(rotated if block % 2 == 0 else rotated[::-1])
    (output / "plan.json").write_text(json.dumps({"config": config, "order": order}, indent=2) + "\n")
    threads = " ".join(str(t) for t in config["threads"])
    for block, arms in enumerate(order, start=1):
        for arm in arms:
            run_arm(config, arm, f'{stata_string(Path(config["data_dir"]))} '
                                 f'{stata_string(results)} {arm} {block} "{threads}" {config["reps"]}',
                    fixture=fixture)
            print(f"block {block} {arm} done", flush=True)


def verify_trace(text: str, arm: str, threads: int) -> dict:
    groups, current = {}, None
    for line in text.splitlines():
        if not line.startswith("FEREG_SOLVER_TRACE|"):
            continue
        fields = dict(part.split("=", 1) for part in line.split("|")[1:] if "=" in part)
        if fields.get("phase") == "input_benchmark":
            current = fields["cell"]
            if current in groups or int(fields["threads"]) != threads:
                raise RuntimeError("duplicate or unexpected input cell")
            groups[current] = {"digests": [], "windows": []}
        elif current and fields.get("phase") == "input_digest":
            groups[current]["digests"].append(fields["digest"])
        elif current and fields.get("phase") == "input_transport" and fields.get("mode") == "parallel":
            groups[current]["windows"].append(fields)
    if set(groups) != set(CELLS):
        raise RuntimeError(f"{arm}: incomplete input cell trace")
    for cell, group in groups.items():
        if len(group["digests"]) != 2 or len(set(group["digests"])) != 1 or len(group["windows"]) < 2:
            raise RuntimeError(f"{arm}/{cell}: incomplete window/digest coverage or unstable input bits")
        if cell == "c11" and arm == "candidate" and any(
                (window.get("range_first"), window.get("range_end")) != ("4995000", "5005000")
                for window in group["windows"]):
            raise RuntimeError("candidate/c11: short original-row envelope did not execute")
    return {cell: group["digests"] for cell, group in groups.items()}


def command_verify(config: dict) -> None:
    output = Path(config["output"])
    output.mkdir(parents=True, exist_ok=True)
    digests = []
    for arm in config["arms"]:
        for threads in config["threads"]:
            trace = output / f"verify-{arm}-{threads}.txt"
            trace.unlink(missing_ok=True)
            scratch = output / f"verify-{arm}-{threads}.csv"
            scratch.write_text(",".join(FIELDS) + "\n")
            environment = dict(os.environ, FEREG_SOLVER_TRACE="1", FEREG_SOLVER_TRACE_FILE=str(trace))
            run_arm(config, arm, f'{stata_string(Path(config["data_dir"]))} {stata_string(scratch)} '
                                 f'{arm} 0 "{threads}" 1', environment)
            digests.append(verify_trace(trace.read_text(), arm, threads))
            print(f"{arm}/{threads}: every input cell executed its numeric window")
    if not digests or any(digest != digests[0] for digest in digests[1:]):
        raise RuntimeError("input digests differ across arms or workers")
    print("INPUT_BITS=EXACT")


def command_report(config: dict, *, cells=CELLS) -> dict:
    with (Path(config["output"]) / "results.csv").open() as stream:
        rows = list(csv.DictReader(stream))
    groups: dict[tuple, dict[str, list[dict]]] = {}
    for row in rows:
        groups.setdefault((row["cell"], int(row["threads"])), {}).setdefault(row["arm"], []).append(row)
    expected = {(cell, threads) for cell in cells for threads in config["threads"]}
    if set(groups) != expected:
        raise RuntimeError("incomplete timing cell coverage")
    identifiers = {(block, rep) for block in range(1, config["blocks"] + 1)
                   for rep in range(1, config["reps"] + 1)}
    for (cell, _), arms in groups.items():
        if set(arms) != set(config["arms"]):
            raise RuntimeError("missing timing arm")
        for arm_rows in arms.values():
            ids = [(int(row["block"]), int(row["rep"])) for row in arm_rows]
            if len(ids) != len(set(ids)) or set(ids) != identifiers:
                raise RuntimeError("duplicate or missing timing observations")
            for row in arm_rows:
                for metric in ("wall_s", "native_s", "load_s", "alloc_peak"):
                    if cell in ("c7", "c14") and metric in ("load_s", "alloc_peak") and row[metric] == ".":
                        continue
                    value = float(row[metric])
                    if not math.isfinite(value) or value < 0:
                        raise RuntimeError(f"invalid timing {cell}/{metric}")
    regressions, comparisons = [], []
    print("reference cell threads metric reference_median candidate_median ratio n_ref n_cand flag")
    for (cell, threads), arms in sorted(groups.items()):
        for reference in config["arms"]:
            if reference == "candidate":
                continue
            for metric in ("wall_s", "load_s", "native_s", "alloc_peak"):
                values = {}
                for arm in (reference, "candidate"):
                    data = [float(r[metric]) for r in arms[arm] if r[metric] not in ("", ".")]
                    values[arm] = (statistics.median(data), len(data)) if data else (float("nan"), 0)
                inc, cand = values[reference][0], values["candidate"][0]
                ratio = cand / inc if inc and inc == inc else float("nan")
                flag = ""
                if metric == "wall_s" and cand > inc * 1.05 and cand - inc > 0.001:
                    flag = "REGRESSION"
                    if [cell, threads] not in regressions:
                        regressions.append([cell, threads])
                comparisons.append(dict(reference=reference, cell=cell, threads=threads,
                                        metric=metric,
                                        reference_median=inc if math.isfinite(inc) else None,
                                        candidate_median=cand if math.isfinite(cand) else None,
                                        ratio=ratio if math.isfinite(ratio) else None,
                                        regression=bool(flag)))
                print(f"{reference} {cell} {threads} {metric} {inc:.6g} {cand:.6g} {ratio:.4f} "
                      f"{values[reference][1]} {values['candidate'][1]} {flag}")

    result = {"status": "regression" if regressions else "passed", "regressions": regressions,
              "observations": len(rows), "cells": len(groups), "resampling": False,
              "comparisons": comparisons}
    (Path(config["output"]) / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[1] not in ("run", "verify", "report"):
        print(__doc__)
        return 2
    config = json.loads(Path(sys.argv[2]).read_text())
    result = {"run": command_run, "verify": command_verify, "report": command_report}[sys.argv[1]](config)
    return 1 if result and result["status"] != "passed" else 0


if __name__ == "__main__":
    sys.exit(main())
