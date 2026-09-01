#!/usr/bin/env python3
"""Licensed-Stata capture boundary for the frozen feppml qualification.

This program is intentionally a transport and provenance boundary, not a
scientific-result generator.  Cell-specific Stata programs must write the
closed candidate/comparator records consumed by the frozen evaluator.  The
runner verifies their immutable inputs, launches Stata in a new process,
observes RSS externally, and refuses incomplete markers or telemetry.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
from typing import Any, Mapping, Sequence


def _sealed_sibling(name: str, filename: str):
    """Load one exact sibling module without consulting PYTHONPATH/packages."""

    path = Path(__file__).resolve().parent / filename
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load sealed capture dependency: {path}")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(name)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        if previous is None:
            sys.modules.pop(name, None)
        else:
            sys.modules[name] = previous
        raise
    return module


raw_normalizer = _sealed_sibling(
    "feppml_sealed_raw_normalizer", "normalize_feppml_stata_raw.py"
)
stata_protocol = _sealed_sibling(
    "feppml_sealed_stata_protocol", "parse_feppml_stata_protocol.py"
)


PLAN_TYPE = "feppml-qualification-capture-plan-v1"
SCIENCE_TYPE = "feppml-science-preflight-v1"
ARM_TYPE = "feppml-qualification-arm-v1"
STATA_BASENAMES = {"stata", "stata-mp", "stata-se"}
OUTPUT_KINDS = {"fit", "prediction", "total_absorbed_effect", "individual_fe"}
QUALIFICATION_ARM_ORDER = (
    ("candidate", "comparator"),  # cold
    ("candidate", "comparator"),  # warm AB
    ("comparator", "candidate"),  # warm BA
    ("candidate", "comparator"),
    ("comparator", "candidate"),
    ("candidate", "comparator"),
    ("comparator", "candidate"),
    ("candidate", "comparator"),
    ("comparator", "candidate"),
    ("candidate", "comparator"),
)
SHA256_RE = __import__("re").compile(r"^[0-9a-f]{64}$")
COMMIT_RE = __import__("re").compile(r"^[0-9a-f]{40}$")
PREFLIGHT_RECEIPT_FIELDS = {
    "schema_version", "receipt_type", "requested", "decision",
    "selected_route", "fast_peak_bytes", "low_peak_bytes",
    "available_bytes", "embedding_resident_bytes", "safety_headroom_bytes",
    "usable_bytes", "planner_allocator_event_growth",
    "planner_allocator_current_growth_bytes",
    "planner_allocator_peak_growth_bytes", "allocation_bytes",
    "workspace_allocation_started", "embedded_git_commit",
    "complete_sentinel",
}
IDENTITY_FIELDS = {
    "source_commit", "source_tree", "cargo_lock_sha256",
    "references_lock_sha256", "manifest_sha256", "generator_sha256",
    "reference_commits",
}


class CaptureFailure(RuntimeError):
    """An immutable input or licensed capture failed closed."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise CaptureFailure(f"{where} must be an object")
    return value


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise CaptureFailure(
            f"{where} is not closed; missing={sorted(fields-set(value))}, "
            f"extra={sorted(set(value)-fields)}"
        )


def _read_json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CaptureFailure(f"cannot read {where}: {error}") from error
    return _mapping(value, where)


def _write_new(path: Path, value: Mapping[str, Any]) -> None:
    data = json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False)
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise CaptureFailure(f"refusing to overwrite output: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(data + "\n")
        target.flush()
        os.fsync(target.fileno())


def _preflight_receipt(
    path: Path,
    *,
    requested: str,
    memory_limit_bytes: int,
    source_commit: str,
) -> dict[str, Any]:
    record = dict(_read_json(path, "core PPML memory-preflight receipt"))
    _closed(record, PREFLIGHT_RECEIPT_FIELDS, "core PPML memory-preflight receipt")
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != "feppml-memory-preflight-v1"
        or record["requested"] != requested
        or record["decision"] not in {"selected", "rejected"}
        or record["selected_route"] not in {None, "fast", "low"}
        or record["available_bytes"] != memory_limit_bytes
        or record["workspace_allocation_started"] is not False
        or record["embedded_git_commit"] != source_commit[:12]
        or record["complete_sentinel"] != "FEPPML_MEMORY_PREFLIGHT_COMPLETE"
    ):
        raise CaptureFailure("core PPML memory-preflight receipt identity is invalid")
    unsigned_fields = {
        "fast_peak_bytes", "low_peak_bytes", "available_bytes",
        "embedding_resident_bytes", "safety_headroom_bytes", "usable_bytes",
        "planner_allocator_event_growth", "planner_allocator_peak_growth_bytes",
        "allocation_bytes",
    }
    for field in unsigned_fields:
        if (
            isinstance(record[field], bool)
            or not isinstance(record[field], int)
            or record[field] < 0
        ):
            raise CaptureFailure(f"core PPML memory-preflight {field} is invalid")
    current_growth = record["planner_allocator_current_growth_bytes"]
    if isinstance(current_growth, bool) or not isinstance(current_growth, int):
        raise CaptureFailure("core PPML memory-preflight current growth is invalid")
    expected_headroom = max(512 * 1024 * 1024, memory_limit_bytes // 5)
    if (
        record["safety_headroom_bytes"] != expected_headroom
        or record["usable_bytes"] != memory_limit_bytes - expected_headroom
        or record["fast_peak_bytes"] <= 0
        or record["low_peak_bytes"] <= 0
        or record["allocation_bytes"]
            != max(abs(current_growth), record["planner_allocator_peak_growth_bytes"])
        or (record["decision"] == "selected")
            != (record["selected_route"] is not None)
    ):
        raise CaptureFailure("core PPML memory-preflight decision arithmetic is invalid")
    return record


def _immutable(
    value: Any,
    where: str,
    *,
    path_resolver: Any | None = None,
) -> dict[str, Any]:
    record = dict(_mapping(value, where))
    _closed(record, {"path", "sha256"}, where)
    source = Path(record["path"])
    path = source.resolve() if path_resolver is None else Path(path_resolver(source))
    if (
        path.is_symlink()
        or not path.is_file()
        or sha256_file(path) != record["sha256"]
    ):
        raise CaptureFailure(f"{where} bytes are absent or changed")
    # Portable replay validates relocated bytes but preserves the recorded SCC
    # path so supervisor argv can be reconstructed byte-for-byte.
    return {
        "path": source if path_resolver is not None else path,
        "sha256": record["sha256"],
    }


def _status_map(value: Any, where: str) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not value:
        raise CaptureFailure(f"{where} must be a nonempty ordered list")
    result = []
    seen = set()
    for index, raw in enumerate(value):
        record = dict(_mapping(raw, f"{where}[{index}]"))
        _closed(record, {"name", "status_code"}, f"{where}[{index}]")
        if (
            not isinstance(record["name"], str)
            or not record["name"]
            or record["name"] in seen
            or isinstance(record["status_code"], bool)
            or not isinstance(record["status_code"], int)
            or record["status_code"] not in range(1, 8)
        ):
            raise CaptureFailure(f"{where}[{index}] is invalid")
        seen.add(record["name"])
        result.append(record)
    return result


def _closure(value: Any) -> dict[str, Any]:
    record = dict(_mapping(value, "closure"))
    fields = {
        "schema_version", "closure_type", "materialization_request_sha256",
        "recipe_manifest_sha256", "recipe_inventory_sha256",
        "dataset_inventory_sha256", "oracle_fixture_inventory_sha256",
        "candidate_source_tree", "candidate_plugin_sha256",
        "candidate_bundle_sha256", "comparator_source_inventory_sha256",
        "comparator_bundle_sha256", "tool_inventory_sha256",
        "expected_inventory_sha256", "stata_executable_sha256",
        "complete_sentinel",
    }
    _closed(record, fields, "closure")
    if (
        record["schema_version"] != 1
        or record["closure_type"] != "feppml-capture-plan-closure-v1"
        or record["complete_sentinel"] != "FEPPML_CAPTURE_PLAN_CLOSURE_COMPLETE"
        or COMMIT_RE.fullmatch(str(record["candidate_source_tree"])) is None
    ):
        raise CaptureFailure("capture plan closure identity is invalid")
    for field in fields - {"schema_version", "closure_type", "candidate_source_tree", "complete_sentinel"}:
        if SHA256_RE.fullmatch(str(record[field])) is None:
            raise CaptureFailure(f"closure.{field} is not sha256")
    return record


def load_plan(
    path: Path,
    *,
    path_resolver: Any | None = None,
    verify_stata_executable: bool = True,
    oracle_interpreter: str | None = None,
    logical_path_resolver: Any | None = None,
) -> dict[str, Any]:
    plan = dict(_read_json(path, "capture plan"))
    _closed(
        plan,
        {
            "schema_version", "plan_type", "qualification_manifest_sha256",
            "development_manifest", "stata", "identity", "cells",
            "constrained_memory", "closure", "complete_sentinel",
        },
        "capture plan",
    )
    if (
        plan["schema_version"] != 1
        or plan["plan_type"] != PLAN_TYPE
        or plan["complete_sentinel"] != "FEPPML_CAPTURE_PLAN_COMPLETE"
        or SHA256_RE.fullmatch(str(plan["qualification_manifest_sha256"])) is None
    ):
        raise CaptureFailure("capture plan identity/sentinel is invalid")
    immutable = lambda value, where: _immutable(  # noqa: E731
        value, where, path_resolver=path_resolver
    )
    plan["development_manifest"] = immutable(
        plan["development_manifest"], "development_manifest"
    )
    plan["closure"] = _closure(plan["closure"])
    identity = dict(_mapping(plan["identity"], "identity"))
    _closed(identity, IDENTITY_FIELDS, "identity")
    for field in ("source_commit", "source_tree"):
        if COMMIT_RE.fullmatch(str(identity[field])) is None:
            raise CaptureFailure(f"identity.{field} is not a full git object ID")
    for field in (
        "cargo_lock_sha256", "references_lock_sha256", "manifest_sha256",
        "generator_sha256",
    ):
        if SHA256_RE.fullmatch(str(identity[field])) is None:
            raise CaptureFailure(f"identity.{field} is not sha256")
    reference_commits = _mapping(
        identity["reference_commits"], "identity.reference_commits"
    )
    if not all(
        isinstance(name, str)
        and name
        and COMMIT_RE.fullmatch(str(commit)) is not None
        for name, commit in reference_commits.items()
    ):
        raise CaptureFailure("identity.reference_commits is invalid")
    if (
        identity["manifest_sha256"] != plan["development_manifest"]["sha256"]
        or identity["source_tree"] != plan["closure"]["candidate_source_tree"]
    ):
        raise CaptureFailure("capture identity is not bound to its immutable inputs")
    plan["identity"] = identity
    stata = dict(_mapping(plan["stata"], "stata"))
    _closed(stata, {"executable", "sha256", "batch_arguments", "comparator_processor_limit"}, "stata")
    executable = Path(stata["executable"])
    if executable.name.lower() not in STATA_BASENAMES:
        raise CaptureFailure("Stata executable basename is not a licensed Stata command")
    if verify_stata_executable and (
        not executable.resolve().is_file()
        or sha256_file(executable.resolve()) != stata["sha256"]
    ):
        raise CaptureFailure("licensed Stata executable bytes are absent or changed")
    if stata["batch_arguments"] != ["-q", "-b", "do"]:
        raise CaptureFailure("stata.batch_arguments is not the exact sealed batch argv")
    if (
        stata["comparator_processor_limit"] != 16
    ):
        raise CaptureFailure(
            "stata.comparator_processor_limit must be the frozen 16-thread ceiling"
        )
    if stata["sha256"] != plan["closure"]["stata_executable_sha256"]:
        raise CaptureFailure("Stata executable differs from the compact plan closure")
    plan["stata"] = {
        **stata,
        "executable": executable if not verify_stata_executable else executable.resolve(),
    }
    cells = plan["cells"]
    if not isinstance(cells, list) or not cells:
        raise CaptureFailure("capture plan cells must be a nonempty list")
    loaded = []
    seen = set()
    for index, raw in enumerate(cells):
        cell = dict(_mapping(raw, f"cells[{index}]"))
        _closed(
            cell,
            {"cell_id", "requested_threads", "data", "candidate", "comparator", "oracle_fixture"},
            f"cells[{index}]",
        )
        if not isinstance(cell["cell_id"], str) or not cell["cell_id"] or cell["cell_id"] in seen:
            raise CaptureFailure("cell IDs must be nonempty and unique")
        seen.add(cell["cell_id"])
        if isinstance(cell["requested_threads"], bool) or not isinstance(cell["requested_threads"], int) or cell["requested_threads"] < 1:
            raise CaptureFailure("cell requested_threads must be a positive integer")
        cell["data"] = immutable(cell["data"], f"{cell['cell_id']}.data")
        for arm in ("candidate", "comparator"):
            recipe = dict(_mapping(cell[arm], f"{cell['cell_id']}.{arm}"))
            _closed(recipe, {"science_do_file", "timing_do_file", "artifact", "science_output", "expected_status_map", "expected_output", "probe_rows", "expected_cluster_dimensions", "expected_saved_channels", "expected_source_provenance"}, f"{cell['cell_id']}.{arm}")
            recipe["science_do_file"] = immutable(recipe["science_do_file"], f"{cell['cell_id']}.{arm}.science_do_file")
            recipe["timing_do_file"] = immutable(recipe["timing_do_file"], f"{cell['cell_id']}.{arm}.timing_do_file")
            recipe["artifact"] = immutable(recipe["artifact"], f"{cell['cell_id']}.{arm}.artifact")
            if not isinstance(recipe["science_output"], str) or not recipe["science_output"]:
                raise CaptureFailure("science_output must be a nonempty relative filename")
            if Path(recipe["science_output"]).is_absolute() or ".." in Path(recipe["science_output"]).parts:
                raise CaptureFailure("science_output must stay within the child directory")
            recipe["expected_status_map"] = _status_map(recipe["expected_status_map"], f"{cell['cell_id']}.{arm}.expected_status_map")
            if recipe["expected_output"] not in OUTPUT_KINDS:
                raise CaptureFailure("expected_output is invalid")
            if isinstance(recipe["probe_rows"], bool) or not isinstance(recipe["probe_rows"], int) or recipe["probe_rows"] < 1:
                raise CaptureFailure("probe_rows must be positive")
            for field in ("expected_cluster_dimensions", "expected_saved_channels"):
                if isinstance(recipe[field], bool) or not isinstance(recipe[field], int) or recipe[field] < 0:
                    raise CaptureFailure(f"{field} must be a nonnegative integer")
            provenance = dict(_mapping(recipe["expected_source_provenance"], f"{cell['cell_id']}.{arm}.expected_source_provenance"))
            _closed(provenance, {"matrices", "scalars", "variables", "receipt_complete"}, f"{cell['cell_id']}.{arm}.expected_source_provenance")
            for field in ("matrices", "scalars", "variables"):
                if not isinstance(provenance[field], list) or not all(isinstance(value, str) and value for value in provenance[field]) or len(set(provenance[field])) != len(provenance[field]):
                    raise CaptureFailure("expected source provenance inventories must be unique string lists")
            if not isinstance(provenance["receipt_complete"], bool):
                raise CaptureFailure("expected source receipt flag must be boolean")
            recipe["expected_source_provenance"] = provenance
            cell[arm] = recipe
        for field in ("expected_status_map", "expected_output", "probe_rows", "expected_cluster_dimensions", "expected_saved_channels"):
            if cell["candidate"][field] != cell["comparator"][field]:
                raise CaptureFailure(f"{cell['cell_id']} arm recipes disagree on {field}")
        if cell["oracle_fixture"] is not None:
            oracle = dict(_mapping(cell["oracle_fixture"], f"{cell['cell_id']}.oracle_fixture"))
            _closed(oracle, {"fixture_id", "data", "canonical_export", "model", "generator", "row_count", "independent"}, f"{cell['cell_id']}.oracle_fixture")
            if not isinstance(oracle["fixture_id"], str) or not oracle["fixture_id"]:
                raise CaptureFailure("oracle fixture ID must be nonempty")
            for field in ("data", "canonical_export", "model", "generator"):
                oracle[field] = immutable(oracle[field], f"{cell['cell_id']}.oracle_fixture.{field}")
            if isinstance(oracle["row_count"], bool) or not isinstance(oracle["row_count"], int) or not 1 <= oracle["row_count"] <= 256:
                raise CaptureFailure("oracle fixture row_count is outside the bounded contract")
            independent = dict(_mapping(oracle["independent"], f"{cell['cell_id']}.oracle_fixture.independent"))
            _closed(independent, {"command", "program", "dependencies", "output"}, f"{cell['cell_id']}.oracle_fixture.independent")
            independent["program"] = immutable(independent["program"], f"{cell['cell_id']}.oracle_fixture.independent.program")
            if not isinstance(independent["dependencies"], list):
                raise CaptureFailure("oracle dependency inventory must be a list")
            independent["dependencies"] = [immutable(value, f"{cell['cell_id']}.oracle_fixture.independent.dependencies[{index}]") for index, value in enumerate(independent["dependencies"])]
            expected_oracle_command = [
                str(Path(oracle_interpreter or sys.executable).resolve()),
                "-I",
                "-B",
                "-S",
                str(independent["program"]["path"]),
            ]
            if independent["command"] != expected_oracle_command:
                raise CaptureFailure("oracle command is not the exact sealed Python/program argv")
            if not isinstance(independent["output"], str) or Path(independent["output"]).is_absolute() or ".." in Path(independent["output"]).parts:
                raise CaptureFailure("oracle output is unsafe")
            oracle["independent"] = independent
            cell["oracle_fixture"] = oracle
        loaded.append(cell)
    plan["cells"] = loaded
    fixtures = plan["constrained_memory"]
    if not isinstance(fixtures, list):
        raise CaptureFailure("constrained_memory must be a fixture list")
    loaded_fixtures = []
    for fixture_index, raw in enumerate(fixtures):
        fixture = dict(_mapping(raw, f"constrained_memory[{fixture_index}]"))
        _closed(fixture, {"id", "data", "attempts"}, f"constrained_memory[{fixture_index}]")
        if not isinstance(fixture["id"], str) or not fixture["id"]:
            raise CaptureFailure("constrained fixture ID must be nonempty")
        fixture["data"] = immutable(fixture["data"], f"{fixture['id']}.data")
        if not isinstance(fixture["attempts"], list) or not fixture["attempts"]:
            raise CaptureFailure("constrained fixture attempts must be nonempty")
        attempts = []
        for attempt_index, raw_attempt in enumerate(fixture["attempts"]):
            attempt = dict(_mapping(raw_attempt, f"{fixture['id']}.attempts[{attempt_index}]"))
            _closed(attempt, {"id", "do_file", "artifact", "science_output", "memory_limit_bytes"}, f"{fixture['id']}.attempts[{attempt_index}]")
            attempt["do_file"] = immutable(attempt["do_file"], f"{fixture['id']}.{attempt['id']}.do_file")
            attempt["artifact"] = immutable(attempt["artifact"], f"{fixture['id']}.{attempt['id']}.artifact")
            if not isinstance(attempt["science_output"], str) or Path(attempt["science_output"]).is_absolute() or ".." in Path(attempt["science_output"]).parts:
                raise CaptureFailure("constrained attempt science_output is unsafe")
            if (
                isinstance(attempt["memory_limit_bytes"], bool)
                or not isinstance(attempt["memory_limit_bytes"], int)
                or attempt["memory_limit_bytes"] <= 512 * 1024 * 1024
            ):
                raise CaptureFailure("constrained attempt memory limit must exceed fixed headroom")
            attempts.append(attempt)
        fixture["attempts"] = attempts
        loaded_fixtures.append(fixture)
    plan["constrained_memory"] = loaded_fixtures
    if logical_path_resolver is not None:
        plan["_logical_path_resolver"] = logical_path_resolver
    return plan


def stata_flavor(plan: Mapping[str, Any]) -> str:
    """Derive the product flavor from the exact sealed executable basename."""

    try:
        basename = Path(plan["stata"]["executable"]).resolve().name.lower()
    except (KeyError, TypeError) as error:
        raise CaptureFailure("sealed Stata executable identity is absent") from error
    flavors = {"stata-mp": "MP", "stata-se": "SE", "stata": "BE"}
    if basename not in flavors:
        raise CaptureFailure("sealed Stata executable has no closed product flavor")
    return flavors[basename]


def _rss_bytes(pid: int) -> int:
    status = Path(f"/proc/{pid}/status")
    if status.is_file():
        for line in status.read_text(encoding="ascii", errors="replace").splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) * 1024
    try:
        completed = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(pid)], check=False,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        )
    except OSError:
        return 0
    text = completed.stdout.strip()
    return int(text) * 1024 if text.isdigit() else 0


SUPERVISOR_FIELDS = {
    "schema_version", "receipt_type", "cell_id", "arm", "pair_index",
    "temperature", "requested_threads", "science_protocol", "command", "cwd",
    "process_id", "return_code", "start_monotonic_ns", "done_monotonic_ns",
    "rss_peak_events", "child_environment", "raw_files", "complete_sentinel",
}


def _file_inventory(root: Path, *, excluded: set[str] | None = None) -> list[dict[str, Any]]:
    """Return a closed, sorted inventory of regular non-symlink child files."""

    excluded = excluded or set()
    result: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*"), key=lambda value: value.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink() or (not path.is_file() and not path.is_dir()):
            raise CaptureFailure("licensed child raw inventory contains an unsafe entry")
        if path.is_file() and relative not in excluded:
            result.append(
                {
                    "path": relative,
                    "sha256": sha256_file(path),
                    "size_bytes": path.stat().st_size,
                }
            )
    return result


def _child_command(
    plan: Mapping[str, Any],
    cell: Mapping[str, Any],
    arm: str,
    work: Path,
    *,
    requested_threads: int,
    pair_index: int,
    temperature: str,
    science_protocol: bool,
    data_override: Mapping[str, Any] | None,
) -> tuple[list[str], Path, str]:
    recipe = cell[arm]
    data = data_override or cell["data"]
    preferred = "science_do_file" if science_protocol else "timing_do_file"
    do_field = preferred if preferred in recipe else "do_file"
    result_path = work / recipe["science_output"]
    child_arguments = [
        str(data["path"]), str(recipe["artifact"]["path"]),
        str(_logical_path(plan, result_path)),
        str(_logical_path(plan, work / "READY")),
        str(_logical_path(plan, work / "START")),
        str(_logical_path(plan, work / "DONE")),
        str(requested_threads),
    ]
    # Every recipe, including constrained-memory recipes, receives the same
    # closed 12-argument envelope.  Omitting the comparator ceiling for a
    # constrained ``do_file`` shifts pair/temperature/cell/arm and makes the
    # retained markers and attempt identity unauditable.
    child_arguments.append(str(plan["stata"]["comparator_processor_limit"]))
    child_arguments.extend([str(pair_index), temperature, str(cell["cell_id"]), arm])
    return (
        [
            str(plan["stata"]["executable"]), *plan["stata"]["batch_arguments"],
            str(recipe[do_field]["path"]), *child_arguments,
        ],
        result_path,
        do_field,
    )


def _logical_path(plan: Mapping[str, Any], path: Path) -> Path:
    resolver = plan.get("_logical_path_resolver")
    return Path(resolver(Path(path))) if callable(resolver) else Path(path)


def _sealed_child_environment(
    plan: Mapping[str, Any],
    work: Path,
    environment_override: Mapping[str, str] | None,
) -> dict[str, str]:
    """Construct the entire licensed child environment without inheritance."""

    executable = Path(plan["stata"]["executable"])
    logical_work = _logical_path(plan, work)
    environment = {
        "HOME": str(logical_work / "home"),
        "TMPDIR": str(logical_work / "tmp"),
        "STATATMP": str(logical_work / "tmp"),
        "PATH": f"{executable.parent}:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }
    for key, value in dict(environment_override or {}).items():
        environment[key] = (
            str(_logical_path(plan, Path(value)))
            if key == "FEREG_PPML_PREFLIGHT_RECEIPT"
            else value
        )
    return environment


def _sealed_oracle_environment(
    plan: Mapping[str, Any], root: Path
) -> dict[str, str]:
    """Construct the entire independent-Python environment without inheritance."""

    logical_root = _logical_path(plan, root)
    return {
        "HOME": str(logical_root / "independent-home"),
        "TMPDIR": str(logical_root / "independent-tmp"),
        "PATH": "/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }


def replay_stata_child(
    plan: Mapping[str, Any],
    cell: Mapping[str, Any],
    arm: str,
    work: Path,
    *,
    requested_threads: int,
    pair_index: int,
    temperature: str,
    science_protocol: bool = False,
    data_override: Mapping[str, Any] | None = None,
    environment_override: Mapping[str, str] | None = None,
) -> tuple[Mapping[str, Any], int, float, int]:
    """Reparse one retained licensed child without executing any process."""

    supervisor_path = work / "supervisor.json"
    supervisor = dict(_read_json(supervisor_path, "licensed child supervisor"))
    _closed(supervisor, SUPERVISOR_FIELDS, "licensed child supervisor")
    command, result_path, _do_field = _child_command(
        plan, cell, arm, work,
        requested_threads=requested_threads, pair_index=pair_index,
        temperature=temperature, science_protocol=science_protocol,
        data_override=data_override,
    )
    expected_environment = _sealed_child_environment(
        plan, work, environment_override
    )
    if (
        supervisor["schema_version"] != 1
        or supervisor["receipt_type"] != "feppml-stata-child-supervisor-v1"
        or supervisor["cell_id"] != cell["cell_id"]
        or supervisor["arm"] != arm
        or supervisor["pair_index"] != pair_index
        or supervisor["temperature"] != temperature
        or supervisor["requested_threads"] != requested_threads
        or supervisor["science_protocol"] is not science_protocol
        or supervisor["command"] != command
        or supervisor["cwd"] != str(_logical_path(plan, work.resolve()))
        or supervisor["return_code"] != 0
        or supervisor["child_environment"] != expected_environment
        or supervisor["complete_sentinel"] != "FEPPML_STATA_CHILD_SUPERVISOR_COMPLETE"
    ):
        raise CaptureFailure("licensed child supervisor identity is invalid")
    process_id = supervisor["process_id"]
    start_ns = supervisor["start_monotonic_ns"]
    done_ns = supervisor["done_monotonic_ns"]
    if (
        isinstance(process_id, bool) or not isinstance(process_id, int) or process_id <= 0
        or isinstance(start_ns, bool) or not isinstance(start_ns, int) or start_ns <= 0
        or isinstance(done_ns, bool) or not isinstance(done_ns, int) or done_ns <= start_ns
    ):
        raise CaptureFailure("licensed child supervisor timing/process fields are invalid")
    events = supervisor["rss_peak_events"]
    if not isinstance(events, list) or not events:
        raise CaptureFailure("licensed child supervisor omits RSS observations")
    previous_time = -1
    previous_rss = -1
    for event in events:
        if not isinstance(event, Mapping) or set(event) != {"monotonic_ns", "rss_bytes"}:
            raise CaptureFailure("licensed child RSS event is not closed")
        observed_time = event["monotonic_ns"]
        observed_rss = event["rss_bytes"]
        if (
            isinstance(observed_time, bool) or not isinstance(observed_time, int)
            or isinstance(observed_rss, bool) or not isinstance(observed_rss, int)
            or observed_time < previous_time or observed_rss <= previous_rss
            or observed_time <= 0 or observed_rss <= 0
        ):
            raise CaptureFailure("licensed child RSS events are invalid")
        previous_time, previous_rss = observed_time, observed_rss
    expected_files = _file_inventory(work, excluded={"supervisor.json"})
    if supervisor["raw_files"] != expected_files:
        raise CaptureFailure("licensed child raw file inventory changed")
    expected_markers = {
        "READY": f"FEPPML_STATA_READY|{cell['cell_id']}|{arm}\n",
        "START": f"FEPPML_STATA_START|{cell['cell_id']}|{arm}\n",
        "DONE": f"FEPPML_STATA_DONE|{cell['cell_id']}|{arm}|SUCCESS\n",
    }
    for name, expected in expected_markers.items():
        try:
            observed = (work / name).read_text(encoding="ascii")
        except OSError as error:
            raise CaptureFailure(f"cannot read Stata terminal marker: {error}") from error
        if observed != expected:
            raise CaptureFailure("licensed Stata marker content/order identity is invalid")
    try:
        resolution = (work / "RESOLUTION").read_text(encoding="ascii").splitlines()
    except OSError as error:
        raise CaptureFailure(f"cannot read staged command resolution: {error}") from error
    artifact_path = str(cell[arm]["artifact"]["path"])
    artifact_dir = str(Path(artifact_path).parent)
    if arm == "candidate":
        expected_ado = f"{artifact_dir}/_fereg_estimate_ppml.ado"
        valid_resolution = resolution == [
            f"ADO|{expected_ado}", f"ARTIFACT|{artifact_path}",
            f"DEPENDENCY|{artifact_dir}/fereg_parse.ado",
            f"DEPENDENCY|{artifact_dir}/_fereg_load.ado",
        ]
    else:
        valid_resolution = (
            len(resolution) == 5
            and resolution[0].startswith(f"ADO|{artifact_dir}/ppmlhdfe/")
            and resolution[0].endswith("/ppmlhdfe.ado")
            and resolution[1] == f"ARTIFACT|{artifact_path}"
            and all(
                resolution[index].startswith(f"DEPENDENCY|{artifact_dir}/{name}/")
                and resolution[index].endswith(f"/{name}.ado")
                for index, name in enumerate(("reghdfe", "ftools", "require"), 2)
            )
        )
    if not valid_resolution:
        raise CaptureFailure("licensed Stata command did not resolve from the sealed staged artifact")
    if science_protocol:
        recipe = cell[arm]
        try:
            parsed = stata_protocol.parse_science_protocol(
                result_path,
                arm=arm,
                expected_status_map=recipe["expected_status_map"],
                expected_output=recipe["expected_output"],
                expected_probe_rows=recipe["probe_rows"],
                expected_cluster_dimensions=recipe["expected_cluster_dimensions"],
                expected_saved_channels=recipe["expected_saved_channels"],
            )
            scientific = raw_normalizer.normalize(parsed["raw"])
        except (stata_protocol.StataProtocolError, raw_normalizer.RawNormalizationError) as error:
            raise CaptureFailure(
                f"{cell['cell_id']}.{arm} science protocol failed closed: {error}"
            ) from error
        if arm == "candidate":
            result: Mapping[str, Any] = {
                "scientific": scientific,
                "diagnostics": parsed["diagnostics"],
                "memory": parsed["memory"],
                "slowdown_explanation": parsed["slowdown_explanation"],
                "stata": parsed["stata"],
                "source_provenance": parsed["source_provenance"],
                "complete_sentinel": parsed["complete_sentinel"],
            }
        else:
            result = {
                "scientific": scientific,
                "stata": parsed["stata"],
                "source_provenance": parsed["source_provenance"],
                "complete_sentinel": parsed["complete_sentinel"],
            }
    else:
        result = _read_json(result_path, f"{cell['cell_id']}.{arm} child output")
    return result, process_id, (done_ns - start_ns) / 1_000_000_000.0, previous_rss


def _stata_child(
    plan: Mapping[str, Any], cell: Mapping[str, Any], arm: str, work: Path,
    *, requested_threads: int, pair_index: int, temperature: str,
    science_protocol: bool = False, data_override: Mapping[str, Any] | None = None,
    environment_override: Mapping[str, str] | None = None,
) -> tuple[Mapping[str, Any], int, float, int]:
    recipe = cell[arm]
    data = data_override or cell["data"]
    _immutable({"path": str(data["path"]), "sha256": data["sha256"]}, f"{cell['cell_id']}.data")
    preferred_do_field = "science_do_file" if science_protocol else "timing_do_file"
    do_field = preferred_do_field if preferred_do_field in recipe else "do_file"
    for field in (do_field, "artifact"):
        _immutable({"path": str(recipe[field]["path"]), "sha256": recipe[field]["sha256"]}, f"{cell['cell_id']}.{arm}.{field}")
    work.mkdir(parents=True, exist_ok=False)
    (work / "home").mkdir(mode=0o700)
    (work / "tmp").mkdir(mode=0o700)
    ready, start, done = (work / name for name in ("READY", "START", "DONE"))
    command, _result_path, expected_do_field = _child_command(
        plan, cell, arm, work,
        requested_threads=requested_threads, pair_index=pair_index,
        temperature=temperature, science_protocol=science_protocol,
        data_override=data_override,
    )
    if expected_do_field != do_field:
        raise CaptureFailure("licensed child do-file selection changed")
    began_ns = time.monotonic_ns()
    with (work / "stata.stdout").open("xb") as stdout, (work / "stata.stderr").open("xb") as stderr:
        if environment_override is not None:
            for key, value in environment_override.items():
                if key == "FEREG_MEMORY_LIMIT_BYTES":
                    if not value.isdigit() or int(value) <= 0:
                        raise CaptureFailure("constrained memory limit is invalid")
                elif key == "FEREG_PPML_PREFLIGHT_RECEIPT":
                    receipt = Path(value)
                    if (
                        receipt.is_symlink()
                        or receipt.exists()
                        or receipt.parent.resolve() != work.resolve()
                        or receipt.name != "core-preflight.json"
                    ):
                        raise CaptureFailure("constrained preflight receipt path is unsafe")
                else:
                    raise CaptureFailure("constrained child environment is not closed")
        child_environment = _sealed_child_environment(
            plan, work, environment_override
        )
        process = subprocess.Popen(
            command, cwd=work, stdout=stdout, stderr=stderr, env=child_environment
        )
        peak = max(0, _rss_bytes(process.pid))
        rss_events: list[dict[str, int]] = []
        if peak > 0:
            rss_events.append({"monotonic_ns": time.monotonic_ns(), "rss_bytes": peak})
        start_time_ns = None
        done_time_ns = None
        deadline_ns = began_ns + 14_400_000_000_000
        while process.poll() is None:
            now_ns = time.monotonic_ns()
            if now_ns > deadline_ns:
                process.kill(); process.wait()
                raise CaptureFailure("licensed Stata child timed out")
            if start.exists() and not ready.exists():
                process.kill(); process.wait()
                raise CaptureFailure("Stata child wrote START before READY")
            if done.exists() and not start.exists():
                process.kill(); process.wait()
                raise CaptureFailure("Stata child wrote DONE before START")
            if start.exists() and start_time_ns is None:
                start_time_ns = now_ns
            if done.exists() and done_time_ns is None:
                done_time_ns = now_ns
            if start_time_ns is not None and not done.exists():
                observed_rss = _rss_bytes(process.pid)
                if observed_rss > peak:
                    peak = observed_rss
                    rss_events.append({"monotonic_ns": now_ns, "rss_bytes": peak})
            time.sleep(0.01)
    if process.returncode != 0:
        raise CaptureFailure(f"licensed Stata child failed rc={process.returncode}")
    if start.exists() and start_time_ns is None:
        start_time_ns = began_ns
    if done.exists() and done_time_ns is None:
        done_time_ns = max(time.monotonic_ns(), start_time_ns + 1 if start_time_ns else 1)
    if start_time_ns is None or done_time_ns is None or peak <= 0 or not rss_events:
        raise CaptureFailure("licensed Stata child omitted timing markers or RSS evidence")
    expected_markers = {
        ready: f"FEPPML_STATA_READY|{cell['cell_id']}|{arm}\n",
        start: f"FEPPML_STATA_START|{cell['cell_id']}|{arm}\n",
        done: f"FEPPML_STATA_DONE|{cell['cell_id']}|{arm}|SUCCESS\n",
    }
    for marker, expected in expected_markers.items():
        try:
            observed = marker.read_text(encoding="ascii")
        except OSError as error:
            raise CaptureFailure(f"cannot read Stata terminal marker: {error}") from error
        if observed != expected:
            raise CaptureFailure("licensed Stata marker content/order identity is invalid")
    _write_new(
        work / "supervisor.json",
        {
            "schema_version": 1,
            "receipt_type": "feppml-stata-child-supervisor-v1",
            "cell_id": cell["cell_id"],
            "arm": arm,
            "pair_index": pair_index,
            "temperature": temperature,
            "requested_threads": requested_threads,
            "science_protocol": science_protocol,
            "command": command,
            "cwd": str(_logical_path(plan, work.resolve())),
            "process_id": process.pid,
            "return_code": process.returncode,
            "start_monotonic_ns": start_time_ns,
            "done_monotonic_ns": done_time_ns,
            "rss_peak_events": rss_events,
            "child_environment": child_environment,
            "raw_files": _file_inventory(work),
            "complete_sentinel": "FEPPML_STATA_CHILD_SUPERVISOR_COMPLETE",
        },
    )
    return replay_stata_child(
        plan, cell, arm, work,
        requested_threads=requested_threads, pair_index=pair_index,
        temperature=temperature, science_protocol=science_protocol,
        data_override=data_override, environment_override=environment_override,
    )


def _cell(plan: Mapping[str, Any], cell_id: str) -> Mapping[str, Any]:
    matches = [cell for cell in plan["cells"] if cell["cell_id"] == cell_id]
    if len(matches) != 1:
        raise CaptureFailure(f"unknown capture cell: {cell_id}")
    return matches[0]


def _science_cell_record(
    cell: Mapping[str, Any],
    candidate_value: Mapping[str, Any],
    comparator_value: Mapping[str, Any],
    oracle_result: Mapping[str, Any] | None,
) -> dict[str, Any]:
    candidate = dict(candidate_value)
    required_candidate = {
        "scientific", "diagnostics", "memory", "slowdown_explanation", "stata",
        "source_provenance", "complete_sentinel",
    }
    _closed(candidate, required_candidate, f"{cell['cell_id']}.candidate output")
    if candidate["complete_sentinel"] != "FEPPML_STATA_SCIENCE_COMPLETE":
        raise CaptureFailure("candidate science output lacks terminal sentinel")
    provenance = dict(_mapping(
        candidate["source_provenance"],
        f"{cell['cell_id']}.candidate.source_provenance",
    ))
    _closed(
        provenance, {"matrices", "scalars", "variables", "receipt_complete"},
        f"{cell['cell_id']}.candidate.source_provenance",
    )
    if provenance != cell["candidate"]["expected_source_provenance"]:
        raise CaptureFailure(
            "candidate science is not sourced from the complete native e() receipt"
        )
    candidate_stata = _mapping(candidate["stata"], f"{cell['cell_id']}.candidate.stata")
    if candidate_stata.get("licensed") is not True:
        raise CaptureFailure("candidate child did not attest a licensed Stata load")
    comparator = dict(comparator_value)
    _closed(
        comparator, {"scientific", "stata", "source_provenance", "complete_sentinel"},
        f"{cell['cell_id']}.comparator output",
    )
    if comparator["complete_sentinel"] != "FEPPML_STATA_SCIENCE_COMPLETE":
        raise CaptureFailure("comparator science output lacks terminal sentinel")
    comparator_provenance = dict(_mapping(
        comparator["source_provenance"],
        f"{cell['cell_id']}.comparator.source_provenance",
    ))
    _closed(comparator_provenance, {"matrices", "scalars", "variables", "receipt_complete"}, f"{cell['cell_id']}.comparator.source_provenance")
    if comparator_provenance != cell["comparator"]["expected_source_provenance"]:
        raise CaptureFailure("comparator science source inventory differs from the sealed recipe")
    comparator_stata = _mapping(
        comparator["stata"], f"{cell['cell_id']}.comparator.stata"
    )
    if comparator_stata.get("licensed") is not True:
        raise CaptureFailure("comparator child did not attest a licensed Stata load")
    return {
        "cell_id": cell["cell_id"],
        "data_sha256": cell["data"]["sha256"],
        "scientific": {
            "candidate": candidate["scientific"],
            "comparator": comparator["scientific"],
            "oracle": oracle_result,
        },
        "timing": {
            arm: {"cold_seconds": 1.0, "warm_seconds": [1.0] * 9}
            for arm in ("candidate", "comparator")
        },
        "execution": {"arm_order": [list(order) for order in QUALIFICATION_ARM_ORDER]},
        "diagnostics": candidate["diagnostics"],
        "memory": candidate["memory"],
        "slowdown_explanation": candidate["slowdown_explanation"],
    }


def _oracle_result(
    cell: Mapping[str, Any],
    fixture_candidate: Mapping[str, Any],
    fixture_comparator: Mapping[str, Any],
    independent_result: Mapping[str, Any],
) -> dict[str, Any]:
    oracle = cell["oracle_fixture"]
    if oracle is None:
        raise CaptureFailure("oracle result requested for a non-oracle cell")
    # Oracle-fixture Stata arms carry the same exact native provenance,
    # licensed-runtime identity, and terminal receipt contract as governing
    # science arms. Validate the complete envelopes before extracting science.
    validated = _science_cell_record(
        cell, fixture_candidate, fixture_comparator, None
    )["scientific"]
    return {
        "schema_version": 1,
        "check_type": "ppml-explicit-dummy-oracle-v1",
        "cell_id": cell["cell_id"],
        "fixture": {
            "fixture_id": oracle["fixture_id"],
            "data_sha256": oracle["data"]["sha256"],
            "canonical_export_sha256": oracle["canonical_export"]["sha256"],
            "model_sha256": oracle["model"]["sha256"],
            "generator_sha256": oracle["generator"]["sha256"],
            "row_count": oracle["row_count"],
        },
        "candidate": validated["candidate"],
        "comparator": validated["comparator"],
        "independent": independent_result,
        "complete_sentinel": "FEPPML_ORACLE_CHECK_COMPLETE",
    }


def _write_oracle_supervisor(
    root: Path,
    command: list[str],
    return_code: int,
    output: Path,
    *,
    child_environment: Mapping[str, str],
) -> None:
    files = []
    for path in (root / "independent.stdout", root / "independent.stderr", output):
        files.append(
            {
                "path": path.relative_to(root).as_posix(),
                "sha256": sha256_file(path),
                "size_bytes": path.stat().st_size,
            }
        )
    _write_new(
        root / "independent-supervisor.json",
        {
            "schema_version": 1,
            "receipt_type": "feppml-independent-oracle-supervisor-v1",
            "command": command,
            "cwd": str(root.resolve()),
            "return_code": return_code,
            "child_environment": dict(child_environment),
            "files": files,
            "complete_sentinel": "FEPPML_INDEPENDENT_ORACLE_SUPERVISOR_COMPLETE",
        },
    )


def _replay_independent_oracle(
    root: Path,
    command: list[str],
    output: Path,
    *,
    logical_root: Path | None = None,
    child_environment: Mapping[str, str],
) -> Mapping[str, Any]:
    record = dict(_read_json(root / "independent-supervisor.json", "oracle supervisor"))
    _closed(
        record,
        {
            "schema_version", "receipt_type", "command", "cwd",
            "return_code", "child_environment", "files", "complete_sentinel",
        },
        "oracle supervisor",
    )
    files = [
        {
            "path": path.relative_to(root).as_posix(),
            "sha256": sha256_file(path),
            "size_bytes": path.stat().st_size,
        }
        for path in (root / "independent.stdout", root / "independent.stderr", output)
    ]
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != "feppml-independent-oracle-supervisor-v1"
        or record["command"] != command
        or record["cwd"] != str(
            root.resolve() if logical_root is None else logical_root
        )
        or record["return_code"] != 0
        or record["child_environment"] != dict(child_environment)
        or record["files"] != files
        or record["complete_sentinel"]
            != "FEPPML_INDEPENDENT_ORACLE_SUPERVISOR_COMPLETE"
    ):
        raise CaptureFailure("independent oracle supervisor is invalid")
    return _read_json(output, "independent oracle output")


def preflight(args: argparse.Namespace, plan: Mapping[str, Any]) -> None:
    if sha256_file(args.development_manifest.resolve()) != plan["development_manifest"]["sha256"]:
        raise CaptureFailure("requested development manifest differs from capture plan")
    if sha256_file(args.qualification_manifest.resolve()) != plan["qualification_manifest_sha256"]:
        raise CaptureFailure("requested qualification manifest differs from capture plan")
    cells = []
    root = args.output.resolve().parent / "licensed-science"
    for cell in plan["cells"]:
        candidate, _, _, _ = _stata_child(plan, cell, "candidate", root / cell["cell_id"] / "candidate", requested_threads=cell["requested_threads"], pair_index=-1, temperature="preflight", science_protocol=True)
        comparator, _, _, _ = _stata_child(plan, cell, "comparator", root / cell["cell_id"] / "comparator", requested_threads=cell["requested_threads"], pair_index=-1, temperature="preflight", science_protocol=True)
        oracle_result = None
        oracle = cell["oracle_fixture"]
        if oracle is not None:
            fixture_root = root / cell["cell_id"] / "oracle-fixture"
            fixture_candidate, _, _, _ = _stata_child(
                plan, cell, "candidate", fixture_root / "candidate",
                requested_threads=cell["requested_threads"], pair_index=-1,
                temperature="preflight", science_protocol=True,
                data_override=oracle["data"],
            )
            fixture_comparator, _, _, _ = _stata_child(
                plan, cell, "comparator", fixture_root / "comparator",
                requested_threads=cell["requested_threads"], pair_index=-1,
                temperature="preflight", science_protocol=True,
                data_override=oracle["data"],
            )
            independent = oracle["independent"]
            destination = fixture_root / independent["output"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            command = [
                *(str(value) for value in independent["command"]),
                "--data", str(oracle["canonical_export"]["path"]),
                "--model", str(oracle["model"]["path"]),
                "--output", str(
                    plan.get("_logical_path_resolver", lambda path: path)(
                        destination
                    )
                ),
            ]
            oracle_environment = _sealed_oracle_environment(plan, fixture_root)
            (fixture_root / "independent-home").mkdir()
            (fixture_root / "independent-tmp").mkdir()
            with (fixture_root / "independent.stdout").open("xb") as stdout, (
                fixture_root / "independent.stderr"
            ).open("xb") as stderr:
                completed = subprocess.run(
                    command,
                    cwd=fixture_root,
                    env=oracle_environment,
                    stdout=stdout,
                    stderr=stderr,
                    check=False,
                )
            if completed.returncode != 0:
                raise CaptureFailure("independent oracle failed")
            _write_oracle_supervisor(
                fixture_root,
                command,
                completed.returncode,
                destination,
                child_environment=oracle_environment,
            )
            independent_result = _replay_independent_oracle(
                fixture_root,
                command,
                destination,
                logical_root=_logical_path(plan, fixture_root.resolve()),
                child_environment=oracle_environment,
            )
            oracle_result = _oracle_result(
                cell, fixture_candidate, fixture_comparator, independent_result
            )
        cells.append(_science_cell_record(cell, candidate, comparator, oracle_result))
    constrained = []
    for fixture in plan["constrained_memory"]:
        attempts = []
        for attempt in fixture["attempts"]:
            attempt_root = root / "constrained" / fixture["id"] / attempt["id"]
            preflight_path = attempt_root / "core-preflight.json"
            pseudo = {
                "cell_id": fixture["id"], "data": fixture["data"],
                "candidate": attempt,
            }
            result, _, _, _ = _stata_child(
                plan, pseudo, "candidate",
                attempt_root,
                requested_threads=1, pair_index=-1, temperature=attempt["id"],
                environment_override={
                    "FEREG_MEMORY_LIMIT_BYTES": str(attempt["memory_limit_bytes"]),
                    "FEREG_PPML_PREFLIGHT_RECEIPT": str(preflight_path),
                },
            )
            result = dict(result)
            _closed(result, {"attempt", "stata", "complete_sentinel"}, f"{fixture['id']}.{attempt['id']} output")
            if result["complete_sentinel"] != "FEPPML_STATA_CONSTRAINED_COMPLETE":
                raise CaptureFailure("constrained fixture lacks terminal sentinel")
            stata = _mapping(result["stata"], f"{fixture['id']}.{attempt['id']}.stata")
            if stata.get("licensed") is not True:
                raise CaptureFailure("constrained fixture lacks licensed Stata identity")
            command = dict(_mapping(
                result["attempt"], f"{fixture['id']}.{attempt['id']}.attempt"
            ))
            _closed(command, {"id", "status", "rc"}, "constrained command result")
            if (
                command["id"] != attempt["id"]
                or command["status"] not in {"success", "failure"}
                or isinstance(command["rc"], bool)
                or not isinstance(command["rc"], int)
                or (command["status"] == "success") != (command["rc"] == 0)
            ):
                raise CaptureFailure("constrained command result is invalid")
            preflight = _preflight_receipt(
                preflight_path,
                requested=attempt["id"],
                memory_limit_bytes=attempt["memory_limit_bytes"],
                source_commit=plan["identity"]["source_commit"],
            )
            attempts.append({
                **command,
                "preflight_receipt_sha256": sha256_file(preflight_path),
                "preflight_payload_sha256": hashlib.sha256(
                    json.dumps(
                        preflight,
                        sort_keys=True,
                        separators=(",", ":"),
                        ensure_ascii=True,
                        allow_nan=False,
                    ).encode("ascii") + b"\n"
                ).hexdigest(),
                "preflight": preflight,
            })
        constrained.append({"id": fixture["id"], "attempts": attempts})
    evidence = {"schema_version": 1, "evidence_type": "ppml-memory-performance-v1-evidence", "manifest": {"id": "ppml_memory_performance_v1", "sha256": plan["development_manifest"]["sha256"]}, "identity": plan["identity"], "cells": cells, "constrained_memory": constrained}
    _write_new(args.output.resolve(), {"schema_version": 1, "evidence_type": SCIENCE_TYPE, "qualification_manifest_sha256": plan["qualification_manifest_sha256"], "timing_placeholder": True, "timed_arm_processes_started": 0, "benchmark_evidence": evidence, "complete_sentinel": "FEPPML_SCIENCE_PREFLIGHT_COMPLETE"})


def replay_preflight(plan: Mapping[str, Any], output: Path) -> Mapping[str, Any]:
    """Rebuild the complete science preflight from retained child bytes only."""

    output = output.resolve()
    root = output.parent / "licensed-science"
    cells: list[dict[str, Any]] = []
    for cell in plan["cells"]:
        candidate, _, _, _ = replay_stata_child(
            plan, cell, "candidate", root / cell["cell_id"] / "candidate",
            requested_threads=cell["requested_threads"], pair_index=-1,
            temperature="preflight", science_protocol=True,
        )
        comparator, _, _, _ = replay_stata_child(
            plan, cell, "comparator", root / cell["cell_id"] / "comparator",
            requested_threads=cell["requested_threads"], pair_index=-1,
            temperature="preflight", science_protocol=True,
        )
        oracle_result = None
        oracle = cell["oracle_fixture"]
        if oracle is not None:
            fixture_root = root / cell["cell_id"] / "oracle-fixture"
            fixture_candidate, _, _, _ = replay_stata_child(
                plan, cell, "candidate", fixture_root / "candidate",
                requested_threads=cell["requested_threads"], pair_index=-1,
                temperature="preflight", science_protocol=True,
                data_override=oracle["data"],
            )
            fixture_comparator, _, _, _ = replay_stata_child(
                plan, cell, "comparator", fixture_root / "comparator",
                requested_threads=cell["requested_threads"], pair_index=-1,
                temperature="preflight", science_protocol=True,
                data_override=oracle["data"],
            )
            independent = oracle["independent"]
            destination = fixture_root / independent["output"]
            command = [
                *(str(value) for value in independent["command"]),
                "--data", str(oracle["canonical_export"]["path"]),
                "--model", str(oracle["model"]["path"]),
                "--output", str(
                    plan.get("_logical_path_resolver", lambda path: path)(
                        destination
                    )
                ),
            ]
            independent_result = _replay_independent_oracle(
                fixture_root,
                command,
                destination,
                logical_root=_logical_path(plan, fixture_root.resolve()),
                child_environment=_sealed_oracle_environment(plan, fixture_root),
            )
            oracle_result = _oracle_result(
                cell, fixture_candidate, fixture_comparator, independent_result
            )
        cells.append(_science_cell_record(cell, candidate, comparator, oracle_result))

    constrained: list[dict[str, Any]] = []
    for fixture in plan["constrained_memory"]:
        attempts = []
        for attempt in fixture["attempts"]:
            attempt_root = root / "constrained" / fixture["id"] / attempt["id"]
            preflight_path = attempt_root / "core-preflight.json"
            pseudo = {
                "cell_id": fixture["id"], "data": fixture["data"],
                "candidate": attempt,
            }
            environment = {
                "FEREG_MEMORY_LIMIT_BYTES": str(attempt["memory_limit_bytes"]),
                "FEREG_PPML_PREFLIGHT_RECEIPT": str(preflight_path),
            }
            result, _, _, _ = replay_stata_child(
                plan, pseudo, "candidate", attempt_root,
                requested_threads=1, pair_index=-1, temperature=attempt["id"],
                environment_override=environment,
            )
            result = dict(result)
            _closed(
                result, {"attempt", "stata", "complete_sentinel"},
                f"{fixture['id']}.{attempt['id']} output",
            )
            if result["complete_sentinel"] != "FEPPML_STATA_CONSTRAINED_COMPLETE":
                raise CaptureFailure("constrained fixture lacks terminal sentinel")
            stata = _mapping(result["stata"], f"{fixture['id']}.{attempt['id']}.stata")
            if stata.get("licensed") is not True:
                raise CaptureFailure("constrained fixture lacks licensed Stata identity")
            command_result = dict(_mapping(
                result["attempt"], f"{fixture['id']}.{attempt['id']}.attempt"
            ))
            _closed(command_result, {"id", "status", "rc"}, "constrained command result")
            if (
                command_result["id"] != attempt["id"]
                or command_result["status"] not in {"success", "failure"}
                or isinstance(command_result["rc"], bool)
                or not isinstance(command_result["rc"], int)
                or (command_result["status"] == "success")
                    != (command_result["rc"] == 0)
            ):
                raise CaptureFailure("constrained command result is invalid")
            core = _preflight_receipt(
                preflight_path,
                requested=attempt["id"],
                memory_limit_bytes=attempt["memory_limit_bytes"],
                source_commit=plan["identity"]["source_commit"],
            )
            attempts.append(
                {
                    **command_result,
                    "preflight_receipt_sha256": sha256_file(preflight_path),
                    "preflight_payload_sha256": hashlib.sha256(
                        json.dumps(
                            core, sort_keys=True, separators=(",", ":"),
                            ensure_ascii=True, allow_nan=False,
                        ).encode("ascii") + b"\n"
                    ).hexdigest(),
                    "preflight": core,
                }
            )
        constrained.append({"id": fixture["id"], "attempts": attempts})
    evidence = {
        "schema_version": 1,
        "evidence_type": "ppml-memory-performance-v1-evidence",
        "manifest": {
            "id": "ppml_memory_performance_v1",
            "sha256": plan["development_manifest"]["sha256"],
        },
        "identity": plan["identity"],
        "cells": cells,
        "constrained_memory": constrained,
    }
    expected = {
        "schema_version": 1,
        "evidence_type": SCIENCE_TYPE,
        "qualification_manifest_sha256": plan["qualification_manifest_sha256"],
        "timing_placeholder": True,
        "timed_arm_processes_started": 0,
        "benchmark_evidence": evidence,
        "complete_sentinel": "FEPPML_SCIENCE_PREFLIGHT_COMPLETE",
    }
    if _read_json(output, "scientific preflight") != expected:
        raise CaptureFailure("scientific preflight differs from retained raw children")
    return evidence


def arm(args: argparse.Namespace, plan: Mapping[str, Any]) -> None:
    cell = _cell(plan, args.cell_id)
    child, pid, seconds, peak = _stata_child(plan, cell, args.arm, args.output.resolve().parent / "licensed-child", requested_threads=args.requested_threads, pair_index=args.pair_index, temperature=args.temperature)
    _closed(child, {"control", "stata", "complete_sentinel"}, "timed arm child output")
    if child["complete_sentinel"] != "FEPPML_STATA_ARM_COMPLETE":
        raise CaptureFailure("timed arm child lacks terminal sentinel")
    control = _mapping(child["control"], "timed arm control")
    _closed(control, {"effective_threads", "stata_processors"}, "timed arm control")
    stata = _mapping(child["stata"], "timed arm Stata identity")
    _closed(stata, {"licensed", "version"}, "timed arm Stata identity")
    if stata["licensed"] is not True or not isinstance(stata["version"], str) or not stata["version"]:
        raise CaptureFailure("timed arm lacks licensed Stata identity")
    artifact = cell[args.arm]["artifact"]
    _write_new(args.output.resolve(), {"schema_version": 1, "evidence_type": ARM_TYPE, "cell_id": args.cell_id, "arm": args.arm, "pair_index": args.pair_index, "temperature": args.temperature, "seconds": seconds, "peak_rss_bytes": peak, "process_id": pid, "node_id": socket.gethostname(), "stata_version": stata["version"], "artifact_sha256": artifact["sha256"], "fresh_process": True, "effective_threads": control["effective_threads"], "stata_processors": control["stata_processors"], "complete_sentinel": "FEPPML_ARM_COMPLETE"})


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--plan", type=Path, required=True)
    modes = result.add_mutually_exclusive_group(required=True)
    modes.add_argument("--qualification-preflight", action="store_true")
    modes.add_argument("--qualification-arm", action="store_true")
    result.add_argument("--qualification-manifest", type=Path)
    result.add_argument("--development-manifest", type=Path)
    result.add_argument("--cell-id")
    result.add_argument("--arm", choices=("candidate", "comparator"))
    result.add_argument("--pair-index", type=int)
    result.add_argument("--temperature", choices=("cold", "warm"))
    result.add_argument("--requested-threads", type=int)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        plan = load_plan(args.plan)
        if args.qualification_preflight:
            if args.qualification_manifest is None or args.development_manifest is None:
                raise CaptureFailure("preflight requires both manifest paths")
            preflight(args, plan)
        else:
            if None in (args.cell_id, args.arm, args.pair_index, args.temperature, args.requested_threads) or args.requested_threads < 1:
                raise CaptureFailure("timed arm arguments are incomplete")
            arm(args, plan)
    except CaptureFailure as error:
        raise SystemExit(f"feppml licensed capture failed closed: {error}") from error
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
