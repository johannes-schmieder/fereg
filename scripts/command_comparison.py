#!/usr/bin/env python3
"""Validate and summarize the registered Stata 19 command comparison."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import time
from typing import Any, Iterable, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = (
    ROOT / "benchmarks/manifests/ols_command_comparison_stata19_v1.json"
)


class ContractError(RuntimeError):
    """The comparison manifest or evidence failed closed."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read comparison manifest: {error}") from error
    if not isinstance(document, dict):
        raise ContractError("comparison manifest must be an object")
    required = {
        "schema_version",
        "manifest_id",
        "status",
        "minimum_stata_version",
        "scientific_contract",
        "dependency_contract",
        "generator_contract",
        "scales",
        "designs",
        "cases",
        "threads",
        "arms",
        "output_modes",
        "repetitions",
        "timing_contract",
        "science_gates",
        "limitations",
    }
    if set(document) != required:
        raise ContractError("comparison manifest fields are not frozen")
    if (
        document["schema_version"] != 1
        or document["manifest_id"] != "ols_command_comparison_stata19_v1"
        or document["status"] != "development_only"
        or document["minimum_stata_version"] != 19
    ):
        raise ContractError("comparison manifest identity is invalid")
    if manifest_dependency := document.get("dependency_contract"):
        if manifest_dependency != {
            "reghdfe": {
                "version": "6.14.1",
                "commit": "29b2b203f534413369d675d0c0f674d61291f536",
            },
            "gtools": {"version": "1.11.8"},
            "areg": {"provider": "StataCorp", "minimum_stata_version": 19},
        }:
            raise ContractError("comparison dependency contract is not frozen")
    scales = _index(document["scales"], "scale")
    designs = _index(document["designs"], "design")
    arms = _index(document["arms"], "arm")
    modes = _index(document["output_modes"], "output mode")
    if set(scales) != {"n100k", "n1m", "n10m"}:
        raise ContractError("comparison scales are incomplete")
    if set(designs) != {
        "one_fe",
        "iid_two_fe",
        "akm_highmob",
        "akm_standard",
        "akm_lowmob",
    }:
        raise ContractError("comparison designs are incomplete")
    if set(arms) != {"fereg", "greg", "reghdfe", "areg"}:
        raise ContractError("comparison arms are incomplete")
    if set(modes) != {"fit", "save_fe"}:
        raise ContractError("comparison output modes are incomplete")
    if modes["fit"]["arms"] != ["fereg", "greg", "reghdfe", "areg"]:
        raise ContractError("fit arm order is not frozen")
    if modes["save_fe"]["arms"] != ["fereg", "greg", "reghdfe"]:
        raise ContractError("saved-FE arm order is not frozen")
    if document["threads"] != [1, 8] or document["repetitions"] != {
        "cold": 1,
        "warm": 5,
    }:
        raise ContractError("comparison cadence is not frozen")
    registered_cases = {
        (case["scale"], design)
        for case in document["cases"]
        for design in case["designs"]
    }
    if len(registered_cases) != 13:
        raise ContractError("comparison case inventory must contain 13 datasets")
    if any(scale not in scales or design not in designs for scale, design in registered_cases):
        raise ContractError("comparison case references an unknown scale or design")
    return document


def _index(values: Any, where: str) -> dict[str, Mapping[str, Any]]:
    if not isinstance(values, list):
        raise ContractError(f"{where} inventory must be a list")
    result: dict[str, Mapping[str, Any]] = {}
    for value in values:
        if not isinstance(value, dict) or not isinstance(value.get("id"), str):
            raise ContractError(f"malformed {where}")
        if value["id"] in result:
            raise ContractError(f"duplicate {where}: {value['id']}")
        result[value["id"]] = value
    return result


def expand_plan(
    manifest: Mapping[str, Any],
    *,
    scales: Sequence[str] | None = None,
    designs: Sequence[str] | None = None,
    threads: Sequence[int] | None = None,
    areg_scope: str = "all",
    areg_cases: set[tuple[str, int]] | None = None,
) -> list[dict[str, Any]]:
    selected_scales = set(scales or [item["id"] for item in manifest["scales"]])
    selected_designs = set(designs or [item["id"] for item in manifest["designs"]])
    selected_threads = list(threads or manifest["threads"])
    scale_map = {item["id"]: item for item in manifest["scales"]}
    design_map = {item["id"]: item for item in manifest["designs"]}
    modes = {item["id"]: item for item in manifest["output_modes"]}
    if selected_scales - set(scale_map):
        raise ContractError("unknown selected scale")
    if selected_designs - set(design_map):
        raise ContractError("unknown selected design")
    if any(value <= 0 for value in selected_threads):
        raise ContractError("thread counts must be positive")
    if areg_scope not in {"all", "one_fe", "off", "selected"}:
        raise ContractError("unknown areg scope")
    if areg_scope == "selected" and areg_cases is None:
        raise ContractError("selected areg scope requires an explicit case inventory")
    plan: list[dict[str, Any]] = []
    for case in manifest["cases"]:
        if case["scale"] not in selected_scales:
            continue
        for design_id in case["designs"]:
            if design_id not in selected_designs:
                continue
            for thread_count in selected_threads:
                for mode_id, mode in modes.items():
                    for arm in mode["arms"]:
                        if arm == "areg" and (
                            areg_scope == "off"
                            or (areg_scope == "one_fe" and design_id != "one_fe")
                            or (
                                areg_scope == "selected"
                                and (f"{design_id}_{case['scale']}", thread_count)
                                not in (areg_cases or set())
                            )
                        ):
                            continue
                        plan.append(
                            {
                                "case_id": f"{design_id}_{case['scale']}",
                                "scale": dict(scale_map[case["scale"]]),
                                "design": dict(design_map[design_id]),
                                "threads": thread_count,
                                "mode": mode_id,
                                "arm": arm,
                            }
                        )
    return plan


def _read_environment(path: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line:
            raise ContractError("malformed environment record")
        key, value = line.split("=", 1)
        result[key] = value
    required = {
        "stata_version",
        "threads",
        "scales",
        "warm_repetitions",
        "areg_scope",
    }
    if not required <= set(result):
        raise ContractError("environment record is incomplete")
    version = float(result["stata_version"])
    areg_scope = result["areg_scope"]
    if version < 18:
        raise ContractError("comparison evidence was not produced by Stata 18+")
    if areg_scope not in {"all", "one_fe", "off", "selected"}:
        raise ContractError("environment has an invalid areg scope")
    if version < 19 and areg_scope == "all":
        raise ContractError("multi-way areg evidence requires Stata 19+")
    return result


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8-sig") as source:
        return list(csv.DictReader(source))


def _write_csv(path: Path, rows: Sequence[Mapping[str, str]]) -> None:
    if not rows:
        raise ContractError(f"cannot write empty CSV: {path}")
    fields = list(rows[0])
    if any(list(row) != fields for row in rows):
        raise ContractError("CSV shard fields are inconsistent")
    with path.open("w", newline="", encoding="utf-8") as destination:
        writer = csv.DictWriter(destination, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def merge_isolated_shards(
    manifest: Mapping[str, Any],
    input_dirs: Sequence[Path],
    output_dir: Path,
) -> dict[str, Any]:
    """Merge fresh-process component shards into the frozen comparison cadence."""
    if not input_dirs:
        raise ContractError("at least one isolated shard is required")
    shards: dict[
        tuple[str, int],
        list[
            tuple[
                dict[str, str],
                list[dict[str, str]],
                list[dict[str, str]],
                list[dict[str, Any]],
            ]
        ],
    ] = {}
    base_environment: dict[str, str] | None = None
    observed_invariants: dict[str, str] = {}
    invariant_environment = {
        "stata_version",
        "stata_flavor",
        "os",
        "machine_type",
        "configured_processors",
        "comparator_root",
        "dependency_fereg",
        "dependency_greg",
        "dependency_gregress",
        "dependency_reghdfe",
        "dependency_areg",
    }
    for directory in input_dirs:
        environment = _read_environment(directory / "environment.txt")
        if base_environment is None:
            base_environment = environment
        for field in invariant_environment:
            observed = environment.get(field)
            if observed is None and field.startswith("dependency_"):
                continue
            if field in observed_invariants and observed != observed_invariants[field]:
                raise ContractError(f"isolated shard environment mismatch: {field}")
            if observed is not None:
                observed_invariants[field] = observed
        runs_path = directory / "command_comparison_runs.csv"
        preflights_path = directory / "command_comparison_preflights.csv"
        failure_path = directory / "failure.json"
        rows = _read_csv(runs_path) if runs_path.exists() else []
        preflights = _read_csv(preflights_path) if preflights_path.exists() else []
        failures = []
        if failure_path.exists():
            failure = json.loads(failure_path.read_text(encoding="utf-8"))
            if not isinstance(failure, dict):
                raise ContractError("component failure record must be an object")
            failures.append(failure)
        scopes = {
            (row["case_id"], int(row["threads"])) for row in rows + preflights
        }
        scopes.update(
            (str(failure.get("case_id", "")), int(failure.get("threads", 0)))
            for failure in failures
        )
        if len(scopes) != 1:
            raise ContractError("each isolated shard must contain one case/thread")
        scope = next(iter(scopes))
        for failure in failures:
            if (
                failure.get("schema_version") != 1
                or failure.get("arm") != "areg"
                or failure.get("mode") != "fit"
                or failure.get("status") not in {"time_limit", "resource_limit", "command_error"}
                or float(failure.get("elapsed_seconds", 0)) <= 0
            ):
                raise ContractError(f"invalid component failure record for {scope}")
        shards.setdefault(scope, []).append((environment, rows, preflights, failures))

    merged_rows: list[dict[str, str]] = []
    merged_preflights: list[dict[str, str]] = []
    scope_areg: dict[tuple[str, int], bool] = {}
    scope_requested_areg: dict[tuple[str, int], bool] = {}
    merged_failures: list[dict[str, Any]] = []
    for scope, pieces in sorted(shards.items()):
        scope_requested_areg[scope] = any(
            environment["areg_scope"] == "all"
            or (
                environment["areg_scope"] == "one_fe"
                and scope[0].startswith("one_fe_")
            )
            for environment, _, _, _ in pieces
        )
        scope_failures = [
            failure for _, _, _, failures in pieces for failure in failures
        ]
        scope_areg[scope] = scope_requested_areg[scope] and not scope_failures
        merged_failures.extend(scope_failures)
        records: dict[tuple[str, str, int, str], tuple[int, dict[str, str]]] = {}
        preflight_candidates: list[dict[str, str]] = []
        for environment, rows, preflights, _ in pieces:
            offset = int(environment.get("rep_offset", "0"))
            preflight_candidates.extend(dict(row) for row in preflights)
            for source in rows:
                row = dict(source)
                if scope_failures and row["arm"] == "areg":
                    continue
                key = (row["mode"], row["phase"], int(row["rep"]), row["arm"])
                if key not in records:
                    records[key] = (offset, row)
                elif row["phase"] == "cold" and offset < records[key][0]:
                    records[key] = (offset, row)
                elif row["phase"] != "cold":
                    raise ContractError(f"duplicate isolated warm record for {scope}/{key}")

        expected_arms = {
            "fit": {"fereg", "greg", "reghdfe"},
            "save_fe": {"fereg", "greg", "reghdfe"},
        }
        if scope_areg[scope]:
            expected_arms["fit"].add("areg")
        for mode, arms in expected_arms.items():
            for arm in arms:
                if (mode, "cold", 0, arm) not in records:
                    raise ContractError(f"isolated cold record is missing for {scope}/{mode}/{arm}")
                reps = {
                    rep
                    for record_mode, phase, rep, record_arm in records
                    if record_mode == mode and phase == "warm" and record_arm == arm
                }
                if reps != {1, 2, 3, 4, 5}:
                    raise ContractError(
                        f"isolated warm repetitions are incomplete for {scope}/{mode}/{arm}"
                    )
        expected_keys = {
            (mode, phase, rep, arm)
            for mode, arms in expected_arms.items()
            for arm in arms
            for phase, reps in (("cold", {0}), ("warm", {1, 2, 3, 4, 5}))
            for rep in reps
        }
        if set(records) != expected_keys:
            raise ContractError(f"isolated record inventory mismatch for {scope}")
        merged_rows.extend(row for _, row in records.values())

        if not preflight_candidates:
            raise ContractError(f"isolated scientific preflight is missing for {scope}")
        if any(
            int(_required_number(row, "science_fail")) != 0
            for row in preflight_candidates
        ):
            raise ContractError(f"isolated scientific preflight failed for {scope}")
        if scope_areg[scope]:
            preflight_candidates = [
                row for row in preflight_candidates if _number(row, "areg_bmax") is not None
            ]
            if not preflight_candidates:
                raise ContractError(f"isolated areg preflight is missing for {scope}")
        merged_preflights.append(preflight_candidates[0])

    assert base_environment is not None
    scale_order = [item["id"] for item in manifest["scales"]]
    environment_candidates = [
        environment
        for pieces in shards.values()
        for environment, _, _, _ in pieces
    ]
    merged_environment = dict(
        max(
            environment_candidates,
            key=lambda value: (
                value.get("areg_scope") == "all",
                sum(key.startswith("dependency_") for key in value),
            ),
        )
    )
    merged_environment["scales"] = " ".join(
        scale for scale in scale_order if any(scope[0].endswith(f"_{scale}") for scope in shards)
    )
    merged_environment["threads"] = " ".join(str(value) for value in sorted({scope[1] for scope in shards}))
    merged_environment["warm_repetitions"] = "5"
    if all(scope_areg.values()):
        merged_environment["areg_scope"] = "all"
    elif any(scope_areg.values()):
        merged_environment["areg_scope"] = "selected"
    else:
        merged_environment["areg_scope"] = "off"
    merged_environment["areg_cases"] = " ".join(
        f"{case_id}@{threads}"
        for (case_id, threads), available in sorted(scope_areg.items())
        if available
    )
    merged_environment["areg_unavailable_cases"] = " ".join(
        f"{case_id}@{threads}"
        for case_id, threads in sorted(
            {
                (str(failure["case_id"]), int(failure["threads"]))
                for failure in merged_failures
            }
        )
    )
    design_order = [item["id"] for item in manifest["designs"]]
    merged_environment["designs"] = " ".join(
        design
        for design in design_order
        if any(scope[0].startswith(f"{design}_") for scope in shards)
    )
    merged_environment["rep_offset"] = "merged"
    merged_environment["isolation"] = "fresh_process_component_shards"
    output_dir.mkdir(parents=True, exist_ok=True)
    _write_csv(output_dir / "command_comparison_runs.csv", merged_rows)
    _write_csv(output_dir / "command_comparison_preflights.csv", merged_preflights)
    (output_dir / "failures.json").write_text(
        json.dumps(merged_failures, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output_dir / "environment.txt").write_text(
        "".join(f"{key}={value}\n" for key, value in merged_environment.items()),
        encoding="utf-8",
    )
    return {
        "scopes": len(shards),
        "timing_rows": len(merged_rows),
        "preflights": len(merged_preflights),
        "failures": len(merged_failures),
    }


def _number(row: Mapping[str, str], field: str) -> float | None:
    raw = row.get(field, "")
    if raw in {"", "."} or (raw.startswith(".") and raw[1:].isalpha()):
        return None
    try:
        value = float(raw)
    except ValueError as error:
        raise ContractError(f"malformed numeric field {field}: {raw}") from error
    if not math.isfinite(value):
        raise ContractError(f"nonfinite numeric field {field}")
    return value


def _required_number(row: Mapping[str, str], field: str) -> float:
    value = _number(row, field)
    if value is None:
        raise ContractError(f"missing numeric field {field}")
    return value


def _within(actual: float, expected: float, atol: float, rtol: float) -> bool:
    return abs(actual - expected) <= atol + rtol * max(abs(actual), abs(expected), 1.0)


def _validate_science(
    rows: Sequence[Mapping[str, str]],
    manifest: Mapping[str, Any],
    areg_scope: str,
    areg_cases: set[tuple[str, int]] | None = None,
) -> None:
    gates = manifest["science_gates"]
    scale_n = {item["id"]: item["n"] for item in manifest["scales"]}
    grouped: dict[tuple[str, int, str, str, int], dict[str, Mapping[str, str]]] = {}
    for row in rows:
        if int(_required_number(row, "rc")) != 0:
            raise ContractError(
                f"timed arm failed: {row.get('case_id')}/{row.get('arm')} rc={row.get('rc')}"
            )
        expected_n = scale_n.get(row.get("scale", ""))
        if expected_n is None or int(_required_number(row, "n")) != expected_n:
            raise ContractError(f"timed sample mismatch for {row.get('case_id')}/{row.get('arm')}")
        key = (
            row["case_id"],
            int(row["threads"]),
            row["mode"],
            row["phase"],
            int(row["rep"]),
        )
        if row["arm"] in grouped.setdefault(key, {}):
            raise ContractError("duplicate timed arm record")
        grouped[key][row["arm"]] = row

    for key, arms in grouped.items():
        expected_arms = {"fereg", "greg", "reghdfe"}
        design = next(iter(arms.values()))["design"]
        if key[2] == "fit" and (
            areg_scope == "all"
            or (areg_scope == "one_fe" and design == "one_fe")
            or (areg_scope == "selected" and (key[0], key[1]) in (areg_cases or set()))
        ):
            expected_arms.add("areg")
        if set(arms) != expected_arms:
            raise ContractError(f"timed arm inventory mismatch for {key}")
        reference = arms["reghdfe"]
        for arm_id, row in arms.items():
            for field in ("b1", "b2"):
                if not _within(
                    _required_number(row, field),
                    _required_number(reference, field),
                    float(gates["coefficient_atol"]),
                    float(gates["coefficient_rtol"]),
                ):
                    raise ContractError(f"timed coefficient parity failed for {key}/{arm_id}")
            if arm_id == "greg":
                continue
            for field in ("v11", "v12", "v22"):
                if not _within(
                    _required_number(row, field),
                    _required_number(reference, field),
                    float(gates["vce_atol"]),
                    float(gates["vce_rtol"]),
                ):
                    raise ContractError(f"timed VCE parity failed for {key}/{arm_id}")
            rss = _required_number(row, "rss")
            reference_rss = _required_number(reference, "rss")
            if not _within(rss, reference_rss, 1e-9, float(gates["rss_rtol"])):
                raise ContractError(f"timed RSS parity failed for {key}/{arm_id}")
            if arm_id == "fereg" and _number(row, "df_a") != _number(reference, "df_a"):
                raise ContractError(f"timed absorbed DoF parity failed for {key}/{arm_id}")


def analyze(
    manifest: Mapping[str, Any],
    runs_path: Path,
    preflights_path: Path,
    environment_path: Path,
    identity_path: Path | None = None,
    manifest_path: Path = DEFAULT_MANIFEST,
) -> dict[str, Any]:
    environment = _read_environment(environment_path)
    threads = [int(item) for item in environment["threads"].split()]
    scales = environment["scales"].split()
    designs = environment.get("designs", "all").split()
    if designs == ["all"]:
        designs = [item["id"] for item in manifest["designs"]]
    warm_repetitions = int(environment["warm_repetitions"])
    rows = _read_csv(runs_path)
    preflights = _read_csv(preflights_path)
    areg_scope = environment["areg_scope"]
    areg_cases = {
        (token.rsplit("@", 1)[0], int(token.rsplit("@", 1)[1]))
        for token in environment.get("areg_cases", "").split()
    }
    plan = expand_plan(
        manifest,
        scales=scales,
        designs=designs,
        threads=threads,
        areg_scope=areg_scope,
        areg_cases=areg_cases,
    )
    expected_rows = len(plan) * (1 + warm_repetitions)
    if len(rows) != expected_rows:
        raise ContractError(f"expected {expected_rows} timing rows, observed {len(rows)}")
    expected_preflights = len({(item["case_id"], item["threads"]) for item in plan})
    if len(preflights) != expected_preflights:
        raise ContractError(
            f"expected {expected_preflights} preflights, observed {len(preflights)}"
        )
    if any(int(_required_number(row, "science_fail")) != 0 for row in preflights):
        raise ContractError("a scientific preflight failed")
    _validate_science(rows, manifest, areg_scope, areg_cases)

    grouped: dict[tuple[str, str, str, int, str, str], list[Mapping[str, str]]] = {}
    for row in rows:
        key = (
            row["case_id"],
            row["scale"],
            row["design"],
            int(row["threads"]),
            row["mode"],
            row["arm"],
        )
        grouped.setdefault(key, []).append(row)
    summaries: list[dict[str, Any]] = []
    for key, group in sorted(grouped.items()):
        cold = [_required_number(row, "seconds") for row in group if row["phase"] == "cold"]
        warm = [_required_number(row, "seconds") for row in group if row["phase"] == "warm"]
        if len(cold) != 1 or len(warm) != warm_repetitions:
            raise ContractError(f"repetition cadence mismatch for {key}")
        median = statistics.median(warm)
        mad = statistics.median(abs(value - median) for value in warm)
        exemplar = next(row for row in group if row["phase"] == "warm")
        summaries.append(
            {
                "case_id": key[0],
                "scale": key[1],
                "design": key[2],
                "threads": key[3],
                "mode": key[4],
                "arm": key[5],
                "cold_seconds": cold[0],
                "warm_median_seconds": median,
                "warm_mad_seconds": mad,
                "warm_min_seconds": min(warm),
                "warm_max_seconds": max(warm),
                "used_solver": _number(exemplar, "used_solver"),
                "iterations": _number(exemplar, "iterations"),
                "pilot_iterations": _number(exemplar, "pilot_iterations"),
                "pilot_contraction": _number(exemplar, "pilot_contraction"),
                "fallback": _number(exemplar, "fallback"),
            }
        )

    by_cell = {
        (item["case_id"], item["threads"], item["mode"], item["arm"]): item
        for item in summaries
    }
    ratios: list[dict[str, Any]] = []
    for item in summaries:
        if item["arm"] == "fereg":
            continue
        candidate = by_cell[(item["case_id"], item["threads"], item["mode"], "fereg")]
        ratios.append(
            {
                "case_id": item["case_id"],
                "scale": item["scale"],
                "design": item["design"],
                "threads": item["threads"],
                "mode": item["mode"],
                "comparator": item["arm"],
                "fereg_over_comparator": candidate["warm_median_seconds"]
                / item["warm_median_seconds"],
            }
        )
    overheads: list[dict[str, Any]] = []
    for item in summaries:
        if item["mode"] != "save_fe":
            continue
        fit = by_cell[(item["case_id"], item["threads"], "fit", item["arm"])]
        overheads.append(
            {
                "case_id": item["case_id"],
                "scale": item["scale"],
                "design": item["design"],
                "threads": item["threads"],
                "arm": item["arm"],
                "save_over_fit": item["warm_median_seconds"]
                / fit["warm_median_seconds"],
                "incremental_seconds": item["warm_median_seconds"]
                - fit["warm_median_seconds"],
            }
        )
    aggregate: list[dict[str, Any]] = []
    aggregate_groups: dict[tuple[str, str, int], list[float]] = {}
    for item in ratios:
        aggregate_groups.setdefault(
            (item["mode"], item["comparator"], item["threads"]), []
        ).append(item["fereg_over_comparator"])
    for key, values in sorted(aggregate_groups.items()):
        aggregate.append(
            {
                "mode": key[0],
                "comparator": key[1],
                "threads": key[2],
                "cells": len(values),
                "geometric_mean_fereg_over_comparator": math.exp(
                    statistics.fmean(math.log(value) for value in values)
                ),
                "fereg_faster_cells": sum(value < 1 for value in values),
                "fereg_slower_cells": sum(value > 1 for value in values),
            }
        )
    identity: Any = None
    if identity_path is not None:
        identity = json.loads(identity_path.read_text(encoding="utf-8"))
    failures_path = runs_path.parent / "failures.json"
    failures: list[dict[str, Any]] = []
    if failures_path.exists():
        loaded_failures = json.loads(failures_path.read_text(encoding="utf-8"))
        if not isinstance(loaded_failures, list):
            raise ContractError("merged failures must be a list")
        failures = loaded_failures
    unavailable: list[dict[str, Any]] = []
    for failure in failures:
        fereg = by_cell.get(
            (str(failure["case_id"]), int(failure["threads"]), "fit", "fereg")
        )
        item = dict(failure)
        if fereg is not None:
            item["fereg_warm_median_seconds"] = fereg["warm_median_seconds"]
            item["fereg_over_areg_upper_bound"] = (
                fereg["warm_median_seconds"] / float(failure["elapsed_seconds"])
            )
        unavailable.append(item)
    return {
        "schema_version": 1,
        "kind": "fereg_command_comparison_stata19",
        "qualification_eligible": False,
        "manifest_id": manifest["manifest_id"],
        "manifest_sha256": _sha256(manifest_path),
        "environment": environment,
        "identity": identity,
        "preflights": preflights,
        "summaries": summaries,
        "ratios": ratios,
        "saved_fe_overheads": overheads,
        "aggregate": aggregate,
        "unavailable": unavailable,
        "limitations": manifest["limitations"],
    }


def render_markdown(evidence: Mapping[str, Any]) -> str:
    display_arms = ["fereg", "greg", "reghdfe"]
    has_areg = any(
        item.get("arm") == "areg"
        for collection in (
            evidence.get("summaries", []),
            evidence.get("unavailable", []),
        )
        for item in collection
    )
    if has_areg:
        display_arms.append("areg")
    lines = [
        "# Cross-platform Stata command comparison",
        "",
        "Warm medians; ratios below 1 mean fereg is faster.",
        "",
    ]
    for mode in ("fit", "save_fe"):
        lines.extend(
            [
                f"## {mode}",
                "",
                "| Scale | Design | Threads | "
                + " | ".join("Greg" if arm == "greg" else arm for arm in display_arms)
                + " |",
                "|---|---|---:|" + "---:|" * len(display_arms),
            ]
        )
        cells: dict[tuple[str, str, int], dict[str, float]] = {}
        for item in evidence["summaries"]:
            if item["mode"] != mode:
                continue
            cells.setdefault((item["scale"], item["design"], item["threads"]), {})[
                item["arm"]
            ] = item["warm_median_seconds"]
        unavailable = {
            (item["scale"], item["design"], int(item["threads"]))
            for item in evidence.get("unavailable", [])
            if item["mode"] == mode and item["arm"] == "areg"
        }
        for key, values in sorted(cells.items()):
            formatted = []
            for arm in display_arms:
                if arm in values:
                    formatted.append(f"{values[arm]:.4f}")
                elif arm == "areg" and key in unavailable:
                    formatted.append("DNF")
                else:
                    formatted.append("—")
            lines.append(
                f"| {key[0]} | {key[1]} | {key[2]} | " + " | ".join(formatted) + " |"
            )
        lines.append("")
    lines.extend(
        [
            "## Aggregate fereg/comparator ratios",
            "",
            "| Mode | Comparator | Threads | Cells | Geometric mean | Faster cells | Slower cells |",
            "|---|---|---:|---:|---:|---:|---:|",
        ]
    )
    for item in evidence["aggregate"]:
        lines.append(
            f"| {item['mode']} | {item['comparator']} | {item['threads']} | "
            f"{item['cells']} | {item['geometric_mean_fereg_over_comparator']:.3f} | "
            f"{item['fereg_faster_cells']} | {item['fereg_slower_cells']} |"
        )
    lines.append("")
    fereg_overheads = [
        item for item in evidence.get("saved_fe_overheads", [])
        if item["arm"] == "fereg"
    ]
    if fereg_overheads:
        lines.extend(
            [
                "## fereg saved-FE overhead",
                "",
                "| Scale | Design | Threads | Save/fit | Increment |",
                "|---|---|---:|---:|---:|",
            ]
        )
        for item in sorted(
            fereg_overheads,
            key=lambda value: (value["scale"], value["design"], value["threads"]),
        ):
            lines.append(
                f"| {item['scale']} | {item['design']} | {item['threads']} | "
                f"{item['save_over_fit']:.3f} | {item['incremental_seconds']:.4f}s |"
            )
        lines.append("")
    science_fields = [
        ("fereg slope max", "fereg_bmax"),
        ("fereg VCE max", "fereg_vmax"),
        ("fereg relative RSS max", "fereg_rssrel"),
        ("fereg reconstruction max", "fereg_reconstruct_max"),
        ("reghdfe reconstruction max", "reghdfe_reconstruct_max"),
        ("fereg/reghdfe predictor max", "fereg_reghdfe_predmax"),
    ]
    science_rows = []
    for label, field in science_fields:
        values = [
            value
            for row in evidence.get("preflights", [])
            if (value := _number(row, field)) is not None
        ]
        if values:
            science_rows.append((label, max(values)))
    if science_rows:
        lines.extend(
            [
                "## Independent science checks",
                "",
                "All preflights passed. The table reports the largest observed distance.",
                "",
                "| Check | Maximum |",
                "|---|---:|",
            ]
        )
        for label, value in science_rows:
            lines.append(f"| {label} | {value:.3e} |")
        lines.append("")
    if evidence.get("unavailable"):
        lines.extend(
            [
                "## Resource-limited cells",
                "",
                "| Case | Threads | Arm | Status | Elapsed lower bound | fereg median | fereg/areg upper bound | Peak RSS |",
                "|---|---:|---|---|---:|---:|---:|---:|",
            ]
        )
        for item in evidence["unavailable"]:
            peak = item.get("peak_rss_bytes")
            peak_text = f"{float(peak) / (1024**3):.1f} GiB" if peak else "—"
            fereg = item.get("fereg_warm_median_seconds")
            fereg_text = f"{float(fereg):.4f}s" if fereg is not None else "—"
            upper = item.get("fereg_over_areg_upper_bound")
            upper_text = f"<{float(upper):.4f}" if upper is not None else "—"
            lines.append(
                f"| {item['case_id']} | {item['threads']} | {item['arm']} | "
                f"{item['status']} | >={float(item['elapsed_seconds']):.0f}s | "
                f"{fereg_text} | {upper_text} | {peak_text} |"
            )
        lines.append("")
    if evidence.get("limitations"):
        lines.extend(["## Registered limitations", ""])
        for key, value in evidence["limitations"].items():
            if not has_areg and key.startswith("areg_"):
                continue
            lines.append(f"- `{key}`: {value}")
        lines.append("")
    return "\n".join(lines)


def run_component(
    command: Sequence[str],
    failure_path: Path,
    *,
    case_id: str,
    scale: str,
    design: str,
    threads: int,
    arm: str,
    mode: str,
    timeout_seconds: float,
) -> int:
    if not command:
        raise ContractError("component command is required")
    failure_path.unlink(missing_ok=True)
    started = time.monotonic()
    process = subprocess.Popen(list(command), start_new_session=True)
    try:
        return process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        elapsed = time.monotonic() - started
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        failure_path.parent.mkdir(parents=True, exist_ok=True)
        _write_json(
            failure_path,
            {
                "schema_version": 1,
                "case_id": case_id,
                "scale": scale,
                "design": design,
                "threads": threads,
                "arm": arm,
                "mode": mode,
                "status": "time_limit",
                "elapsed_seconds": elapsed,
                "timeout_seconds": timeout_seconds,
            },
        )
        return 124


def _write_json(path: Path, document: Any) -> None:
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    subparsers = parser.add_subparsers(dest="action", required=True)
    plan_parser = subparsers.add_parser("plan")
    plan_parser.add_argument("--scale", action="append", dest="scales")
    plan_parser.add_argument("--threads", type=int, action="append")
    plan_parser.add_argument("--output", type=Path)
    analyze_parser = subparsers.add_parser("analyze")
    analyze_parser.add_argument("--runs", type=Path, required=True)
    analyze_parser.add_argument("--preflights", type=Path, required=True)
    analyze_parser.add_argument("--environment", type=Path, required=True)
    analyze_parser.add_argument("--identity", type=Path)
    analyze_parser.add_argument("--output", type=Path, required=True)
    analyze_parser.add_argument("--markdown", type=Path, required=True)
    merge_parser = subparsers.add_parser("merge")
    merge_parser.add_argument("--input", type=Path, action="append", required=True)
    merge_parser.add_argument("--output-dir", type=Path, required=True)
    run_parser = subparsers.add_parser("run-component")
    run_parser.add_argument("--failure-json", type=Path, required=True)
    run_parser.add_argument("--case-id", required=True)
    run_parser.add_argument("--scale", required=True)
    run_parser.add_argument("--design", required=True)
    run_parser.add_argument("--threads", type=int, required=True)
    run_parser.add_argument("--arm", default="areg")
    run_parser.add_argument("--mode", default="fit")
    run_parser.add_argument("--timeout-seconds", type=float, required=True)
    run_parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    try:
        manifest = load_manifest(args.manifest)
        if args.action == "run-component":
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            return run_component(
                command,
                args.failure_json,
                case_id=args.case_id,
                scale=args.scale,
                design=args.design,
                threads=args.threads,
                arm=args.arm,
                mode=args.mode,
                timeout_seconds=args.timeout_seconds,
            )
        if args.action == "plan":
            document: Any = expand_plan(
                manifest, scales=args.scales, threads=args.threads
            )
            if args.output:
                _write_json(args.output, document)
            else:
                print(json.dumps(document, indent=2, sort_keys=True))
            return 0
        if args.action == "merge":
            document = merge_isolated_shards(manifest, args.input, args.output_dir)
            print(json.dumps(document, indent=2, sort_keys=True))
            return 0
        evidence = analyze(
            manifest,
            args.runs,
            args.preflights,
            args.environment,
            args.identity,
            args.manifest,
        )
        _write_json(args.output, evidence)
        args.markdown.write_text(render_markdown(evidence), encoding="utf-8")
        return 0
    except (ContractError, OSError, json.JSONDecodeError) as error:
        print(f"command_comparison: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
