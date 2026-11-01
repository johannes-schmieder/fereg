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
import os
import random
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CELLS = ("c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8")
FIELDS = ("arm", "block", "cell", "threads", "rep", "rows", "n", "wall_s", "load_s",
          "native_s", "alloc_peak")


def stata_string(path: Path) -> str:
    text = str(Path(path).resolve())
    if '"' in text:
        raise ValueError(f"unsupported path: {text}")
    return f'"{text}"'


def run_arm(config: dict, arm: str, args: str, environment: dict | None = None) -> str:
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
            f"do {stata_string(ROOT / 'bench/parallel_input.do')} {args}",
        ]
        (run / "run.do").write_text("\n".join(lines) + "\n")
        subprocess.run([config["stata"], "-q", "-b", "do", "run.do"], cwd=run, check=False,
                       env=environment)
        log = (run / "run.log").read_text(errors="replace")
    if "END_FIXTURE|parallel_input|rc=0" not in log:
        raise RuntimeError(f"{arm} failed:\n{log[-4000:]}")
    return log


def command_run(config: dict) -> None:
    output = Path(config["output"])
    output.mkdir(parents=True, exist_ok=True)
    results = output / "results.csv"
    if results.exists():
        raise SystemExit(f"{results} exists; a frozen run is never resumed or resampled")
    results.write_text(",".join(FIELDS) + "\n")
    rng = random.Random(config["seed"])
    first = rng.choice(sorted(config["arms"]))
    order = []
    for block in range(config["blocks"]):
        pair = [first, next(arm for arm in config["arms"] if arm != first)]
        order.append(pair if block % 2 == 0 else pair[::-1])
    (output / "plan.json").write_text(json.dumps({"config": config, "order": order}, indent=2) + "\n")
    threads = " ".join(str(t) for t in config["threads"])
    for block, arms in enumerate(order, start=1):
        for arm in arms:
            run_arm(config, arm, f'{stata_string(Path(config["data_dir"]))} '
                                 f'{stata_string(results)} {arm} {block} "{threads}" {config["reps"]}')
            print(f"block {block} {arm} done", flush=True)


def command_verify(config: dict) -> None:
    output = Path(config["output"])
    output.mkdir(parents=True, exist_ok=True)
    threads = max(config["threads"])
    for arm in config["arms"]:
        trace = output / f"verify-{arm}.txt"
        trace.unlink(missing_ok=True)
        scratch = output / f"verify-{arm}.csv"
        scratch.write_text(",".join(FIELDS) + "\n")
        environment = dict(os.environ, FEREG_SOLVER_TRACE="1", FEREG_SOLVER_TRACE_FILE=str(trace))
        run_arm(config, arm, f'{stata_string(Path(config["data_dir"]))} {stata_string(scratch)} '
                             f'{arm} 0 "{threads}" 1', environment)
        text = trace.read_text() if trace.exists() else ""
        parallel = sum("phase=input_transport|mode=parallel" in line for line in text.splitlines())
        print(f"{arm}: parallel window reads={parallel}")


def command_report(config: dict) -> None:
    rows = list(csv.DictReader((Path(config["output"]) / "results.csv").open()))
    groups: dict[tuple, dict[str, list[dict]]] = {}
    for row in rows:
        groups.setdefault((row["cell"], int(row["threads"])), {}).setdefault(row["arm"], []).append(row)
    print("cell threads metric incumbent_median candidate_median ratio n_inc n_cand flag")
    for (cell, threads), arms in sorted(groups.items()):
        for metric in ("wall_s", "load_s", "native_s", "alloc_peak"):
            values = {}
            for arm in ("incumbent", "candidate"):
                data = [float(r[metric]) for r in arms.get(arm, []) if r[metric] not in ("", ".")]
                values[arm] = (statistics.median(data), len(data)) if data else (float("nan"), 0)
            inc, cand = values["incumbent"][0], values["candidate"][0]
            ratio = cand / inc if inc and inc == inc else float("nan")
            flag = ""
            if metric == "wall_s" and cand > inc * 1.05 and cand - inc > 0.001:
                flag = "REGRESSION"
            print(f"{cell} {threads} {metric} {inc:.6g} {cand:.6g} {ratio:.4f} "
                  f"{values['incumbent'][1]} {values['candidate'][1]} {flag}")


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[1] not in ("run", "verify", "report"):
        print(__doc__)
        return 2
    config = json.loads(Path(sys.argv[2]).read_text())
    {"run": command_run, "verify": command_verify, "report": command_report}[sys.argv[1]](config)
    return 0


if __name__ == "__main__":
    sys.exit(main())
