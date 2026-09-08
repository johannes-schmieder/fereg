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
from typing import Callable, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "benchmarks/manifests/feppml_solver_development_v5.json"
DRIVER = ROOT / "tests/stata/development/feppml_solver_development.do"
KKT_DRIVER = ROOT / "tests/stata/development/feppml_public_state_kkt.do"
SOLVERS = ("map", "lsmr", "cmg")
LOCAL_PROFILES = (
    "local_science", "local_smoke", "local_one_fe_science", "local_breadth",
    "local_one_fe", "local_difficulty", "local_checkpoint", "local_cmg_envelope",
    "local_width", "local_scaling", "local_large_easy", "local_holdout",
)
COMPARATORS = ("ppmlhdfe",)
ADJUDICATOR = "ppmlhdfe_strict"
KKT_ADJUDICATOR = "independent_public_state_kkt_v3"
REFERENCE_ACCELERATION = "conjugate_gradient"
REFERENCE_TRANSFORM = "symmetric_kaczmarz"
ROUTE_CODES = {"map": 2, "lsmr": 3, "cmg": 7}
SOLVER_REPORT_ROWS = 4
SOLVER_REPORT_COLUMNS = 22
SOLVER_REPORT_CELLS = SOLVER_REPORT_ROWS * SOLVER_REPORT_COLUMNS
PUBLIC_STATE_METRICS = (
    "finite_score_absinf", "fe_score_absinf", "score_relinf", "score_rel2",
    "score_scale", "recomputed_ll", "recomputed_deviance", "min_mu", "max_mu",
    "modelspace_algorithm_code", "modelspace_status_code", "modelspace_iterations",
    "modelspace_columns", "modelspace_eta_scale", "modelspace_resid_maxabs",
    "modelspace_resid_rms", "modelspace_resid_relrms", "modelspace_tail_relrms",
    "modelspace_normal_rel2",
    "modelspace_normal_relinf", "modelspace_recurrence_relrms",
    "modelspace_breakdown_code", "projected_ll", "projected_deviance",
    "projected_min_mu", "projected_max_mu",
)
SAFE_ID = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$")
JOB_ID = re.compile(r"^[1-9][0-9]*$")
RUN_ROOT = re.compile(r"^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$")
IMPLEMENTATION_FILES = (
    "benchmarks/manifests/feppml_solver_development_v5.json",
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
    "tests/stata/development/feppml_public_state_kkt.do",
)


class DevelopmentError(RuntimeError):
    """A development input or receipt failed closed."""


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode()


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def file_receipt(path: Path) -> dict[str, object]:
    return {
        "path": str(path.resolve()),
        "size": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def valid_file_receipt(value: object) -> bool:
    return (
        isinstance(value, dict)
        and set(value) == {"path", "size", "sha256"}
        and isinstance(value["path"], str)
        and bool(value["path"])
        and isinstance(value["size"], int)
        and not isinstance(value["size"], bool)
        and value["size"] > 0
        and isinstance(value["sha256"], str)
        and bool(re.fullmatch(r"[0-9a-f]{64}", value["sha256"]))
    )


def valid_content_receipt(value: object) -> bool:
    return (
        isinstance(value, dict)
        and set(value) == {"size", "sha256"}
        and isinstance(value["size"], int)
        and not isinstance(value["size"], bool)
        and value["size"] > 0
        and isinstance(value["sha256"], str)
        and bool(re.fullmatch(r"[0-9a-f]{64}", value["sha256"]))
    )


def parsed_file_binding(receipt: Mapping[str, object], value: object) -> str:
    return canonical_sha256({
        "file": {"size": receipt["size"], "sha256": receipt["sha256"]},
        "parsed": value,
    })


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
            "solver_eligibility", "comparators", "scientific_adjudication", "fixed_policy", "science",
            "generator", "topologies", "profiles", "scc_campaign", "receipt_contract",
        },
        "manifest",
    )
    if (
        manifest["schema_version"] != 5
        or manifest["manifest_id"] != "feppml_solver_development_v5"
        or manifest["status"] != "development_only"
        or manifest["qualification_eligible"] is not False
        or manifest["auto_calibration"] is not False
        or manifest["public_command"] != "feppml"
    ):
        raise DevelopmentError("invalid v5 development identity")
    if manifest["protocols"] != {
        "request_wire": "1.5", "result_wire": "1.2", "action_protocol": "1.5",
        "development_result_table": "2.6",
    }:
        raise DevelopmentError("stale protocol identity")
    solvers = manifest["solvers"]
    if not isinstance(solvers, list) or [row.get("id") for row in solvers] != list(SOLVERS):
        raise DevelopmentError("public solver inventory must be map, lsmr, cmg")
    if {str(row["id"]): int(row["route_code"]) for row in solvers} != ROUTE_CODES:
        raise DevelopmentError("public route-code inventory changed")
    if manifest["solver_eligibility"] != {
        "source": "generated_dataset_metadata",
        "effect_count": "positive_level_counts",
        "sample_requirement": "every_successful_arm_equals_generated_N",
        "map_minimum_effects": 1,
        "lsmr_minimum_effects": 2,
        "cmg_effect_counts": [2, 3],
        "cmg_core_effects": "two_largest_level_counts",
        "cmg_max_nonbase_nuisance_columns": 64,
        "ineligible_solver_action": "exclude_timed_arm_and_test_public_r498_separately",
    }:
        raise DevelopmentError("public solver eligibility contract changed")
    comparators = manifest["comparators"]
    if not isinstance(comparators, list) or [row.get("id") for row in comparators] != list(COMPARATORS):
        raise DevelopmentError("comparator inventory changed")
    if comparators != [{
        "id": "ppmlhdfe", "timed": True,
        "acceleration": REFERENCE_ACCELERATION, "transform": REFERENCE_TRANSFORM,
        "one_effect_acceleration": "none",
    }]:
        raise DevelopmentError("default comparator contract changed")
    if manifest["scientific_adjudication"] != {
        "strict_reference": {
            "id": ADJUDICATOR, "timed": False,
            "invoke_only_after_default_reference_mismatch": True,
            "tolerance": 1e-10, "itolerance": 1e-12,
            "acceleration": REFERENCE_ACCELERATION,
            "one_effect_acceleration": "none",
            "transform": REFERENCE_TRANSFORM,
            "preserve_default_mismatch": True,
        },
        "public_state_kkt": {
            "id": KKT_ADJUDICATOR, "timed": False,
            "invoke_only_after_default_and_strict_coefficient_mismatch": True,
            "candidate_solver": "map",
            "reference_arms": ["ppmlhdfe", "ppmlhdfe_strict"],
            "score_tolerance": 1e-8,
            "candidate_objective_replay_rtol": 1e-8,
            "candidate_objective_replay_atol": 1e-10,
            "reference_public_state_objective_replay_rtol": 1e-8,
            "reference_public_state_objective_replay_atol": 1e-10,
            "authoritative_reference_objective_source": "reported_fit",
            "additional_objective_witness": (
                "raw_prediction_if_modelspace_feasible_and_stationary"
            ),
            "projected_state_role": "diagnostic_only",
            "objective_invariant_rtol": 1e-12,
            "objective_invariant_atol": 1e-8,
            "objective_dominance_rtol": 1e-8,
            "objective_dominance_atol": 1e-10,
            "model_space_projection": {
                "algorithm": "scaled_cgls_v1", "algorithm_code": 1,
                "max_iterations": 20_000, "refresh_interval": 32,
                "normal_rel2_loose": 5e-13,
                "normal_rel2_final": 5e-14,
                "normal_relinf_final": 5e-14,
                "recurrence_relrms_max": 1e-10,
                "tail_relrms_max": 1e-10,
                "feasible_relrms_max": 1e-10,
                "status_codes": {
                    "feasible": 1, "indeterminate": 3,
                },
                "breakdown_codes": {
                    "none": 0, "invalid_design": 1,
                    "nonpositive_curvature": 2, "nonfinite_recurrence": 3,
                    "recurrence_drift": 4, "iteration_limit": 5,
                    "final_certificate": 6, "projected_objective": 7,
                },
            },
            "indeterminate_or_nonstationary_reference_state_role": (
                "diagnostic_only"
            ),
            "preserve_default_and_strict_mismatches": True,
        },
    }:
        raise DevelopmentError("strict scientific adjudicator contract changed")
    if manifest["topologies"] != [
        "balanced_1fe", "skewed_1fe",
        "balanced_2fe", "skewed_2fe", "chain_2fe", "hub_2fe", "dense_2fe",
        "many_levels_2fe", "balanced_3fe", "sparse_3fe",
    ]:
        raise DevelopmentError("topology inventory changed")
    profiles = manifest["profiles"]
    if not isinstance(profiles, dict) or set(profiles) != set(LOCAL_PROFILES):
        raise DevelopmentError("local profile inventory changed")
    for profile_id in profiles:
        profile_scopes(dict(manifest), profile_id)
    tasks = campaign_tasks(dict(manifest))
    if (
        manifest["generator"].get("mixed_graph_memberships")
        != "degree_preserving_selected_g2_permutation_fixed_mt64_seed_104729"
        or manifest["generator"].get("sample_preflight")
        != "minimum_two_observations_per_retained_fe_level"
    ):
        raise DevelopmentError("generator sample-preservation contract changed")
    if manifest["generator"].get("same_data_for_thread_scaling_within_bundle") is not True:
        raise DevelopmentError("thread-scaling dataset-reuse contract changed")
    if any(
        len({dataset_id(scope) for scope in task["scopes"]}) != 1
        for task in tasks if str(task["bundle"]).startswith("scaling_")
    ):
        raise DevelopmentError("thread-scaling scopes must share one generated dataset")
    campaign = manifest["scc_campaign"]
    if len(tasks) != 16 or sum(len(task["scopes"]) for task in tasks) != 56:
        raise DevelopmentError("SCC campaign must contain 16 tasks and 56 scopes")
    if any(len(task["scopes"]) not in {2, 3, 4} for task in tasks):
        raise DevelopmentError("each SCC task must contain two to four sequential scopes")
    expected_scheduler = {
        "project": "welfgr", "parallel_environment": "omp", "slots": 28,
        "m_core": 28, "cpu_arch": "broadwell", "cpu_type": "E5-2680v4",
        "mem_per_core": "4G", "h_rt": "03:00:00",
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
        required = {"id", "n", "topology", "controls", "difficulty", "threads", "seed"}
        optional = {"mixing_percent", "third_fe_columns"}
        if not required <= set(scope) or set(scope) - required - optional:
            raise DevelopmentError(f"invalid scope fields: {scope}")
        if not SAFE_ID.fullmatch(str(scope["id"])) or str(scope["id"]) in ids:
            raise DevelopmentError(f"invalid or duplicate scope id: {scope['id']}")
        ids.add(str(scope["id"]))
        if int(scope["n"]) <= 0 or int(scope["controls"]) < 0 or int(scope["threads"]) <= 0:
            raise DevelopmentError(f"invalid scope dimensions: {scope['id']}")
        if scope["difficulty"] not in {"easy", "long"}:
            raise DevelopmentError(f"invalid difficulty: {scope['id']}")
        for name, maximum in (("mixing_percent", 100), ("third_fe_columns", 65)):
            if name in scope and (
                isinstance(scope[name], bool) or not isinstance(scope[name], int)
                or not 0 <= scope[name] <= maximum
            ):
                raise DevelopmentError(f"invalid {name}: {scope['id']}")
        if "third_fe_columns" in scope and not str(scope["topology"]).endswith("_3fe"):
            raise DevelopmentError("third-FE width requires a three-effect topology")


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
    seeds: dict[tuple[object, ...], int] = {}
    scopes = []
    for index, scope in enumerate(raw, 1):
        shape = tuple(scope.get(key) for key in (
            "n", "topology", "controls", "difficulty", "mixing_percent", "third_fe_columns",
        ))
        seed = int(scope.get("seed", seeds.setdefault(shape, seed_base + index)))
        scopes.append({**scope, "seed": seed})
    _validate_scopes(scopes, int(profile["expected_scopes"]))
    return scopes


def campaign_tasks(manifest: dict[str, object]) -> list[dict[str, object]]:
    campaign = manifest["scc_campaign"]
    shapes = campaign["bundle_shapes"]
    seed_base = int(manifest["generator"]["seed_namespace"]) + 1000
    tasks: list[dict[str, object]] = []
    dataset_ordinal = 0
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
        seeds: dict[tuple[int, str], int] = {}
        for control, difficulty, thread in dimensions:
            dataset_shape = (control, difficulty)
            if dataset_shape not in seeds:
                dataset_ordinal += 1
                seeds[dataset_shape] = seed_base + dataset_ordinal
            observations = (
                int(campaign["long_observations"])
                if difficulty == "long"
                else int(shape["n"])
            )
            scopes.append({
                "id": f"{bundle['id']}-k{control}-{difficulty}-t{thread}",
                "n": observations, "topology": str(bundle["topology"]),
                "controls": control, "difficulty": difficulty, "threads": thread,
                "seed": seeds[dataset_shape],
            })
        tasks.append({"task": int(bundle["task"]), "bundle": str(bundle["id"]), "scopes": scopes})
    if [task["task"] for task in tasks] != list(range(1, int(campaign["task_count"]) + 1)):
        raise DevelopmentError("SCC task IDs must be contiguous")
    all_scopes = [scope for task in tasks for scope in task["scopes"]]
    _validate_scopes(all_scopes, int(campaign["expected_scopes"]))
    return tasks


def dataset_id(scope: Mapping[str, object]) -> str:
    """Identify generated values independently of the requested thread count."""
    value = (
        f"{scope['topology']}-n{scope['n']}-k{scope['controls']}-"
        f"{scope['difficulty']}-s{scope['seed']}"
    )
    for name, suffix in (("mixing_percent", "mix"), ("third_fe_columns", "nu")):
        if name in scope:
            value += f"-{suffix}{scope[name]}"
    if not SAFE_ID.fullmatch(value):
        raise DevelopmentError(f"invalid generated dataset id: {value}")
    return value


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
    try:
        m_core_value = float(values["m_core"])
    except ValueError as error:
        raise DevelopmentError("qhost m_core value is not numeric") from error
    if not math.isfinite(m_core_value) or not m_core_value.is_integer():
        raise DevelopmentError("qhost m_core value is not an integer")
    observed = {
        "m_core": int(m_core_value),
        "cpu_arch": values["cpu_arch"],
        "cpu_type": values["cpu_type"],
    }
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


def reference_acceleration(scope: Mapping[str, object]) -> str:
    return "none" if str(scope.get("topology", "")).endswith("_1fe") else REFERENCE_ACCELERATION


def verified_reference_route(console: Path, scope: Mapping[str, object]) -> dict[str, str]:
    routes = set(re.findall(
        r"Running solver \(acceleration=([a-z_]+), transform=([a-z_]+)\b",
        process_log_text(console),
    ))
    expected = {(reference_acceleration(scope), REFERENCE_TRANSFORM)}
    if routes != expected:
        raise DevelopmentError(
            f"reference route differs: observed={sorted(routes)}, expected={sorted(expected)}"
        )
    return {
        "acceleration": reference_acceleration(scope),
        "transform": REFERENCE_TRANSFORM,
    }


def run_stata(
    stata: str, environment: Mapping[str, str], console: Path, driver: Path = DRIVER,
) -> dict[str, object]:
    console.parent.mkdir(parents=True, exist_ok=True)
    clean = {key: value for key, value in os.environ.items() if not key.startswith("FEREG_PPML_SOLVER_")}
    clean.update(environment)
    clean["FEREG_PPML_SOLVER_STATA_LOG"] = str(console.with_suffix(".stata.log"))
    if clean.get("FEREG_SOLVER_TRACE"):
        clean["FEREG_SOLVER_TRACE_FILE"] = str(console.with_suffix(".solver.trace"))
    with console.open("wb") as target:
        completed = subprocess.run([stata, "-b", "do", str(driver)], cwd=console.parent, env=clean, stdout=target, stderr=subprocess.STDOUT, check=False)
    return {"command": [stata, "-b", "do", str(driver)], "console": str(console), "process_returncode": completed.returncode}


def base_environment(repository: Path, reference_root: Path, scope: Mapping[str, object], dataset: Path, result: Path) -> dict[str, str]:
    return {
        "FEREG_PPML_SOLVER_REPOSITORY": str(repository), "FEREG_PPML_SOLVER_REFERENCE_ROOT": str(reference_root),
        "FEREG_PPML_SOLVER_DATASET": str(dataset), "FEREG_PPML_SOLVER_RESULT": str(result),
        "FEREG_PPML_SOLVER_N": str(scope["n"]), "FEREG_PPML_SOLVER_K": str(scope["controls"]),
        "FEREG_PPML_SOLVER_TOPOLOGY": str(scope["topology"]), "FEREG_PPML_SOLVER_DIFFICULTY": str(scope["difficulty"]),
        "FEREG_PPML_SOLVER_THREADS": str(scope["threads"]), "FEREG_PPML_SOLVER_SEED": str(scope["seed"]),
        "FEREG_PPML_SOLVER_MIXING_PERCENT": str(scope.get("mixing_percent", 0)),
        "FEREG_PPML_SOLVER_THIRD_COLUMNS": str(scope.get("third_fe_columns", "")),
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


def _valid_solver_matrix(value: object) -> bool:
    return (
        isinstance(value, list)
        and len(value) == SOLVER_REPORT_CELLS
        and all(
            item is None
            or (
                not isinstance(item, bool)
                and isinstance(item, (int, float))
                and math.isfinite(float(item))
            )
            for item in value
        )
    )


def solver_engine_summary(rows: Sequence[Mapping[str, object]]) -> dict[str, object]:
    fields = {
        "ordinary_frames": 7, "checkpoint_frames": 8, "retries": 9,
        "fallbacks": 10, "iterations": 12, "setup_seconds": 13,
        "solve_seconds": 14, "certificate_seconds": 15,
    }
    matrices = [row["solver_matrix_values"] for row in rows if int(row["rc"]) == 0]
    if not matrices or any(not _valid_solver_matrix(matrix) for matrix in matrices):
        return {}
    output = {}
    for index, engine in enumerate(("map", "lsmr_fused", "lsmr_joint", "cmg_prepared")):
        summary = {}
        for field, column in fields.items():
            values = [matrix[index * SOLVER_REPORT_COLUMNS + column] for matrix in matrices]
            summary[field] = statistics.median(values) if all(value is not None for value in values) else None
        output[engine] = summary
    return output


def parse_table(path: Path, scope: Mapping[str, object], implementation: str) -> list[dict[str, object]]:
    required = {
        "arm", "repetition", "rc", "timed", "seconds", "N", "ll", "deviance", "converged", "rank", "df_a", "df_resid",
        "inner_iterations", "route_code", "selected_engine_code", "effective_engine_code",
        "fallback_count", "effective_threads", "b_names", "b_values", "v_names",
        "v_values", "certificate_values", "runtime_values", "solver_matrix_values",
        "reference_acceleration", "reference_transform",
    }
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        if set(reader.fieldnames or []) != required:
            raise DevelopmentError(f"result columns differ: {path}")
        rows = list(reader)
    output: list[dict[str, object]] = []
    for row in rows:
        arm = row["arm"]
        if arm not in (*SOLVERS, *COMPARATORS, ADJUDICATOR):
            raise DevelopmentError(f"unknown result arm: {arm}")
        seconds = optional_float(row["seconds"])
        timed = int(row["timed"])
        if timed not in {0, 1}:
            raise DevelopmentError(f"invalid timing marker for {arm}")
        if arm == ADJUDICATOR:
            if implementation != "adjudicator" or timed or seconds is not None:
                raise DevelopmentError("strict adjudicator must be an untimed adjudicator record")
        elif implementation == "adjudicator" or not timed or seconds is None or seconds < 0.0:
            raise DevelopmentError(f"invalid elapsed time for {arm}")
        expected_reference_route = arm in (*COMPARATORS, ADJUDICATOR)
        if expected_reference_route and (
            row["reference_acceleration"] != reference_acceleration(scope)
            or row["reference_transform"] != REFERENCE_TRANSFORM
        ):
            raise DevelopmentError(f"unverified reference route for {arm}")
        if not expected_reference_route and (
            row["reference_acceleration"] or row["reference_transform"]
        ):
            raise DevelopmentError(f"unexpected reference route for {arm}")
        solver_matrix_values = _optional_vector(row["solver_matrix_values"])
        if arm in SOLVERS and int(row["rc"]) == 0:
            if len(solver_matrix_values) != SOLVER_REPORT_CELLS:
                raise DevelopmentError(
                    f"successful {arm} result lacks its 4x22 solver matrix"
                )
        elif solver_matrix_values:
            raise DevelopmentError(f"unexpected solver matrix for {arm}")
        output.append({
            **scope, "scope_id": str(scope["id"]), "implementation": implementation,
            "arm_type": (
                "solver" if arm in SOLVERS else
                "comparator" if arm in COMPARATORS else "adjudicator"
            ),
            "arm": arm, "repetition": int(row["repetition"]), "rc": int(row["rc"]),
            "timed": bool(timed), "seconds": seconds,
            "sample_n": optional_float(row["N"]), "ll": optional_float(row["ll"]), "deviance": optional_float(row["deviance"]),
            "converged": optional_float(row["converged"]), "rank": optional_float(row["rank"]), "df_a": optional_float(row["df_a"]),
            "df_resid": optional_float(row["df_resid"]), "inner_iterations": optional_float(row["inner_iterations"]),
            "route_code": optional_float(row["route_code"]),
            "selected_engine_code": optional_float(row["selected_engine_code"]),
            "effective_engine_code": optional_float(row["effective_engine_code"]),
            "fallback_count": optional_float(row["fallback_count"]),
            "effective_threads": optional_float(row["effective_threads"]), "b_names": row["b_names"].split(),
            "b_values": _float_vector(row["b_values"]), "v_names": row["v_names"].split(), "v_values": _float_vector(row["v_values"]),
            "certificate_values": _optional_vector(row["certificate_values"]), "runtime_values": _optional_vector(row["runtime_values"]),
            "solver_matrix_values": solver_matrix_values,
            "reference_acceleration": row["reference_acceleration"],
            "reference_transform": row["reference_transform"],
        })
    return output


def parse_public_state_fit(
    path: Path, scope: Mapping[str, object], implementation: str, expected_arm: str,
) -> dict[str, object]:
    required = {
        "arm", "N", "ll", "deviance", "converged", "rank", "df_a", "df_resid",
        "route_code", "selected_engine_code", "effective_engine_code", "fallback_count",
        "effective_threads", "certificate_passed", "b_names", "b_values",
        "reference_acceleration", "reference_transform",
    }
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        if set(reader.fieldnames or []) != required:
            raise DevelopmentError(f"public-state fit columns differ: {path}")
        rows = list(reader)
    if len(rows) != 1 or rows[0]["arm"] != expected_arm:
        raise DevelopmentError("public-state fit arm inventory differs")
    row = rows[0]
    reference = expected_arm in (*COMPARATORS, ADJUDICATOR)
    if reference != (implementation == "reference"):
        raise DevelopmentError("public-state fit implementation differs from arm")
    if reference and (
        row["reference_acceleration"] != reference_acceleration(scope)
        or row["reference_transform"] != REFERENCE_TRANSFORM
    ):
        raise DevelopmentError("public-state reference route differs")
    if not reference and (
        row["reference_acceleration"] or row["reference_transform"]
    ):
        raise DevelopmentError("public-state candidate has a reference route")
    return {
        **scope, "scope_id": str(scope["id"]), "implementation": implementation,
        "arm": expected_arm, "sample_n": optional_float(row["N"]),
        "ll": optional_float(row["ll"]), "deviance": optional_float(row["deviance"]),
        "converged": optional_float(row["converged"]), "rank": optional_float(row["rank"]),
        "df_a": optional_float(row["df_a"]), "df_resid": optional_float(row["df_resid"]),
        "route_code": optional_float(row["route_code"]),
        "selected_engine_code": optional_float(row["selected_engine_code"]),
        "effective_engine_code": optional_float(row["effective_engine_code"]),
        "fallback_count": optional_float(row["fallback_count"]),
        "effective_threads": optional_float(row["effective_threads"]),
        "certificate_passed": optional_float(row["certificate_passed"]),
        "b_names": row["b_names"].split(), "b_values": _float_vector(row["b_values"]),
        "reference_acceleration": row["reference_acceleration"],
        "reference_transform": row["reference_transform"],
    }


def parse_public_state_kkt(path: Path) -> dict[str, dict[str, float]]:
    required = {"arm", *PUBLIC_STATE_METRICS}
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        if set(reader.fieldnames or []) != required:
            raise DevelopmentError(f"public-state KKT columns differ: {path}")
        rows = list(reader)
    if [row["arm"] for row in rows] != ["map", "ppmlhdfe", ADJUDICATOR]:
        raise DevelopmentError("public-state KKT arm inventory differs")
    parsed: dict[str, dict[str, float]] = {}
    for row in rows:
        metrics: dict[str, float] = {}
        for field in PUBLIC_STATE_METRICS:
            value = optional_float(row[field])
            if value is None:
                raise DevelopmentError("public-state KKT metric is missing")
            metrics[field] = value
        parsed[row["arm"]] = metrics
    return parsed


def parse_data_meta(path: Path) -> dict[str, int]:
    with path.open(encoding="utf-8", newline="") as source:
        rows = list(csv.DictReader(source, delimiter="\t"))
    if len(rows) != 1 or set(rows[0]) != {"N", "levels1", "levels2", "levels3"}:
        raise DevelopmentError("dataset metadata must contain one exact row")
    return {key: int(value) for key, value in rows[0].items()}


def solver_eligibility(
    metadata: Mapping[str, object], topology: str, manifest: Mapping[str, object],
) -> dict[str, object]:
    if set(metadata) != {"N", "levels1", "levels2", "levels3"}:
        raise DevelopmentError("dataset metadata fields differ")
    values = [metadata["N"], *(metadata[f"levels{index}"] for index in range(1, 4))]
    if any(isinstance(value, bool) or not isinstance(value, int) for value in values):
        raise DevelopmentError("dataset metadata dimensions must be exact integers")
    observations = int(metadata["N"])
    level_counts = [int(metadata[f"levels{index}"]) for index in range(1, 4)]
    if observations <= 0 or any(level < 0 or level > observations for level in level_counts):
        raise DevelopmentError("dataset metadata level counts must lie between zero and N")
    if topology.endswith("_1fe"):
        valid_topology = level_counts[0] > 0 and level_counts[1:] == [0, 0]
    elif topology.endswith("_2fe"):
        valid_topology = level_counts[0] > 0 and level_counts[1] > 0 and level_counts[2] == 0
    elif topology.endswith("_3fe"):
        valid_topology = all(level > 0 for level in level_counts)
    else:
        valid_topology = False
    if topology not in manifest["topologies"] or not valid_topology:
        raise DevelopmentError("dataset level counts differ from the declared topology")
    effects = sorted((level for level in level_counts if level > 0), reverse=True)
    policy = manifest["solver_eligibility"]
    if len(effects) < int(policy["map_minimum_effects"]):
        raise DevelopmentError("development dataset is outside the explicit-solver envelope")
    cmg_effect_counts = {int(value) for value in policy["cmg_effect_counts"]}
    nuisance_columns = sum(level - 1 for level in effects[2:])
    eligible_solvers = ["map"]
    excluded = {}
    if len(effects) >= int(policy["lsmr_minimum_effects"]):
        eligible_solvers.append("lsmr")
    else:
        excluded["lsmr"] = "requires_at_least_two_categorical_effects"
    if len(effects) not in cmg_effect_counts:
        excluded["cmg"] = "requires_two_or_three_categorical_effects"
    elif nuisance_columns > int(policy["cmg_max_nonbase_nuisance_columns"]):
        excluded["cmg"] = "three_fe_nuisance_columns_exceed_64"
    else:
        eligible_solvers.append("cmg")
    return {
        "effect_count": len(effects),
        "level_counts_descending": effects,
        "cmg_nuisance_columns": nuisance_columns,
        "eligible_solvers": eligible_solvers,
        "excluded_solvers": excluded,
    }


def _configurations(
    repositories: Mapping[str, Path], eligible_solvers: Sequence[str],
    include_comparators: bool, rotation: int,
) -> list[tuple[str, str]]:
    if not eligible_solvers or any(solver not in SOLVERS for solver in eligible_solvers):
        raise DevelopmentError("invalid eligible solver inventory")
    values = [
        (implementation, solver)
        for implementation in repositories for solver in eligible_solvers
    ]
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
    process_runner: Callable[..., dict[str, object]] | None = None,
) -> dict[str, object]:
    if output_dir.exists():
        raise DevelopmentError(f"output already exists: {output_dir}")
    output_dir.mkdir(parents=True)
    run_process = process_runner or run_stata
    repositories = {"candidate": candidate_root.resolve()}
    if baseline_root is not None:
        repositories["baseline"] = baseline_root.resolve()
    records: list[dict[str, object]] = []
    processes: list[dict[str, object]] = []
    datasets: list[dict[str, object]] = []
    public_state_adjudications: list[dict[str, object]] = []
    projection_policy = manifest["scientific_adjudication"]["public_state_kkt"][
        "model_space_projection"
    ]
    dataset_cache: dict[str, tuple[Path, Path, Path, str, dict[str, object]]] = {}
    if prepare_comparators:
        console = output_dir / "reference-setup.log"
        process = run_process(
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
        generated_id = dataset_id(scope)
        cached = dataset_cache.get(generated_id)
        if cached is None:
            data_dir = output_dir / "datasets" / generated_id
            data_dir.mkdir(parents=True)
            dataset = data_dir / "data.dta"
            data_csv = data_dir / "data.csv"
            meta = data_dir / "data-meta.tsv"
            env = base_environment(candidate_root, reference_root, scope, dataset, scope_dir / "unused.tsv")
            env.update({"FEREG_PPML_SOLVER_MODE": "generate", "FEREG_PPML_SOLVER_DATA_CSV": str(data_csv), "FEREG_PPML_SOLVER_DATA_META": str(meta)})
            process = run_process(stata, env, scope_dir / "generate.log")
            status, reason = stata_stage_status(scope_dir / "generate.log", int(process["process_returncode"]), "FEREG_PPML_SOLVER_DATA_PASS")
            processes.append({**process, "stage": "generate", "scope_id": scope["id"], "dataset_id": generated_id, "accepted_rc": status, "status_reason": reason})
            if status:
                raise DevelopmentError(f"dataset generation failed for {scope['id']}: {reason}")
            file_identity = {
                "dta": {"sha256": sha256_file(dataset), "size": dataset.stat().st_size},
                "csv": {"sha256": sha256_file(data_csv), "size": data_csv.stat().st_size},
                "metadata_file": {"sha256": sha256_file(meta), "size": meta.stat().st_size},
                "metadata": parse_data_meta(meta),
            }
            generated_by_scope_id = str(scope["id"])
            dataset_cache[generated_id] = (
                dataset, data_csv, meta, generated_by_scope_id, file_identity,
            )
        else:
            dataset, data_csv, meta, generated_by_scope_id, file_identity = cached
        dataset_receipt = {
            "scope": dict(scope), "dataset_id": generated_id,
            "generated_by_scope_id": generated_by_scope_id, **file_identity,
        }
        eligibility = solver_eligibility(
            file_identity["metadata"], str(scope["topology"]), manifest,
        )
        if int(file_identity["metadata"]["N"]) != int(scope["n"]):
            raise DevelopmentError("generated dataset observation count differs from scope")
        dataset_receipt["solver_eligibility"] = eligibility
        datasets.append(dataset_receipt)
        if include_oracle:
            records.append({**scope, "scope_id": scope["id"], "implementation": "independent", "arm_type": "oracle", "arm": "explicit_dummy", **explicit_dummy_oracle(data_csv, int(scope["controls"]))})
        for implementation, arm in _configurations(
            repositories, eligibility["eligible_solvers"], include_comparators,
            scope_index,
        ):
            repository = candidate_root if implementation == "reference" else repositories[implementation]
            result = scope_dir / f"{implementation}-{arm}.tsv"
            console = scope_dir / f"{implementation}-{arm}.log"
            run_env = base_environment(repository, reference_root, scope, dataset, result)
            run_env.update({"FEREG_PPML_SOLVER_MODE": "run", "FEREG_PPML_SOLVER_ARM": arm, "FEREG_PPML_SOLVER_REPETITIONS": str(repetitions)})
            process = run_process(stata, run_env, console)
            status, reason = stata_stage_status(console, int(process["process_returncode"]), "FEREG_PPML_SOLVER_RUN_PASS")
            reference_route = None
            if not status and arm in COMPARATORS:
                try:
                    reference_route = verified_reference_route(console, scope)
                except DevelopmentError:
                    status, reason = 498, "unverified-reference-route"
            processes.append({**process, "stage": "run", "scope_id": scope["id"], "implementation": implementation, "arm": arm, "accepted_rc": status, "status_reason": reason, "reference_route": reference_route})
            # A killed process may leave a truncated table or a valid-looking
            # prefix. Neither is a completed scientific result.
            if result.is_file() and not process.get("budget_exhausted", False):
                records.extend(parse_table(result, scope, implementation))
        if include_comparators and _needs_strict_adjudication(
            records, scope, list(repositories), repetitions, manifest["science"],
            eligibility["eligible_solvers"],
        ):
            result = scope_dir / f"adjudicator-{ADJUDICATOR}.tsv"
            console = scope_dir / f"adjudicator-{ADJUDICATOR}.log"
            run_env = base_environment(candidate_root, reference_root, scope, dataset, result)
            run_env.update({
                "FEREG_PPML_SOLVER_MODE": "run", "FEREG_PPML_SOLVER_ARM": ADJUDICATOR,
                "FEREG_PPML_SOLVER_REPETITIONS": "1",
            })
            process = run_process(stata, run_env, console)
            status, reason = stata_stage_status(
                console, int(process["process_returncode"]),
                "FEREG_PPML_SOLVER_RUN_PASS",
            )
            reference_route = None
            if not status:
                try:
                    reference_route = verified_reference_route(console, scope)
                except DevelopmentError:
                    status, reason = 498, "unverified-reference-route"
            processes.append({
                **process, "stage": "adjudicate", "scope_id": scope["id"],
                "implementation": "adjudicator", "arm": ADJUDICATOR,
                "accepted_rc": status, "status_reason": reason,
                "reference_route": reference_route,
            })
            if result.is_file() and not process.get("budget_exhausted", False):
                records.extend(parse_table(result, scope, "adjudicator"))
        kkt_implementations = _public_state_kkt_implementations(
            records, scope, list(repositories), repetitions, manifest["science"],
            eligibility["eligible_solvers"],
        ) if include_comparators else []
        if kkt_implementations:
            state_fits: dict[tuple[str, str], dict[str, object]] = {}
            state_files: dict[tuple[str, str], dict[str, object]] = {}
            fit_specs = [
                (implementation, "map", repositories[implementation])
                for implementation in kkt_implementations
            ]
            fit_specs.extend([
                ("reference", "ppmlhdfe", candidate_root),
                ("reference", ADJUDICATOR, candidate_root),
            ])
            for implementation, arm, repository in fit_specs:
                stem = f"public-state-{implementation}-{arm}"
                result = scope_dir / f"{stem}.tsv"
                state = scope_dir / f"{stem}.dta"
                console = scope_dir / f"{stem}.log"
                run_env = base_environment(repository, reference_root, scope, dataset, result)
                run_env.update({
                    "FEREG_PPML_SOLVER_KKT_MODE": "fit",
                    "FEREG_PPML_SOLVER_ARM": arm,
                    "FEREG_PPML_SOLVER_KKT_STATE": str(state),
                    "FEREG_PPML_SOLVER_KKT_PLUGIN": (
                        str(artifacts[implementation]["path"])
                        if implementation != "reference" else ""
                    ),
                })
                process = run_process(stata, run_env, console, KKT_DRIVER)
                status, reason = stata_stage_status(
                    console, int(process["process_returncode"]),
                    "FEREG_PPML_SOLVER_KKT_FIT_PASS",
                )
                reference_route = None
                if not status and implementation == "reference":
                    try:
                        reference_route = verified_reference_route(console, scope)
                    except DevelopmentError:
                        status, reason = 498, "unverified-reference-route"
                state_receipt = file_receipt(state) if state.is_file() else None
                result_receipt = file_receipt(result) if result.is_file() else None
                parsed_fit = None
                parsed_result_sha256 = None
                if not status and state_receipt is not None and result_receipt is not None:
                    parsed_fit = parse_public_state_fit(
                        result, scope, implementation, arm,
                    )
                    parsed_result_sha256 = parsed_file_binding(
                        result_receipt, parsed_fit,
                    )
                processes.append({
                    **process, "stage": "public-state-fit", "scope_id": scope["id"],
                    "dataset_id": generated_id,
                    "dataset_sha256": file_identity["dta"]["sha256"],
                    "implementation": implementation, "arm": arm,
                    "source_sha256": canonical_sha256(
                        references_before if implementation == "reference" else sources[implementation]
                    ),
                    "artifact_sha256": (
                        None if implementation == "reference"
                        else artifacts[implementation]["sha256"]
                    ),
                    "accepted_rc": status, "status_reason": reason,
                    "reference_route": reference_route,
                    "state": state_receipt, "result": result_receipt,
                    "parsed_result_sha256": parsed_result_sha256,
                })
                if parsed_fit is None:
                    continue
                key = (implementation, arm)
                state_fits[key] = parsed_fit
                state_files[key] = state_receipt
            reference_keys = [
                ("reference", "ppmlhdfe"), ("reference", ADJUDICATOR),
            ]
            for implementation in kkt_implementations:
                candidate_key = (implementation, "map")
                required_keys = [candidate_key, *reference_keys]
                if any(key not in state_fits for key in required_keys):
                    continue
                result = scope_dir / f"public-state-audit-{implementation}.tsv"
                console = scope_dir / f"public-state-audit-{implementation}.log"
                before = {arm: dict(state_files[key]) for arm, key in (
                    ("map", candidate_key),
                    ("ppmlhdfe", reference_keys[0]),
                    (ADJUDICATOR, reference_keys[1]),
                )}
                run_env = base_environment(
                    repositories[implementation], reference_root, scope, dataset, result,
                )
                run_env.update({
                    "FEREG_PPML_SOLVER_KKT_MODE": "audit",
                    "FEREG_PPML_SOLVER_KKT_CANDIDATE_STATE": str(Path(before["map"]["path"])),
                    "FEREG_PPML_SOLVER_KKT_DEFAULT_STATE": str(Path(before["ppmlhdfe"]["path"])),
                    "FEREG_PPML_SOLVER_KKT_STRICT_STATE": str(Path(before[ADJUDICATOR]["path"])),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_MAXITER": str(
                        projection_policy["max_iterations"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_REFRESH": str(
                        projection_policy["refresh_interval"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_NORMAL_LOOSE": str(
                        projection_policy["normal_rel2_loose"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_NORMAL_FINAL": str(
                        projection_policy["normal_rel2_final"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_NORMAL_INFINITY": str(
                        projection_policy["normal_relinf_final"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_RECURRENCE": str(
                        projection_policy["recurrence_relrms_max"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_TAIL": str(
                        projection_policy["tail_relrms_max"]
                    ),
                    "FEREG_PPML_SOLVER_KKT_PROJECTION_FEASIBLE": str(
                        projection_policy["feasible_relrms_max"]
                    ),
                })
                process = run_process(stata, run_env, console, KKT_DRIVER)
                status, reason = stata_stage_status(
                    console, int(process["process_returncode"]),
                    "FEREG_PPML_SOLVER_KKT_AUDIT_PASS",
                )
                after = {
                    arm: file_receipt(Path(receipt["path"]))
                    for arm, receipt in before.items()
                }
                if before != after:
                    raise DevelopmentError("public-state KKT input changed during audit")
                result_receipt = file_receipt(result) if result.is_file() else None
                metrics = None
                parsed_result_sha256 = None
                if not status and result_receipt is not None:
                    metrics = parse_public_state_kkt(result)
                    parsed_result_sha256 = parsed_file_binding(result_receipt, metrics)
                processes.append({
                    **process, "stage": "public-state-audit", "scope_id": scope["id"],
                    "dataset_id": generated_id,
                    "dataset_sha256": file_identity["dta"]["sha256"],
                    "implementation": implementation, "arm": KKT_ADJUDICATOR,
                    "source_sha256": canonical_sha256(sources[implementation]),
                    "artifact_sha256": artifacts[implementation]["sha256"],
                    "accepted_rc": status, "status_reason": reason,
                    "inputs": before, "result": result_receipt,
                    "parsed_result_sha256": parsed_result_sha256,
                })
                if metrics is None:
                    continue
                public_state_adjudications.append({
                    **scope, "scope_id": str(scope["id"]),
                    "implementation": implementation, "method": KKT_ADJUDICATOR,
                    "fits": {
                        "map": state_fits[candidate_key],
                        "ppmlhdfe": state_fits[reference_keys[0]],
                        ADJUDICATOR: state_fits[reference_keys[1]],
                    },
                    "states": before, "audit_result": result_receipt,
                    "metrics": metrics,
                })
    references_after = reference_identity(reference_root)
    if references_before != references_after:
        raise DevelopmentError("pinned reference closure changed during execution")
    return {
        "schema_version": 5, "kind": "feppml_solver_development_run", "qualification_eligible": False,
        "auto_calibration": False, "manifest": {"path": str(MANIFEST), "sha256": sha256_file(MANIFEST)},
        "implementation": implementation_identity(), "repetitions": repetitions, "include_comparators": include_comparators,
        "include_oracle": include_oracle, "sources": sources, "artifacts": artifacts, "references": references_after,
        "host": cpu_identity(), "task_identity": dict(task_identity) if task_identity else None,
        "scopes": [dict(scope) for scope in scopes], "datasets": datasets, "records": records,
        "public_state_adjudications": public_state_adjudications,
        "processes": processes,
    }


def _close(left: float | None, right: float | None, rtol: float, atol: float) -> bool:
    return (
        left is not None
        and right is not None
        and abs(left - right) <= atol + rtol * max(abs(left), abs(right))
    )


def _finite_float(value: object, *, nonnegative: bool = False) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    numeric = float(value)
    if not math.isfinite(numeric) or (nonnegative and numeric < 0.0):
        return None
    return numeric


def _objective_pair(ll: object, deviance: object) -> tuple[float, float] | None:
    numeric_ll = _finite_float(ll)
    numeric_deviance = _finite_float(deviance, nonnegative=True)
    if numeric_ll is None or numeric_deviance is None:
        return None
    return numeric_ll, numeric_deviance


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


def _row_matches_scope(row: Mapping[str, object], scope: Mapping[str, object]) -> bool:
    return all(row.get(field) == value for field, value in scope.items())


def _science_issues(
    row: Mapping[str, object], target: Mapping[str, object], controls: int,
    science: Mapping[str, object], target_name: str, dof_notes: set[str],
) -> list[str]:
    issues: list[str] = []
    if row["sample_n"] != target["sample_n"]:
        issues.append(f"{target_name}_sample")
    if (
        row["b_names"][:controls] != target["b_names"][:controls]
        or not _vectors_close(
            row["b_values"][:controls], target["b_values"][:controls],
            float(science["coefficient_rtol"]), float(science["coefficient_atol"]),
        )
    ):
        issues.append(f"{target_name}_coefficients")
    if (
        row["v_names"][:controls] != target["v_names"][:controls]
        or not _vectors_close(
            _control_covariance(row, controls), _control_covariance(target, controls),
            float(science["vce_rtol"]), float(science["vce_atol"]),
        )
    ):
        issues.append(f"{target_name}_vce")
    for scalar in ("ll", "deviance"):
        if not _close(
            row[scalar], target[scalar],
            float(science["likelihood_rtol"]), float(science["likelihood_atol"]),
        ):
            issues.append(f"{target_name}_{scalar}")
    if target_name == "map":
        for scalar in ("rank", "df_a", "df_resid"):
            if row[scalar] != target[scalar]:
                issues.append(f"{target_name}_{scalar}")
        return issues

    # ADR-0022 gates ppmlhdfe on total model DoF. The comparator assigns
    # the normalized constant to df_a, while fereg exposes it as an omitted
    # finite slot.
    row_model_dof = _model_dof(row)
    target_model_dof = _model_dof(target)
    if row_model_dof is None or target_model_dof is None or row_model_dof != target_model_dof:
        issues.append(f"{target_name}_model_dof")
        return issues
    row_residual_dof = row["df_resid"]
    expected_row_residual_dof = float(row["sample_n"]) - row_model_dof
    if row_residual_dof is None or float(row_residual_dof) != expected_row_residual_dof:
        issues.append(f"{target_name}_candidate_df_resid")
    target_residual_dof = target["df_resid"]
    expected_target_residual_dof = float(target["sample_n"]) - target_model_dof
    if target_residual_dof is not None and float(target_residual_dof) != expected_target_residual_dof:
        issues.append(f"{target_name}_reference_df_resid")
    if row["rank"] != target["rank"] or row["df_a"] != target["df_a"]:
        dof_notes.add(
            "fereg and ppmlhdfe partition rank versus absorbed DoF differently; "
            "the gate compares their equal total model DoF"
        )
    if target_residual_dof is None:
        dof_notes.add(
            "ppmlhdfe does not publish residual DoF for robust VCE; the gate "
            "derives it from N minus total model DoF"
        )
    return issues


def _needs_strict_adjudication(
    records: Sequence[Mapping[str, object]], scope: Mapping[str, object],
    implementations: Sequence[str], repetitions: int, science: Mapping[str, object],
    eligible_solvers: Sequence[str],
) -> bool:
    scope_id = str(scope["id"])
    controls = int(scope["controls"])
    indexed = {
        (str(row["implementation"]), str(row["arm"]), int(row["repetition"])): row
        for row in records if str(row["scope_id"]) == scope_id and row["arm_type"] != "oracle"
    }
    for implementation in implementations:
        for repetition in range(1, repetitions + 1):
            target = indexed.get(("reference", "ppmlhdfe", repetition))
            if target is None or int(target["rc"]) != 0 or target["converged"] != 1.0:
                continue
            for solver in eligible_solvers:
                row = indexed.get((implementation, solver, repetition))
                if row is not None and int(row["rc"]) == 0 and row["converged"] == 1.0:
                    if _science_issues(row, target, controls, science, "ppmlhdfe", set()):
                        return True
    return False


def _public_state_kkt_implementations(
    records: Sequence[Mapping[str, object]], scope: Mapping[str, object],
    implementations: Sequence[str], repetitions: int, science: Mapping[str, object],
    eligible_solvers: Sequence[str],
) -> list[str]:
    scope_id = str(scope["id"])
    controls = int(scope["controls"])
    indexed = {
        (str(row["implementation"]), str(row["arm"]), int(row["repetition"])): row
        for row in records if str(row["scope_id"]) == scope_id and row["arm_type"] != "oracle"
    }
    strict = indexed.get(("adjudicator", ADJUDICATOR, 1))
    if strict is None or int(strict["rc"]) != 0 or strict["converged"] != 1.0:
        return []
    required: list[str] = []
    for implementation in implementations:
        unresolved = False
        eligible = True
        for repetition in range(1, repetitions + 1):
            default = indexed.get(("reference", "ppmlhdfe", repetition))
            if default is None or int(default["rc"]) != 0 or default["converged"] != 1.0:
                eligible = False
                break
            for solver in eligible_solvers:
                row = indexed.get((implementation, solver, repetition))
                if row is None or int(row["rc"]) != 0 or row["converged"] != 1.0:
                    eligible = False
                    break
                default_issues = _science_issues(
                    row, default, controls, science, "ppmlhdfe", set(),
                )
                if not default_issues:
                    continue
                if default_issues != ["ppmlhdfe_coefficients"]:
                    eligible = False
                    break
                strict_issues = _science_issues(
                    row, strict, controls, science, "strict_ppmlhdfe", set(),
                )
                if not strict_issues:
                    continue
                if strict_issues != ["strict_ppmlhdfe_coefficients"]:
                    eligible = False
                    break
                unresolved = True
            if not eligible:
                break
        if eligible and unresolved:
            required.append(implementation)
    return required


def _fit_binding_issues(
    fit: Mapping[str, object], target: Mapping[str, object], controls: int,
    science: Mapping[str, object], label: str,
) -> list[str]:
    issues: list[str] = []
    for field in ("sample_n", "converged", "rank", "df_a", "df_resid"):
        if fit.get(field) != target.get(field):
            issues.append(f"{label}_{field}")
    if (
        fit.get("b_names", [])[:controls] != target.get("b_names", [])[:controls]
        or not _vectors_close(
            fit.get("b_values", [])[:controls], target.get("b_values", [])[:controls],
            float(science["coefficient_rtol"]), float(science["coefficient_atol"]),
        )
    ):
        issues.append(f"{label}_coefficients")
    for field in ("ll", "deviance"):
        if not _close(
            fit.get(field), target.get(field),
            float(science["likelihood_rtol"]), float(science["likelihood_atol"]),
        ):
            issues.append(f"{label}_{field}")
    return issues


def _reference_objective_diagnostic(
    fit: Mapping[str, object], metrics: Mapping[str, object],
    policy: Mapping[str, object],
) -> dict[str, object]:
    if (
        policy.get("authoritative_reference_objective_source") != "reported_fit"
        or policy.get("additional_objective_witness")
        != "raw_prediction_if_modelspace_feasible_and_stationary"
        or policy.get("projected_state_role") != "diagnostic_only"
    ):
        raise DevelopmentError("reference objective source contract changed")
    reported = _objective_pair(fit.get("ll"), fit.get("deviance"))
    public = _objective_pair(
        metrics.get("recomputed_ll"), metrics.get("recomputed_deviance"),
    )
    projected = _objective_pair(
        metrics.get("projected_ll"), metrics.get("projected_deviance"),
    )
    replay_rtol = float(policy["reference_public_state_objective_replay_rtol"])
    replay_atol = float(policy["reference_public_state_objective_replay_atol"])
    ll_replays = (
        reported is not None and public is not None
        and _close(public[0], reported[0], replay_rtol, replay_atol)
    )
    deviance_replays = (
        reported is not None and public is not None
        and _close(public[1], reported[1], replay_rtol, replay_atol)
    )
    public_state_replays_fit = ll_replays and deviance_replays
    status = _finite_float(metrics.get("modelspace_status_code"))
    breakdown = _finite_float(metrics.get("modelspace_breakdown_code"))
    feasible_code = float(policy["model_space_projection"]["status_codes"]["feasible"])
    projection_policy = policy["model_space_projection"]
    residual_relrms = _finite_float(metrics.get("modelspace_resid_relrms"))
    normal_rel2 = _finite_float(metrics.get("modelspace_normal_rel2"))
    normal_relinf = _finite_float(metrics.get("modelspace_normal_relinf"))
    tail_relrms = _finite_float(metrics.get("modelspace_tail_relrms"))
    recurrence_relrms = _finite_float(metrics.get("modelspace_recurrence_relrms"))
    modelspace_feasible = (
        status == feasible_code
        and breakdown == 0.0
        and residual_relrms is not None
        and residual_relrms <= float(projection_policy["feasible_relrms_max"])
        and normal_rel2 is not None
        and normal_rel2 <= float(projection_policy["normal_rel2_final"])
        and normal_relinf is not None
        and normal_relinf <= float(projection_policy["normal_relinf_final"])
        and tail_relrms is not None
        and tail_relrms <= float(projection_policy["tail_relrms_max"])
        and recurrence_relrms is not None
        and recurrence_relrms <= float(projection_policy["recurrence_relrms_max"])
    )
    score_relinf = _finite_float(metrics.get("score_relinf"))
    score_rel2 = _finite_float(metrics.get("score_rel2"))
    score_tolerance = float(policy["score_tolerance"])
    stationary = (
        score_relinf is not None
        and score_relinf >= 0.0
        and score_relinf <= score_tolerance
        and score_rel2 is not None
        and score_rel2 >= 0.0
        and score_rel2 <= score_tolerance
    )
    raw_public_state_eligible = modelspace_feasible and stationary and public is not None
    eligible: list[tuple[str, tuple[float, float]]] = []
    if reported is not None:
        eligible.append(("reported_fit", reported))
    if raw_public_state_eligible and public is not None:
        eligible.append(("raw_public_state", public))
    selected = max(eligible, key=lambda item: item[1][0]) if eligible else None
    return {
        "public_state_replays_fit": public_state_replays_fit,
        "ll_replays_fit": ll_replays,
        "deviance_replays_fit": deviance_replays,
        "modelspace_status_code": status,
        "modelspace_feasible": modelspace_feasible,
        "stationary": stationary,
        "raw_public_state_eligible": raw_public_state_eligible,
        "projected_state_is_witness": False,
        "projected_ll": projected[0] if projected else None,
        "projected_deviance": projected[1] if projected else None,
        "eligible_sources": [item[0] for item in eligible],
        "comparison_source": selected[0] if selected else None,
        "comparison_ll": selected[1][0] if selected else None,
        "comparison_deviance": selected[1][1] if selected else None,
    }


def _public_state_kkt_issues(
    evidence: Mapping[str, object], indexed: Mapping[tuple[str, str, str, int], Mapping[str, object]],
    strict: Mapping[str, object], scope: Mapping[str, object], implementation: str,
    repetitions: int, expected_modelspace_columns: int,
    science: Mapping[str, object], policy: Mapping[str, object],
) -> list[str]:
    issues: list[str] = []
    scope_id = str(scope["id"])
    controls = int(scope["controls"])
    fits = evidence["fits"]
    metrics = evidence["metrics"]
    candidate_fit = fits["map"]
    if (
        candidate_fit.get("route_code") != float(ROUTE_CODES["map"])
        or candidate_fit.get("selected_engine_code") != 1.0
        or candidate_fit.get("effective_engine_code") != 1.0
        or candidate_fit.get("fallback_count") != 0.0
        or candidate_fit.get("effective_threads") != float(scope["threads"])
        or candidate_fit.get("certificate_passed") != 1.0
    ):
        issues.append("candidate_route_or_certificate")
    for arm in ("ppmlhdfe", ADJUDICATOR):
        fit = fits[arm]
        if (
            fit.get("reference_acceleration") != reference_acceleration(scope)
            or fit.get("reference_transform") != REFERENCE_TRANSFORM
            or any(
                fit.get(field) is not None
                for field in (
                    "route_code", "selected_engine_code", "effective_engine_code",
                    "fallback_count", "effective_threads", "certificate_passed",
                )
            )
        ):
            issues.append(f"{arm}_route")
    for repetition in range(1, repetitions + 1):
        candidate = indexed.get((scope_id, implementation, "map", repetition))
        default = indexed.get((scope_id, "reference", "ppmlhdfe", repetition))
        if candidate is None or default is None:
            issues.append(f"trigger_r{repetition}_missing")
            continue
        issues.extend(
            _fit_binding_issues(
                candidate_fit, candidate, controls, science,
                f"candidate_map_r{repetition}",
            )
        )
        issues.extend(
            _fit_binding_issues(
                fits["ppmlhdfe"], default, controls, science,
                f"ppmlhdfe_r{repetition}",
            )
        )
    issues.extend(
        _fit_binding_issues(
            fits[ADJUDICATOR], strict, controls, science, ADJUDICATOR,
        )
    )
    if any(
        fit.get("sample_n") != float(scope["n"])
        or fit.get("converged") != 1.0
        for fit in fits.values()
    ):
        issues.append("fit_sample_or_convergence")

    projection_policy = policy["model_space_projection"]
    feasible_code = float(projection_policy["status_codes"]["feasible"])
    expected_algorithm = float(projection_policy["algorithm_code"])
    for arm, row in metrics.items():
        if set(row) != set(PUBLIC_STATE_METRICS):
            issues.append(f"{arm}_metric_inventory")
            continue
        numeric = {field: _finite_float(row.get(field)) for field in PUBLIC_STATE_METRICS}
        if any(value is None for value in numeric.values()):
            issues.append(f"{arm}_invalid_metric")
            continue
        values = {field: float(value) for field, value in numeric.items() if value is not None}
        status = values["modelspace_status_code"]
        allowed_status_codes = {
            float(code) for code in projection_policy["status_codes"].values()
        }
        if (
            values["finite_score_absinf"] < 0.0
            or values["fe_score_absinf"] < 0.0
            or values["score_relinf"] < 0.0
            or values["score_rel2"] < 0.0
            or values["score_scale"] <= 0.0
            or values["recomputed_deviance"] < 0.0
            or values["min_mu"] <= 0.0
            or values["max_mu"] < values["min_mu"]
            or values["modelspace_algorithm_code"] != expected_algorithm
            or values["modelspace_breakdown_code"] != 0.0
            or status not in allowed_status_codes
            or values["modelspace_iterations"] < 0.0
            or values["modelspace_iterations"] != int(values["modelspace_iterations"])
            or values["modelspace_iterations"] > float(projection_policy["max_iterations"])
            or values["modelspace_columns"] != expected_modelspace_columns
            or values["modelspace_columns"] != int(values["modelspace_columns"])
            or values["modelspace_eta_scale"] < 1.0
            or values["modelspace_resid_maxabs"] < values["modelspace_resid_rms"]
            or any(
                values[field] < 0.0 for field in (
                    "modelspace_resid_rms", "modelspace_resid_relrms",
                    "modelspace_tail_relrms", "modelspace_normal_rel2",
                    "modelspace_normal_relinf",
                    "modelspace_recurrence_relrms",
                )
            )
            or values["modelspace_normal_rel2"] > float(projection_policy["normal_rel2_final"])
            or values["modelspace_normal_relinf"] > float(
                projection_policy["normal_relinf_final"]
            )
            or values["modelspace_tail_relrms"] > float(
                projection_policy["tail_relrms_max"]
            )
            or values["modelspace_recurrence_relrms"] > float(
                projection_policy["recurrence_relrms_max"]
            )
            or values["projected_deviance"] < 0.0
            or values["projected_min_mu"] <= 0.0
            or values["projected_max_mu"] < values["projected_min_mu"]
            or not _close(
                values["modelspace_resid_relrms"],
                values["modelspace_resid_rms"] / values["modelspace_eta_scale"],
                1e-12, 1e-15,
            )
        ):
            issues.append(f"{arm}_invalid_metric")
        if (
            status == feasible_code
            and values["modelspace_resid_relrms"]
            > float(projection_policy["feasible_relrms_max"])
        ):
            issues.append(f"{arm}_modelspace_classification")
        if (
            status != feasible_code
            and values["modelspace_resid_relrms"]
            <= float(projection_policy["feasible_relrms_max"])
        ):
            issues.append(f"{arm}_modelspace_classification")
        if arm == "map" and status != feasible_code:
            issues.append("candidate_modelspace_certificate")
    tolerance = float(policy["score_tolerance"])
    candidate_metrics = metrics["map"]
    if (
        candidate_metrics["score_relinf"] > tolerance
        or candidate_metrics["score_rel2"] > tolerance
    ):
        issues.append("candidate_score_certificate")
    replay_rtol = float(policy["candidate_objective_replay_rtol"])
    replay_atol = float(policy["candidate_objective_replay_atol"])
    for metric, fit_field in (
        ("recomputed_ll", "ll"), ("recomputed_deviance", "deviance"),
    ):
        if not _close(
            candidate_metrics[metric], candidate_fit[fit_field],
            replay_rtol, replay_atol,
        ):
            issues.append(f"candidate_{metric}_replay")
    invariant_rtol = float(policy["objective_invariant_rtol"])
    invariant_atol = float(policy["objective_invariant_atol"])
    candidate_pair = _objective_pair(
        candidate_metrics.get("recomputed_ll"),
        candidate_metrics.get("recomputed_deviance"),
    )
    if candidate_pair is None:
        issues.append("candidate_objective_invalid")
        objective_constant = None
    else:
        objective_constant = candidate_pair[1] + 2.0 * candidate_pair[0]
    objective_pairs: list[tuple[str, tuple[float, float] | None]] = []
    for arm in ("map", "ppmlhdfe", ADJUDICATOR):
        objective_pairs.extend((
            (f"{arm}_reported", _objective_pair(fits[arm].get("ll"), fits[arm].get("deviance"))),
            (f"{arm}_public", _objective_pair(
                metrics[arm].get("recomputed_ll"), metrics[arm].get("recomputed_deviance"),
            )),
            (f"{arm}_projected", _objective_pair(
                metrics[arm].get("projected_ll"), metrics[arm].get("projected_deviance"),
            )),
        ))
    for label, pair in objective_pairs:
        if pair is None:
            issues.append(f"{label}_objective_invalid")
        elif objective_constant is not None and not _close(
            pair[1] + 2.0 * pair[0], objective_constant,
            invariant_rtol, invariant_atol,
        ):
            issues.append(f"{label}_objective_invariant")
    dominance_rtol = float(policy["objective_dominance_rtol"])
    dominance_atol = float(policy["objective_dominance_atol"])
    for arm in ("ppmlhdfe", ADJUDICATOR):
        objective = _reference_objective_diagnostic(
            fits[arm], metrics[arm], policy,
        )
        reference_ll = objective["comparison_ll"]
        reference_deviance = objective["comparison_deviance"]
        if reference_ll is None or reference_deviance is None:
            issues.append(f"{arm}_objective_missing")
            continue
        ll_slack = dominance_atol + dominance_rtol * max(
            abs(candidate_metrics["recomputed_ll"]),
            abs(reference_ll),
        )
        deviance_slack = dominance_atol + dominance_rtol * max(
            abs(candidate_metrics["recomputed_deviance"]),
            abs(reference_deviance),
        )
        if candidate_metrics["recomputed_ll"] + ll_slack < reference_ll:
            issues.append(f"candidate_ll_worse_than_{arm}")
        if (
            candidate_metrics["recomputed_deviance"]
            > reference_deviance + deviance_slack
        ):
            issues.append(f"candidate_deviance_worse_than_{arm}")
    return issues


def analyze_document(document: Mapping[str, object], manifest: dict[str, object] | None = None) -> dict[str, object]:
    manifest = manifest or load_manifest()
    if document.get("schema_version") != 5 or document.get("kind") not in {"feppml_solver_development_run", "feppml_solver_development_merged_run"}:
        raise DevelopmentError("not a v5 solver-development run")
    if document.get("qualification_eligible") is not False or document.get("auto_calibration") is not False:
        raise DevelopmentError("development evidence cannot qualify or calibrate auto")
    manifest_receipt = document.get("manifest")
    if (
        not isinstance(manifest_receipt, dict)
        or set(manifest_receipt) != {"path", "sha256"}
        or not isinstance(manifest_receipt["path"], str)
        or not manifest_receipt["path"]
        or manifest_receipt["sha256"] != sha256_file(MANIFEST)
    ):
        raise DevelopmentError("development evidence manifest differs from the active manifest")
    repetitions = int(document["repetitions"])
    implementations = list(document["sources"])
    if document.get("include_comparators") is not True:
        raise DevelopmentError("solver development analysis requires its pinned comparator")
    scopes = document["scopes"]
    records = document["records"]
    public_state_evidence = document.get("public_state_adjudications")
    if not isinstance(public_state_evidence, list):
        raise DevelopmentError("run must contain a public-state adjudication inventory")
    datasets = document.get("datasets")
    if not isinstance(datasets, list) or len(datasets) != len(scopes):
        raise DevelopmentError("run must contain one dataset receipt per scope")
    scope_by_id = {str(scope["id"]): scope for scope in scopes}
    if len(scope_by_id) != len(scopes):
        raise DevelopmentError("duplicate run scope")
    eligibility_by_scope: dict[str, dict[str, object]] = {}
    metadata_n_by_scope: dict[str, int] = {}
    modelspace_columns_by_scope: dict[str, int] = {}
    dataset_sha_by_scope: dict[str, str] = {}
    dataset_id_by_scope: dict[str, str] = {}
    generated_scope_to_dataset: dict[str, str] = {}
    for dataset in datasets:
        if not isinstance(dataset, dict) or not isinstance(dataset.get("scope"), dict):
            raise DevelopmentError("invalid dataset receipt")
        scope_id = str(dataset["scope"].get("id"))
        if scope_id not in scope_by_id or dataset["scope"] != scope_by_id[scope_id]:
            raise DevelopmentError("dataset receipt scope differs from run scope")
        if scope_id in eligibility_by_scope:
            raise DevelopmentError("duplicate dataset receipt scope")
        generated_scope = str(dataset.get("generated_by_scope_id"))
        dataset_id_value = str(dataset.get("dataset_id"))
        if generated_scope not in scope_by_id or not SAFE_ID.fullmatch(dataset_id_value):
            raise DevelopmentError("dataset generation identity is invalid")
        previous_dataset = generated_scope_to_dataset.setdefault(
            generated_scope, dataset_id_value,
        )
        if previous_dataset != dataset_id_value:
            raise DevelopmentError("one generator scope is bound to multiple datasets")
        if not valid_content_receipt(dataset.get("dta")):
            raise DevelopmentError("dataset DTA receipt is invalid")
        derived = solver_eligibility(
            dataset.get("metadata", {}), str(scope_by_id[scope_id]["topology"]), manifest,
        )
        if int(dataset["metadata"]["N"]) != int(scope_by_id[scope_id]["n"]):
            raise DevelopmentError("dataset observation count differs from run scope")
        if dataset.get("solver_eligibility") != derived:
            raise DevelopmentError("dataset solver eligibility differs from metadata")
        eligibility_by_scope[scope_id] = derived
        metadata_n_by_scope[scope_id] = int(dataset["metadata"]["N"])
        modelspace_columns_by_scope[scope_id] = int(scope_by_id[scope_id]["controls"]) + sum(
            int(dataset["metadata"][f"levels{index}"])
            for index in range(1, 4)
            if int(dataset["metadata"][f"levels{index}"]) > 0
        )
        dataset_sha_by_scope[scope_id] = str(dataset.get("dta", {}).get("sha256", ""))
        dataset_id_by_scope[scope_id] = dataset_id_value
    indexed: dict[tuple[str, str, str, int], Mapping[str, object]] = {}
    adjudicators: dict[str, Mapping[str, object]] = {}
    oracle: dict[str, Mapping[str, object]] = {}
    failures: list[str] = []
    route_failures: list[str] = []
    reference_adjudications: list[dict[str, object]] = []
    used_adjudicators: set[str] = set()
    public_state_by_key: dict[tuple[str, str], Mapping[str, object]] = {}
    used_public_state: set[tuple[str, str]] = set()
    dof_convention_notes: set[str] = set()
    for row in records:
        if row["arm_type"] == "oracle":
            scope_id = str(row.get("scope_id"))
            if (
                not document.get("include_oracle")
                or scope_id not in scope_by_id
                or row.get("implementation") != "independent"
                or row.get("arm") != "explicit_dummy"
                or not _row_matches_scope(row, scope_by_id.get(scope_id, {}))
                or scope_id in oracle
            ):
                failures.append(f"{scope_id}:oracle:unexpected")
            else:
                oracle[scope_id] = row
                if row.get("sample_n") != float(metadata_n_by_scope[scope_id]):
                    failures.append(f"{scope_id}:oracle:sample_differs_from_metadata")
            continue
        if row["arm_type"] == "adjudicator":
            scope_id = str(row["scope_id"])
            if (
                scope_id not in scope_by_id
                or row["implementation"] != "adjudicator"
                or row["arm"] != ADJUDICATOR
                or not _row_matches_scope(row, scope_by_id.get(scope_id, {}))
                or int(row["repetition"]) != 1
                or row.get("timed") is not False
                or row.get("seconds") is not None
                or row.get("reference_acceleration") != reference_acceleration(scope_by_id.get(scope_id, {}))
                or row.get("reference_transform") != REFERENCE_TRANSFORM
                or row.get("solver_matrix_values") != []
                or scope_id in adjudicators
            ):
                failures.append(f"{scope_id}:adjudicator:{ADJUDICATOR}:unexpected")
            else:
                adjudicators[scope_id] = row
                if int(row["rc"]) == 0 and row.get("sample_n") != float(metadata_n_by_scope[scope_id]):
                    failures.append(
                        f"{scope_id}:adjudicator:{ADJUDICATOR}:sample_differs_from_metadata"
                    )
            continue
        key = (str(row["scope_id"]), str(row["implementation"]), str(row["arm"]), int(row["repetition"]))
        solver_pair = key[1] in implementations and key[2] in SOLVERS
        comparator_pair = key[1] == "reference" and key[2] in COMPARATORS
        if (
            key[0] not in scope_by_id
            or not (solver_pair or comparator_pair)
            or not _row_matches_scope(row, scope_by_id.get(key[0], {}))
            or row["arm_type"] != ("solver" if solver_pair else "comparator")
            or not 1 <= key[3] <= repetitions
        ):
            failures.append(":".join(map(str, key)) + ":unexpected")
            continue
        if (
            key[2] in SOLVERS
            and key[2] not in eligibility_by_scope.get(key[0], {}).get("eligible_solvers", [])
        ):
            failures.append(":".join(map(str, key)) + ":ineligible_solver")
            continue
        if int(row["rc"]) == 0 and row.get("sample_n") != float(metadata_n_by_scope[key[0]]):
            failures.append(":".join(map(str, key)) + ":sample_differs_from_metadata")
        solver_matrix = row.get("solver_matrix_values")
        if solver_pair:
            if int(row["rc"]) == 0 and not _valid_solver_matrix(solver_matrix):
                failures.append(":".join(map(str, key)) + ":solver_matrix")
            elif int(row["rc"]) != 0 and solver_matrix != []:
                failures.append(":".join(map(str, key)) + ":failed_solver_matrix")
        elif solver_matrix != []:
            failures.append(":".join(map(str, key)) + ":unexpected_solver_matrix")
        if row.get("timed") is not True:
            failures.append(":".join(map(str, key)) + ":untimed")
        if key[2] in COMPARATORS:
            if (
                row.get("reference_acceleration") != reference_acceleration(scope_by_id[key[0]])
                or row.get("reference_transform") != REFERENCE_TRANSFORM
            ):
                failures.append(":".join(map(str, key)) + ":reference_route")
        elif row.get("reference_acceleration") or row.get("reference_transform"):
            failures.append(":".join(map(str, key)) + ":unexpected_reference_route")
        if key in indexed:
            failures.append(":".join(map(str, key)) + ":duplicate")
        indexed[key] = row
    metric_fields = set(PUBLIC_STATE_METRICS)
    for evidence in public_state_evidence:
        scope_id = str(evidence.get("scope_id"))
        implementation = str(evidence.get("implementation"))
        key = (scope_id, implementation)
        fits = evidence.get("fits")
        states = evidence.get("states")
        metrics = evidence.get("metrics")
        if (
            scope_id not in scope_by_id
            or implementation not in implementations
            or evidence.get("method") != KKT_ADJUDICATOR
            or not _row_matches_scope(evidence, scope_by_id.get(scope_id, {}))
            or key in public_state_by_key
            or not isinstance(fits, dict)
            or set(fits) != {"map", "ppmlhdfe", ADJUDICATOR}
            or not isinstance(states, dict)
            or set(states) != {"map", "ppmlhdfe", ADJUDICATOR}
            or not all(valid_file_receipt(receipt) for receipt in states.values())
            or not valid_file_receipt(evidence.get("audit_result"))
            or not isinstance(metrics, dict)
            or set(metrics) != {"map", "ppmlhdfe", ADJUDICATOR}
            or any(
                not isinstance(row, dict)
                or set(row) != metric_fields
                or any(
                    isinstance(value, bool) or not isinstance(value, (int, float))
                    or not math.isfinite(float(value))
                    for value in row.values()
                )
                for row in metrics.values()
            )
        ):
            failures.append(f"{scope_id}:{implementation}:{KKT_ADJUDICATOR}:unexpected")
            continue
        expected_fit_identity = {
            "map": implementation,
            "ppmlhdfe": "reference",
            ADJUDICATOR: "reference",
        }
        if any(
            not isinstance(fit, dict)
            or fit.get("scope_id") != scope_id
            or fit.get("implementation") != expected_fit_identity[arm]
            or fit.get("arm") != arm
            or not _row_matches_scope(fit, scope_by_id[scope_id])
            for arm, fit in fits.items()
        ):
            failures.append(f"{scope_id}:{implementation}:{KKT_ADJUDICATOR}:fit_identity")
            continue
        public_state_by_key[key] = evidence

    expected_runs = {
        (scope_id, implementation, solver)
        for scope_id, eligibility in eligibility_by_scope.items()
        for implementation in implementations
        for solver in eligibility["eligible_solvers"]
    }
    expected_runs.update(
        (scope_id, "reference", comparator)
        for scope_id in scope_by_id for comparator in COMPARATORS
    )
    observed_runs: set[tuple[str, str, str]] = set()
    observed_generators: set[str] = set()
    observed_adjudicators: set[str] = set()
    observed_public_fits: dict[tuple[str, str, str], Mapping[str, object]] = {}
    observed_public_audits: dict[tuple[str, str], Mapping[str, object]] = {}
    prepare_reference_processes = 0
    processes = document.get("processes", [])
    if not isinstance(processes, list):
        raise DevelopmentError("process inventory must be a list")
    for process in processes:
        stage = str(process.get("stage", ""))
        process_scope = str(process.get("scope_id"))
        process_arm = str(process.get("arm", ""))
        process_implementation = str(process.get("implementation", ""))
        if stage == "run":
            solver_pair = process_implementation in implementations and process_arm in SOLVERS
            comparator_pair = process_implementation == "reference" and process_arm in COMPARATORS
            if (
                process_scope not in scope_by_id
                or not (solver_pair or comparator_pair)
                or process.get("reference_route") != (
                    {
                        "acceleration": reference_acceleration(scope_by_id[process_scope]),
                        "transform": REFERENCE_TRANSFORM,
                    }
                    if comparator_pair else None
                )
                or (
                    solver_pair
                    and process_arm not in eligibility_by_scope[process_scope]["eligible_solvers"]
                )
            ):
                failures.append(
                    f"{process_scope}:{process_implementation}:{process_arm}:unexpected_process"
                )
            else:
                process_key = (process_scope, process_implementation, process_arm)
                if process_key in observed_runs:
                    failures.append(":".join(process_key) + ":duplicate_process")
                observed_runs.add(process_key)
        elif stage == "adjudicate":
            if (
                process_scope not in scope_by_id
                or process_implementation != "adjudicator"
                or process_arm != ADJUDICATOR
                or process.get("reference_route") != {
                    "acceleration": reference_acceleration(scope_by_id[process_scope]),
                    "transform": REFERENCE_TRANSFORM,
                }
            ):
                failures.append(f"{process_scope}:adjudicator:{process_arm}:unexpected_process")
            elif process_scope in observed_adjudicators:
                failures.append(f"{process_scope}:adjudicator:{process_arm}:duplicate_process")
            else:
                observed_adjudicators.add(process_scope)
        elif stage == "public-state-fit":
            fit_key = (process_scope, process_implementation, process_arm)
            valid_pair = (
                process_implementation in implementations and process_arm == "map"
            ) or (
                process_implementation == "reference"
                and process_arm in {"ppmlhdfe", ADJUDICATOR}
            )
            expected_source = canonical_sha256(
                document["references"]
                if process_implementation == "reference"
                else document["sources"].get(process_implementation, {})
            )
            expected_artifact = (
                None if process_implementation == "reference"
                else document["artifacts"].get(process_implementation, {}).get("sha256")
            )
            expected_reference_route = (
                {
                    "acceleration": reference_acceleration(scope_by_id.get(process_scope, {})),
                    "transform": REFERENCE_TRANSFORM,
                }
                if process_implementation == "reference" else None
            )
            if (
                process_scope not in scope_by_id
                or not valid_pair
                or process.get("dataset_id") != dataset_id_by_scope.get(process_scope)
                or process.get("dataset_sha256") != dataset_sha_by_scope.get(process_scope)
                or process.get("source_sha256") != expected_source
                or process.get("artifact_sha256") != expected_artifact
                or process.get("reference_route") != expected_reference_route
                or not valid_file_receipt(process.get("state"))
                or not valid_file_receipt(process.get("result"))
                or not isinstance(process.get("parsed_result_sha256"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", process["parsed_result_sha256"])
                or fit_key in observed_public_fits
            ):
                failures.append(":".join(fit_key) + ":unexpected_process")
            else:
                observed_public_fits[fit_key] = process
        elif stage == "public-state-audit":
            audit_key = (process_scope, process_implementation)
            inputs = process.get("inputs")
            if (
                process_scope not in scope_by_id
                or process_implementation not in implementations
                or process_arm != KKT_ADJUDICATOR
                or process.get("dataset_id") != dataset_id_by_scope.get(process_scope)
                or process.get("dataset_sha256") != dataset_sha_by_scope.get(process_scope)
                or process.get("source_sha256") != canonical_sha256(
                    document["sources"].get(process_implementation, {})
                )
                or process.get("artifact_sha256")
                != document["artifacts"].get(process_implementation, {}).get("sha256")
                or not isinstance(inputs, dict)
                or set(inputs) != {"map", "ppmlhdfe", ADJUDICATOR}
                or not all(valid_file_receipt(receipt) for receipt in inputs.values())
                or not valid_file_receipt(process.get("result"))
                or not isinstance(process.get("parsed_result_sha256"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", process["parsed_result_sha256"])
                or audit_key in observed_public_audits
            ):
                failures.append(":".join(audit_key) + ":unexpected_kkt_process")
            else:
                observed_public_audits[audit_key] = process
        elif stage == "generate":
            if (
                process_scope not in generated_scope_to_dataset
                or str(process.get("dataset_id")) != generated_scope_to_dataset.get(process_scope)
                or process_arm or process_implementation
            ):
                failures.append(f"{process_scope}:generate:unexpected_process")
            elif process_scope in observed_generators:
                failures.append(f"{process_scope}:generate:duplicate_process")
            else:
                observed_generators.add(process_scope)
        elif stage == "prepare-reference":
            if process_arm or process_implementation or process.get("scope_id") is not None:
                failures.append("prepare-reference:unexpected_process")
            prepare_reference_processes += 1
        else:
            failures.append(f"{process_scope}:{stage or 'missing-stage'}:unexpected_process")
        if int(process.get("accepted_rc", 0)):
            failures.append(
                f"{process.get('scope_id')}:{process.get('implementation', process.get('stage'))}:"
                f"{process.get('arm', '')}:process"
            )
    for scope_id, implementation, arm in sorted(expected_runs - observed_runs):
        failures.append(f"{scope_id}:{implementation}:{arm}:missing_process")
    for scope_id in sorted(set(generated_scope_to_dataset) - observed_generators):
        failures.append(f"{scope_id}:generate:missing_process")
    for scope_id in sorted(set(adjudicators) - observed_adjudicators):
        failures.append(f"{scope_id}:adjudicator:{ADJUDICATOR}:missing_process")
    for scope_id in sorted(observed_adjudicators - set(adjudicators)):
        failures.append(f"{scope_id}:adjudicator:{ADJUDICATOR}:missing_record")
    expected_public_audits = set(public_state_by_key)
    expected_public_fits = {
        (scope_id, implementation, "map")
        for scope_id, implementation in expected_public_audits
    }
    expected_public_fits.update(
        (scope_id, "reference", arm)
        for scope_id in {key[0] for key in expected_public_audits}
        for arm in ("ppmlhdfe", ADJUDICATOR)
    )
    for key in sorted(expected_public_fits - set(observed_public_fits)):
        failures.append(":".join(key) + ":missing_public_state_fit_process")
    for key in sorted(set(observed_public_fits) - expected_public_fits):
        failures.append(":".join(key) + ":unneeded_public_state_fit_process")
    for key in sorted(expected_public_audits - set(observed_public_audits)):
        failures.append(":".join(key) + ":missing_public_state_audit_process")
    for key in sorted(set(observed_public_audits) - expected_public_audits):
        failures.append(":".join(key) + ":unneeded_public_state_audit_process")
    for key, evidence in public_state_by_key.items():
        audit = observed_public_audits.get(key)
        if audit is None:
            continue
        scope_id, implementation = key
        fit_processes = {
            "map": observed_public_fits.get((scope_id, implementation, "map")),
            "ppmlhdfe": observed_public_fits.get((scope_id, "reference", "ppmlhdfe")),
            ADJUDICATOR: observed_public_fits.get((scope_id, "reference", ADJUDICATOR)),
        }
        if (
            audit.get("inputs") != evidence["states"]
            or audit.get("result") != evidence["audit_result"]
            or audit.get("parsed_result_sha256")
            != parsed_file_binding(evidence["audit_result"], evidence["metrics"])
            or any(
                process is None
                or process.get("state") != evidence["states"][arm]
                or process.get("parsed_result_sha256")
                != parsed_file_binding(process["result"], evidence["fits"][arm])
                for arm, process in fit_processes.items()
            )
        ):
            failures.append(f"{scope_id}:{implementation}:{KKT_ADJUDICATOR}:process_binding")
    if prepare_reference_processes > 1:
        failures.append("prepare-reference:duplicate_process")
    science = manifest["science"]
    kkt_policy = manifest["scientific_adjudication"]["public_state_kkt"]
    public_state_objective_diagnostics = [
        {
            "scope_id": scope_id,
            "implementation": implementation,
            "arm": arm,
            **_reference_objective_diagnostic(
                evidence["fits"][arm], evidence["metrics"][arm], kkt_policy,
            ),
        }
        for (scope_id, implementation), evidence in sorted(public_state_by_key.items())
        for arm in ("ppmlhdfe", ADJUDICATOR)
    ]
    summaries: list[dict[str, object]] = []
    solver_exclusions: list[dict[str, object]] = []
    for scope in scopes:
        scope_id = str(scope["id"])
        controls = int(scope["controls"])
        eligibility = eligibility_by_scope[scope_id]
        eligible_solvers = list(eligibility["eligible_solvers"])
        solver_exclusions.extend({
            "scope_id": scope_id, "solver": solver, "reason": reason,
            "effect_count": eligibility["effect_count"],
            "cmg_nuisance_columns": eligibility["cmg_nuisance_columns"],
        } for solver, reason in eligibility["excluded_solvers"].items())
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
                for solver in eligible_solvers:
                    prefix = f"{scope_id}:{implementation}:{solver}:r{repetition}"
                    row = indexed.get((scope_id, implementation, solver, repetition))
                    if row is None:
                        failures.append(prefix + ":missing")
                        continue
                    if int(row["rc"]) != 0 or row["converged"] != 1.0:
                        failures.append(prefix + ":execution")
                    if row["route_code"] != float(ROUTE_CODES[solver]):
                        route_failures.append(prefix + ":route")
                    expected_engine = {
                        "map": 1,
                        "lsmr": 2 if controls == 0 else 3,
                        "cmg": 4,
                    }[solver]
                    if row["selected_engine_code"] != float(expected_engine):
                        route_failures.append(prefix + ":selected_engine")
                    if row["effective_engine_code"] != float(expected_engine):
                        route_failures.append(prefix + ":effective_engine")
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
                    if baseline is None:
                        failures.append(prefix + ":missing_map")
                    else:
                        failures.extend(
                            prefix + ":" + issue
                            for issue in _science_issues(
                                row, baseline, controls, science, "map", dof_convention_notes,
                            )
                        )
                    if default_reference is None:
                        failures.append(prefix + ":missing_ppmlhdfe")
                        continue
                    default_issues = _science_issues(
                        row, default_reference, controls, science, "ppmlhdfe",
                        dof_convention_notes,
                    )
                    if not default_issues:
                        continue
                    strict_reference = adjudicators.get(scope_id)
                    if strict_reference is None:
                        failures.extend(prefix + ":" + issue for issue in default_issues)
                        continue
                    used_adjudicators.add(scope_id)
                    if int(strict_reference["rc"]) != 0 or strict_reference["converged"] != 1.0:
                        failures.append(prefix + ":strict_ppmlhdfe_execution")
                        failures.extend(prefix + ":" + issue for issue in default_issues)
                        continue
                    strict_issues = _science_issues(
                        row, strict_reference, controls, science, "strict_ppmlhdfe",
                        dof_convention_notes,
                    )
                    if strict_issues:
                        public_state_key = (scope_id, implementation)
                        evidence = public_state_by_key.get(public_state_key)
                        if (
                            default_issues == ["ppmlhdfe_coefficients"]
                            and strict_issues == ["strict_ppmlhdfe_coefficients"]
                            and evidence is not None
                        ):
                            kkt_issues = _public_state_kkt_issues(
                                evidence, indexed, strict_reference, scope,
                                implementation, repetitions,
                                modelspace_columns_by_scope[scope_id],
                                science, kkt_policy,
                            )
                            if not kkt_issues:
                                used_public_state.add(public_state_key)
                                reference_adjudications.append({
                                    "scope_id": scope_id,
                                    "implementation": implementation,
                                    "solver": solver,
                                    "repetition": repetition,
                                    "default_reference": "ppmlhdfe",
                                    "default_science_mismatches": default_issues,
                                    "strict_reference": ADJUDICATOR,
                                    "strict_science_mismatches": strict_issues,
                                    "adjudicator": KKT_ADJUDICATOR,
                                    "timed": False,
                                    "score_tolerance": float(kkt_policy["score_tolerance"]),
                                })
                                continue
                            failures.extend(
                                prefix + ":public_state_kkt_" + issue
                                for issue in kkt_issues
                            )
                        failures.extend(prefix + ":" + issue for issue in default_issues)
                        failures.extend(prefix + ":" + issue for issue in strict_issues)
                        continue
                    reference_adjudications.append({
                        "scope_id": scope_id, "implementation": implementation,
                        "solver": solver, "repetition": repetition,
                        "default_reference": "ppmlhdfe",
                        "default_science_mismatches": default_issues,
                        "adjudicator": ADJUDICATOR, "timed": False,
                        "tolerance": 1e-10, "itolerance": 1e-12,
                        "acceleration": reference_acceleration(scope),
                        "transform": REFERENCE_TRANSFORM,
                    })
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
        for solver in eligible_solvers:
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
                baseline_values = [float(indexed[(scope_id, "baseline", solver, repetition)]["seconds"]) for repetition in range(1, repetitions + 1) if (scope_id, "baseline", solver, repetition) in indexed]
                selected_engines = sorted({
                    int(indexed[(scope_id, "candidate", solver, repetition)]["selected_engine_code"])
                    for repetition in range(1, repetitions + 1)
                    if (scope_id, "candidate", solver, repetition) in indexed
                    and indexed[(scope_id, "candidate", solver, repetition)]["selected_engine_code"] is not None
                })
                effective_engines = sorted({
                    int(indexed[(scope_id, "candidate", solver, repetition)]["effective_engine_code"])
                    for repetition in range(1, repetitions + 1)
                    if (scope_id, "candidate", solver, repetition) in indexed
                    and indexed[(scope_id, "candidate", solver, repetition)]["effective_engine_code"] is not None
                })
                prefix = f"{scope_id}:candidate:{solver}:"
                eligible = (
                    len(values) == repetitions
                    and all(value > 0.0 and math.isfinite(value) for value in values)
                    and not any(item.startswith(prefix) for item in (*failures, *route_failures))
                )
                reference_median = statistics.median(reference_values) if reference_values else None
                baseline_median = statistics.median(baseline_values) if baseline_values else None
                summaries.append({
                    "scope_id": scope_id, "solver": solver, "performance_eligible": eligible,
                    "selected_engine_codes": selected_engines,
                    "effective_engine_codes": effective_engines,
                    "candidate_median_seconds": statistics.median(values), "candidate_min_seconds": min(values), "candidate_max_seconds": max(values),
                    "candidate_median_inner_iterations": statistics.median(inner_iterations) if inner_iterations else None,
                    "ppmlhdfe_median_seconds": reference_median,
                    "candidate_over_ppmlhdfe": statistics.median(values) / reference_median if reference_median and reference_median > 0.0 else None,
                    "baseline_median_seconds": baseline_median,
                    "candidate_over_baseline": statistics.median(values) / baseline_median if baseline_median and baseline_median > 0.0 else None,
                    "engine_medians": {
                        implementation: solver_engine_summary([
                            indexed[(scope_id, implementation, solver, repetition)]
                            for repetition in range(1, repetitions + 1)
                            if (scope_id, implementation, solver, repetition) in indexed
                        ])
                        for implementation in implementations
                    },
                })
    for scope_id in sorted(set(adjudicators) - used_adjudicators):
        failures.append(f"{scope_id}:adjudicator:{ADJUDICATOR}:not_required")
    for scope_id, implementation in sorted(set(public_state_by_key) - used_public_state):
        failures.append(
            f"{scope_id}:{implementation}:{KKT_ADJUDICATOR}:not_required_or_failed"
        )
    return {
        "schema_version": 5, "kind": "feppml_solver_development_analysis", "qualification_eligible": False,
        "auto_calibration": False, "science_pass": not failures, "route_pass": not route_failures,
        "science_failures": sorted(set(failures)), "route_failures": sorted(set(route_failures)),
        "reference_adjudications": reference_adjudications,
        "public_state_objective_diagnostics": public_state_objective_diagnostics,
        "dof_convention_notes": sorted(dof_convention_notes),
        "solver_summary": summaries, "solver_exclusions": solver_exclusions,
        "scopes": len(scopes), "repetitions": repetitions,
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
        "schema_version": 5, "kind": "feppml_solver_development_config", "manifest_sha256": manifest_sha,
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
        "schema_version": 5, "kind": "feppml_solver_development_setup", "job_id": str(args.job_id),
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
        "schema_version": 5, "kind": "feppml_solver_development_task", "task_identity": identity,
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
        "schema_version": 5, "kind": "feppml_solver_development_merged_run", "qualification_eligible": False,
        "auto_calibration": False, "manifest": documents[0]["manifest"], "implementation": documents[0]["implementation"],
        "repetitions": int(config["repetitions"]), "include_comparators": True, "include_oracle": False,
        "sources": documents[0]["sources"], "artifacts": documents[0]["artifacts"], "references": documents[0]["references"],
        "task_receipts": [str(run_root / f"receipts/task-{task_id:02d}.json") for task_id in range(1, 17)],
        "scopes": scopes, "datasets": [row for document in documents for row in document["datasets"]],
        "records": [row for document in documents for row in document["records"]],
        "public_state_adjudications": [
            row for document in documents
            for row in document["public_state_adjudications"]
        ],
        "processes": [row for document in documents for row in document["processes"]],
    }
    run_path = run_root / "output/merged-run.json"
    exclusive_json(run_path, merged)
    analysis = analyze_document(merged, manifest)
    exclusive_json(run_root / "output/analysis.json", analysis)
    if not analysis["science_pass"] or not analysis["route_pass"]:
        raise DevelopmentError("merged campaign failed science or route checks")
    exclusive_json(run_root / "receipts/merge.json", {
        "schema_version": 5, "kind": "feppml_solver_development_merge", "task_count": 16, "scope_count": 56,
        "job_id": collector_job, "host": cpu_identity(),
        "merged_run_sha256": sha256_file(run_path), "analysis_sha256": sha256_file(run_root / "output/analysis.json"),
    })


def record_submission(args: argparse.Namespace) -> None:
    run_root = validate_run_root(args.run_root)
    values = {"setup": str(args.setup_job), "array": str(args.array_job), "collector": str(args.collector_job)}
    if any(not JOB_ID.fullmatch(value) for value in values.values()):
        raise DevelopmentError("submission job IDs must be positive integers")
    exclusive_json(run_root / "receipts/submission.json", {"schema_version": 5, "kind": "feppml_solver_development_submission", "jobs": values})


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
        "schema_version": 5, "kind": "feppml_solver_development_close", "jobs": submission,
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
    scopes = profile_scopes(manifest, args.profile)
    if args.scope:
        requested = set(args.scope)
        if requested - {str(scope["id"]) for scope in scopes}:
            raise DevelopmentError("requested scope is not registered in the selected profile")
        scopes = [scope for scope in scopes if scope["id"] in requested]
    document = execute_scopes(
        manifest, scopes, stata=args.stata,
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
    cells.add_argument("--profile", choices=(*LOCAL_PROFILES, "scc_screen"), required=True)
    local = commands.add_parser("run-local")
    local.add_argument("--profile", choices=LOCAL_PROFILES, required=True)
    local.add_argument("--scope", action="append", help="Run only these registered scope IDs")
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
