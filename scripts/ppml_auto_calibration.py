#!/usr/bin/env python3
"""Prospective PPML auto calibration using the unchanged scientific adjudicator."""
from __future__ import annotations

import argparse
import importlib.util
import itertools
import json
import math
import os
from pathlib import Path
import re
import signal
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "benchmarks/manifests/feppml_auto_calibration_v3.json"
spec = importlib.util.spec_from_file_location("ppml_development", ROOT / "scripts/ppml_solver_development.py")
assert spec and spec.loader
dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dev)


def manifest() -> dict:
    value = dev.read_json(MANIFEST)
    if (value["schema_version"] != 3 or value["qualification_eligible"] is not False
            or value["development_manifest_sha256"] != dev.sha256_file(dev.MANIFEST)):
        raise dev.DevelopmentError("calibration manifest or scientific harness changed")
    if (value["scheduler"]["slots"] != 28
            or value["scheduler"]["h_rt"] != "12:00:00"
            or value["local_validation"]["maximum_seconds"] != 14400):
        raise dev.DevelopmentError("calibration requires 28 SCC slots, <=12-hour jobs and a four-hour local budget")
    return value


def source_files(root: Path) -> dict:
    # Bind actual input bytes, including untracked campaign files, rather than
    # attributing a dirty artifact to its parent Git commit.
    names = subprocess.check_output(
        ["git", "-C", str(root), "ls-files", "-z", "--cached", "--others", "--exclude-standard"]
    ).decode().split("\0")
    prefixes = ("crates/", "plugin/", "stata/", "scripts/", "tests/", "benchmarks/manifests/", "benchmarks/scc/")
    fixed = {"Cargo.toml", "Cargo.lock", "references.lock.toml", "rust-toolchain.toml", ".cargo/config.toml"}
    return {name: dev.sha256_file(root / name) for name in sorted(set(names))
            if (name in fixed or name.startswith(prefixes)) and (root / name).is_file()}


def identity(root: Path) -> dict:
    files = source_files(root)
    return {"files": files, "sha256": dev.canonical_sha256(files),
            "git": dev.source_identity(root), "artifact": dev.artifact_identity(root)}


def controller_identity(candidate: Path) -> dict:
    files = source_files(ROOT)
    original = source_files(candidate)
    operational = {"scripts/ppml_auto_calibration.py", "tests/python/test_ppml_auto_calibration.py"}
    operational.update(f"benchmarks/scc/feppml_auto_calibration_{name}.sh" for name in
                       ("job", "submit", "setup_job", "policy_job", "collect_job", "placement_job"))
    if any(files.get(k) != original.get(k) for k in files.keys() | original.keys() if k not in operational):
        raise dev.DevelopmentError("placement controller changes scientific source or manifests")
    return {"root": str(ROOT), "git": dev.source_identity(ROOT), "sha256": dev.canonical_sha256(files)}


def check_controller(root: Path) -> dict:
    config = dev.read_json(root / "input/config.json")
    submission = root / "receipts/submission.json"
    if submission.exists() and dev.read_json(submission)["config"] != dev.file_receipt(root / "input/config.json"):
        raise dev.DevelopmentError("submitted campaign configuration changed")
    if config["controller"] != controller_identity(Path(config["candidate_root"])):
        raise dev.DevelopmentError("execution controller changed after preparation")
    for receipt in config.get("recovery", {}).get("original_inputs", []):
        if dev.file_receipt(Path(receipt["path"])) != receipt:
            raise dev.DevelopmentError("original campaign input changed during recovery")
    return config


def scheduled_task(config: dict, stage: str, index: int) -> int:
    tasks = config.get("recovery", {}).get("retry_tasks") if stage == "scc_train" else None
    inventory = tasks if tasks is not None else list(range(1, task_count(stage) + 1))
    if not 1 <= index <= len(inventory):
        raise dev.DevelopmentError("scheduler index outside submitted inventory")
    return inventory[index - 1]


def scopes(stage: str, task: int | None = None) -> list[dict]:
    m = manifest()
    d = dev.load_manifest()
    if stage == "scc_train":
        result = [scope for p in m["training_profiles"] for scope in dev.profile_scopes(d, p)]
    elif stage == "scc_holdout":
        result = holdout_scopes(m, "local_holdout") + holdout_scopes(m, "scc")
    else:
        raise dev.DevelopmentError("unknown calibration stage")
    dev._validate_scopes(result, len(result))
    if task is not None:
        tasks = partition_scopes(result, m["partition"])
        if not 1 <= task <= len(tasks):
            raise dev.DevelopmentError("SCC task outside registered inventory")
        return tasks[task - 1]
    return result


def partition_scopes(scopes: list[dict], policy: dict) -> list[list[dict]]:
    tasks, pending = [], []
    for scope in scopes:
        isolated = (scope["topology"] == policy["isolated_topology"]
                    and scope["controls"] >= policy["isolated_minimum_controls"])
        if isolated:
            if pending:
                tasks.append(pending)
                pending = []
            tasks.append([scope])
        else:
            pending.append(scope)
            if len(pending) == policy["maximum_scopes_per_task"]:
                tasks.append(pending)
                pending = []
    if pending:
        tasks.append(pending)
    return tasks


def task_count(stage: str) -> int:
    return len(partition_scopes(scopes(stage), manifest()["partition"]))


def holdout_scopes(m: dict, stage: str) -> list[dict]:
    result = []
    threads = m["local_holdout_threads"] if stage == "local_holdout" else m["scc_threads"]
    sizes = [m["local_holdout_n"]] if stage == "local_holdout" else m["scc_n"]
    seed_base = m["holdout_seed"] if stage == "local_holdout" else m["scc_seed"]
    for topology_index, topology in enumerate(m["holdout_topologies"]):
        hard = topology in {"chain_2fe", "many_levels_2fe", "sparse_3fe"}
        for k_index, k in enumerate(m["holdout_controls"]):
            actual_sizes = [m["scc_hard_n"]] if stage == "scc" and hard else sizes
            for n_index, n in enumerate(actual_sizes):
                seed = seed_base + 100 * topology_index + 10 * k_index + n_index
                for t in threads:
                    result.append({"id": f"{stage}-{topology}-n{n}-k{k}-t{t}",
                                   "n": n, "controls": k, "topology": topology,
                                   "difficulty": "long" if hard else "easy",
                                   "threads": t, "seed": seed})
    return result


def features(scope: dict, metadata: dict) -> dict:
    levels = sorted((int(metadata[k]) for k in ("levels1", "levels2", "levels3") if int(metadata[k]) > 0), reverse=True)
    return {"effects": len(levels), "controls": scope["controls"], "threads": scope["threads"],
            "observations_per_largest_level": int(metadata["N"]) / levels[0],
            "core_level_ratio": levels[0] / levels[1] if len(levels) > 1 else 1,
            "nonbase_nuisance_columns": sum(x - 1 for x in levels[2:])}


def matches(row: dict, predicates: list) -> bool:
    return all((row["features"][name] <= cutoff) == lower for name, cutoff, lower in predicates)


def default_solver(row: dict) -> str:
    return "map" if row["features"]["effects"] == 1 else "lsmr"


def select(row: dict, policy: dict) -> str:
    if row["features"]["effects"] == 1:
        return "map"
    for region in policy["regions"]:
        eligible = (region["solver"] != "cmg" or
                    row["features"]["effects"] in {2, 3} and row["features"]["nonbase_nuisance_columns"] <= 64)
        if matches(row, region["predicates"]) and eligible:
            return region["solver"]
    return "lsmr"


def dataset_geomean(rows: list[dict], ratios: list[float]) -> float:
    grouped = {}
    for row, ratio in zip(rows, ratios, strict=True):
        if not math.isfinite(ratio) or ratio <= 0:
            raise dev.DevelopmentError("invalid calibration timing ratio")
        grouped.setdefault(row["dataset_id"], []).append(math.log(ratio))
    return math.exp(statistics.mean(statistics.mean(v) for v in grouped.values()))


def fit(rows: list[dict]) -> dict:
    p = manifest()["policy"]
    multi = [r for r in rows if r["features"]["effects"] > 1]
    if not multi or any("lsmr" not in r["times"] for r in multi):
        raise dev.DevelopmentError("training lacks a certified LSMR default in every multi-FE cell")
    predicates = [(name, cutoff, lower) for name, values in p["features"].items()
                  for cutoff in values for lower in (True, False)]
    best_score = 1.0
    best_region = None
    for width in range(1, p["maximum_region_predicates"] + 1):
        for ps in itertools.combinations(predicates, width):
            if len({x[0] for x in ps}) != width:
                continue
            region = [r for r in multi if matches(r, list(ps))]
            if len({r["dataset_id"] for r in region}) < p["minimum_training_datasets"]:
                continue
            for solver in p["candidate_region_solvers"]:
                if any(solver not in r["times"] for r in region):
                    continue
                ratios = [r["times"][solver] / r["times"]["lsmr"] for r in region]
                if (dataset_geomean(region, ratios) > 1 - p["minimum_geomean_gain"]
                        or max(ratios) > p["maximum_cell_ratio"]
                        or sum(x < 1 for x in ratios) / len(ratios) < p["minimum_win_fraction"]):
                    continue
                score = dataset_geomean(multi, [r["times"][solver] / r["times"]["lsmr"]
                                                if matches(r, list(ps)) else 1.0 for r in multi])
                if score < best_score - 1e-12:
                    best_score = score
                    best_region = {"solver": solver, "predicates": list(ps)}
    return {"one_fe": "map", "default_multi_fe": "lsmr", "unsupported_or_unadmitted": "map",
            "regions": [best_region] if best_region else [], "training_ratio_to_default": best_score}


def validate_policy(rows: list[dict], policy: dict) -> dict:
    ratios, selected, missing = [], [], []
    for row in rows:
        solver = select(row, policy)
        selected.append(solver)
        if solver not in row["times"] or default_solver(row) not in row["times"]:
            missing.append(row["scope_id"])
            continue
        ratios.append(row["times"][solver] / row["times"][default_solver(row)])
    if missing:
        return {"pass": False, "missing_or_uncertified_selected_route": missing}
    p = manifest()["policy"]
    gm = dataset_geomean(rows, ratios)
    return {"pass": gm <= p["holdout_maximum_geomean_ratio"] and max(ratios) <= p["holdout_maximum_cell_ratio"],
            "geomean_to_default": gm, "worst_to_default": max(ratios),
            "selected_counts": {s: selected.count(s) for s in dev.SOLVERS},
            "geomean_to_map": dataset_geomean(rows, [r["times"][s] / r["times"]["map"] for r, s in zip(rows, selected)]),
            "geomean_to_ppmlhdfe": dataset_geomean(rows, [r["times"][s] / r["reference"] for r, s in zip(rows, selected)]),
            "geomean_to_hindsight": dataset_geomean(rows, [r["times"][s] / min(r["times"].values()) for r, s in zip(rows, selected)])}


def rows_from_run(run: dict, analysis: dict) -> list[dict]:
    if not analysis["science_pass"]:
        raise dev.DevelopmentError("scientific failure cannot enter calibration")
    rows = []
    for dataset in run["datasets"]:
        scope = dataset["scope"]
        summaries = [s for s in analysis["solver_summary"] if s["scope_id"] == scope["id"]]
        rows.append({"scope_id": scope["id"], "dataset_id": dataset["dataset_id"],
                     "features": features(scope, dataset["metadata"]),
                     "times": {s["solver"]: s["candidate_median_seconds"] for s in summaries if s["performance_eligible"]},
                     "reference": summaries[0]["ppmlhdfe_median_seconds"]})
    return rows


def memory_check(document: dict) -> dict:
    ratios, failures = [], []
    for row in document["records"]:
        if row.get("implementation") != "candidate" or row.get("arm") not in dev.SOLVERS or row.get("rc") != 0:
            continue
        values = row.get("runtime_values", [])
        ratio = values[22] if len(values) == 108 else None
        if ratio is None or not math.isfinite(ratio) or ratio < 0:
            failures.append(f"{row['scope_id']}:{row['arm']}:missing_allocation_model")
        else:
            ratios.append(ratio)
            if ratio > manifest()["maximum_native_allocation_over_model"]:
                failures.append(f"{row['scope_id']}:{row['arm']}:allocation_model")
    return {"pass": bool(ratios) and not failures, "maximum_allocation_over_model": max(ratios, default=None),
            "failures": sorted(set(failures))}


def host_receipt(path: Path) -> dict:
    captured = path.read_text()
    values = {}
    for key in ("m_core", "cpu_arch", "cpu_type"):
        matches = set(re.findall(rf"(?:^|[:\s]){key}=([^\s]+)", captured, re.MULTILINE))
        if len(matches) != 1:
            raise dev.DevelopmentError(f"SCC host receipt lacks a unique {key}")
        values[key] = matches.pop()
    if float(values["m_core"]) != 28:
        raise dev.DevelopmentError("calibration requires a 28-core host")
    return {"sha256": dev.sha256_file(path), "resources": values}


def run(args: argparse.Namespace) -> None:
    m = manifest()
    planned = scopes(args.stage, args.task)
    if (args.stage == "scc_train") != (args.policy is None):
        raise dev.DevelopmentError("training must precede the policy; holdout requires it")
    if args.stage == "scc_holdout":
        policy = dev.read_json(args.policy)
        if policy["manifest_sha256"] != dev.sha256_file(MANIFEST):
            raise dev.DevelopmentError("holdout policy does not match frozen campaign")
    if os.environ.get("NSLOTS") != "28" or not os.environ.get("JOB_ID", "").isdigit():
        raise dev.DevelopmentError("SCC calibration requires an actual 28-slot SGE allocation")
    initial = identity(args.candidate_root)
    root = dev.validate_run_root(args.run_root)
    config = check_controller(root)
    scheduler_task = int(os.environ.get("SGE_TASK_ID", "0"))
    setup = dev.read_json(root / "receipts/setup.json")
    submission = dev.read_json(root / "receipts/submission.json")
    if (initial != setup["source"]
            or os.environ["JOB_ID"] != submission["jobs"][args.stage]
            or args.task != scheduled_task(config, args.stage, scheduler_task)):
        raise dev.DevelopmentError("SCC task is not bound to its setup and submission")
    if args.policy and policy["source_sha256"] != initial["sha256"]:
        raise dev.DevelopmentError("holdout source differs from frozen training policy")
    host = host_receipt(root / "work" / f"qhost-{args.stage}-{args.task:03d}.txt")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    prospective = {"manifest_sha256": dev.sha256_file(MANIFEST), "source": initial,
                   "stage": args.stage, "task": args.task, "scopes": planned,
                   "policy_sha256": dev.sha256_file(args.policy) if args.policy else None}
    dev.exclusive_json(args.output_dir / "prospective.json", prospective)
    document = dev.execute_scopes(
        dev.load_manifest(), planned, stata=args.stata, candidate_root=args.candidate_root,
        baseline_root=None, reference_root=args.reference_root, output_dir=args.output_dir / "capture",
        repetitions=m["training_repetitions"] if args.stage == "scc_train" else m["holdout_repetitions"],
        prepare_comparators=True,
        task_identity=({"job_id": os.environ["JOB_ID"], "task_id": scheduler_task,
                        "slots": 28, "scheduler_host": host, **dev.cpu_identity()}),
    )
    dev.exclusive_json(args.output_dir / "capture/run.json", document)
    analysis = dev.analyze_document(document)
    dev.exclusive_json(args.output_dir / "capture/analysis.json", analysis)
    after = identity(args.candidate_root)
    check_controller(root)
    if initial["sha256"] != after["sha256"] or initial["artifact"] != after["artifact"]:
        raise dev.DevelopmentError("source or artifact changed during calibration")
    rows = rows_from_run(document, analysis) if analysis["science_pass"] else []
    receipt = {"schema_version": 3, "kind": "feppml_auto_calibration_measurement",
               "qualification_eligible": False, "auto_calibration": True,
               "prospective_sha256": dev.sha256_file(args.output_dir / "prospective.json"),
               "manifest_sha256": prospective["manifest_sha256"], "source": initial,
               "stage": args.stage, "task": args.task,
               "run_sha256": dev.sha256_file(args.output_dir / "capture/run.json"),
               "analysis_sha256": dev.sha256_file(args.output_dir / "capture/analysis.json"),
               "science_pass": analysis["science_pass"], "route_pass": analysis["route_pass"],
               "memory": memory_check(document), "rows": rows}
    if args.policy and rows:
        receipt["policy_validation"] = validate_policy(rows, policy["policy"])
    dev.exclusive_json(args.output_dir / "receipt.json", receipt)
    if (not analysis["science_pass"] or not analysis["route_pass"]
            or not receipt["memory"]["pass"]):
        raise dev.DevelopmentError("calibration task failed science or route checks; evidence preserved")
    print(args.output_dir, flush=True)


def fit_runs(args: argparse.Namespace) -> None:
    rows, receipts, sources, seen_tasks = [], [], set(), set()
    for root in args.runs:
        task = dev.read_json(root / "receipt.json")["task"]
        receipt = verify_measurement(root, "scc_train", task)
        if receipt["task"] in seen_tasks:
            raise dev.DevelopmentError("duplicate training task")
        seen_tasks.add(receipt["task"])
        sources.add(dev.canonical_sha256(receipt["source"]))
        receipts.append(dev.file_receipt(root / "receipt.json"))
        rows.extend(receipt["rows"])
    if seen_tasks != set(range(1, task_count("scc_train") + 1)) or len(sources) != 1:
        raise dev.DevelopmentError("training campaign is incomplete or mixes source snapshots")
    policy = fit(rows)
    dev.exclusive_json(args.output, {"schema_version": 3, "manifest_sha256": dev.sha256_file(MANIFEST),
                                    "source_sha256": receipt["source"]["sha256"], "inputs": receipts,
                                    "policy": policy, "training": validate_policy(rows, policy)})
    print(args.output)


def verify_measurement(root: Path, stage: str, task: int | None) -> dict:
    receipt = dev.read_json(root / "receipt.json")
    prospective = dev.read_json(root / "prospective.json")
    document = dev.read_json(root / "capture/run.json")
    analysis = dev.analyze_document(document)
    if (receipt["stage"] != stage or receipt["task"] != task
            or prospective["stage"] != stage or prospective["task"] != task
            or receipt["manifest_sha256"] != dev.sha256_file(MANIFEST)
            or receipt["prospective_sha256"] != dev.sha256_file(root / "prospective.json")
            or receipt["run_sha256"] != dev.sha256_file(root / "capture/run.json")
            or receipt["analysis_sha256"] != dev.sha256_file(root / "capture/analysis.json")
            or prospective["scopes"] != scopes(stage, task)
            or document["scopes"] != prospective["scopes"]
            or receipt["source"] != prospective["source"]
            or document["sources"]["candidate"] != receipt["source"]["git"]
            or document["artifacts"]["candidate"] != receipt["source"]["artifact"]
            or analysis != dev.read_json(root / "capture/analysis.json")
            or not analysis["science_pass"] or not analysis["route_pass"]
            or memory_check(document) != receipt["memory"] or not receipt["memory"]["pass"]
            or rows_from_run(document, analysis) != receipt["rows"]):
        raise dev.DevelopmentError("calibration measurement does not replay")
    return receipt


def local_validation_jobs() -> list[tuple[str, str, list[str]]]:
    jobs = [(p, p, []) for p in manifest()["local_validation"]["profiles"]]
    jobs.append(("modelspace-regression", "local_checkpoint", ["--scope",
                 manifest()["local_validation"]["required_regression_scope"], "--repetitions", "1"]))
    return jobs


def run_local_validation(args: argparse.Namespace) -> None:
    budget = args.budget_seconds
    if not 0 < budget <= manifest()["local_validation"]["maximum_seconds"]:
        raise dev.DevelopmentError("local validation exceeds the four-hour budget")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    initial = identity(args.candidate_root)
    start = time.monotonic()
    for name, profile, extra in local_validation_jobs():
        command = [sys.executable, str(args.candidate_root / "scripts/ppml_solver_development.py"),
                   "run-local", "--profile", profile, "--candidate-root", str(args.candidate_root),
                   "--reference-root", str(args.reference_root), "--stata", args.stata,
                   "--output-dir", str(args.output_dir / name), *extra]
        with (args.output_dir / f"{name}.log").open("x") as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                rc = process.wait(timeout=max(0, budget - (time.monotonic() - start)))
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                raise dev.DevelopmentError("local validation reached its budget; partial evidence is not accepted")
        if rc != 0:
            raise dev.DevelopmentError(f"local {name} failed; do not submit calibration")
        print(f"local {name} passed; elapsed={time.monotonic() - start:.1f}s", flush=True)
    if identity(args.candidate_root) != initial:
        raise dev.DevelopmentError("local source or artifact changed")
    dev.exclusive_json(args.output_dir / "local-validation.json", {
        "manifest_sha256": dev.sha256_file(MANIFEST), "source": initial,
        "budget_seconds": budget, "elapsed_seconds": time.monotonic() - start,
        "gates": {name: {"run": dev.sha256_file(args.output_dir / name / "run.json"),
                         "analysis": dev.sha256_file(args.output_dir / name / "analysis.json")}
                  for name, _, _ in local_validation_jobs()},
    })


def verify_local_validation(path: Path, candidate: Path) -> dict:
    receipt = dev.read_json(path / "local-validation.json")
    if (receipt["manifest_sha256"] != dev.sha256_file(MANIFEST)
            or not 0 < receipt["elapsed_seconds"] <= receipt["budget_seconds"] <= 14400
            or receipt["source"]["sha256"] != dev.canonical_sha256(source_files(candidate))
            or set(receipt["gates"]) != {name for name, _, _ in local_validation_jobs()}):
        raise dev.DevelopmentError("local validation source, budget or inventory differs")
    for name, profile, extra in local_validation_jobs():
        document = dev.read_json(path / name / "run.json")
        analysis = dev.analyze_document(document)
        expected = dev.profile_scopes(dev.load_manifest(), profile)
        repetitions = dev.load_manifest()["profiles"][profile]["repetitions"]
        if extra:
            expected = [s for s in expected if s["id"] == extra[1]]
            repetitions = 1
        if (document["scopes"] != expected or document["repetitions"] != repetitions
                or document["sources"]["candidate"] != receipt["source"]["git"]
                or document["artifacts"]["candidate"] != receipt["source"]["artifact"]
                or document["host"]["system"] != "Darwin"
                or not analysis["science_pass"] or not analysis["route_pass"]
                or not memory_check(document)["pass"]
                or analysis != dev.read_json(path / name / "analysis.json")
                or receipt["gates"][name] != {"run": dev.sha256_file(path / name / "run.json"),
                                             "analysis": dev.sha256_file(path / name / "analysis.json")}):
            raise dev.DevelopmentError(f"local gate {name} does not replay")
    return receipt


def prepare_scc(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    local = verify_local_validation(args.local_validation, args.candidate_root)
    if root.exists() and any(root.iterdir()):
        raise dev.DevelopmentError("SCC run directory must be new")
    for name in ("input", "work", "logs", "shards", "receipts", "output"):
        (root / name).mkdir(parents=True, exist_ok=True)
    dev.exclusive_json(root / "input/config.json", {
        "candidate_root": str(args.candidate_root.resolve()),
        "controller": controller_identity(args.candidate_root),
        "reference_root": str(args.reference_root.resolve()), "stata": args.stata,
        "manifest_sha256": dev.sha256_file(MANIFEST),
        "source_sha256": local["source"]["sha256"],
        "local_validation": dev.file_receipt(args.local_validation / "local-validation.json"),
        "tasks": {stage: [{"task": t, "scopes": scopes(stage, t)}
                           for t in range(1, task_count(stage) + 1)]
                  for stage in ("scc_train", "scc_holdout")},
    })
    print(root)


def placement_history(original: Path, evidence: Path, retries: list[int] | None = None,
                      *, complete: bool = False) -> tuple[list[dict], list[int]]:
    job = dev.read_json(original / "receipts/submission.json")["jobs"]["scc_train"]
    if not dev.JOB_ID.fullmatch(job):
        raise dev.DevelopmentError("invalid original training job")
    captured = subprocess.run(["qacct", "-j", job], text=True, capture_output=True, check=False)
    path = evidence / "logs" / f"qacct-original-training-{time.time_ns()}.txt"
    dev.exclusive_text(path, captured.stdout)
    if captured.returncode:
        raise dev.DevelopmentError("original training accounting unavailable")
    rows = dev.parse_qacct(path)
    rejected = validate_placement_history(original, rows, job, complete=complete)
    if retries is not None and rejected != retries:
        raise dev.DevelopmentError("original placement failures changed during recovery")
    return rows, rejected


def validate_placement_history(original: Path, rows: list[dict], job: str,
                               *, complete: bool) -> list[int]:
    required = dev.load_manifest()["receipt_contract"]["qacct_required"]
    observed, rejected = [], []
    for row in rows:
        if set(required) - row.keys():
            raise dev.DevelopmentError("incomplete original accounting record")
        task = int(row["taskid"])
        observed.append(task)
        elapsed = float(row["ru_wallclock"])
        if (row["jobnumber"] != job or row["owner"] != "johannes" or row["project"] != "welfgr"
                or int(row["slots"]) != 28 or not 0 <= elapsed <= 43200):
            raise dev.DevelopmentError("original accounting identity or budget differs")
        if row["failed"] == "0" and row["exit_status"] == "0":
            continue
        # Only the demonstrated pre-estimation placement rejection is retryable.
        log = original / "logs" / f"feppml_auto_cal.o{job}.{task}"
        host = original / "work" / f"qhost-scc_train-{task:03d}.txt"
        cores = re.findall(r"(?:^|[:\s])m_core=([^\s]+)", host.read_text())
        if (row["failed"] != "0" or row["exit_status"] != "1" or elapsed > 10
                or log.read_text().strip() != "PPML auto calibration failed: calibration requires a 28-core host"
                or len(cores) != 1 or not math.isfinite(float(cores[0])) or float(cores[0]) == 28
                or (original / "shards/scc_train" / f"task-{task:03d}").exists()):
            raise dev.DevelopmentError("scientific or ambiguous failure is not a placement retry")
        rejected.append(task)
    expected = set(range(1, task_count("scc_train") + 1))
    if (len(set(observed)) != len(observed) or not set(observed) <= expected
            or (complete and set(observed) != expected)):
        raise dev.DevelopmentError("original accounting task inventory differs")
    return sorted(rejected)


def prepare_recovery(args: argparse.Namespace) -> None:
    root, original = dev.validate_run_root(args.run_root), dev.validate_run_root(args.original_root)
    config = dev.read_json(original / "input/config.json")
    setup = dev.read_json(original / "receipts/setup.json")
    if ("recovery" in config or (original / "input/policy.json").exists()
            or any((original / "shards/scc_holdout").glob("task-*"))):
        raise dev.DevelopmentError("recovery is single-attempt and must precede policy/holdouts")
    candidate = Path(config["candidate_root"])
    if (identity(candidate) != setup["source"] or config["manifest_sha256"] != dev.sha256_file(MANIFEST)
            or not (original / "work/SETUP.SUCCESS").is_file()):
        raise dev.DevelopmentError("recovery source or setup differs")
    controller = controller_identity(candidate)
    if set(setup["gates"]) != {"local_science", "local_one_fe_science", "local_smoke"}:
        raise dev.DevelopmentError("recovery setup gate inventory differs")
    for name, receipt in setup["gates"].items():
        document = dev.read_json(original / "work" / name / "run.json")
        analysis = dev.analyze_document(document)
        if (dev.file_receipt(Path(receipt["path"])) != receipt
                or analysis != dev.read_json(Path(receipt["path"]))
                or not analysis["science_pass"] or not analysis["route_pass"]
                or not memory_check(document)["pass"]
                or document["artifacts"]["candidate"] != setup["source"]["artifact"]):
            raise dev.DevelopmentError("reused setup gate does not replay")
    if root.exists():
        raise dev.DevelopmentError("recovery requires a new run root")
    for name in ("input", "work", "logs", "shards", "receipts", "output"):
        (root / name).mkdir(parents=True, exist_ok=True)
    _, retries = placement_history(original, root)
    if not retries:
        raise dev.DevelopmentError("no proven placement failures to recover")
    recovery = {"original_root": str(original), "retry_tasks": retries,
                "original_inputs": [dev.file_receipt(original / p) for p in
                                    ("input/config.json", "receipts/setup.json", "receipts/submission.json")]}
    job = dev.read_json(original / "receipts/submission.json")["jobs"]["scc_train"]
    recovery["original_inputs"].extend(dev.file_receipt(original / p) for t in retries for p in
                                      (f"logs/feppml_auto_cal.o{job}.{t}", f"work/qhost-scc_train-{t:03d}.txt"))
    dev.exclusive_json(root / "input/config.json", {**config, "controller": controller, "recovery": recovery})
    dev.exclusive_json(root / "receipts/setup.json", setup)
    dev.exclusive_text(root / "work/SETUP.SUCCESS", "FEPPML_AUTO_SETUP_REUSED\n")
    print(root)


def record_setup(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    config = dev.read_json(root / "input/config.json")
    candidate = Path(config["candidate_root"])
    current = identity(candidate)
    submission = dev.read_json(root / "receipts/submission.json")
    if (current["sha256"] != config["source_sha256"]
            or os.environ["JOB_ID"] != submission["jobs"]["setup"]):
        raise dev.DevelopmentError("SCC build source differs from local calibration")
    gates = {}
    for name in ("local_science", "local_one_fe_science", "local_smoke"):
        path = root / "work" / name
        document = dev.read_json(path / "run.json")
        replay = dev.analyze_document(document)
        if (document["scopes"] != dev.profile_scopes(dev.load_manifest(), name)
                or document["artifacts"]["candidate"] != current["artifact"]
                or document["sources"]["candidate"] != current["git"]
                or replay != dev.read_json(path / "analysis.json")
                or not memory_check(document)["pass"]
                or not replay["science_pass"] or not replay["route_pass"]):
            raise dev.DevelopmentError("SCC compute-node setup gate failed")
        gates[name] = dev.file_receipt(path / "analysis.json")
    dev.exclusive_json(root / "receipts/setup.json", {
        "source": current, "gates": gates, "job_id": os.environ["JOB_ID"],
        "references": dev.reference_identity(Path(config["reference_root"])),
    })
    dev.exclusive_text(root / "work/SETUP.SUCCESS", "FEPPML_AUTO_SETUP_PASS\n")


def accept_accounting(root: Path, stage: str) -> list[dict]:
    submission = dev.read_json(root / "receipts/submission.json")
    job = submission["jobs"][stage]
    expected = submission["array_tasks"][stage] if stage.startswith("scc_") else [0]
    required = dev.load_manifest()["receipt_contract"]["qacct_required"]
    for attempt in range(13):
        path = root / "logs" / f"qacct-{stage}-{time.time_ns()}-{attempt:02d}.txt"
        captured = subprocess.run(["qacct", "-j", job], text=True, capture_output=True, check=False)
        dev.exclusive_text(path, captured.stdout)
        rows = dev.parse_qacct(path)
        if len(rows) == len(expected):
            accepted = dev._accept_qacct(path, job, expected, submission["slots"][stage], required)
            if any(float(r["ru_wallclock"]) > 43200 for r in accepted):
                raise dev.DevelopmentError("SCC task exceeded the 12-hour budget")
            return accepted
        if attempt < 12:
            time.sleep(10)
    raise dev.DevelopmentError(f"accounting remains incomplete for {stage}")


def verify_task_accounting(path: Path, task: int, rows: list[dict], scheduler_task: int | None = None) -> None:
    task = task if scheduler_task is None else scheduler_task
    identity = dev.read_json(path / "capture/run.json")["task_identity"]
    account = next((r for r in rows if int(r["taskid"]) == task), None)
    if (account is None or identity["job_id"] != account["jobnumber"]
            or identity["task_id"] != task or identity["slots"] != 28
            or identity["host"].split(".")[0] != account["hostname"].split(".")[0]):
        raise dev.DevelopmentError("task execution identity differs from scheduler accounting")


def verify_host_receipt(root: Path, stage: str, task: int) -> None:
    measured = dev.read_json(root / "shards" / stage / f"task-{task:03d}/capture/run.json")
    if measured["task_identity"]["scheduler_host"] != host_receipt(root / "work" / f"qhost-{stage}-{task:03d}.txt"):
        raise dev.DevelopmentError("task host receipt differs from captured scheduler resources")


def fit_scc(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    config = check_controller(root)
    accept_accounting(root, "setup")
    accounting = accept_accounting(root, "scc_train")
    setup = dev.read_json(root / "receipts/setup.json")
    submission = dev.read_json(root / "receipts/submission.json")
    if os.environ["JOB_ID"] != submission["jobs"]["policy"]:
        raise dev.DevelopmentError("policy execution differs from scheduler submission")
    recovery = config.get("recovery")
    original_rows = []
    if recovery:
        original_rows, _ = placement_history(Path(recovery["original_root"]), root,
                                             recovery["retry_tasks"], complete=True)
    runs = []
    for t in range(1, task_count("scc_train") + 1):
        origin, physical, rows = root, t, accounting
        if recovery:
            if t in recovery["retry_tasks"]:
                physical = recovery["retry_tasks"].index(t) + 1
            else:
                origin, rows = Path(recovery["original_root"]), original_rows
        path = origin / "shards/scc_train" / f"task-{t:03d}"
        runs.append(path)
        verify_task_accounting(path, t, rows, physical)
        verify_host_receipt(origin, "scc_train", t)
        if verify_measurement(path, "scc_train", t)["source"] != setup["source"]:
            raise dev.DevelopmentError("training differs from setup source/artifact")
    fit_runs(argparse.Namespace(runs=runs, output=root / "input/policy.json"))
    dev.exclusive_json(root / "receipts/policy.json", {
        "job_id": os.environ["JOB_ID"], "accounting": accounting,
        "original_training_accounting": original_rows,
        "policy_sha256": dev.sha256_file(root / "input/policy.json"),
    })
    dev.exclusive_text(root / "work/POLICY.SUCCESS", "FEPPML_AUTO_POLICY_FROZEN\n")


def collect(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    check_controller(root)
    setup = dev.read_json(root / "receipts/setup.json")
    policy = dev.read_json(root / "input/policy.json")
    frozen = dev.read_json(root / "receipts/policy.json")
    submission = dev.read_json(root / "receipts/submission.json")
    if (frozen["policy_sha256"] != dev.sha256_file(root / "input/policy.json")
            or frozen["job_id"] != submission["jobs"]["policy"]
            or os.environ["JOB_ID"] != submission["jobs"]["collector"]):
        raise dev.DevelopmentError("collector or frozen policy differs from scheduler submission")
    rows, measurements = [], []
    accounting = {stage: accept_accounting(root, stage) for stage in ("policy", "scc_holdout")}
    for t in range(1, task_count("scc_holdout") + 1):
        task_root = root / "shards/scc_holdout" / f"task-{t:03d}"
        verify_task_accounting(task_root, t, accounting["scc_holdout"])
        verify_host_receipt(root, "scc_holdout", t)
        receipt = verify_measurement(task_root, "scc_holdout", t)
        prospective = dev.read_json(task_root / "prospective.json")
        if (receipt["source"] != setup["source"]
                or prospective["policy_sha256"] != dev.sha256_file(root / "input/policy.json")):
            raise dev.DevelopmentError("SCC task differs from setup or frozen policy")
        rows.extend(receipt["rows"])
        measurements.append(dev.file_receipt(task_root / "receipt.json"))
    result = {population: validate_policy([r for r in rows if r["scope_id"].startswith(population + "-")], policy["policy"])
              for population in ("local_holdout", "scc")}
    dev.exclusive_json(root / "output/analysis.json", {
        "qualification_eligible": False, "scheduler_acceptance": "pending_qacct",
        "policy_validation": result, "measurements": measurements, "rows": rows, "accounting": accounting,
    })
    if not all(r["pass"] for r in result.values()):
        raise dev.DevelopmentError("SCC policy validation failed")
    dev.exclusive_text(root / "output/APPLICATION.SUCCESS", "FEPPML_AUTO_APPLICATION_PASS\n")


def record_submission(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    config = check_controller(root)
    jobs = {stage: getattr(args, stage + "_job") for stage in ("setup", "scc_train", "policy", "scc_holdout", "collector")}
    if any(not dev.JOB_ID.fullmatch(j) for j in jobs.values()) or len(set(jobs.values())) != len(jobs):
        raise dev.DevelopmentError("invalid or duplicate scheduler submission identity")
    dev.exclusive_json(root / "receipts/submission.json", {
        "jobs": jobs, "slots": {"setup": 4, "scc_train": 28, "policy": 1, "scc_holdout": 28, "collector": 1},
        "array_tasks": {s: list(range(1, (len(config["recovery"]["retry_tasks"])
                        if s == "scc_train" and "recovery" in config else task_count(s)) + 1))
                        for s in ("scc_train", "scc_holdout")},
        "config": dev.file_receipt(root / "input/config.json"),
        "placement": dev.file_receipt(root / "receipts/placement.json") if "recovery" in config else None,
    })


def close(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    config = check_controller(root)
    submission = dev.read_json(root / "receipts/submission.json")
    accounting = {stage: accept_accounting(root, stage) for stage in submission["jobs"]}
    if "recovery" in config:
        accounting["placement"] = accept_placement(root)
        r = config["recovery"]
        accounting["original_training"], _ = placement_history(Path(r["original_root"]), root,
                                                               r["retry_tasks"], complete=True)
    if not (root / "output/APPLICATION.SUCCESS").is_file():
        raise dev.DevelopmentError("campaign application has not passed")
    result = dev.read_json(root / "output/analysis.json")
    if not all(r["pass"] for r in result["policy_validation"].values()):
        raise dev.DevelopmentError("campaign policy has not passed")
    dev.exclusive_json(root / "receipts/close.json", {
        "qualification_eligible": False, "status": "calibration_validation_passed",
        "accounting": accounting, "analysis_sha256": dev.sha256_file(root / "output/analysis.json"),
    })
    dev.exclusive_text(root / "RUN.SUCCESS", "FEPPML_AUTO_CALIBRATION_PASS\n")


def placement_check(args: argparse.Namespace) -> None:
    root = dev.validate_run_root(args.run_root)
    config = check_controller(root)
    if os.environ.get("NSLOTS") != "28" or not dev.JOB_ID.fullmatch(os.environ.get("JOB_ID", "")):
        raise dev.DevelopmentError("placement check requires a real 28-slot job")
    host = host_receipt(root / "work/qhost-placement.txt")
    document = dev.read_json(root / "work/placement-science/run.json")
    analysis = dev.analyze_document(document)
    source = identity(Path(config["candidate_root"]))
    if (document["sources"]["candidate"] != source["git"]
            or document["artifacts"]["candidate"] != source["artifact"]
            or document["scopes"] != dev.profile_scopes(dev.load_manifest(), "local_one_fe_science")
            or not analysis["science_pass"] or not analysis["route_pass"]
            or analysis != dev.read_json(root / "work/placement-science/analysis.json")
            or not memory_check(document)["pass"]):
        raise dev.DevelopmentError("placement retest science or identity differs")
    dev.exclusive_json(root / "receipts/placement.json", {
        "job_id": os.environ["JOB_ID"], "host": host, "source": source,
        "analysis": dev.file_receipt(root / "work/placement-science/analysis.json"),
    })
    dev.exclusive_text(root / "work/PLACEMENT.SUCCESS", "FEPPML_AUTO_PLACEMENT_PASS\n")


def accept_placement(root: Path) -> list[dict]:
    config = check_controller(root)
    receipt = dev.read_json(root / "receipts/placement.json")
    if (receipt["source"] != identity(Path(config["candidate_root"]))
            or not (root / "work/PLACEMENT.SUCCESS").is_file()
            or receipt["analysis"] != dev.file_receipt(root / "work/placement-science/analysis.json")
            or receipt["host"] != host_receipt(root / "work/qhost-placement.txt")):
        raise dev.DevelopmentError("placement retest evidence changed")
    job = receipt["job_id"]
    capture = subprocess.run(["qacct", "-j", job], text=True, capture_output=True, check=False)
    path = root / "logs" / f"qacct-placement-{time.time_ns()}.txt"
    dev.exclusive_text(path, capture.stdout)
    rows = dev._accept_qacct(path, job, [0], 28, dev.load_manifest()["receipt_contract"]["qacct_required"])
    if capture.returncode or any(not 0 <= float(r["ru_wallclock"]) <= 900 for r in rows):
        raise dev.DevelopmentError("placement retest accounting failed")
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("plan")
    cmd = commands.add_parser("validate-local")
    cmd.add_argument("--candidate-root", type=Path, default=ROOT)
    cmd.add_argument("--reference-root", type=Path, required=True)
    cmd.add_argument("--stata", required=True)
    cmd.add_argument("--output-dir", type=Path, required=True)
    cmd.add_argument("--budget-seconds", type=float, default=14400)
    cmd = commands.add_parser("run")
    cmd.add_argument("--stage", choices=("scc_train", "scc_holdout"), required=True)
    cmd.add_argument("--task", type=int, required=True)
    cmd.add_argument("--run-root", type=Path, required=True)
    cmd.add_argument("--candidate-root", type=Path, default=ROOT)
    cmd.add_argument("--reference-root", type=Path, required=True)
    cmd.add_argument("--stata", required=True)
    cmd.add_argument("--output-dir", type=Path, required=True)
    cmd.add_argument("--policy", type=Path)
    cmd = commands.add_parser("prepare-scc")
    cmd.add_argument("--run-root", type=Path, required=True)
    cmd.add_argument("--candidate-root", type=Path, required=True)
    cmd.add_argument("--reference-root", type=Path, required=True)
    cmd.add_argument("--local-validation", type=Path, required=True)
    cmd.add_argument("--stata", required=True)
    cmd = commands.add_parser("prepare-recovery")
    cmd.add_argument("--run-root", type=Path, required=True)
    cmd.add_argument("--original-root", type=Path, required=True)
    cmd = commands.add_parser("scheduled-task")
    cmd.add_argument("--run-root", type=Path, required=True)
    cmd.add_argument("--stage", choices=("scc_train", "scc_holdout"), required=True)
    cmd.add_argument("--index", type=int, required=True)
    for name in ("record-setup", "fit-scc", "collect", "close", "placement-check", "accept-placement"):
        cmd = commands.add_parser(name)
        cmd.add_argument("--run-root", type=Path, required=True)
    cmd = commands.add_parser("record-submission")
    cmd.add_argument("--run-root", type=Path, required=True)
    for name in ("setup", "scc-train", "policy", "scc-holdout", "collector"):
        cmd.add_argument(f"--{name}-job", required=True)
    args = parser.parse_args()
    try:
        if args.command == "plan":
            print(json.dumps({s: {"scopes": len(scopes(s)), "tasks": task_count(s)}
                              for s in ("scc_train", "scc_holdout")}, indent=2))
        elif args.command == "validate-local":
            run_local_validation(args)
        elif args.command == "run":
            if args.stage == "scc_holdout" and args.policy is None:
                raise dev.DevelopmentError("holdout requires a policy frozen before execution")
            if args.stage == "scc_train" and args.policy is not None:
                raise dev.DevelopmentError("training cannot consume a holdout policy")
            run(args)
        elif args.command == "prepare-scc":
            prepare_scc(args)
        elif args.command == "prepare-recovery":
            prepare_recovery(args)
        elif args.command == "scheduled-task":
            print(scheduled_task(check_controller(args.run_root), args.stage, args.index))
        elif args.command == "placement-check":
            placement_check(args)
        elif args.command == "accept-placement":
            accept_placement(args.run_root)
            print("FEPPML_AUTO_PLACEMENT_ACCEPTED")
        elif args.command == "record-setup":
            record_setup(args)
        elif args.command == "fit-scc":
            fit_scc(args)
        elif args.command == "record-submission":
            record_submission(args)
        elif args.command == "close":
            close(args)
        else:
            collect(args)
    except (dev.DevelopmentError, ValueError, KeyError) as error:
        print(f"PPML auto calibration failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
