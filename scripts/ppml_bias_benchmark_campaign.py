#!/usr/bin/env python3
"""Execute one frozen paired correction matrix without replacing attempts."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("plan", "reference", "work", "plugin", "source-manifest"):
        p.add_argument(f"--{name}", type=Path, required=True)
    p.add_argument("--stata", required=True)
    args = p.parse_args()
    root = Path(__file__).resolve().parents[1]
    plan = json.loads(args.plan.read_text())
    inputs = {
        "controller_sha256": Path(__file__),
        "script_sha256": root / "scripts/ppml_bias_benchmark.py",
        "mata_sha256": root / "tests/stata/bias/mata_correction.do",
        "monitor_sha256": root / "benchmarks/two_fe_dense/route_development.py",
        "source_manifest_sha256": args.source_manifest,
        "reference_sha256": args.reference,
        "plugin_sha256": args.plugin,
    }
    for field, path in inputs.items():
        if sha(path) != plan[field]:
            raise ValueError(f"Frozen benchmark input changed: {field}")
    args.work.mkdir(parents=True, exist_ok=False)
    for case, settings in enumerate(plan["cases"]):
        for block, order in enumerate(plan["orders"]):
            for engine in order:
                label = f"matrix-{case}-{block}-{engine}"
                command = [sys.executable, str(root / "scripts/ppml_bias_benchmark.py"),
                           "--reference", str(args.reference), "--out", str(args.work / label),
                           "--engine", engine, "--warm", str(plan["warm_calls"]),
                           "--stata", args.stata, "--source-manifest", str(args.source_manifest),
                           "--plugin", str(args.plugin), "--refit-reference"]
                for key, value in settings.items():
                    option = "levels" if key == "levels_each" else key
                    command.extend((f"--{option}", str(value)))
                print(f"Starting {label}", flush=True)
                subprocess.run(command, cwd=root, check=True)
    subprocess.run([sys.executable, str(root / "scripts/ppml_bias_benchmark_analyze.py"),
                    "--plan", str(args.plan), "--directory", str(args.work),
                    "--prefix", "matrix", "--source-manifest", str(args.source_manifest),
                    "--out", str(args.work / "summary.json")], cwd=root, check=True)


if __name__ == "__main__":
    main()
