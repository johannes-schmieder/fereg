#!/usr/bin/env python3
"""Run and audit the non-qualifying explicit PPML solver campaign."""
from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import re
import socket
import statistics
import subprocess
import sys
import tomllib
from typing import Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "benchmarks/manifests/feppml_solver_development_v2.json"
DRIVER = ROOT / "tests/stata/development/feppml_solver_development.do"
SOLVERS = ("map", "lsmr", "cmg")
COMPARATORS = ("ppmlhdfe", "ppmlhdfe_lsmr")
ROUTE_CODES = {"map": 2, "lsmr": 3, "cmg": 7}
SAFE_ID = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$")
JOB_ID = re.compile(r"^[1-9][0-9]*$")
RUN_ROOT = re.compile(r"^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$")
IMPLEMENTATION_FILES = (
    "benchmarks/manifests/feppml_solver_development_v2.json",
    "benchmarks/scc/feppml_solver_dev_job.sh",
    "benchmarks/scc/feppml_solver_dev_setup_job.sh",
    "benchmarks/scc/feppml_solver_dev_collector_job.sh",
    "benchmarks/scc/feppml_solver_dev_submit.sh",
    "benchmarks/scc/feppml_solver_dev_close.sh",
    "references.lock.toml",
    "scripts/ppml_solver_development.py",
    "tests/python/test_ppml_solver_development.py",
    "tests/python/ppml_explicit_dummy_oracle.py",
    "tests/stata/development/feppml_solver_development.do",
)


class DevelopmentError(RuntimeError):
    """A development input or receipt failed closed."""


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def exclusive_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(canonical_bytes(value))
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def exclusive_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as target:
            target.write(value)
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise DevelopmentError(f"expected JSON object: {path}")
    return value


def _keys(value: Mapping[str, object], expected: set[str], label: str) -> None:
    if set(value) != expected:
        raise DevelopmentError(f"{label} fields differ: {sorted(set(value) ^ expected)}")


def load_manifest(path: Path = MANIFEST) -> dict[str, object]:
    manifest = read_json(path)
    validate_manifest(manifest)
    return manifest


def validate_manifest(manifest: Mapping[str, object]) -> None:
    _keys(
        manifest,
        {
            "schema_version", "manifest_id", "status", "qualification_eligible",
            "auto_calibration", "purpose", "public_command", "protocols", "solvers",
            "comparators", "fixed_policy", "science", "generator", "topologies",
            "profiles", "scc_campaign", "receipt_contract",
        },
        "manifest",
    )
    if (
        manifest["schema_version"] != 2
        or manifest["manifest_id"] != "feppml_solver_development_v2"
        or manifest["status"] != "development_only"
        or manifest["qualification_eligible"] is not False
        or manifest["auto_calibration"] is not False
        or manifest["public_command"] != "feppml"
    ):
        raise DevelopmentError("invalid v2 development identity")
    if manifest["protocols"] != {
        "request_wire": "1.5", "result_wire": "1.2", "action_protocol": "1.5", "trace_schema": 1
    }:
        raise DevelopmentError("stale protocol identity")
    solvers = manifest["solvers"]
    if not isinstance(solvers, list) or [row.get("id") for row in solvers] != list(SOLVERS):
        raise DevelopmentError("public solver inventory must be map, lsmr, cmg")
    if {str(row["id"]): int(row["route_code"]) for row in solvers} != ROUTE_CODES:
        raise DevelopmentError("public route-code inventory changed")
    comparators = manifest["comparators"]
    if not isinstance(comparators, list) or [row.get("id") for row in comparators] != list(COMPARATORS):
        raise DevelopmentError("comparator inventory changed")
    if manifest["topologies"] != [
        "balanced_2fe", "skewed_2fe", "chain_2fe", "hub_2fe", "dense_2fe",
        "many_levels_2fe", "balanced_3fe", "sparse_3fe",
    ]:
        raise DevelopmentError("topology inventory changed")
    profiles = manifest["profiles"]
    if not isinstance(profiles, dict) or set(profiles) != {"local_science", "local_smoke"}:
        raise DevelopmentError("local profile inventory changed")
    for profile_id in profiles:
        profile_scopes(dict(manifest), profile_id)
    tasks = campaign_tasks(dict(manifest))
    campaign = manifest["scc_campaign"]
    if len(tasks) != 16 or sum(len(task["scopes"]) for task in tasks) != 56:
        raise DevelopmentError("SCC campaign must contain 16 tasks and 56 scopes")
    if any(len(task["scopes"]) not in {2, 3, 4} for task in tasks):
        raise DevelopmentError("each SCC task must contain two to four sequential scopes")
    expected_scheduler = {
        "project": "welfgr", "parallel_environment": "omp", "slots": 28,
        "m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4",
        "mem_per_core": "4G", "h_rt": "02:00:00",
    }
    if campaign["scheduler"] != expected_scheduler:
        raise DevelopmentError("SCC scheduler class changed")
    required_qacct = [
        "jobnumber", "taskid", "owner", "project", "hostname", "slots", "failed",
        "exit_status", "ru_wallclock", "cpu", "maxvmem",
    ]
    if manifest["receipt_contract"]["qacct_required"] != required_qacct:
        raise DevelopmentError("qacct receipt inventory changed")


def _validate_scopes(scopes: Sequence[Mapping[str, object]], expected: int) -> None:
    if len(scopes) != expected:
        raise DevelopmentError(f"scope inventory is {len(scopes)}, expected {expected}")
    ids: set[str] = set()
    for scope in scopes:
        if set(scope) != {"id", "n", "topology", "controls", "difficulty", "threads", "seed"}:
            raise DevelopmentError(f"invalid scope fields: {scope}")
        if not SAFE_ID.fullmatch(str(scope["id"])) or str(scope["id"]) in ids:
            raise DevelopmentError(f"invalid or duplicate scope id: {scope['id']}")
        ids.add(str(scope["id"]))
        if int(scope["n"]) <= 0 or int(scope["controls"]) < 0 or int(scope["threads"]) <= 0:
            raise DevelopmentError(f"invalid scope dimensions: {scope['id']}")
        if scope["difficulty"] not in {"easy", "long"}:
            raise DevelopmentError(f"invalid difficulty: {scope['id']}")


def profile_scopes(manifest: dict[str, object], profile_id: str) -> list[dict[str, object]]:
    profiles = manifest["profiles"]
    if profile_id not in profiles:
        raise DevelopmentError(f"unknown local profile: {profile_id}")
    profile = profiles[profile_id]
    seed_base = int(manifest["generator"]["seed_namespace"])
    raw: list[dict[str, object]] = []
    if "scopes" in profile:
        raw = [dict(scope) for scope in profile["scopes"]]
    else:
        for topology in profile["topologies"]:
            for controls in profile["controls"]:
                for difficulty in profile["difficulties"]:
                    for threads in profile["threads"]:
                        raw.append({
                            "id": f"{profile_id}-{topology}-k{controls}-{difficulty}-t{threads}",
                            "n": int(profile["observations"]), "topology": topology,
                            "controls": int(controls), "difficulty": difficulty, "threads": int(threads),
                        })
    scopes = [{**scope, "seed": seed_base + index} for index, scope in enumerate(raw, 1)]
    _validate_scopes(scopes, int(profile["expected_scopes"]))
    return scopes


def campaign_tasks(manifest: dict[str, object]) -> list[dict[str, object]]:
    campaign = manifest["scc_campaign"]
    shapes = campaign["bundle_shapes"]
    seed_base = int(manifest["generator"]["seed_namespace"]) + 1000
    tasks: list[dict[str, object]] = []
    ordinal = 0
    for bundle in campaign["bundles"]:
        shape = shapes[bundle["shape"]]
        dimensions: list[tuple[int, str, int]] = []
        if bundle["shape"] == "breadth":
            dimensions = [(int(control), str(bundle["difficulty"]), int(shape["threads"][0])) for control in shape["controls"]]
        elif bundle["shape"] == "difficulty":
            dimensions = [
                (int(control), str(difficulty), int(shape["threads"][0]))
                for control in shape["controls"] for difficulty in shape["difficulties"]
            ]
        elif bundle["shape"] == "scaling":
            dimensions = [(int(shape["controls"][0]), str(bundle["difficulty"]), int(thread)) for thread in shape["threads"]]
        else:
            raise DevelopmentError(f"unknown bundle shape: {bundle['shape']}")
        scopes: list[dict[str, object]] = []
        for control, difficulty, thread in dimensions:
            ordinal += 1
            scopes.append({
                "id": f"{bundle['id']}-k{control}-{difficulty}-t{thread}",
                "n": int(shape["n"]), "topology": str(bundle["topology"]),
                "controls": control, "difficulty": difficulty, "threads": thread,
                "seed": seed_base + ordinal,
            })
        tasks.append({"task": int(bundle["task"]), "bundle": str(bundle["id"]), "scopes": scopes})
    if [task["task"] for task in tasks] != list(range(1, int(campaign["task_count"]) + 1)):
        raise DevelopmentError("SCC task IDs must be contiguous")
    all_scopes = [scope for task in tasks for scope in task["scopes"]]
    _validate_scopes(all_scopes, int(campaign["expected_scopes"]))
    return tasks


def _git(repository: Path, *arguments: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(repository), *arguments], text=True, stderr=subprocess.STDOUT
    ).strip()


def source_identity(repository: Path) -> dict[str, object]:
    diff = subprocess.check_output(["git", "-C", str(repository), "diff", "--binary", "HEAD"])
    files: dict[str, str] = {}
    for relative in ("Cargo.lock", "references.lock.toml", "stata/feppml.ado"):
        path = repository / relative
        if path.is_file():
            files[relative] = sha256_file(path)
    return {
        "repository": str(repository.resolve()), "commit": _git(repository, "rev-parse", "HEAD^{commit}"),
        "tree": _git(repository, "rev-parse", "HEAD^{tree}"),
        "dirty": bool(_git(repository, "status", "--short")),
        "working_diff_sha256": hashlib.sha256(diff).hexdigest(), "source_files": files,
    }


def implementation_identity() -> dict[str, object]:
    files: dict[str, str] = {}
    for relative in IMPLEMENTATION_FILES:
        path = ROOT / relative
        if not path.is_file():
            raise DevelopmentError(f"missing live solver-development input: {relative}")
        files[relative] = sha256_file(path)
    return {"files": files, "sha256": hashlib.sha256(canonical_bytes(files)).hexdigest()}


def artifact_identity(repository: Path) -> dict[str, object]:
    dist = repository / "dist"
    plugin_name = {"Darwin": "fereg_macos.plugin", "Linux": "fereg_linux.plugin", "Windows": "fereg_windows.plugin"}.get(platform.system())
    if plugin_name is None:
        raise DevelopmentError(f"unsupported host platform: {platform.system()}")
    plugin = dist / plugin_name
    if not plugin.is_file():
        raise DevelopmentError(f"missing platform plugin: {plugin}")
    return {"path": str(plugin.resolve()), "name": plugin.name, "size": plugin.stat().st_size, "sha256": sha256_file(plugin)}


def reference_identity(reference_root: Path) -> dict[str, object]:
    lock = tomllib.loads((ROOT / "references.lock.toml").read_text(encoding="utf-8"))
    records = {record["name"]: record for record in lock["reference"]}
    output: dict[str, object] = {}
    for name in ("require", "ftools", "reghdfe", "ppmlhdfe"):
        record = records[name]
        captured: dict[str, str] = {}
        for relative, expected in record["required_capture_sha256"].items():
            path = reference_root / name / relative
            if not path.is_file() or sha256_file(path) != expected:
                raise DevelopmentError(f"pinned comparator file missing or changed: {path}")
            captured[relative] = expected
        output[name] = {"commit": record["commit"], "required_files": captured}
        if "source_archive_url" in record:
            archive = reference_root / f"{name}.tar.gz"
            if (
                not archive.is_file()
                or archive.stat().st_size != int(record["source_archive_size_bytes"])
                or sha256_file(archive) != record["source_archive_sha256"]
            ):
                raise DevelopmentError(f"pinned comparator archive missing or changed: {archive}")
            output[name]["source_archive"] = {
                "url": record["source_archive_url"], "size": archive.stat().st_size,
                "sha256": record["source_archive_sha256"],
            }
        generated = reference_root / "ppmlhdfe/src/l/lftools.mlib"
        if name == "ppmlhdfe" and generated.is_file():
            output[name]["generated_lftools_mlib"] = {"sha256": sha256_file(generated), "size": generated.stat().st_size}
    return output


def cpu_identity() -> dict[str, object]:
    model = platform.processor() or platform.machine()
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        match = re.search(r"^model name\s*:\s*(.+)$", cpuinfo.read_text(errors="replace"), re.MULTILINE)
        if match:
            model = match.group(1).strip()
    return {
        "host": socket.gethostname(), "cpu_model": model, "logical_cpus": os.cpu_count(),
        "system": platform.system(), "platform": platform.platform(),
    }


def parse_qhost_class(text: str) -> dict[str, object]:
    values: dict[str, str] = {}
    for key in ("m_core", "cpu_arch", "cpu_type"):
        matches = re.findall(rf"(?:^|[:\s]){key}=([^\s]+)", text, re.MULTILINE)
        if len(set(matches)) != 1:
            raise DevelopmentError(f"qhost does not report one {key} value")
        values[key] = matches[0]
    observed = {"m_core": int(values["m_core"]), "cpu_arch": values["cpu_arch"], "cpu_type": values["cpu_type"]}
    expected = {"m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4"}
    if observed != expected:
        raise DevelopmentError(f"wrong SCC host class: {observed}")
    return observed


def process_log_text(console: Path) -> str:
    parts = [console.read_text(encoding="utf-8", errors="replace") if console.is_file() else ""]
    stata_log = console.with_suffix(".stata.log")
    if stata_log.is_file():
        parts.append(stata_log.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(parts)


def stata_stage_status(console: Path, returncode: int, success: str) -> tuple[int, str]:
    text = process_log_text(console)
    failures = re.findall(r"^FEREG_PPML_SOLVER_[A-Z_]*FAIL\|[^\r\n]*\|rc=([0-9]+)\s*$", text, re.MULTILINE)
    if failures:
        return int(failures[-1]), "failure-sentinel"
    if returncode:
        return returncode, "process-exit"
    if not re.search(rf"^{re.escape(success)}(?:\||$)", text, re.MULTILINE):
        return 999, "missing-success-sentinel"
    return 0, "success-sentinel"


def run_stata(stata: str, environment: Mapping[str, str], console: Path) -> dict[str, object]:
    console.parent.mkdir(parents=True, exist_ok=True)
    clean = {key: value for key, value in os.environ.items() if not key.startswith("FEREG_PPML_SOLVER_")}
    clean.update(environment)
    clean["FEREG_PPML_SOLVER_STATA_LOG"] = str(console.with_suffix(".stata.log"))
    with console.open("wb") as target:
        completed = subprocess.run([stata, "-b", "do", str(DRIVER)], cwd=console.parent, env=clean, stdout=target, stderr=subprocess.STDOUT, check=False)
    return {"command": [stata, "-b", "do", str(DRIVER)], "console": str(console), "process_returncode": completed.returncode}


def base_environment(repository: Path, reference_root: Path, scope: Mapping[str, object], dataset: Path, result: Path) -> dict[str, str]:
    return {
        "FEREG_PPML_SOLVER_REPOSITORY": str(repository), "FEREG_PPML_SOLVER_REFERENCE_ROOT": str(reference_root),
        "FEREG_PPML_SOLVER_DATASET": str(dataset), "FEREG_PPML_SOLVER_RESULT": str(result),
        "FEREG_PPML_SOLVER_N": str(scope["n"]), "FEREG_PPML_SOLVER_K": str(scope["controls"]),
        "FEREG_PPML_SOLVER_TOPOLOGY": str(scope["topology"]), "FEREG_PPML_SOLVER_DIFFICULTY": str(scope["difficulty"]),
        "FEREG_PPML_SOLVER_THREADS": str(scope["threads"]), "FEREG_PPML_SOLVER_SEED": str(scope["seed"]),
    }


def explicit_dummy_oracle(data_csv: Path, controls: int) -> dict[str, object]:
    path = ROOT / "tests/python/ppml_explicit_dummy_oracle.py"
    spec = importlib.util.spec_from_file_location("fereg_ppml_solver_oracle", path)
    if spec is None or spec.loader is None:
        raise DevelopmentError("cannot load explicit-dummy oracle")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    with data_csv.open(encoding="utf-8", newline="") as source:
        rows = list(csv.DictReader(source))
    design = [[float(row[f"x{index}"]) for index in range(1, controls + 1)] for row in rows]
    for group in ("g1", "g2", "g3"):
        if group not in rows[0]:
            continue
        dummies, _ = module.dummy_columns([int(row[group]) for row in rows])
        for values, additions in zip(design, dummies):
            values.extend(additions)
    for values in design:
        values.append(1.0)
    fitted = module.fit_explicit_ppml(design, [float(row["y"]) for row in rows], vce="robust", tolerance=1e-11, max_iterations=500)
    return {
        "sample_n": float(len(rows)), "deviance": float(fitted.deviance),
        "control_names": [f"x{index}" for index in range(1, controls + 1)],
        "control_values": [float(value) for value in fitted.coefficients[:controls]], "iterations": fitted.iterations,
    }


def optional_float(value: str) -> float | None:
    value = value.strip()
    if not value or value == "." or re.fullmatch(r"\.[a-z]", value):
        return None
    parsed = float(value)
    if not math.isfinite(parsed):
        raise DevelopmentError(f"nonfinite numeric output: {value}")
    return parsed


stata_optional_float = optional_float


def _float_vector(value: str) -> list[float]:
    return [float(item) for item in value.split()] if value.strip() else []


def _optional_vector(value: str) -> list[float | None]:
    return [optional_float(item) for item in value.split()] if value.strip() else []


def parse_table(path: Path, scope: Mapping[str, object], implementation: str) -> list[dict[str, object]]:
    required = {
        "arm", "repetition", "rc", "seconds", "N", "ll", "deviance", "converged", "rank", "df_a", "df_resid",
        "inner_iterations", "route_code", "fallback_count", "effective_threads", "b_names", "b_values", "v_names",
        "v_values", "certificate_values", "runtime_values",
    }
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        if set(reader.fieldnames or []) != required:
            raise DevelopmentError(f"result columns differ: {path}")
        rows = list(reader)
    output: list[dict[str, object]] = []
    for row in rows:
        arm = row["arm"]
        if arm not in (*SOLVERS, *COMPARATORS):
            raise DevelopmentError(f"unknown result arm: {arm}")
        seconds = optional_float(row["seconds"])
        if seconds is None or seconds < 0.0:
            raise DevelopmentError(f"invalid elapsed time for {arm}")
        output.append({
            **scope, "scope_id": str(scope["id"]), "implementation": implementation,
            "arm_type": "solver" if arm in SOLVERS else "comparator", "arm": arm,
            "repetition": int(row["repetition"]), "rc": int(row["rc"]), "seconds": seconds,
            "sample_n": optional_float(row["N"]), "ll": optional_float(row["ll"]), "deviance": optional_float(row["deviance"]),
            "converged": optional_float(row["converged"]), "rank": optional_float(row["rank"]), "df_a": optional_float(row["df_a"]),
            "df_resid": optional_float(row["df_resid"]), "inner_iterations": optional_float(row["inner_iterations"]),
            "route_code": optional_float(row["route_code"]), "fallback_count": optional_float(row["fallback_count"]),
            "effective_threads": optional_float(row["effective_threads"]), "b_names": row["b_names"].split(),
            "b_values": _float_vector(row["b_values"]), "v_names": row["v_names"].split(), "v_values": _float_vector(row["v_values"]),
            "certificate_values": _optional_vector(row["certificate_values"]), "runtime_values": _optional_vector(row["runtime_values"]),
        })
    return output


def parse_data_meta(path: Path) -> dict[str, int]:
    with path.open(encoding="utf-8", newline="") as source:
        rows = list(csv.DictReader(source, delimiter="\t"))
    if len(rows) != 1 or set(rows[0]) != {"N", "levels1", "levels2", "levels3"}:
        raise DevelopmentError("dataset metadata must contain one exact row")
    return {key: int(value) for key, value in rows[0].items()}


def _configurations(repositories: Mapping[str, Path], include_comparators: bool, rotation: int) -> list[tuple[str, str]]:
    values = [(implementation, solver) for implementation in repositories for solver in SOLVERS]
    if include_comparators:
        values.extend(("reference", comparator) for comparator in COMPARATORS)
    offset = rotation % len(values)
    return values[offset:] + values[:offset]


def execute_scopes(
    manifest: dict[str, object], scopes: Sequence[Mapping[str, object]], *, stata: str,
    candidate_root: Path, baseline_root: Path | None, reference_root: Path,
    output_dir: Path, repetitions: int, include_comparators: bool = True,
    include_oracle: bool = False, task_identity: Mapping[str, object] | None = None,
    prepare_comparators: bool = False,
) -> dict[str, object]:
    if output_dir.exists():
        raise DevelopmentError(f"output already exists: {output_dir}")
    output_dir.mkdir(parents=True)
    repositories = {"candidate": candidate_root.resolve()}
    if baseline_root is not None:
        repositories["baseline"] = baseline_root.resolve()
    records: list[dict[str, object]] = []
    processes: list[dict[str, object]] = []
    datasets: list[dict[str, object]] = []
    if prepare_comparators:
        console = output_dir / "reference-setup.log"
        process = run_stata(
            stata,
            {
                "FEREG_PPML_SOLVER_MODE": "prepare-reference",
                "FEREG_PPML_SOLVER_REFERENCE_ROOT": str(reference_root),
            },
            console,
        )
        status, reason = stata_stage_status(console, int(process["process_returncode"]), "FEREG_PPML_SOLVER_REFERENCE_PASS")
        processes.append({**process, "stage": "prepare-reference", "accepted_rc": status, "status_reason": reason})
        if status:
            raise DevelopmentError(f"comparator setup failed: {reason}")
    sources = {name: source_identity(root) for name, root in repositories.items()}
    artifacts = {name: artifact_identity(root) for name, root in repositories.items()}
    references_before = reference_identity(reference_root)
    for scope_index, scope in enumerate(scopes):
        scope_dir = output_dir / "scopes" / str(scope["id"])
        scope_dir.mkdir(parents=True)
        dataset = scope_dir / "data.dta"
        data_csv = scope_dir / "data.csv"
        meta = scope_dir / "data-meta.tsv"
        env = base_environment(candidate_root, reference_root, scope, dataset, scope_dir / "unused.tsv")
        env.update({"FEREG_PPML_SOLVER_MODE": "generate", "FEREG_PPML_SOLVER_DATA_CSV": str(data_csv), "FEREG_PPML_SOLVER_DATA_META": str(meta)})
        process = run_stata(stata, env, scope_dir / "generate.log")
        status, reason = stata_stage_status(scope_dir / "generate.log", int(process["process_returncode"]), "FEREG_PPML_SOLVER_DATA_PASS")
        processes.append({**process, "stage": "generate", "scope_id": scope["id"], "accepted_rc": status, "status_reason": reason})
        if status:
            raise DevelopmentError(f"dataset generation failed for {scope['id']}: {reason}")
        dataset_receipt = {
            "scope": dict(scope), "dta": {"sha256": sha256_file(dataset), "size": dataset.stat().st_size},
            "csv": {"sha256": sha256_file(data_csv), "size": data_csv.stat().st_size},
            "metadata_file": {"sha256": sha256_file(meta), "size": meta.stat().st_size},
            "metadata": parse_data_meta(meta),
        }
        datasets.append(dataset_receipt)
        if include_oracle:
            records.append({**scope, "scope_id": scope["id"], "implementation": "independent", "arm_type": "oracle", "arm": "explicit_dummy", **explicit_dummy_oracle(data_csv, int(scope["controls"]))})
        for implementation, arm in _configurations(repositories, include_comparators, scope_index):
            repository = candidate_root if implementation == "reference" else repositories[implementation]
            result = scope_dir / f"{implementation}-{arm}.tsv"
            console = scope_dir / f"{implementation}-{arm}.log"
            run_env = base_environment(repository, reference_root, scope, dataset, result)
            run_env.update({"FEREG_PPML_SOLVER_MODE": "run", "FEREG_PPML_SOLVER_ARM": arm, "FEREG_PPML_SOLVER_REPETITIONS": str(repetitions)})
            process = run_stata(stata, run_env, console)
            status, reason = stata_stage_status(console, int(process["process_returncode"]), "FEREG_PPML_SOLVER_RUN_PASS")
            processes.append({**process, "stage": "run", "scope_id": scope["id"], "implementation": implementation, "arm": arm, "accepted_rc": status, "status_reason": reason})
            if result.is_file():
                records.extend(parse_table(result, scope, implementation))
    references_after = reference_identity(reference_root)
    if references_before != references_after:
        raise DevelopmentError("pinned reference closure changed during execution")
    return {
        "schema_version": 2, "kind": "feppml_solver_development_run", "qualification_eligible": False,
        "auto_calibration": False, "manifest": {"path": str(MANIFEST), "sha256": sha256_file(MANIFEST)},
        "implementation": implementation_identity(), "repetitions": repetitions, "include_comparators": include_comparators,
        "include_oracle": include_oracle, "sources": sources, "artifacts": artifacts, "references": references_after,
        "host": cpu_identity(), "task_identity": dict(task_identity) if task_identity else None,
        "scopes": [dict(scope) for scope in scopes], "datasets": datasets, "records": records, "processes": processes,
    }


def _close(left: float | None, right: float | None, rtol: float, atol: float) -> bool:
    return (
        left is not None
        and right is not None
        and abs(left - right) <= atol + rtol * max(abs(left), abs(right))
    )


def _vectors_close(left: Sequence[float], right: Sequence[float], rtol: float, atol: float) -> bool:
    return len(left) == len(right) and all(_close(a, b, rtol, atol) for a, b in zip(left, right))


def _control_covariance(row: Mapping[str, object], controls: int) -> list[float]:
    names = row["v_names"]
    values = row["v_values"]
    width = len(names)
    if len(values) != width * width or width < controls:
        return []
    return [values[left * width + right] for left in range(controls) for right in range(controls)]


def _model_dof(row: Mapping[str, object]) -> float | None:
    rank = row["rank"]
    absorbed = row["df_a"]
    if rank is None or absorbed is None:
        return None
    return float(rank) + float(absorbed)


def analyze_document(document: Mapping[str, object], manifest: dict[str, object] | None = None) -> dict[str, object]:
    manifest = manifest or load_manifest()
    if document.get("schema_version") != 2 or document.get("kind") not in {"feppml_solver_development_run", "feppml_solver_development_merged_run"}:
        raise DevelopmentError("not a v2 solver-development run")
    if document.get("qualification_eligible") is not False or document.get("auto_calibration") is not False:
        raise DevelopmentError("development evidence cannot qualify or calibrate auto")
    repetitions = int(document["repetitions"])
    implementations = list(document["sources"])
    scopes = document["scopes"]
    records = document["records"]
    indexed: dict[tuple[str, str, str, int], Mapping[str, object]] = {}
    oracle: dict[str, Mapping[str, object]] = {}
    failures: list[str] = []
    route_failures: list[str] = []
    comparator_diagnostics: list[str] = []
    dof_convention_notes: set[str] = set()
    for row in records:
        if row["arm_type"] == "oracle":
            oracle[str(row["scope_id"])] = row
            continue
        key = (str(row["scope_id"]), str(row["implementation"]), str(row["arm"]), int(row["repetition"]))
        if key[1] not in {*implementations, "reference"} or key[2] not in {*SOLVERS, *COMPARATORS} or not 1 <= key[3] <= repetitions:
            failures.append(":".join(map(str, key)) + ":unexpected")
            continue
        if key in indexed:
            failures.append(":".join(map(str, key)) + ":duplicate")
        indexed[key] = row
    for process in document.get("processes", []):
        if int(process.get("accepted_rc", 0)):
            failures.append(
                f"{process.get('scope_id')}:{process.get('implementation', process.get('stage'))}:"
                f"{process.get('arm', '')}:process"
            )
    science = manifest["science"]
    summaries: list[dict[str, object]] = []
    for scope in scopes:
        scope_id = str(scope["id"])
        controls = int(scope["controls"])
        for implementation in implementations:
            for repetition in range(1, repetitions + 1):
                baseline = indexed.get((scope_id, implementation, "map", repetition))
                references = {
                    comparator: indexed.get((scope_id, "reference", comparator, repetition))
                    for comparator in COMPARATORS
                }
                for comparator, reference in references.items():
                    prefix = f"{scope_id}:reference:{comparator}:r{repetition}"
                    if reference is None:
                        failures.append(prefix + ":missing")
                    elif int(reference["rc"]) != 0 or reference["converged"] != 1.0:
                        failures.append(prefix + ":execution")
                default_reference = references["ppmlhdfe"]
                lsmr_reference = references["ppmlhdfe_lsmr"]
                if default_reference is not None and lsmr_reference is not None:
                    prefix = f"{scope_id}:reference:comparators:r{repetition}"
                    if default_reference["sample_n"] != lsmr_reference["sample_n"]:
                        failures.append(prefix + ":sample")
                    if default_reference["b_names"][:controls] != lsmr_reference["b_names"][:controls] or not _vectors_close(default_reference["b_values"][:controls], lsmr_reference["b_values"][:controls], float(science["coefficient_rtol"]), float(science["coefficient_atol"])):
                        comparator_diagnostics.append(prefix + ":coefficients")
                    if default_reference["v_names"][:controls] != lsmr_reference["v_names"][:controls] or not _vectors_close(_control_covariance(default_reference, controls), _control_covariance(lsmr_reference, controls), float(science["vce_rtol"]), float(science["vce_atol"])):
                        comparator_diagnostics.append(prefix + ":vce")
                    if _model_dof(default_reference) != _model_dof(lsmr_reference):
                        failures.append(prefix + ":model_dof")
                    for scalar in ("ll", "deviance"):
                        if not _close(default_reference[scalar], lsmr_reference[scalar], float(science["likelihood_rtol"]), float(science["likelihood_atol"])):
                            comparator_diagnostics.append(prefix + f":{scalar}")
                for solver in SOLVERS:
                    prefix = f"{scope_id}:{implementation}:{solver}:r{repetition}"
                    row = indexed.get((scope_id, implementation, solver, repetition))
                    if row is None:
                        failures.append(prefix + ":missing")
                        continue
                    if int(row["rc"]) != 0 or row["converged"] != 1.0:
                        failures.append(prefix + ":execution")
                    if row["route_code"] != float(ROUTE_CODES[solver]):
                        route_failures.append(prefix + ":route")
                    if row["fallback_count"] != 0.0:
                        route_failures.append(prefix + ":fallback")
                    if row["effective_threads"] != float(scope["threads"]):
                        route_failures.append(prefix + ":threads")
                    certificate = row["certificate_values"]
                    if len(certificate) != 16 or certificate[-1] != 1.0:
                        failures.append(prefix + ":certificate")
                    if row["inner_iterations"] is None or not math.isfinite(float(row["inner_iterations"])) or float(row["inner_iterations"]) < 0.0:
                        failures.append(prefix + ":inner_iterations")
                    if len(row["runtime_values"]) != 108:
                        failures.append(prefix + ":runtime")
                    # The forced-LSMR ppmlhdfe arm is a performance comparator,
                    # not a second scientific authority. ADR-0022 uses the
                    # default pinned ppmlhdfe route for the compatibility gate.
                    for target_name, target in (("map", baseline), ("ppmlhdfe", default_reference)):
                        if target is None:
                            failures.append(prefix + f":missing_{target_name}")
                            continue
                        if row["sample_n"] != target["sample_n"]:
                            failures.append(prefix + f":{target_name}_sample")
                        if row["b_names"][:controls] != target["b_names"][:controls] or not _vectors_close(row["b_values"][:controls], target["b_values"][:controls], float(science["coefficient_rtol"]), float(science["coefficient_atol"])):
                            failures.append(prefix + f":{target_name}_coefficients")
                        if row["v_names"][:controls] != target["v_names"][:controls] or not _vectors_close(_control_covariance(row, controls), _control_covariance(target, controls), float(science["vce_rtol"]), float(science["vce_atol"])):
                            failures.append(prefix + f":{target_name}_vce")
                        for scalar in ("ll", "deviance"):
                            if not _close(row[scalar], target[scalar], float(science["likelihood_rtol"]), float(science["likelihood_atol"])):
                                failures.append(prefix + f":{target_name}_{scalar}")
                        if target_name == "map":
                            for scalar in ("rank", "df_a", "df_resid"):
                                if row[scalar] != target[scalar]:
                                    failures.append(prefix + f":{target_name}_{scalar}")
                            continue

                        # ADR-0022 gates ppmlhdfe on total model DoF. The
                        # comparator assigns the normalized constant to df_a,
                        # while fereg exposes it as an omitted finite slot.
                        row_model_dof = _model_dof(row)
                        target_model_dof = _model_dof(target)
                        if row_model_dof is None or target_model_dof is None or row_model_dof != target_model_dof:
                            failures.append(prefix + f":{target_name}_model_dof")
                            continue
                        row_residual_dof = row["df_resid"]
                        expected_row_residual_dof = float(row["sample_n"]) - row_model_dof
                        if row_residual_dof is None or float(row_residual_dof) != expected_row_residual_dof:
                            failures.append(prefix + f":{target_name}_candidate_df_resid")
                        target_residual_dof = target["df_resid"]
                        expected_target_residual_dof = float(target["sample_n"]) - target_model_dof
                        if target_residual_dof is not None and float(target_residual_dof) != expected_target_residual_dof:
                            failures.append(prefix + f":{target_name}_reference_df_resid")
                        if row["rank"] != target["rank"] or row["df_a"] != target["df_a"]:
                            dof_convention_notes.add(
                                "fereg and ppmlhdfe partition rank versus absorbed DoF differently; "
                                "the gate compares their equal total model DoF"
                            )
                        if target_residual_dof is None:
                            dof_convention_notes.add(
                                "ppmlhdfe does not publish residual DoF for robust VCE; the gate "
                                "derives it from N minus total model DoF"
                            )
        if document.get("include_oracle"):
            target = oracle.get(scope_id)
            candidate_map = indexed.get((scope_id, "candidate", "map", 1))
            if target is None or candidate_map is None:
                failures.append(scope_id + ":independent_oracle_missing")
            else:
                if candidate_map["b_names"][:controls] != target["control_names"] or not _vectors_close(candidate_map["b_values"][:controls], target["control_values"], float(science["coefficient_rtol"]), float(science["coefficient_atol"])):
                    failures.append(scope_id + ":independent_oracle_coefficients")
                if not _close(candidate_map["deviance"], target["deviance"], float(science["likelihood_rtol"]), float(science["likelihood_atol"])):
                    failures.append(scope_id + ":independent_oracle_deviance")
        for solver in SOLVERS:
            values = [
                float(indexed[(scope_id, "candidate", solver, repetition)]["seconds"])
                for repetition in range(1, repetitions + 1)
                if (scope_id, "candidate", solver, repetition) in indexed
            ]
            if values:
                inner_iterations = [
                    float(indexed[(scope_id, "candidate", solver, repetition)]["inner_iterations"])
                    for repetition in range(1, repetitions + 1)
                    if (scope_id, "candidate", solver, repetition) in indexed
                    and indexed[(scope_id, "candidate", solver, repetition)]["inner_iterations"] is not None
                ]
                reference_values = [float(indexed[(scope_id, "reference", "ppmlhdfe", repetition)]["seconds"]) for repetition in range(1, repetitions + 1) if (scope_id, "reference", "ppmlhdfe", repetition) in indexed]
                reference_lsmr_values = [float(indexed[(scope_id, "reference", "ppmlhdfe_lsmr", repetition)]["seconds"]) for repetition in range(1, repetitions + 1) if (scope_id, "reference", "ppmlhdfe_lsmr", repetition) in indexed]
                baseline_values = [float(indexed[(scope_id, "baseline", solver, repetition)]["seconds"]) for repetition in range(1, repetitions + 1) if (scope_id, "baseline", solver, repetition) in indexed]
                prefix = f"{scope_id}:candidate:{solver}:"
                eligible = (
                    len(values) == repetitions
                    and all(value > 0.0 and math.isfinite(value) for value in values)
                    and not any(item.startswith(prefix) for item in (*failures, *route_failures))
                )
                reference_median = statistics.median(reference_values) if reference_values else None
                reference_lsmr_median = statistics.median(reference_lsmr_values) if reference_lsmr_values else None
                baseline_median = statistics.median(baseline_values) if baseline_values else None
                summaries.append({
                    "scope_id": scope_id, "solver": solver, "performance_eligible": eligible,
                    "candidate_median_seconds": statistics.median(values), "candidate_min_seconds": min(values), "candidate_max_seconds": max(values),
                    "candidate_median_inner_iterations": statistics.median(inner_iterations) if inner_iterations else None,
                    "ppmlhdfe_median_seconds": reference_median,
                    "candidate_over_ppmlhdfe": statistics.median(values) / reference_median if reference_median and reference_median > 0.0 else None,
                    "ppmlhdfe_lsmr_median_seconds": reference_lsmr_median,
                    "candidate_over_ppmlhdfe_lsmr": statistics.median(values) / reference_lsmr_median if reference_lsmr_median and reference_lsmr_median > 0.0 else None,
                    "baseline_median_seconds": baseline_median,
                    "candidate_over_baseline": statistics.median(values) / baseline_median if baseline_median and baseline_median > 0.0 else None,
                })
    return {
        "schema_version": 2, "kind": "feppml_solver_development_analysis", "qualification_eligible": False,
        "auto_calibration": False, "science_pass": not failures, "route_pass": not route_failures,
        "science_failures": sorted(set(failures)), "route_failures": sorted(set(route_failures)),
        "comparator_diagnostics": sorted(set(comparator_diagnostics)),
        "dof_convention_notes": sorted(dof_convention_notes),
        "solver_summary": summaries, "scopes": len(scopes), "repetitions": repetitions,
    }


def validate_run_root(path: Path) -> Path:
    resolved = path.resolve()
    if not RUN_ROOT.fullmatch(str(resolved)):
        raise DevelopmentError("run root must be one exact /projectnb/welfgr/fereg/dev child")
    return resolved


def _source_fingerprint(identity: Mapping[str, object]) -> dict[str, object]:
    return {
        key: identity[key]
        for key in ("commit", "tree", "working_diff_sha256", "source_files")
    }


def local_gate_identity(
    output_dir: Path, profile: str, candidate_source: Mapping[str, object],
    implementation: Mapping[str, object], manifest_sha256: str, manifest: dict[str, object],
) -> dict[str, object]:
    run_path = output_dir / "run.json"
    analysis_path = output_dir / "analysis.json"
    run = read_json(run_path)
    analysis = read_json(analysis_path)
    expected_scopes = profile_scopes(manifest, profile)
    reproduced = analyze_document(run, manifest)
    if (
        run.get("kind") != "feppml_solver_development_run"
        or run.get("profile") != profile
        or run.get("manifest", {}).get("sha256") != manifest_sha256
        or run.get("implementation") != implementation
        or run.get("host", {}).get("system") != "Darwin"
        or run.get("scopes") != expected_scopes
        or int(run.get("repetitions", 0)) != int(manifest["profiles"][profile]["repetitions"])
        or analysis != reproduced
        or analysis.get("science_pass") is not True
        or analysis.get("route_pass") is not True
        or analysis.get("auto_calibration") is not False
    ):
        raise DevelopmentError(f"local {profile} gate is not accepted")
    observed_source = run.get("sources", {}).get("candidate")
    if not isinstance(observed_source, dict) or _source_fingerprint(observed_source) != _source_fingerprint(candidate_source):
        raise DevelopmentError(f"local {profile} source differs from SCC candidate")
    return {
        "profile": profile, "run_sha256": sha256_file(run_path),
        "analysis_sha256": sha256_file(analysis_path), "host": run["host"],
        "candidate_artifact": run["artifacts"]["candidate"],
        "candidate_source": _source_fingerprint(observed_source),
    }


def prepare_run(args: argparse.Namespace, manifest: dict[str, object]) -> None:
    run_root = validate_run_root(args.run_root)
    if run_root.exists() and any(run_root.iterdir()):
        raise DevelopmentError("run root must be absent or empty")
    implementation = implementation_identity()
    candidate_root = Path(args.candidate_root).resolve()
    candidate_source = source_identity(candidate_root)
    manifest_sha = sha256_file(MANIFEST)
    gates = {
        profile: local_gate_identity(path, profile, candidate_source, implementation, manifest_sha, manifest)
        for profile, path in (
            ("local_science", args.local_science), ("local_smoke", args.local_smoke)
        )
    }
    task_plan = {"tasks": campaign_tasks(manifest)}
    config = {
        "schema_version": 2, "kind": "feppml_solver_development_config", "manifest_sha256": manifest_sha,
        "implementation": implementation, "candidate_root": str(candidate_root),
        "baseline_root": str(Path(args.baseline_root).resolve()) if args.baseline_root else None,
        "reference_root": str(Path(args.reference_root).resolve()), "stata": str(Path(args.stata).resolve()),
        "repetitions": int(args.repetitions), "task_count": 16, "candidate_source": candidate_source,
        "local_gates": gates, "task_plan_sha256": hashlib.sha256(canonical_bytes(task_plan)).hexdigest(),
    }
    for directory in ("input", "work", "logs", "shards", "output", "receipts"):
        (run_root / directory).mkdir(parents=True, exist_ok=True)
    exclusive_json(run_root / "input/config.json", config)
    exclusive_json(run_root / "input/tasks.json", task_plan)
    print(run_root)


def record_setup(args: argparse.Namespace) -> None:
    run_root = validate_run_root(args.run_root)
    config = read_json(run_root / "input/config.json")
    if (
        not JOB_ID.fullmatch(str(args.job_id))
        or config["manifest_sha256"] != sha256_file(MANIFEST)
        or config["implementation"] != implementation_identity()
        or config["task_plan_sha256"] != sha256_file(run_root / "input/tasks.json")
    ):
        raise DevelopmentError("setup job or manifest identity differs from preparation")
    candidate = Path(config["candidate_root"])
    repositories = {"candidate": candidate}
    if config["baseline_root"]:
        repositories["baseline"] = Path(config["baseline_root"])
    sources = {name: source_identity(path) for name, path in repositories.items()}
    if _source_fingerprint(sources["candidate"]) != _source_fingerprint(config["candidate_source"]):
        raise DevelopmentError("setup candidate source differs from preparation and local gates")
    receipt = {
        "schema_version": 2, "kind": "feppml_solver_development_setup", "job_id": str(args.job_id),
        "manifest_sha256": config["manifest_sha256"], "implementation": implementation_identity(),
        "sources": sources,
        "artifacts": {name: artifact_identity(path) for name, path in repositories.items()},
        "references": reference_identity(Path(config["reference_root"])), "host": cpu_identity(),
        "local_gates": config["local_gates"], "task_plan_sha256": config["task_plan_sha256"],
    }
    exclusive_json(run_root / "receipts/setup.json", receipt)
    exclusive_text(run_root / "work/SETUP.SUCCESS", f"job_id={args.job_id}\n")


def task_identity_from_environment(qhost: Path) -> dict[str, object]:
    task = os.environ.get("SGE_TASK_ID", "")
    job = os.environ.get("JOB_ID", "")
    slots = os.environ.get("NSLOTS", "")
    if not JOB_ID.fullmatch(job) or not task.isdigit() or not 1 <= int(task) <= 16 or slots != "28":
        raise DevelopmentError("invalid SGE task identity or slot allocation")
    return {"job_id": job, "task_id": int(task), "slots": int(slots), **cpu_identity(), **parse_qhost_class(qhost.read_text(encoding="utf-8"))}


def run_task(args: argparse.Namespace, manifest: dict[str, object]) -> None:
    run_root = validate_run_root(args.run_root)
    if not (run_root / "work/SETUP.SUCCESS").is_file():
        raise DevelopmentError("setup has not completed")
    identity = task_identity_from_environment(args.qhost)
    config = read_json(run_root / "input/config.json")
    setup = read_json(run_root / "receipts/setup.json")
    if (
        config["manifest_sha256"] != sha256_file(MANIFEST)
        or config["task_plan_sha256"] != sha256_file(run_root / "input/tasks.json")
        or setup["task_plan_sha256"] != config["task_plan_sha256"]
    ):
        raise DevelopmentError("task manifest or task plan differs from setup")
    tasks = read_json(run_root / "input/tasks.json")["tasks"]
    task = tasks[int(identity["task_id"]) - 1]
    if int(task["task"]) != identity["task_id"]:
        raise DevelopmentError("task plan identity mismatch")
    output = run_root / "shards" / f"task-{identity['task_id']:02d}"
    document = execute_scopes(
        manifest, task["scopes"], stata=str(config["stata"]), candidate_root=Path(config["candidate_root"]),
        baseline_root=Path(config["baseline_root"]) if config["baseline_root"] else None,
        reference_root=Path(config["reference_root"]), output_dir=output, repetitions=int(config["repetitions"]),
        include_comparators=True, task_identity=identity,
    )
    document["profile"] = "scc_screen"
    if (
        document["implementation"] != setup["implementation"]
        or document["sources"] != setup["sources"]
        or document["artifacts"] != setup["artifacts"]
        or document["references"] != setup["references"]
    ):
        raise DevelopmentError("task inputs differ from the sealed setup receipt")
    run_path = output / "run.json"
    exclusive_json(run_path, document)
    analysis = analyze_document(document, manifest)
    exclusive_json(output / "analysis.json", analysis)
    if not analysis["science_pass"] or not analysis["route_pass"]:
        raise DevelopmentError(f"task {identity['task_id']} failed science or route checks")
    receipt = {
        "schema_version": 2, "kind": "feppml_solver_development_task", "task_identity": identity,
        "bundle": task["bundle"], "scope_ids": [scope["id"] for scope in task["scopes"]],
        "run_sha256": sha256_file(run_path), "analysis_sha256": sha256_file(output / "analysis.json"),
        "source_identity": document["sources"], "artifact_identity": document["artifacts"],
        "datasets": document["datasets"], "qhost_sha256": sha256_file(args.qhost),
    }
    exclusive_json(run_root / f"receipts/task-{identity['task_id']:02d}.json", receipt)
    exclusive_text(output / "TASK.SUCCESS", f"task={identity['task_id']}\n")


def merge_tasks(args: argparse.Namespace, manifest: dict[str, object]) -> None:
    run_root = validate_run_root(args.run_root)
    collector_job = os.environ.get("JOB_ID", "")
    if not JOB_ID.fullmatch(collector_job) or os.environ.get("NSLOTS") != "1":
        raise DevelopmentError("merge must run in the one-slot collector job")
    config = read_json(run_root / "input/config.json")
    documents: list[dict[str, object]] = []
    receipts: list[dict[str, object]] = []
    for task_id in range(1, 17):
        receipt_path = run_root / f"receipts/task-{task_id:02d}.json"
        run_path = run_root / f"shards/task-{task_id:02d}/run.json"
        receipt = read_json(receipt_path)
        if receipt["run_sha256"] != sha256_file(run_path):
            raise DevelopmentError(f"task {task_id} run digest mismatch")
        document = read_json(run_path)
        if (
            receipt["analysis_sha256"] != sha256_file(run_path.with_name("analysis.json"))
            or receipt["datasets"] != document["datasets"]
            or receipt["task_identity"] != document["task_identity"]
        ):
            raise DevelopmentError(f"task {task_id} receipt content mismatch")
        receipts.append(receipt)
        documents.append(document)
    if any(receipt["source_identity"] != receipts[0]["source_identity"] or receipt["artifact_identity"] != receipts[0]["artifact_identity"] for receipt in receipts[1:]):
        raise DevelopmentError("task source or artifact identities differ")
    setup = read_json(run_root / "receipts/setup.json")
    if receipts[0]["source_identity"] != setup["sources"] or receipts[0]["artifact_identity"] != setup["artifacts"]:
        raise DevelopmentError("task inputs differ from setup")
    scopes = [scope for document in documents for scope in document["scopes"]]
    expected = [scope for task in campaign_tasks(manifest) for scope in task["scopes"]]
    if scopes != expected:
        raise DevelopmentError("merged task scope order or content differs")
    merged = {
        "schema_version": 2, "kind": "feppml_solver_development_merged_run", "qualification_eligible": False,
        "auto_calibration": False, "manifest": documents[0]["manifest"], "implementation": documents[0]["implementation"],
        "repetitions": int(config["repetitions"]), "include_comparators": True, "include_oracle": False,
        "sources": documents[0]["sources"], "artifacts": documents[0]["artifacts"], "references": documents[0]["references"],
        "task_receipts": [str(run_root / f"receipts/task-{task_id:02d}.json") for task_id in range(1, 17)],
        "scopes": scopes, "datasets": [row for document in documents for row in document["datasets"]],
        "records": [row for document in documents for row in document["records"]],
        "processes": [row for document in documents for row in document["processes"]],
    }
    run_path = run_root / "output/merged-run.json"
    exclusive_json(run_path, merged)
    analysis = analyze_document(merged, manifest)
    exclusive_json(run_root / "output/analysis.json", analysis)
    if not analysis["science_pass"] or not analysis["route_pass"]:
        raise DevelopmentError("merged campaign failed science or route checks")
    exclusive_json(run_root / "receipts/merge.json", {
        "schema_version": 2, "kind": "feppml_solver_development_merge", "task_count": 16, "scope_count": 56,
        "job_id": collector_job, "host": cpu_identity(),
        "merged_run_sha256": sha256_file(run_path), "analysis_sha256": sha256_file(run_root / "output/analysis.json"),
    })


def record_submission(args: argparse.Namespace) -> None:
    run_root = validate_run_root(args.run_root)
    values = {"setup": str(args.setup_job), "array": str(args.array_job), "collector": str(args.collector_job)}
    if any(not JOB_ID.fullmatch(value) for value in values.values()):
        raise DevelopmentError("submission job IDs must be positive integers")
    exclusive_json(run_root / "receipts/submission.json", {"schema_version": 2, "kind": "feppml_solver_development_submission", "jobs": values})


def parse_qacct(path: Path) -> list[dict[str, str]]:
    blocks = re.split(r"^=+\s*$", path.read_text(encoding="utf-8", errors="replace"), flags=re.MULTILINE)
    rows: list[dict[str, str]] = []
    for block in blocks:
        row: dict[str, str] = {}
        for line in block.splitlines():
            parts = line.split(None, 1)
            if len(parts) == 2:
                row[parts[0]] = parts[1].strip()
        if "jobnumber" in row:
            rows.append(row)
    return rows


def _accept_qacct(path: Path, job: str, expected_tasks: Sequence[int], slots: int, required: Sequence[str]) -> list[dict[str, str]]:
    rows = parse_qacct(path)
    if len(rows) != len(expected_tasks):
        raise DevelopmentError(f"qacct record count differs for job {job}")
    observed: list[int] = []
    for row in rows:
        missing = set(required) - set(row)
        if missing:
            raise DevelopmentError(f"qacct record missing {sorted(missing)}")
        task_id = int(row["taskid"]) if row["taskid"] not in {"undefined", "NONE"} else 0
        observed.append(task_id)
        if row["jobnumber"] != job or row["owner"] != "johannes" or row["project"] != "welfgr" or int(row["slots"]) != slots or row["failed"] != "0" or row["exit_status"] != "0":
            raise DevelopmentError(f"qacct record rejected for job {job}, task {task_id}")
    if sorted(observed) != sorted(expected_tasks):
        raise DevelopmentError(f"qacct task inventory differs for job {job}")
    return rows


def close_run(args: argparse.Namespace, manifest: dict[str, object]) -> None:
    run_root = validate_run_root(args.run_root)
    if (run_root / "RUN.SUCCESS").exists() or not (run_root / "receipts/merge.json").is_file():
        raise DevelopmentError("run is already closed or merge is missing")
    submission = read_json(run_root / "receipts/submission.json")["jobs"]
    required = manifest["receipt_contract"]["qacct_required"]
    accounting = {
        "setup": _accept_qacct(args.setup_qacct, submission["setup"], [0], 4, required),
        "array": _accept_qacct(args.array_qacct, submission["array"], list(range(1, 17)), 28, required),
        "collector": _accept_qacct(args.collector_qacct, submission["collector"], [0], 1, required),
    }
    setup = read_json(run_root / "receipts/setup.json")
    merge = read_json(run_root / "receipts/merge.json")
    if setup["job_id"] != submission["setup"] or merge["job_id"] != submission["collector"]:
        raise DevelopmentError("setup or collector receipt is not bound to its submitted job")
    if str(setup["host"]["host"]).split(".")[0] != accounting["setup"][0]["hostname"].split(".")[0]:
        raise DevelopmentError("setup receipt host differs from qacct")
    if str(merge["host"]["host"]).split(".")[0] != accounting["collector"][0]["hostname"].split(".")[0]:
        raise DevelopmentError("collector receipt host differs from qacct")
    array_accounting = {int(row["taskid"]): row for row in accounting["array"]}
    for task_id in range(1, 17):
        task_receipt = read_json(run_root / f"receipts/task-{task_id:02d}.json")
        task = task_receipt["task_identity"]
        account = array_accounting[task_id]
        qhost = run_root / f"work/qhost-task-{task_id:02d}.txt"
        if (
            task["job_id"] != submission["array"]
            or int(task["task_id"]) != task_id
            or int(task["slots"]) != 28
            or int(task["m_core"]) != 28
            or task["cpu_arch"] != "broadwell"
            or task["cpu_type"] != "E5-2680v4"
            or not str(task["cpu_model"])
            or str(task["host"]).split(".")[0] != account["hostname"].split(".")[0]
            or not qhost.is_file()
            or task_receipt["qhost_sha256"] != sha256_file(qhost)
        ):
            raise DevelopmentError(f"task {task_id} receipt differs from qacct")
    raw_qacct = {
        "setup": run_root / "receipts/qacct-setup.txt",
        "array": run_root / "receipts/qacct-array.txt",
        "collector": run_root / "receipts/qacct-collector.txt",
    }
    for name, source in (
        ("setup", args.setup_qacct), ("array", args.array_qacct),
        ("collector", args.collector_qacct),
    ):
        exclusive_text(raw_qacct[name], source.read_text(encoding="utf-8", errors="replace"))
    exclusive_json(run_root / "receipts/close.json", {
        "schema_version": 2, "kind": "feppml_solver_development_close", "jobs": submission,
        "qacct": accounting, "analysis_sha256": sha256_file(run_root / "output/analysis.json"),
        "merge_receipt_sha256": sha256_file(run_root / "receipts/merge.json"),
        "qacct_files": {
            name: sha256_file(path) for name, path in raw_qacct.items()
        },
    })
    exclusive_text(run_root / "RUN.SUCCESS", f"analysis_sha256={sha256_file(run_root / 'output/analysis.json')}\n")


def run_local(args: argparse.Namespace, manifest: dict[str, object]) -> None:
    profile = manifest["profiles"][args.profile]
    repetitions = args.repetitions or int(profile["repetitions"])
    document = execute_scopes(
        manifest, profile_scopes(manifest, args.profile), stata=args.stata,
        candidate_root=args.candidate_root, baseline_root=args.baseline_root,
        reference_root=args.reference_root, output_dir=args.output_dir, repetitions=repetitions,
        include_comparators=bool(profile["comparators"]), include_oracle=bool(profile["independent_oracle"]),
        prepare_comparators=bool(profile["comparators"]),
    )
    document["profile"] = args.profile
    exclusive_json(args.output_dir / "run.json", document)
    analysis = analyze_document(document, manifest)
    exclusive_json(args.output_dir / "analysis.json", analysis)
    if not analysis["science_pass"] or not analysis["route_pass"]:
        raise DevelopmentError("local profile failed science or route checks")
    print(args.output_dir)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    commands.add_parser("validate-manifest")
    cells = commands.add_parser("cells")
    cells.add_argument("--profile", choices=("local_science", "local_smoke", "scc_screen"), required=True)
    local = commands.add_parser("run-local")
    local.add_argument("--profile", choices=("local_science", "local_smoke"), required=True)
    local.add_argument("--stata", required=True)
    local.add_argument("--candidate-root", type=Path, default=ROOT)
    local.add_argument("--baseline-root", type=Path)
    local.add_argument("--reference-root", type=Path, required=True)
    local.add_argument("--output-dir", type=Path, required=True)
    local.add_argument("--repetitions", type=int)
    analyze = commands.add_parser("analyze")
    analyze.add_argument("run", type=Path)
    analyze.add_argument("--output", type=Path, required=True)
    prepare = commands.add_parser("prepare-run")
    prepare.add_argument("--run-root", type=Path, required=True)
    prepare.add_argument("--candidate-root", type=Path, required=True)
    prepare.add_argument("--baseline-root", type=Path)
    prepare.add_argument("--reference-root", type=Path, required=True)
    prepare.add_argument("--local-science", type=Path, required=True)
    prepare.add_argument("--local-smoke", type=Path, required=True)
    prepare.add_argument("--stata", required=True)
    prepare.add_argument("--repetitions", type=int, default=3)
    setup = commands.add_parser("record-setup")
    setup.add_argument("--run-root", type=Path, required=True)
    setup.add_argument("--job-id", required=True)
    verify = commands.add_parser("verify-host-class")
    verify.add_argument("--qhost", type=Path, required=True)
    task = commands.add_parser("run-task")
    task.add_argument("--run-root", type=Path, required=True)
    task.add_argument("--qhost", type=Path, required=True)
    merge = commands.add_parser("merge")
    merge.add_argument("--run-root", type=Path, required=True)
    submit = commands.add_parser("record-submission")
    submit.add_argument("--run-root", type=Path, required=True)
    submit.add_argument("--setup-job", required=True)
    submit.add_argument("--array-job", required=True)
    submit.add_argument("--collector-job", required=True)
    close = commands.add_parser("close")
    close.add_argument("--run-root", type=Path, required=True)
    close.add_argument("--setup-qacct", type=Path, required=True)
    close.add_argument("--array-qacct", type=Path, required=True)
    close.add_argument("--collector-qacct", type=Path, required=True)
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        manifest = load_manifest()
        if args.command == "validate-manifest":
            print(f"{manifest['manifest_id']} {sha256_file(MANIFEST)}")
        elif args.command == "cells":
            values = campaign_tasks(manifest) if args.profile == "scc_screen" else profile_scopes(manifest, args.profile)
            print(json.dumps(values, indent=2, sort_keys=True))
        elif args.command == "run-local":
            run_local(args, manifest)
        elif args.command == "analyze":
            exclusive_json(args.output, analyze_document(read_json(args.run), manifest))
        elif args.command == "prepare-run":
            prepare_run(args, manifest)
        elif args.command == "record-setup":
            record_setup(args)
        elif args.command == "verify-host-class":
            print(json.dumps(parse_qhost_class(args.qhost.read_text(encoding="utf-8")), sort_keys=True))
        elif args.command == "run-task":
            run_task(args, manifest)
        elif args.command == "merge":
            merge_tasks(args, manifest)
        elif args.command == "record-submission":
            record_submission(args)
        elif args.command == "close":
            close_run(args, manifest)
        return 0
    except (DevelopmentError, FileExistsError, KeyError, ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"ppml-solver-development: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
