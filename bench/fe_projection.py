#!/usr/bin/env python3
"""M3.6 FE-stage timing; separate from public-estimator cutover benchmarks."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import numpy as np
import pandas as pd
from generate import generate, sha256
from run import ROOT, launch, load_suite, source_identity
from fe_projection_compare import summarize

CELLS = ("ols-2fe-akm-strong", "ols-2fe-akm-weak", "ols-3fe", "ols-2fe-slope")
ARMS = ("legacy-auto", "next-auto", "next-lsmr", "next-cmg")


def export_input(cell, seed, directory):
    dataset = directory / "input.dta"
    metadata = generate(cell, seed, dataset)
    frame = pd.read_stata(dataset, convert_categoricals=False)
    target = directory / "input.fe"
    with target.open("wb") as stream:
        stream.write(b"FEREGFE1")
        stream.write(struct.pack("<QQQ", len(frame), cell["regressors"], cell["effects"]))
        for effect in range(1, cell["effects"] + 1):
            codes = frame[f"g{effect}"].to_numpy(dtype=np.int64) - 1
            slopes = int(effect == 1 and cell.get("slopes", False))
            stream.write(struct.pack("<QQ", int(codes.max()) + 1, slopes))
            stream.write(codes.astype("<u4").tobytes())
            if slopes:
                stream.write(frame.slope.to_numpy(dtype="<f8").tobytes())
        for name in ["y"] + [f"x{j}" for j in range(1, cell["regressors"] + 1)]:
            stream.write(frame[name].to_numpy(dtype="<f8").tobytes())
    metadata.update(fe_sha256=sha256(target), fe_bytes=target.stat().st_size)
    (directory / "input.json").write_text(json.dumps(metadata, indent=2) + "\n")
    return target


def measure(root, binary, cell, arm, threads, repetition, tolerance, timeout):
    destination = root / "commands" / cell["id"] / f"{arm}-t{threads}-{repetition}"
    destination.mkdir(parents=True)
    command = ["/usr/bin/env", "FEREG_PHASE_TIMING=1", str(binary),
               str(root / "inputs" / cell["id"] / "input.fe"), arm, str(threads),
               str(tolerance), str(destination)]
    result = {"cell": cell["id"], "arm": arm, "threads": threads,
              "repetition": repetition, "tolerance": tolerance,
              **launch(command, destination, timeout)}
    (destination / "process.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def run(args):
    root = args.root.resolve()
    if root.exists():
        raise ValueError("run root must be new; earlier evidence is never overwritten")
    if (args.rows <= 0 or args.repetitions <= 0 or not args.threads
            or any(t not in (1, 8) for t in args.threads)):
        raise ValueError("invalid dimensions, repetitions or native thread count")
    if (not args.cells or args.timeout <= 0 or len(set(args.cells)) != len(args.cells)
            or len(set(args.threads)) != len(args.threads) or not set(args.cells) <= set(CELLS)):
        raise ValueError("duplicate threads or unknown benchmark cells")
    suite = load_suite()
    commit, dirty = source_identity(ROOT)
    partial = (args.rows != 1_000_000 or args.repetitions != 5
               or set(args.cells) != set(CELLS) or set(args.threads) != {1, 8}
               or args.timeout != 1800)
    if not partial and (dirty or sys.platform != "darwin"):
        raise ValueError("formal M3.6 timing requires a clean committed source on the Mac")
    cells = [{**suite["defaults"], **cell, "rows": args.rows}
             for cell in suite["cells"] if cell["id"] in args.cells]
    root.mkdir(parents=True)
    identity = {"schema": 1, "commit": commit, "dirty": dirty, "partial": partial,
                "platform": sys.platform, "machine": platform.machine(),
                "processor": platform.processor(), "cpu_count": os.cpu_count(),
                "seed": suite["seed"], "cells": cells, "threads": args.threads,
                "warmups": 1, "repetitions": args.repetitions,
                "timeout_seconds": args.timeout, "tolerance": 1e-8,
                "reference_tolerance": suite["protocol"]["reference_tolerance"],
                "numpy": np.__version__, "pandas": pd.__version__, "inputs": {}}
    with (root / "build.log").open("w") as log:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "fereg-oracle",
                        "--example", "fe_projection"], cwd=ROOT, stdout=log, stderr=log, check=True)
    binary = root / "fe_projection"
    shutil.copy2(ROOT / "target/release/examples/fe_projection", binary)
    identity["binary_sha256"] = sha256(binary)
    identity["rustc"] = subprocess.check_output(["rustc", "-Vv"], text=True).strip()
    identity["cargo_lock_sha256"] = sha256(ROOT / "Cargo.lock")
    (root / "run.json").write_text(json.dumps(identity, indent=2) + "\n")
    for cell in cells:
        directory = root / "inputs" / cell["id"]
        directory.mkdir(parents=True)
        export_input(cell, suite["seed"], directory)
        identity["inputs"][cell["id"]] = sha256(directory / "input.fe")
        (root / "run.json").write_text(json.dumps(identity, indent=2) + "\n")
        for arm in ("legacy-auto", "next-auto"):
            measure(root, binary, cell, arm, 1, "reference", identity["reference_tolerance"], args.timeout)
        for threads in args.threads:
            for arm in ARMS:
                if arm == "next-cmg" and cell.get("slopes"):
                    continue
                if source_identity(ROOT) != (commit, dirty):
                    raise ValueError("source identity changed during benchmark")
                for repetition in ["warmup"] + list(range(1, args.repetitions + 1)):
                    result = measure(root, binary, cell, arm, threads, repetition, 1e-8, args.timeout)
                    if result["process_status"] != "ok":
                        break
                summarize(root, root / "summary.csv")
                print(f'{cell["id"]} T{threads} {arm}: {result["process_status"]}', flush=True)
    if source_identity(ROOT) != (commit, dirty) or sha256(binary) != identity["binary_sha256"]:
        raise ValueError("source or executable changed during benchmark")
    if any(sha256(root / "inputs" / cell / "input.fe") != digest
           for cell, digest in identity["inputs"].items()):
        raise ValueError("benchmark input changed during measurement")
    summarize(root, root / "summary.csv")
    (root / "complete.json").write_text(json.dumps({"commit": commit, "complete": True}) + "\n")
    return root / "summary.csv"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--rows", type=int, default=1_000_000)
    parser.add_argument("--cells", nargs="+", choices=CELLS, default=list(CELLS))
    parser.add_argument("--threads", nargs="+", type=int, default=[1, 8])
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--timeout", type=int, default=1800)
    args = parser.parse_args()
    print(run(args))


if __name__ == "__main__":
    main()
