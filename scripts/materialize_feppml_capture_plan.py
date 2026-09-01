#!/usr/bin/env python3
"""Materialize and verify a setup-time sealed feppml capture plan.

Preparation can freeze only semantic intent: exact build products and Stata
``.dta`` bytes do not exist yet.  Setup calls this module after building the
candidate, staging the pinned comparator, compiling recipes, and materializing
all datasets.  The resulting plan contains only immutable file records and a
compact closure digest record; the companion sealed receipt retains the full
per-file inventories behind those digests.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
from typing import Any, Mapping, Sequence


REQUEST_TYPE = "feppml-capture-materialization-request-v1"
PLAN_TYPE = "feppml-qualification-capture-plan-v1"
CLOSURE_TYPE = "feppml-capture-plan-closure-v1"
REQUEST_FIELDS = {
    "schema_version",
    "request_type",
    "qualification_manifest_sha256",
    "development_manifest_sha256",
    "cell_ids",
    "oracle_cell_ids",
    "constrained_fixture_ids",
    "stata_batch_arguments",
    "comparator_processor_limit",
    "complete_sentinel",
}
CLOSURE_FIELDS = {
    "schema_version",
    "closure_type",
    "materialization_request_sha256",
    "recipe_manifest_sha256",
    "recipe_inventory_sha256",
    "dataset_inventory_sha256",
    "oracle_fixture_inventory_sha256",
    "candidate_source_tree",
    "candidate_plugin_sha256",
    "candidate_bundle_sha256",
    "comparator_source_inventory_sha256",
    "comparator_bundle_sha256",
    "tool_inventory_sha256",
    "expected_inventory_sha256",
    "stata_executable_sha256",
    "complete_sentinel",
}
PLAN_FIELDS = {
    "schema_version",
    "plan_type",
    "qualification_manifest_sha256",
    "development_manifest",
    "stata",
    "identity",
    "cells",
    "constrained_memory",
    "closure",
    "complete_sentinel",
}


class MaterializationError(ValueError):
    """A request, concrete input, plan, or closure receipt failed closed."""


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def digest_file(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def _sha(value: Any, where: str, *, length: int = 64) -> str:
    if (
        not isinstance(value, str)
        or len(value) != length
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise MaterializationError(f"{where} must be a lowercase {length}-hex digest")
    return value


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise MaterializationError(f"{where} must be an object")
    return value


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise MaterializationError(
            f"{where} is not closed; missing={sorted(fields-set(value))}, "
            f"extra={sorted(set(value)-fields)}"
        )


def _read(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise MaterializationError(f"cannot read {where}: {error}") from error
    return _mapping(value, where)


def file_record(path: Path) -> dict[str, Any]:
    source = Path(path)
    if source.is_symlink():
        raise MaterializationError(f"immutable input must be a regular non-symlink file: {source}")
    path = source.resolve()
    if not path.is_file():
        raise MaterializationError(f"immutable input must be a regular non-symlink file: {path}")
    return {
        "path": str(path),
        "sha256": digest_file(path),
        "size_bytes": path.stat().st_size,
    }


def immutable(path: Path) -> dict[str, Any]:
    record = file_record(path)
    return {"path": record["path"], "sha256": record["sha256"]}


def tree_inventory(root: Path) -> dict[str, Any]:
    source_root = Path(root)
    if source_root.is_symlink():
        raise MaterializationError(f"bundle root must be a real directory: {source_root}")
    root = source_root.resolve()
    if not root.is_dir():
        raise MaterializationError(f"bundle root must be a real directory: {root}")
    files = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise MaterializationError(f"bundle tree contains a symlink: {path}")
        if not path.is_file():
            continue
        mode = stat.S_IMODE(path.stat().st_mode)
        files.append(
            {
                "relative_path": path.relative_to(root).as_posix(),
                "sha256": digest_file(path),
                "size_bytes": path.stat().st_size,
                "executable": bool(mode & 0o111),
            }
        )
    if not files:
        raise MaterializationError(f"bundle tree is empty: {root}")
    return {"root": str(root), "files": files, "sha256": digest(files)}


def load_request(path: Path) -> dict[str, Any]:
    request = dict(_read(path, "capture materialization request"))
    _closed(request, REQUEST_FIELDS, "capture materialization request")
    if (
        request["schema_version"] != 1
        or request["request_type"] != REQUEST_TYPE
        or request["complete_sentinel"] != "FEPPML_CAPTURE_MATERIALIZATION_REQUEST_COMPLETE"
    ):
        raise MaterializationError("materialization request identity/sentinel is invalid")
    _sha(request["qualification_manifest_sha256"], "qualification manifest SHA")
    _sha(request["development_manifest_sha256"], "development manifest SHA")
    for field in ("cell_ids", "oracle_cell_ids", "constrained_fixture_ids"):
        values = request[field]
        if (
            not isinstance(values, list)
            or not all(isinstance(value, str) and value for value in values)
            or len(values) != len(set(values))
        ):
            raise MaterializationError(f"{field} must be a unique ordered string list")
    if not request["cell_ids"] or not set(request["oracle_cell_ids"]) < set(request["cell_ids"]):
        raise MaterializationError("oracle inventory must be a strict nonempty cell subset")
    if not request["constrained_fixture_ids"]:
        raise MaterializationError("constrained fixture inventory must be nonempty")
    arguments = request["stata_batch_arguments"]
    if arguments != ["-q", "-b", "do"]:
        raise MaterializationError(
            "Stata batch arguments must be the exact noninteractive argv -q -b do"
        )
    limit = request["comparator_processor_limit"]
    if limit != 16:
        raise MaterializationError(
            "comparator processor limit must be the frozen 16-thread ceiling"
        )
    return request


def validate_closure(value: Any) -> dict[str, Any]:
    closure = dict(_mapping(value, "capture-plan closure"))
    _closed(closure, CLOSURE_FIELDS, "capture-plan closure")
    if (
        closure["schema_version"] != 1
        or closure["closure_type"] != CLOSURE_TYPE
        or closure["complete_sentinel"] != "FEPPML_CAPTURE_PLAN_CLOSURE_COMPLETE"
    ):
        raise MaterializationError("capture-plan closure identity/sentinel is invalid")
    for field in CLOSURE_FIELDS - {
        "schema_version", "closure_type", "candidate_source_tree", "complete_sentinel"
    }:
        _sha(closure[field], f"closure.{field}")
    _sha(closure["candidate_source_tree"], "closure.candidate_source_tree", length=40)
    return closure


def _verify_file_records(
    records: Any, where: str, *, path_resolver: Any | None = None
) -> None:
    if not isinstance(records, list) or not records:
        raise MaterializationError(f"{where} must be a nonempty file-record list")
    for index, raw in enumerate(records):
        record = _mapping(raw, f"{where}[{index}]")
        if not {"path", "sha256", "size_bytes"} <= set(record):
            raise MaterializationError(f"{where}[{index}] omits immutable identity")
        source = Path(record["path"])
        path = source.resolve() if path_resolver is None else Path(path_resolver(source))
        if (
            source.is_symlink()
            or not path.is_file()
            or path.is_symlink()
            or path.stat().st_size != record["size_bytes"]
            or digest_file(path) != record["sha256"]
        ):
            raise MaterializationError(f"{where}[{index}] bytes changed: {path}")


def _verify_nested_immutable(
    value: Any, where: str, *, path_resolver: Any | None = None
) -> int:
    """Verify every closed file record nested in a canonical inventory."""

    verified = 0
    if isinstance(value, Mapping):
        keys = set(value)
        if {"path", "sha256", "size_bytes"} <= keys:
            source = Path(value["path"])
            path = source.resolve() if path_resolver is None else Path(path_resolver(source))
            if (
                source.is_symlink()
                or not path.is_file()
                or path.is_symlink()
                or path.stat().st_size != value["size_bytes"]
                or digest_file(path) != value["sha256"]
            ):
                raise MaterializationError(f"{where} immutable bytes changed: {path}")
            verified += 1
        for key, child in value.items():
            if key not in {"path", "sha256", "size_bytes"}:
                verified += _verify_nested_immutable(
                    child, f"{where}.{key}", path_resolver=path_resolver
                )
    elif isinstance(value, list):
        for index, child in enumerate(value):
            verified += _verify_nested_immutable(
                child, f"{where}[{index}]", path_resolver=path_resolver
            )
    return verified


def _normal_json(value: Any) -> Any:
    """Round-trip through canonical JSON to reject Paths and exotic mappings."""

    try:
        return json.loads(canonical(value))
    except (TypeError, ValueError) as error:
        raise MaterializationError(f"materialized inventory is not canonical JSON: {error}") from error


def materialize(
    *,
    request_path: Path,
    development_manifest_path: Path,
    stata_executable: Path,
    identity: Mapping[str, Any],
    cells: Sequence[Mapping[str, Any]],
    constrained_memory: Sequence[Mapping[str, Any]],
    recipe_manifest_path: Path,
    recipe_root: Path,
    datasets: Sequence[Mapping[str, Any]],
    oracle_fixtures: Sequence[Mapping[str, Any]],
    candidate_source: Mapping[str, Any],
    candidate_plugin_path: Path,
    candidate_bundle_root: Path,
    comparator_sources: Mapping[str, Any],
    comparator_bundle_root: Path,
    tools: Sequence[Path],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Return a closed executable plan and its full seal payload."""

    request = load_request(request_path)
    if digest_file(development_manifest_path) != request["development_manifest_sha256"]:
        raise MaterializationError("development manifest differs from the request")
    if not stata_executable.resolve().is_file():
        raise MaterializationError("licensed Stata executable is absent")
    cell_values = _normal_json(list(cells))
    constrained_values = _normal_json(list(constrained_memory))
    observed_cells = [record.get("cell_id") for record in cell_values if isinstance(record, Mapping)]
    if observed_cells != request["cell_ids"]:
        raise MaterializationError("materialized cell inventory/order differs from request")
    observed_oracles = [
        record["cell_id"] for record in cell_values
        if isinstance(record, Mapping) and record.get("oracle_fixture") is not None
    ]
    if observed_oracles != request["oracle_cell_ids"]:
        raise MaterializationError("materialized oracle inventory/order differs from request")
    observed_constrained = [
        record.get("id") for record in constrained_values if isinstance(record, Mapping)
    ]
    if observed_constrained != request["constrained_fixture_ids"]:
        raise MaterializationError("materialized constrained inventory/order differs from request")

    recipe_tree = tree_inventory(recipe_root)
    recipe_manifest = file_record(recipe_manifest_path)
    if not any(
        record["relative_path"] == Path(recipe_manifest_path).resolve().relative_to(recipe_root.resolve()).as_posix()
        and record["sha256"] == recipe_manifest["sha256"]
        for record in recipe_tree["files"]
    ):
        raise MaterializationError("recipe manifest is outside or absent from recipe inventory")
    dataset_values = _normal_json(list(datasets))
    oracle_values = _normal_json(list(oracle_fixtures))
    _verify_file_records(dataset_values, "dataset inventory")
    if _verify_nested_immutable(oracle_values, "oracle fixture inventory") < len(oracle_values):
        raise MaterializationError("each oracle fixture must bind at least one immutable file")
    candidate_source_value = _normal_json(dict(candidate_source))
    _sha(candidate_source_value.get("tree"), "candidate source tree", length=40)
    candidate_plugin = file_record(candidate_plugin_path)
    candidate_bundle = tree_inventory(candidate_bundle_root)
    if not any(
        record["sha256"] == candidate_plugin["sha256"]
        and (candidate_bundle_root.resolve() / record["relative_path"]).resolve()
            == candidate_plugin_path.resolve()
        for record in candidate_bundle["files"]
    ):
        raise MaterializationError("candidate plugin is absent from candidate bundle inventory")
    comparator_source_values = _normal_json(dict(comparator_sources))
    comparator_bundle = tree_inventory(comparator_bundle_root)
    tool_records = [file_record(path) for path in tools]
    if len({record["path"] for record in tool_records}) != len(tool_records):
        raise MaterializationError("tool inventory contains duplicate paths")
    expected = {
        "cell_ids": request["cell_ids"],
        "oracle_cell_ids": request["oracle_cell_ids"],
        "constrained_fixture_ids": request["constrained_fixture_ids"],
    }
    stata = {
        "executable": str(stata_executable.resolve()),
        "sha256": digest_file(stata_executable.resolve()),
        "batch_arguments": list(request["stata_batch_arguments"]),
        "comparator_processor_limit": request["comparator_processor_limit"],
    }
    closure = validate_closure(
        {
            "schema_version": 1,
            "closure_type": CLOSURE_TYPE,
            "materialization_request_sha256": digest_file(request_path),
            "recipe_manifest_sha256": recipe_manifest["sha256"],
            "recipe_inventory_sha256": recipe_tree["sha256"],
            "dataset_inventory_sha256": digest(dataset_values),
            "oracle_fixture_inventory_sha256": digest(oracle_values),
            "candidate_source_tree": candidate_source_value["tree"],
            "candidate_plugin_sha256": candidate_plugin["sha256"],
            "candidate_bundle_sha256": candidate_bundle["sha256"],
            "comparator_source_inventory_sha256": digest(comparator_source_values),
            "comparator_bundle_sha256": comparator_bundle["sha256"],
            "tool_inventory_sha256": digest(tool_records),
            "expected_inventory_sha256": digest(expected),
            "stata_executable_sha256": stata["sha256"],
            "complete_sentinel": "FEPPML_CAPTURE_PLAN_CLOSURE_COMPLETE",
        }
    )
    plan = {
        "schema_version": 1,
        "plan_type": PLAN_TYPE,
        "qualification_manifest_sha256": request["qualification_manifest_sha256"],
        "development_manifest": immutable(development_manifest_path),
        "stata": stata,
        "identity": _normal_json(dict(identity)),
        "cells": cell_values,
        "constrained_memory": constrained_values,
        "closure": closure,
        "complete_sentinel": "FEPPML_CAPTURE_PLAN_COMPLETE",
    }
    _closed(plan, PLAN_FIELDS, "materialized capture plan")
    payload = {
        "schema_version": 1,
        "kind": "feppml_capture_plan_materialization",
        "request": {"record": file_record(request_path), "document": request},
        "development_manifest": file_record(development_manifest_path),
        "recipes": {"manifest": recipe_manifest, "tree": recipe_tree},
        "datasets": dataset_values,
        "oracle_fixtures": oracle_values,
        "candidate": {
            "source": candidate_source_value,
            "plugin": candidate_plugin,
            "bundle": candidate_bundle,
        },
        "comparator": {
            "sources": comparator_source_values,
            "bundle": comparator_bundle,
        },
        "tools": tool_records,
        "expected_inventory": expected,
        "stata": stata,
        "closure": closure,
        "plan_sha256": hashlib.sha256(canonical(plan)).hexdigest(),
        "plan_file_sha256": hashlib.sha256(
            (json.dumps(plan, sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False) + "\n").encode("ascii")
        ).hexdigest(),
        "complete_sentinel": "FEPPML_CAPTURE_PLAN_MATERIALIZED",
    }
    return plan, payload


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {"payload": dict(payload), "seal": hashlib.sha256(canonical(payload)).hexdigest()}


def verify_receipt(
    plan_path: Path,
    receipt_path: Path,
    *,
    path_resolver: Any | None = None,
) -> Mapping[str, Any]:
    plan = dict(_read(plan_path, "materialized capture plan"))
    _closed(plan, PLAN_FIELDS, "materialized capture plan")
    closure = validate_closure(plan["closure"])
    envelope = dict(_read(receipt_path, "capture-plan materialization receipt"))
    _closed(envelope, {"payload", "seal"}, "capture-plan receipt envelope")
    payload = _mapping(envelope["payload"], "capture-plan receipt payload")
    if envelope["seal"] != hashlib.sha256(canonical(payload)).hexdigest():
        raise MaterializationError("capture-plan receipt seal mismatch")
    if (
        payload.get("kind") != "feppml_capture_plan_materialization"
        or payload.get("complete_sentinel") != "FEPPML_CAPTURE_PLAN_MATERIALIZED"
        or payload.get("closure") != closure
        or payload.get("plan_sha256") != hashlib.sha256(canonical(plan)).hexdigest()
        or digest_file(plan_path) != payload.get("plan_file_sha256")
    ):
        raise MaterializationError("capture plan and materialization receipt are misbound")
    # Revalidate every concrete byte used to derive the closure.  Trees are
    # checked by rebuilding their exact canonical inventories.
    for section in ("datasets", "tools"):
        _verify_file_records(
            payload[section], section, path_resolver=path_resolver
        )
    if _verify_nested_immutable(
        payload["oracle_fixtures"],
        "oracle_fixtures",
        path_resolver=path_resolver,
    ) < len(payload["oracle_fixtures"]):
        raise MaterializationError("oracle fixture receipt lacks immutable prerequisites")
    for section, field in (("candidate", "bundle"), ("comparator", "bundle")):
        stored = _mapping(payload[section], section)[field]
        root = Path(stored["root"])
        observed = tree_inventory(
            root if path_resolver is None else Path(path_resolver(root))
        )
        if path_resolver is not None:
            observed["root"] = stored["root"]
        if observed != stored:
            raise MaterializationError(f"{section} bundle changed after plan sealing")
    recipes = _mapping(payload["recipes"], "recipes")
    recipe_root = Path(recipes["tree"]["root"])
    observed_recipes = tree_inventory(
        recipe_root
        if path_resolver is None
        else Path(path_resolver(recipe_root))
    )
    if path_resolver is not None:
        observed_recipes["root"] = recipes["tree"]["root"]
    if observed_recipes != recipes["tree"]:
        raise MaterializationError("recipe inventory changed after plan sealing")
    if closure["dataset_inventory_sha256"] != digest(payload["datasets"]):
        raise MaterializationError("dataset closure digest mismatch")
    if closure["oracle_fixture_inventory_sha256"] != digest(payload["oracle_fixtures"]):
        raise MaterializationError("oracle closure digest mismatch")
    if closure["tool_inventory_sha256"] != digest(payload["tools"]):
        raise MaterializationError("tool closure digest mismatch")
    if closure["expected_inventory_sha256"] != digest(payload["expected_inventory"]):
        raise MaterializationError("expected-inventory closure digest mismatch")
    if closure["recipe_inventory_sha256"] != recipes["tree"]["sha256"]:
        raise MaterializationError("recipe closure digest mismatch")
    if closure["candidate_bundle_sha256"] != payload["candidate"]["bundle"]["sha256"]:
        raise MaterializationError("candidate bundle closure digest mismatch")
    if closure["comparator_bundle_sha256"] != payload["comparator"]["bundle"]["sha256"]:
        raise MaterializationError("comparator bundle closure digest mismatch")
    return payload


def verify_receipt_portable(
    plan_path: Path, receipt_path: Path, run_root: Path
) -> Mapping[str, Any]:
    """Verify copied SCC materialization bytes without recreating SCC paths."""

    envelope = _read(receipt_path, "capture-plan materialization receipt")
    payload = _mapping(envelope.get("payload"), "capture-plan receipt payload")
    request = _mapping(payload.get("request"), "capture-plan request")
    request_record = _mapping(request.get("record"), "capture-plan request record")
    recorded_request = Path(str(request_record.get("path", "")))
    suffix = Path("input/capture/materialization-request.json")
    if (
        not recorded_request.is_absolute()
        or len(recorded_request.parts) <= len(suffix.parts)
        or recorded_request.parts[-len(suffix.parts):] != suffix.parts
    ):
        raise MaterializationError("portable receipt has no sealed SCC run root")
    recorded_root = recorded_request.parents[2]
    run_root = run_root.resolve()
    if recorded_root.name != run_root.name:
        raise MaterializationError("portable receipt run ID differs from collected run")

    def relocate(path: Path) -> Path:
        try:
            relative = Path(path).relative_to(recorded_root)
        except ValueError as error:
            raise MaterializationError(
                f"portable materialization path is outside the sealed run: {path}"
            ) from error
        local = (run_root / relative).resolve()
        try:
            local.relative_to(run_root)
        except ValueError as error:
            raise MaterializationError("portable materialization path escapes run") from error
        return local

    return verify_receipt(
        plan_path, receipt_path, path_resolver=relocate
    )


def write_new(path: Path, value: Mapping[str, Any]) -> None:
    data = json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True, allow_nan=False) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise MaterializationError(f"refusing to overwrite materialized output: {path}") from error
    with os.fdopen(descriptor, "w", encoding="ascii") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())
