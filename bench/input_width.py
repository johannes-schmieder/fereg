"""Fixed width-boundary suite, dispatched by `qualify.py input`.

The same generated dataset supplies 63/64/65/66/130 input-column models,
with and without fixed effects, through public and forced-serial commands.
Science precedes timing: independent Stata fits, exact posted results and
input bits across arms, repeatability, and traces proving the wide route.
"""
from __future__ import annotations

import json
import os
from pathlib import Path

import parallel_input as base

WIDTHS = (63, 64, 65, 66, 130)
CELLS = tuple(f"w{width}-{effect}-{mode}" for width in WIDTHS
              for effect in ("none", "fe") for mode in ("parallel", "serial"))


def command_run(config):
    base.command_run(config, fixture="input_width")


def command_report(config):
    return base.command_report(config, cells=CELLS)


def command_verify(config):
    output = Path(config["output"])
    output.mkdir(parents=True, exist_ok=True)
    expected = None
    for arm in config["arms"]:
        for threads in config["threads"]:
            trace = output / f"verify-{arm}-{threads}.txt"
            science = output / f"science-{arm}-{threads}.txt"
            for path in (trace, science):
                if path.exists():
                    raise RuntimeError(f"preflight output already exists: {path}")
            scratch = output / f"verify-{arm}-{threads}.csv"
            scratch.write_text(",".join(base.FIELDS) + "\n")
            environment = dict(os.environ, FEREG_SOLVER_TRACE="1",
                               FEREG_SOLVER_TRACE_FILE=str(trace), FEREG_WIDTH_SCIENCE=str(science))
            log = base.run_arm(config, arm,
                               f'{base.stata_string(Path(config["data_dir"]))} '
                               f'{base.stata_string(scratch)} {arm} 0 "{threads}" 1 '
                               f'{base.stata_string(base.ROOT / "bench/input_width_extra.do")}',
                               environment, fixture="input_width")
            (output / f"verify-{arm}-{threads}.log").write_text(log)
            groups, current = {}, None
            for line in trace.read_text().splitlines():
                if not line.startswith("FEREG_SOLVER_TRACE|"):
                    continue
                fields = dict(p.split("=", 1) for p in line.split("|")[1:] if "=" in p)
                if fields.get("phase") == "input_benchmark":
                    current = fields["cell"]
                    if current in groups:
                        raise RuntimeError("duplicate width cell")
                    groups[current] = {"digests": [], "layouts": [], "windows": []}
                elif fields.get("phase") == "width_extra":
                    current = None
                elif current:
                    key = {"input_digest": "digests", "input_layout": "layouts",
                           "input_transport": "windows"}.get(fields.get("phase"))
                    if key:
                        groups[current][key].append(fields)
            if set(groups) != set(CELLS):
                raise RuntimeError("incomplete width trace")
            digests = {}
            for cell, group in groups.items():
                values = [d["digest"] for d in group["digests"]]
                if len(values) < 2 or len(set(values)) != 1:
                    raise RuntimeError(f"{arm}/{cell}: unstable input bits")
                digests[cell] = values[0]
                parallel = [w for w in group["windows"] if w.get("mode") == "parallel"]
                if bool(parallel) != cell.endswith("parallel"):
                    raise RuntimeError(f"{arm}/{cell}: wrong transport executed")
                if arm == "candidate" and "-fe-" in cell:
                    if not group["layouts"] or any(
                            layout.get("reserved") != "true" or layout.get("packed_rhs") != "true"
                            for layout in group["layouts"]):
                        raise RuntimeError(f"{arm}/{cell}: widened packed input did not execute")
            results = {}
            for line in science.read_text().splitlines():
                cell, value = line.split("|", 1)
                if cell in results and results[cell] != value:
                    raise RuntimeError(f"{arm}/{cell}: fixed-worker result changed")
                results[cell] = value
            if set(results) != set(CELLS) | {f"extra{i}" for i in range(1, 10)}:
                raise RuntimeError("incomplete width science")
            for width in WIDTHS:
                for effect in ("none", "fe"):
                    parallel, serial = (f"w{width}-{effect}-{mode}" for mode in ("parallel", "serial"))
                    if results[parallel] != results[serial] or digests[parallel] != digests[serial]:
                        raise RuntimeError(f"{arm}/{width}/{effect}: public/forced science differs")
            # Transport changes must be exact at each worker count. Independent
            # Stata regress/areg checks also execute in the fixture, outside timing.
            witness = (digests, results)
            if expected is None:
                expected = {}
            if threads in expected and expected[threads] != witness:
                raise RuntimeError(f"{arm}/{threads}: science differs across plugins")
            expected[threads] = witness
            print(f"{arm}/{threads}: width input, repeatability, and Stata oracle PASS", flush=True)
    (output / "science-summary.json").write_text(json.dumps({
        "status": "passed", "cells": len(CELLS), "threads": config["threads"],
        "arms": list(config["arms"]), "exact": ["input_bits", "b", "V", "N", "sample"],
        "independent": ["regress", "areg"], "resampling": False,
    }, indent=2) + "\n")
