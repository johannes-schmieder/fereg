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


def load_suite(manifest=None):
    path = Path(manifest) if manifest else ROOT / "bench/suite.toml"
    return json.loads(path.read_text()) if path.suffix == ".json" else tomllib.loads(path.read_text())


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
    return sorted(set(cell.get("threads", spec["threads"]) + (spec.get("ladder_threads", []) if cell["id"] in spec.get("ladder_cells", []) else [])))


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


def native_identity(engine, root=None):
    root = Path(root) if root else ROOT
    platform_name = {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}[platform.system()]
    name = f"fereg_{'legacy_' if engine == 'legacy' else ''}{platform_name}.plugin"
    plugin = root / 'dist' / name
    binary_hash = sha256(plugin)
    if engine == 'legacy':
        baseline = json.loads((root / 'legacy/baseline.json').read_text())
        if binary_hash != baseline['assets'][platform_name]['sha256']:
            raise ValueError('timings require the canonical frozen comparator')
    build_source = None
    manifest = root / 'dist/RELEASE_MANIFEST.json'
    if manifest.exists():
        metadata = json.loads(manifest.read_text())
        selected = next((p for p in metadata['plugins'] if p['path'] == name), None)
        if selected is None or selected['sha256'] != binary_hash or selected['engine'] != engine:
            raise ValueError('native binary differs from the qualified package manifest')
        build_source = selected['source_sha']
    files = {str(p.relative_to(root)): sha256(p) for p in sorted((root / 'stata').iterdir())
             if p.suffix in {'.ado', '.mo'}}
    frontend_hash = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    return {'plugin_filename': name, 'plugin_sha256': binary_hash, 'build_source_commit': build_source,
            'frontend_sha256': frontend_hash, 'frontend_files': files}


def tooling_identity(manifest=None):
    paths = [ROOT / 'bench' / name for name in ['run.py', 'generate.py', 'compare.py', 'stata_runner.do']]
    paths += [ROOT / 'references.lock.toml', ROOT / 'bench/clock.c', ROOT / 'bench/build_clock.py']
    return {**{str(path.relative_to(ROOT)): sha256(path) for path in paths},
            'manifest': sha256(Path(manifest) if manifest else ROOT / 'bench/suite.toml')}


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
    role = getattr(args, 'artifact_role', '')
    if role:
        label = role + '-' + label
    directory = args.root / "commands" / cell["id"] / label
    directory.mkdir(parents=True)
    settings = {"source": str(getattr(args, "source_root", None) or ROOT), "refs": environment["references"], "r_library": environment["r_library"],
                "clock_plugin":str(getattr(args,'clock_plugin',None) or ''),
                "dataset": str(dataset), "output": str(directory), "arm": arm, "model": cell["model"],
                "k": cell["regressors"], "effects": cell["effects"], "threads": threads,
                "processor_cap": args.stata_processors, "fixed_host_processors": int(args.fereg_only), "slopes": int(cell.get("slopes", False)),
                "outputs": int(bool(cell["outputs"])), "output_mode": cell.get("output_mode", "combined" if cell["outputs"] else "none"),
                "finite_factor": int(cell.get("finite_factor", False)),
                "weights": cell["weights"], "vce": cell["vce"],
                "engine": args.engine, "engine_option": int(suite["engine_option"]),
                "plugin_name": args.native_identity["plugin_filename"],
                "solver": cell.get("solver", "auto" if reference else args.solver), "memory": cell.get("memory", "auto"),
                "bias": cell.get("bias", ""), "separation": cell.get("separation_method", ""),
                "tolerance": suite["protocol"]["reference_tolerance"] if reference else cell.get("tolerance", "")}
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
                   "frontend_sha256": args.native_identity["frontend_sha256"],
                   "requested_solver":settings["solver"], "memory":settings["memory"], "output_mode":settings["output_mode"]})
    result['artifact_role'] = role
    result['timing_identity'] = getattr(args,'timing_identity', {'method':'stata-timer'})
    result["status"] = result["process_status"]
    if result["status"] == "ok" and (metadata.get("complete") != "1" or metadata.get("rc") != "0"):
        result["status"] = "error"
    if arm == "fereg" and result["status"] == "ok" and metadata.get("actual_engine") != args.engine:
        result["status"] = "error"
    if result["status"] == "ok" and (not (directory / "coefficients.csv").exists() or not (directory / "sample.csv").exists()):
        result["status"] = "error"
    print(f"{cell['id']} {label}: {result['status']} {metadata.get('seconds', '')}s", flush=True)
    return result


def paired_arguments(args):
    """Two identified artifacts share data and interleaved fresh-process commands."""
    parent_root = getattr(args, 'paired_source_root', None)
    if parent_root is None:
        return [args]
    if not args.fereg_only or args.solver_comparison:
        raise ValueError('artifact pairs require --fereg-only without solver comparison')
    result = []
    for role, source in [('parent', parent_root), ('candidate', getattr(args, 'source_root', None) or ROOT)]:
        selected = copy.copy(args)
        selected.source_root = source
        selected.artifact_role = role
        selected.native_identity = native_identity(args.engine, source)
        selected.source_sha, selected.source_dirty = source_identity(source)
        if selected.source_dirty:
            raise ValueError('paired artifact source must be clean or an identified snapshot')
        result.append(selected)
    return result


def execute_cell(args, artifacts, suite, cell, dataset, environment, records, save):
    for artifact in artifacts:
        records.append(measure(artifact, suite, cell, 'fereg', 1, 'reference', dataset, environment, reference=True))
    threads = selected_threads(suite, args.tier, cell, args.threads_only)
    reverse = getattr(args, 'reverse_order', False)
    for workers in threads[::-1] if reverse else threads:
        arms = suite['comparators'][cell['model']]
        for arm in arms[::-1] if reverse else arms:
            for repetition in ['warmup', *range(1, suite['protocol']['repetitions']+1)]:
                # Alternate artifact order; the confirmation block flips every pair.
                pair_reverse = reverse ^ (isinstance(repetition, int) and repetition % 2 == 0)
                for artifact in artifacts[::-1] if pair_reverse else artifacts:
                    record = measure(artifact, suite, cell, arm, workers, repetition, dataset, environment)
                    records.append(record)
                    save()
                    if record['status'] != 'ok':
                        break
                if any(r['status'] != 'ok' for r in records[-len(artifacts):]):
                    break


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
    suite = load_suite(getattr(args, "manifest", None))
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
    reverse = getattr(args, "reverse_order", False)
    if reverse:
        cells.reverse()
    plan = [(cell, threads) for cell in cells for threads in selected_threads(suite, args.tier, cell, args.threads_only)]
    if platform.system() == "Darwin" and any(threads > 16 for _, threads in plan):
        raise ValueError("Mac timings must use at most 16 native threads; full tier belongs on SCC")
    if args.plan_only:
        print(json.dumps(plan, indent=2))
        return
    artifact_root = getattr(args, "source_root", None)
    args.native_identity = native_identity(args.engine, artifact_root)
    artifacts = paired_arguments(args)
    clock = getattr(args,'clock_plugin',None)
    if suite.get('program') and clock is None:
        raise ValueError('program protocol requires --clock-plugin; no timing fallback')
    if clock is not None and not args.fereg_only:
        raise ValueError('command clock requires --fereg-only; contextual R arms retain their declared timer')
    args.timing_identity = {'method':'monotonic-command-v1','plugin_sha256':sha256(clock)} if clock else {'method':'stata-timer'}
    for a in artifacts:
        a.timing_identity = args.timing_identity
    environment = json.loads((args.environment / "environment.json").read_text())
    validate_environment(environment)
    if args.root.exists():
        raise ValueError("run root already exists; runs never overwrite or automatically retry commands")
    args.root.mkdir(parents=True)
    started = time.monotonic()
    records = []
    source_sha, source_dirty = source_identity(artifact_root or ROOT)
    tools = tooling_identity(getattr(args, 'manifest', None))
    campaign = {"source_sha": source_sha, "source_dirty": source_dirty, "platform": platform.system().lower(),
                "tier": args.tier, "comparison": "solver" if args.solver_comparison else ("engine" if args.fereg_only else "default"), "partial": bool(args.cells or args.threads_only is not None), "engine": args.engine, "solver": args.solver,
                "protocol": suite["protocol"], "suite_sha256": sha256(getattr(args, "manifest", None) or ROOT / "bench/suite.toml"),
                "environment": environment["source_hashes"], "native_identity": args.native_identity,
                "runner_sha256": sha256(ROOT / "bench/stata_runner.do"),
                "tooling_identity": tools, "datasets": {},
                "program": suite.get('program'),
                "timing_identity": args.timing_identity,
                "paired_artifacts": {a.artifact_role: {'native_identity': a.native_identity, 'source_sha': a.source_sha}
                                     for a in artifacts if getattr(a, 'artifact_role', '')},
                "fereg_only": args.fereg_only, "reverse_order": reverse, "records": records,
                "expected_groups": [[cell["id"], threads, arm, getattr(a, 'artifact_role', '')] for cell, threads in plan
                    for arm in suite["comparators"][cell["model"]] for a in artifacts]}
    for cell in cells:
        if cell["design"] == "veneto":
            if args.veneto_data is None:
                raise ValueError("full tier requires --veneto-data with the prepared external dataset")
            dataset = args.root / "data" / (cell["id"] + ".dta")
            prepare_external(cell, args.veneto_data, dataset)
        else:
            dataset = args.root / "data" / (cell["id"] + ".dta")
            generate(cell, cell.get("seed", suite["seed"]), dataset)
        campaign['datasets'][cell['id']] = sha256(dataset)
        execute_cell(args, artifacts, suite, cell, dataset, environment, records,
                     lambda: (args.root / 'results.json').write_text(json.dumps(campaign, indent=2) + '\n'))
    if any(native_identity(a.engine, getattr(a, 'source_root', None)) != a.native_identity for a in artifacts):
        raise ValueError('native binary or frontend changed during timing')
    if tooling_identity(getattr(args, 'manifest', None)) != tools:
        raise ValueError('benchmark tooling or manifest changed during timing')
    if clock is not None and sha256(clock) != args.timing_identity['plugin_sha256']:
        raise ValueError('command clock binary changed during timing')
    campaign['identities_verified'] = True
    campaign['complete'] = True
    campaign["wall_seconds"] = time.monotonic()-started
    campaign["wall_budget_seconds"] = suite["tiers"][args.tier].get("wall_budget_seconds")
    (args.root / "results.json").write_text(json.dumps(campaign, indent=2) + "\n")
    from compare import summarize
    summarize(args.root / "results.json", args.summary)


def arguments():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", default="smoke")
    parser.add_argument("--manifest", type=Path, help="versioned suite manifest; defaults to suite.toml")
    parser.add_argument("--source-root", type=Path, help="identified frozen source/frontend/dist for baseline measurements")
    parser.add_argument('--paired-source-root', type=Path, help='identified parent/fixed baseline, interleaved with --source-root candidate')
    parser.add_argument('--clock-plugin',type=Path,help='built benchmark-only command stopwatch; mandatory for program protocol')
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
    parser.add_argument("--reverse-order", action="store_true", help="reverse cell, thread and comparator order for the bounded confirmation block")
    args = parser.parse_args()
    for name in ["root", "environment", "summary", "veneto_data", "manifest", "source_root", "paired_source_root", "clock_plugin"]:
        value = getattr(args, name)
        if value is not None:
            setattr(args, name, value.expanduser().resolve())
    return args


if __name__ == "__main__":
    run(arguments())
