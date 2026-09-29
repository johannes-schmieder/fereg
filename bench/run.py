#!/usr/bin/env python3
"""One untimed warm-up and five fresh-process measurements per declared command."""
from __future__ import annotations
import argparse
import csv
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time
import tomllib
from generate import generate, sha256

ROOT = Path(__file__).resolve().parents[1]


def load_suite():
    return tomllib.loads((ROOT / "bench/suite.toml").read_text())


def selected_cells(suite, tier):
    spec = suite["tiers"][tier]
    cells = [{**suite["defaults"], **cell} for cell in suite["cells"]]
    if spec.get("cells"):
        cells = [cell for cell in cells if cell["id"] in spec["cells"]]
    if spec.get("non_scc"):
        cells = [cell for cell in cells if not cell["scc"]]
    if "rows" in spec:
        cells = [{**cell, "rows": spec["rows"]} for cell in cells]
    return cells


def thread_counts(suite, tier, cell):
    spec = suite["tiers"][tier]
    return sorted(set(spec["threads"] + (spec.get("ladder_threads", []) if cell["id"] in spec.get("ladder_cells", []) else [])))


def read_metadata(path):
    if not path.exists():
        return {}
    return dict(line.rstrip("\n").split("\t", 1) for line in path.read_text().splitlines() if "\t" in line)


def launch(command, directory, timeout):
    start = time.monotonic()
    with (directory / "process.log").open("w") as log:
        process = subprocess.Popen(command, cwd=directory, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            rc = process.wait(timeout=timeout)
            status = "ok" if rc == 0 else "error"
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            rc, status = None, "timeout"
    return {"process_status": status, "process_rc": rc, "wall_seconds": time.monotonic()-start}


def write_configuration(directory, settings):
    for value in settings.values():
        if any(char in str(value) for char in '\t\n\r"`$'):
            raise ValueError("benchmark configuration contains an unsafe Stata path/token")
    (directory / "config.tsv").write_text("".join(f"{key}\t{value}\n" for key, value in settings.items()))
    (directory / "config.do").write_text("".join(f'local {key} "{value}"\n' for key, value in settings.items()))


def measure(args, suite, cell, arm, threads, repetition, dataset, environment, reference=False):
    label = "reference" if reference else f"{arm}-t{threads}-{repetition}"
    directory = args.root / "commands" / cell["id"] / label
    directory.mkdir(parents=True)
    settings = {"source": str(ROOT), "refs": environment["references"], "r_library": environment["r_library"],
                "dataset": str(dataset), "output": str(directory), "arm": arm, "model": cell["model"],
                "k": cell["regressors"], "effects": cell["effects"], "threads": threads,
                "processor_cap": args.stata_processors, "slopes": int(cell.get("slopes", False)),
                "outputs": int(cell["outputs"]), "weights": cell["weights"], "vce": cell["vce"],
                "engine": args.engine, "engine_option": int(suite["engine_option"]),
                "solver": "auto" if reference else args.solver, "tolerance": suite["protocol"]["reference_tolerance"] if reference else ""}
    write_configuration(directory, settings)
    if arm.startswith("fixest"):
        command = [args.rscript, str(ROOT / "bench/r_runner.R"), str(directory / "config.tsv")]
    else:
        (directory / "command.do").write_text(f'do "{ROOT / "bench/stata_runner.do"}" "{directory / "config.do"}"\n')
        command = [args.stata, "-b", "do", str(directory / "command.do")]
    result = launch(command, directory, suite["tiers"][args.tier]["timeout_seconds"])
    metadata = read_metadata(directory / "metadata.tsv")
    result.update({"cell": cell["id"], "arm": arm, "threads": threads, "repetition": repetition,
                   "reference": reference, "directory": str(directory), "metadata": metadata})
    result["status"] = result["process_status"]
    if result["status"] == "ok" and (metadata.get("complete") != "1" or metadata.get("rc") != "0"):
        result["status"] = "error"
    if result["status"] == "ok" and (not (directory / "coefficients.csv").exists() or not (directory / "sample.csv").exists()):
        result["status"] = "error"
    print(f"{cell['id']} {label}: {result['status']} {metadata.get('seconds', '')}s", flush=True)
    return result


def validate_environment(environment):
    lock = {entry["name"]: entry for entry in tomllib.loads((ROOT / "references.lock.toml").read_text())["reference"]}
    for name in ["reghdfe", "ftools", "require", "ppmlhdfe", "fixest"]:
        if environment["source_hashes"].get(name) != lock[name]["source_archive_sha256"]:
            raise ValueError(f"benchmark environment is not pinned: {name}")
        for filename, expected in lock[name].get("required_capture_sha256", {}).items():
            path = Path(environment["references"]) / Path(filename).name
            if path.suffix in {".ado", ".mata", ".sthlp"} and sha256(path) != expected:
                raise ValueError(f"benchmark runtime source changed: {name}/{filename}")


def run(args):
    suite = load_suite()
    if args.engine == "next" and not suite["engine_option"]:
        raise ValueError("engine(next) is unavailable before the M5 seam; it cannot run as legacy")
    cells = selected_cells(suite, args.tier)
    if args.cells:
        names = set(args.cells.split(","))
        if not names <= {cell["id"] for cell in cells}:
            raise ValueError("unknown cell or cell outside this tier")
        cells = [cell for cell in cells if cell["id"] in names]
    plan = [(cell, threads) for cell in cells for threads in thread_counts(suite, args.tier, cell)]
    if platform.system() == "Darwin" and any(threads > 16 for _, threads in plan):
        raise ValueError("Mac timings must use at most 16 native threads; full tier belongs on SCC")
    if args.plan_only:
        print(json.dumps(plan, indent=2))
        return
    environment = json.loads((args.environment / "environment.json").read_text())
    validate_environment(environment)
    if args.root.exists():
        raise ValueError("run root already exists; runs never overwrite or automatically retry commands")
    args.root.mkdir(parents=True)
    started = time.monotonic()
    records = []
    source_sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    source_dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip())
    campaign = {"source_sha": source_sha, "source_dirty": source_dirty, "platform": platform.system().lower(),
                "tier": args.tier, "partial": bool(args.cells), "engine": args.engine, "solver": args.solver,
                "protocol": suite["protocol"], "suite_sha256": sha256(ROOT / "bench/suite.toml"),
                "environment": environment["source_hashes"], "records": records,
                "expected_groups": [[cell["id"], threads, arm] for cell, threads in plan
                    for arm in suite["comparators"][cell["model"]]]}
    for cell in cells:
        if cell["design"] == "veneto":
            if args.veneto_data is None:
                raise ValueError("full tier requires --veneto-data with the prepared external dataset")
            dataset = args.veneto_data
        else:
            dataset = args.root / "data" / (cell["id"] + ".dta")
            generate(cell, suite["seed"], dataset)
        records.append(measure(args, suite, cell, "fereg", 1, "reference", dataset, environment, reference=True))
        for threads in thread_counts(suite, args.tier, cell):
            for arm in suite["comparators"][cell["model"]]:
                for repetition in ["warmup", *range(1, suite["protocol"]["repetitions"]+1)]:
                    record = measure(args, suite, cell, arm, threads, repetition, dataset, environment)
                    records.append(record)
                    (args.root / "results.json").write_text(json.dumps(campaign, indent=2) + "\n")
                    if record["status"] != "ok":
                        break
    campaign["wall_seconds"] = time.monotonic()-started
    campaign["wall_budget_seconds"] = suite["tiers"][args.tier].get("wall_budget_seconds")
    (args.root / "results.json").write_text(json.dumps(campaign, indent=2) + "\n")
    from compare import summarize
    summarize(args.root / "results.json", args.summary)


def arguments():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", choices=["smoke","standard","full"], default="smoke")
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--environment", type=Path, required=True)
    parser.add_argument("--summary", type=Path)
    parser.add_argument("--engine", choices=["legacy","next"], default="legacy")
    parser.add_argument("--solver", choices=["auto","lsmr","cmg"], default="auto")
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN", "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp" if platform.system()=="Darwin" else "stata-mp"))
    parser.add_argument("--rscript", default="Rscript")
    parser.add_argument("--stata-processors", type=int, default=8 if platform.system()=="Darwin" else 4)
    parser.add_argument("--veneto-data", type=Path)
    parser.add_argument("--cells")
    parser.add_argument("--plan-only", action="store_true")
    args = parser.parse_args()
    for name in ["root", "environment", "summary", "veneto_data"]:
        value = getattr(args, name)
        if value is not None:
            setattr(args, name, value.expanduser().resolve())
    return args


if __name__ == "__main__":
    run(arguments())
