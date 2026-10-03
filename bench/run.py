#!/usr/bin/env python3
"""One untimed warm-up and five fresh-process measurements per declared command."""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import time
import tomllib
from generate import generate, prepare_external, sha256

ROOT = Path(__file__).resolve().parents[1]


def load_suite():
    return tomllib.loads((ROOT / "bench/suite.toml").read_text())


def solver_comparison_suite(suite, tier):
    selected = copy.deepcopy(suite)
    subset = selected["solver_comparison"]
    selected["tiers"][tier] = {"cells": subset["cells"], "rows": subset["rows"],
                               "threads": subset["threads"],
                               "timeout_seconds": suite["tiers"][tier]["timeout_seconds"]}
    selected["comparators"] = {"ols": ["fereg"], "ppml": ["fereg"]}
    return selected


def selected_cells(suite, tier):
    spec = suite["tiers"][tier]
    cells = [{**suite["defaults"], **cell} for cell in suite["cells"]]
    cells = [cell for cell in cells if not cell.get("m1_only") or tier == "m1"]
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


def source_identity(root):
    snapshot = root / ".fereg-source.json"
    if snapshot.exists():
        source = json.loads(snapshot.read_text())
        if not re.fullmatch(r"[0-9a-f]{40}", source["commit"]) or not re.fullmatch(r"[0-9a-f]{64}", source["archive_sha256"]):
            raise ValueError("invalid staged source identity")
        return source["commit"], False
    sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True).strip())
    return sha, dirty


def native_identity(engine):
    platform_name = {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}[platform.system()]
    name = f"fereg_{'legacy_' if engine == 'legacy' else ''}{platform_name}.plugin"
    plugin = ROOT / 'dist' / name
    binary_hash = sha256(plugin)
    if engine == 'legacy':
        baseline = json.loads((ROOT / 'legacy/baseline.json').read_text())
        if binary_hash != baseline['assets'][platform_name]['sha256']:
            raise ValueError('timings require the canonical frozen comparator')
    build_source = None
    manifest = ROOT / 'dist/RELEASE_MANIFEST.json'
    if manifest.exists():
        metadata = json.loads(manifest.read_text())
        selected = next((p for p in metadata['plugins'] if p['path'] == name), None)
        if selected is None or selected['sha256'] != binary_hash or selected['engine'] != engine:
            raise ValueError('native binary differs from the qualified package manifest')
        build_source = selected['source_sha']
    files = {str(p.relative_to(ROOT)): sha256(p) for p in sorted((ROOT / 'stata').iterdir())
             if p.suffix in {'.ado', '.mo'}}
    frontend_hash = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    return {'plugin_filename': name, 'plugin_sha256': binary_hash, 'build_source_commit': build_source,
            'frontend_sha256': frontend_hash, 'frontend_files': files}


def selected_threads(suite, tier, cell, only):
    declared = thread_counts(suite, tier, cell)
    if only is not None and only not in declared:
        raise ValueError("thread selection is outside the declared cell/tier")
    return [only] if only is not None else declared


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
                "processor_cap": args.stata_processors, "fixed_host_processors": int(args.fereg_only), "slopes": int(cell.get("slopes", False)),
                "outputs": int(cell["outputs"]), "weights": cell["weights"], "vce": cell["vce"],
                "engine": args.engine, "engine_option": int(suite["engine_option"]),
                "plugin_name": args.native_identity["plugin_filename"],
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
                   "reference": reference, "directory": str(directory), "metadata": metadata,
                   "engine": args.engine, "plugin_sha256": args.native_identity["plugin_sha256"],
                   "native_build_source": args.native_identity["build_source_commit"],
                   "frontend_sha256": args.native_identity["frontend_sha256"]})
    result["status"] = result["process_status"]
    if result["status"] == "ok" and (metadata.get("complete") != "1" or metadata.get("rc") != "0"):
        result["status"] = "error"
    if arm == "fereg" and result["status"] == "ok" and metadata.get("actual_engine") != args.engine:
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
    if args.solver != "auto" and not args.solver_comparison:
        raise ValueError("explicit solvers require --solver-comparison to keep baseline counts separate")
    if args.solver_comparison:
        suite = solver_comparison_suite(suite, args.tier)
    if args.engine == "next" and not suite["engine_option"]:
        raise ValueError("the benchmark suite must enable the engine selector")
    if args.fereg_only:
        suite["comparators"] = {"ols": ["fereg"], "ppml": ["fereg"]}
    cells = selected_cells(suite, args.tier)
    if args.cells:
        names = set(args.cells.split(","))
        if not names <= {cell["id"] for cell in cells}:
            raise ValueError("unknown cell or cell outside this tier")
        cells = [cell for cell in cells if cell["id"] in names]
    plan = [(cell, threads) for cell in cells for threads in selected_threads(suite, args.tier, cell, args.threads_only)]
    if platform.system() == "Darwin" and any(threads > 16 for _, threads in plan):
        raise ValueError("Mac timings must use at most 16 native threads; full tier belongs on SCC")
    if args.plan_only:
        print(json.dumps(plan, indent=2))
        return
    args.native_identity = native_identity(args.engine)
    environment = json.loads((args.environment / "environment.json").read_text())
    validate_environment(environment)
    if args.root.exists():
        raise ValueError("run root already exists; runs never overwrite or automatically retry commands")
    args.root.mkdir(parents=True)
    started = time.monotonic()
    records = []
    source_sha, source_dirty = source_identity(ROOT)
    campaign = {"source_sha": source_sha, "source_dirty": source_dirty, "platform": platform.system().lower(),
                "tier": args.tier, "comparison": "solver" if args.solver_comparison else ("engine" if args.fereg_only else "default"), "partial": bool(args.cells or args.threads_only is not None), "engine": args.engine, "solver": args.solver,
                "protocol": suite["protocol"], "suite_sha256": sha256(ROOT / "bench/suite.toml"),
                "environment": environment["source_hashes"], "native_identity": args.native_identity,
                "fereg_only": args.fereg_only, "records": records,
                "expected_groups": [[cell["id"], threads, arm] for cell, threads in plan
                    for arm in suite["comparators"][cell["model"]]]}
    for cell in cells:
        if cell["design"] == "veneto":
            if args.veneto_data is None:
                raise ValueError("full tier requires --veneto-data with the prepared external dataset")
            dataset = args.root / "data" / (cell["id"] + ".dta")
            prepare_external(cell, args.veneto_data, dataset)
        else:
            dataset = args.root / "data" / (cell["id"] + ".dta")
            generate(cell, suite["seed"], dataset)
        records.append(measure(args, suite, cell, "fereg", 1, "reference", dataset, environment, reference=True))
        for threads in selected_threads(suite, args.tier, cell, args.threads_only):
            for arm in suite["comparators"][cell["model"]]:
                for repetition in ["warmup", *range(1, suite["protocol"]["repetitions"]+1)]:
                    record = measure(args, suite, cell, arm, threads, repetition, dataset, environment)
                    records.append(record)
                    (args.root / "results.json").write_text(json.dumps(campaign, indent=2) + "\n")
                    if record["status"] != "ok":
                        break
    if native_identity(args.engine) != args.native_identity:
        raise ValueError("native binary or frontend changed during timing")
    campaign["wall_seconds"] = time.monotonic()-started
    campaign["wall_budget_seconds"] = suite["tiers"][args.tier].get("wall_budget_seconds")
    (args.root / "results.json").write_text(json.dumps(campaign, indent=2) + "\n")
    from compare import summarize
    summarize(args.root / "results.json", args.summary)


def arguments():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", choices=["smoke","standard","full","m1"], default="smoke")
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--environment", type=Path, required=True)
    parser.add_argument("--summary", type=Path)
    parser.add_argument("--engine", choices=["legacy","next"], default="next")
    parser.add_argument("--fereg-only", action="store_true", help="time only the selected engine; keep external comparator campaigns separate")
    parser.add_argument("--solver-comparison", action="store_true")
    parser.add_argument("--solver", choices=["auto","lsmr","cmg"], default="auto")
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN", "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp" if platform.system()=="Darwin" else "stata-mp"))
    parser.add_argument("--rscript", default="Rscript")
    parser.add_argument("--stata-processors", type=int, default=8 if platform.system()=="Darwin" else 4)
    parser.add_argument("--veneto-data", type=Path)
    parser.add_argument("--cells")
    parser.add_argument("--threads-only", type=int)
    parser.add_argument("--plan-only", action="store_true")
    args = parser.parse_args()
    for name in ["root", "environment", "summary", "veneto_data"]:
        value = getattr(args, name)
        if value is not None:
            setattr(args, name, value.expanduser().resolve())
    return args


if __name__ == "__main__":
    run(arguments())
