#!/usr/bin/env python3
"""Validate, merge, and summarize the expanded Stata 19 command matrix."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = (
    ROOT / "benchmarks/manifests/ols_command_comparison_stata19_v2.json"
)
RUNS_NAME = "command_comparison_v2_runs.csv"
PREFLIGHTS_NAME = "command_comparison_v2_preflights.csv"


class ContractError(RuntimeError):
    """The V2 benchmark contract or its evidence failed closed."""


def _index(values: Any, where: str) -> dict[str, Mapping[str, Any]]:
    if not isinstance(values, list):
        raise ContractError(f"{where} inventory must be a list")
    output: dict[str, Mapping[str, Any]] = {}
    for value in values:
        if not isinstance(value, dict) or not isinstance(value.get("id"), str):
            raise ContractError(f"malformed {where}")
        if value["id"] in output:
            raise ContractError(f"duplicate {where}: {value['id']}")
        output[value["id"]] = value
    return output


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read V2 comparison manifest: {error}") from error
    required = {
        "schema_version", "manifest_id", "status", "minimum_stata_version",
        "scientific_contract", "dependency_contract", "generator_contract",
        "scales", "designs", "control_counts", "cases", "requested_threads",
        "arms", "output_modes", "repetitions", "timing_contract",
        "science_gates", "limitations",
    }
    if not isinstance(document, dict) or set(document) != required:
        raise ContractError("V2 comparison manifest fields are not frozen")
    if (
        document["schema_version"] != 2
        or document["manifest_id"] != "ols_command_comparison_stata19_v2"
        or document["status"] != "development_only"
        or document["minimum_stata_version"] != 19
    ):
        raise ContractError("V2 comparison manifest identity is invalid")
    if document["dependency_contract"] != {
        "reghdfe": {
            "version": "6.14.1",
            "commit": "29b2b203f534413369d675d0c0f674d61291f536",
        },
        "gtools": {"version": "1.11.8"},
    }:
        raise ContractError("V2 comparison dependency contract is not frozen")
    scales = _index(document["scales"], "scale")
    designs = _index(document["designs"], "design")
    arms = _index(document["arms"], "arm")
    modes = _index(document["output_modes"], "output mode")
    if set(scales) != {"n1m", "n10m"}:
        raise ContractError("V2 comparison scales are incomplete")
    if set(designs) != {
        "one_fe", "akm_standard", "akm_lowmob",
        "akm_standard_year", "akm_lowmob_year",
    }:
        raise ContractError("V2 comparison designs are incomplete")
    if set(arms) != {"fereg", "greg", "reghdfe"}:
        raise ContractError("V2 comparison arms are incomplete")
    if set(modes) != {"fit", "save_fe"} or any(
        mode["arms"] != ["fereg", "greg", "reghdfe"] for mode in modes.values()
    ):
        raise ContractError("V2 output capabilities are not frozen")
    if document["control_counts"] != [0, 2, 10, 30]:
        raise ContractError("V2 control-count inventory is not frozen")
    if document["requested_threads"] != [1, 2, 4, 8, 16]:
        raise ContractError("V2 requested-thread inventory is not frozen")
    if document["repetitions"] != {"cold": 1, "warm": 5}:
        raise ContractError("V2 repetition cadence is not frozen")
    seen: set[tuple[str, str, int, int]] = set()
    for case in document["cases"]:
        if set(case) != {"scale", "design", "controls", "threads"}:
            raise ContractError("V2 case fields are malformed")
        if case["scale"] not in scales or case["design"] not in designs:
            raise ContractError("V2 case references an unknown scale or design")
        if not set(case["controls"]) <= set(document["control_counts"]):
            raise ContractError("V2 case references an unknown control count")
        if not set(case["threads"]) <= set(document["requested_threads"]):
            raise ContractError("V2 case references an unknown thread count")
        for controls in case["controls"]:
            for threads in case["threads"]:
                key = (case["scale"], case["design"], controls, threads)
                if key in seen:
                    raise ContractError(f"duplicate V2 benchmark scope: {key}")
                seen.add(key)
    if len(seen) != 116:
        raise ContractError("V2 benchmark scope inventory must contain 116 cells")
    return document


def expand_scopes(
    manifest: Mapping[str, Any],
    *,
    scales: Sequence[str] | None = None,
    designs: Sequence[str] | None = None,
    controls: Sequence[int] | None = None,
    threads: Sequence[int] | None = None,
) -> list[dict[str, Any]]:
    scale_filter = set(scales or [item["id"] for item in manifest["scales"]])
    design_filter = set(designs or [item["id"] for item in manifest["designs"]])
    control_filter = set(controls or manifest["control_counts"])
    thread_filter = set(threads or manifest["requested_threads"])
    scale_map = {item["id"]: item for item in manifest["scales"]}
    design_map = {item["id"]: item for item in manifest["designs"]}
    if scale_filter - set(scale_map):
        raise ContractError("unknown selected V2 scale")
    if design_filter - set(design_map):
        raise ContractError("unknown selected V2 design")
    if control_filter - set(manifest["control_counts"]):
        raise ContractError("unknown selected V2 control count")
    if thread_filter - set(manifest["requested_threads"]):
        raise ContractError("unknown selected V2 thread count")
    output = []
    for case in manifest["cases"]:
        if case["scale"] not in scale_filter or case["design"] not in design_filter:
            continue
        for control_count in case["controls"]:
            if control_count not in control_filter:
                continue
            for requested_threads in case["threads"]:
                if requested_threads not in thread_filter:
                    continue
                design = dict(design_map[case["design"]])
                scale = dict(scale_map[case["scale"]])
                output.append({
                    "case_id": (
                        f"{design['id']}_{scale['id']}_k{control_count}"
                    ),
                    "scale": scale,
                    "design": design,
                    "controls": control_count,
                    "requested_threads": requested_threads,
                })
    return output


def expand_plan(manifest: Mapping[str, Any], **selection: Any) -> list[dict[str, Any]]:
    output = []
    for scope in expand_scopes(manifest, **selection):
        for mode in manifest["output_modes"]:
            for arm in mode["arms"]:
                output.append({**scope, "mode": mode["id"], "arm": arm})
    return output


def _read_environment(path: Path) -> dict[str, str]:
    output: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line:
            raise ContractError("malformed V2 environment record")
        key, value = line.split("=", 1)
        output[key] = value
    required = {
        "stata_version", "stata_flavor", "os", "machine_type",
        "processors_lic", "processors_mach", "requested_threads",
        "stata_processors", "scale", "design", "controls",
        "warm_repetitions", "dependency_fereg", "dependency_greg",
        "dependency_gregress", "dependency_reghdfe",
    }
    if not required <= set(output):
        raise ContractError("V2 environment record is incomplete")
    if float(output["stata_version"]) < 19:
        raise ContractError("V2 comparison requires Stata 19+")
    return output


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8-sig") as source:
        return list(csv.DictReader(source))


def _write_csv(path: Path, rows: Sequence[Mapping[str, str]]) -> None:
    if not rows:
        raise ContractError(f"cannot write empty V2 CSV: {path}")
    fields = list(rows[0])
    if any(list(row) != fields for row in rows):
        raise ContractError("V2 CSV fields are inconsistent")
    with path.open("w", newline="", encoding="utf-8") as destination:
        writer = csv.DictWriter(destination, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def merge_shards(
    manifest: Mapping[str, Any], input_dirs: Sequence[Path], output_dir: Path
) -> dict[str, int]:
    if not input_dirs:
        raise ContractError("at least one V2 shard is required")
    all_rows: list[dict[str, str]] = []
    all_preflights: list[dict[str, str]] = []
    environments: list[dict[str, str]] = []
    scope_keys: set[tuple[str, int]] = set()
    row_keys: set[tuple[str, int, str, str, str, int]] = set()
    invariant_fields = {
        "stata_version", "stata_flavor", "os", "machine_type",
        "processors_lic", "processors_mach", "dependency_fereg",
        "dependency_greg", "dependency_gregress", "dependency_reghdfe",
    }
    invariants: dict[str, str] = {}
    for directory in input_dirs:
        environment = _read_environment(directory / "environment.txt")
        environments.append(environment)
        for field in invariant_fields:
            value = environment[field]
            if field in invariants and invariants[field] != value:
                raise ContractError(f"V2 shard environment mismatch: {field}")
            invariants[field] = value
        rows = _read_csv(directory / RUNS_NAME)
        preflights = _read_csv(directory / PREFLIGHTS_NAME)
        if len(preflights) != 1:
            raise ContractError("each V2 shard must contain one scientific preflight")
        scope = (preflights[0]["case_id"], int(preflights[0]["requested_threads"]))
        if scope in scope_keys:
            raise ContractError(f"duplicate V2 shard scope: {scope}")
        scope_keys.add(scope)
        expected = {
            (mode, phase, rep, arm)
            for mode in ("fit", "save_fe")
            for phase, repetitions in (("cold", {0}), ("warm", {1, 2, 3, 4, 5}))
            for rep in repetitions
            for arm in ("fereg", "greg", "reghdfe")
        }
        observed = {
            (row["mode"], row["phase"], int(row["rep"]), row["arm"])
            for row in rows
        }
        if observed != expected or len(rows) != len(expected):
            raise ContractError(f"V2 shard cadence is incomplete for {scope}")
        for row in rows:
            key = (
                row["case_id"], int(row["requested_threads"]), row["mode"],
                row["phase"], row["arm"], int(row["rep"]),
            )
            if key in row_keys:
                raise ContractError("duplicate V2 timing row")
            row_keys.add(key)
        all_rows.extend(rows)
        all_preflights.extend(preflights)
    expected_scopes = {
        (scope["case_id"], scope["requested_threads"])
        for scope in expand_scopes(manifest)
    }
    if scope_keys != expected_scopes:
        missing = sorted(expected_scopes - scope_keys)[:5]
        extra = sorted(scope_keys - expected_scopes)[:5]
        raise ContractError(f"V2 merged scope mismatch; missing={missing}, extra={extra}")
    output_dir.mkdir(parents=True, exist_ok=True)
    all_rows.sort(key=lambda row: (
        row["scale"], row["design"], int(row["controls"]),
        int(row["requested_threads"]), row["mode"], row["phase"],
        int(row["rep"]), row["arm"],
    ))
    all_preflights.sort(key=lambda row: (
        row["scale"], row["design"], int(row["controls"]),
        int(row["requested_threads"]),
    ))
    _write_csv(output_dir / RUNS_NAME, all_rows)
    _write_csv(output_dir / PREFLIGHTS_NAME, all_preflights)
    merged_environment = dict(environments[0])
    merged_environment.update({
        "requested_threads": "1 2 4 8 16",
        "stata_processors": "varies_by_license_cap",
        "scale": "n1m n10m",
        "design": " ".join(item["id"] for item in manifest["designs"]),
        "controls": "0 2 10 30",
        "warm_repetitions": "5",
        "shards": str(len(input_dirs)),
    })
    (output_dir / "environment.txt").write_text(
        "".join(f"{key}={value}\n" for key, value in merged_environment.items()),
        encoding="utf-8",
    )
    return {
        "scopes": len(scope_keys),
        "timing_rows": len(all_rows),
        "preflights": len(all_preflights),
    }


def _number(row: Mapping[str, str], field: str) -> float | None:
    value = row.get(field, "")
    if value in {"", "."}:
        return None
    try:
        number = float(value)
    except ValueError as error:
        raise ContractError(f"invalid numeric V2 field {field}") from error
    if not math.isfinite(number):
        raise ContractError(f"nonfinite numeric V2 field {field}")
    return number


def _required_number(row: Mapping[str, str], field: str) -> float:
    value = _number(row, field)
    if value is None:
        raise ContractError(f"missing numeric V2 field {field}")
    return value


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def analyze(
    manifest: Mapping[str, Any], runs_path: Path, preflights_path: Path,
    environment_path: Path, manifest_path: Path = DEFAULT_MANIFEST,
) -> dict[str, Any]:
    rows = _read_csv(runs_path)
    preflights = _read_csv(preflights_path)
    expected_scopes = expand_scopes(manifest)
    expected_rows = len(expected_scopes) * 2 * 3 * 6
    if len(rows) != expected_rows or len(preflights) != len(expected_scopes):
        raise ContractError(
            f"V2 evidence inventory mismatch: rows={len(rows)}/{expected_rows}, "
            f"preflights={len(preflights)}/{len(expected_scopes)}"
        )
    if any(int(_required_number(row, "science_fail")) != 0 for row in preflights):
        raise ContractError("a V2 scientific preflight failed")
    scale_n = {item["id"]: item["n"] for item in manifest["scales"]}
    for row in rows:
        if int(_required_number(row, "rc")) != 0:
            raise ContractError(f"V2 timed command failed: {row['case_id']}/{row['arm']}")
        if int(_required_number(row, "n")) != scale_n[row["scale"]]:
            raise ContractError(f"V2 timed sample mismatch: {row['case_id']}/{row['arm']}")
        if row["arm"] == "fereg":
            effective = int(_required_number(row, "fereg_effective_threads"))
            if effective != int(row["requested_threads"]):
                raise ContractError(f"fereg did not honor V2 native threads: {row['case_id']}")

    grouped: dict[tuple[str, str, str, int, int, str, str], list[dict[str, str]]] = {}
    for row in rows:
        key = (
            row["case_id"], row["scale"], row["design"], int(row["controls"]),
            int(row["requested_threads"]), row["mode"], row["arm"],
        )
        grouped.setdefault(key, []).append(row)
    summaries = []
    for key, group in sorted(grouped.items()):
        cold = [_required_number(row, "seconds") for row in group if row["phase"] == "cold"]
        warm = [_required_number(row, "seconds") for row in group if row["phase"] == "warm"]
        if len(cold) != 1 or len(warm) != 5:
            raise ContractError(f"V2 repetition cadence mismatch: {key}")
        median = statistics.median(warm)
        exemplar = next(row for row in group if row["phase"] == "warm")
        summaries.append({
            "case_id": key[0], "scale": key[1], "design": key[2],
            "controls": key[3], "requested_threads": key[4],
            "mode": key[5], "arm": key[6], "cold_seconds": cold[0],
            "warm_median_seconds": median,
            "warm_mad_seconds": statistics.median(abs(value - median) for value in warm),
            "warm_min_seconds": min(warm), "warm_max_seconds": max(warm),
            "stata_processors": int(_required_number(exemplar, "stata_processors")),
            "fereg_effective_threads": _number(exemplar, "fereg_effective_threads"),
            "used_solver": _number(exemplar, "used_solver"),
            "iterations": _number(exemplar, "iterations"),
            "fallback": _number(exemplar, "fallback"),
        })
    by_cell = {
        (item["case_id"], item["requested_threads"], item["mode"], item["arm"]): item
        for item in summaries
    }
    ratios = []
    overheads = []
    scaling = []
    for item in summaries:
        if item["arm"] != "fereg":
            candidate = by_cell[(
                item["case_id"], item["requested_threads"], item["mode"], "fereg"
            )]
            ratios.append({
                "case_id": item["case_id"], "scale": item["scale"],
                "design": item["design"], "controls": item["controls"],
                "requested_threads": item["requested_threads"], "mode": item["mode"],
                "comparator": item["arm"],
                "fereg_over_comparator": (
                    candidate["warm_median_seconds"] / item["warm_median_seconds"]
                ),
            })
        if item["mode"] == "save_fe":
            fit = by_cell[(
                item["case_id"], item["requested_threads"], "fit", item["arm"]
            )]
            overheads.append({
                "case_id": item["case_id"], "scale": item["scale"],
                "design": item["design"], "controls": item["controls"],
                "requested_threads": item["requested_threads"], "arm": item["arm"],
                "save_over_fit": item["warm_median_seconds"] / fit["warm_median_seconds"],
                "incremental_seconds": item["warm_median_seconds"] - fit["warm_median_seconds"],
            })
        single = by_cell.get((item["case_id"], 1, item["mode"], item["arm"]))
        if single is not None:
            scaling.append({
                "case_id": item["case_id"], "scale": item["scale"],
                "design": item["design"], "controls": item["controls"],
                "requested_threads": item["requested_threads"], "mode": item["mode"],
                "arm": item["arm"],
                "speedup_over_thread1": (
                    single["warm_median_seconds"] / item["warm_median_seconds"]
                ),
            })
    aggregate = []
    buckets: dict[tuple[str, str, int], list[float]] = {}
    for item in ratios:
        buckets.setdefault((
            item["mode"], item["comparator"], item["requested_threads"]
        ), []).append(item["fereg_over_comparator"])
    for key, values in sorted(buckets.items()):
        aggregate.append({
            "mode": key[0], "comparator": key[1], "requested_threads": key[2],
            "cells": len(values),
            "geometric_mean_fereg_over_comparator": math.exp(
                statistics.fmean(math.log(value) for value in values)
            ),
            "fereg_faster_cells": sum(value < 1 for value in values),
            "fereg_slower_cells": sum(value > 1 for value in values),
        })
    return {
        "schema_version": 2,
        "kind": "fereg_command_comparison_stata19_expanded",
        "qualification_eligible": False,
        "manifest_id": manifest["manifest_id"],
        "manifest_sha256": _sha256(manifest_path),
        "environment": _read_environment(environment_path),
        "preflights": preflights,
        "summaries": summaries,
        "ratios": ratios,
        "saved_fe_overheads": overheads,
        "scaling": scaling,
        "aggregate": aggregate,
        "limitations": manifest["limitations"],
    }


def render_markdown(evidence: Mapping[str, Any]) -> str:
    lines = [
        "# Expanded fereg command comparison", "",
        "Warm medians. Requested threads are native fereg threads; the recorded Stata "
        "processor count is license/machine capped for Stata-side comparators.", "",
    ]
    for mode in ("fit", "save_fe"):
        lines.extend([
            f"## {mode}", "",
            "| Scale | Design | K | Requested | Stata | fereg | Greg | reghdfe |",
            "|---|---|---:|---:|---:|---:|---:|---:|",
        ])
        cells: dict[tuple[str, str, int, int, int], dict[str, float]] = {}
        for item in evidence["summaries"]:
            if item["mode"] != mode:
                continue
            key = (
                item["scale"], item["design"], item["controls"],
                item["requested_threads"], item["stata_processors"],
            )
            cells.setdefault(key, {})[item["arm"]] = item["warm_median_seconds"]
        for key, values in sorted(cells.items()):
            lines.append(
                f"| {key[0]} | {key[1]} | {key[2]} | {key[3]} | {key[4]} | "
                f"{values['fereg']:.4f} | {values['greg']:.4f} | {values['reghdfe']:.4f} |"
            )
        lines.append("")
    lines.extend([
        "## Aggregate fereg/comparator ratios", "",
        "| Mode | Comparator | Requested | Cells | Geometric mean | Faster | Slower |",
        "|---|---|---:|---:|---:|---:|---:|",
    ])
    for item in evidence["aggregate"]:
        lines.append(
            f"| {item['mode']} | {item['comparator']} | {item['requested_threads']} | "
            f"{item['cells']} | {item['geometric_mean_fereg_over_comparator']:.3f} | "
            f"{item['fereg_faster_cells']} | {item['fereg_slower_cells']} |"
        )
    lines.extend([
        "", "## fereg saved-FE overhead", "",
        "| Scale | Design | K | Requested | Save/fit | Increment |",
        "|---|---|---:|---:|---:|---:|",
    ])
    for item in sorted(
        (value for value in evidence["saved_fe_overheads"] if value["arm"] == "fereg"),
        key=lambda value: (
            value["scale"], value["design"], value["controls"],
            value["requested_threads"],
        ),
    ):
        lines.append(
            f"| {item['scale']} | {item['design']} | {item['controls']} | "
            f"{item['requested_threads']} | {item['save_over_fit']:.3f} | "
            f"{item['incremental_seconds']:.4f}s |"
        )
    lines.extend(["", "## Registered limitations", ""])
    for key, value in evidence.get("limitations", {}).items():
        lines.append(f"- `{key}`: {value}")
    lines.append("")
    return "\n".join(lines)


def _write_json(path: Path, document: Any) -> None:
    path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    actions = parser.add_subparsers(dest="action", required=True)
    plan = actions.add_parser("plan")
    plan.add_argument("--scopes-only", action="store_true")
    plan.add_argument("--output", type=Path)
    merge = actions.add_parser("merge")
    merge.add_argument("--input", type=Path, action="append", required=True)
    merge.add_argument("--output-dir", type=Path, required=True)
    analyze_parser = actions.add_parser("analyze")
    analyze_parser.add_argument("--runs", type=Path, required=True)
    analyze_parser.add_argument("--preflights", type=Path, required=True)
    analyze_parser.add_argument("--environment", type=Path, required=True)
    analyze_parser.add_argument("--output", type=Path, required=True)
    analyze_parser.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        manifest = load_manifest(args.manifest)
        if args.action == "plan":
            document: Any = (
                expand_scopes(manifest) if args.scopes_only else expand_plan(manifest)
            )
            if args.output:
                _write_json(args.output, document)
            else:
                print(json.dumps(document, indent=2, sort_keys=True))
            return 0
        if args.action == "merge":
            print(json.dumps(merge_shards(manifest, args.input, args.output_dir), sort_keys=True))
            return 0
        evidence = analyze(
            manifest, args.runs, args.preflights, args.environment, args.manifest
        )
        _write_json(args.output, evidence)
        args.markdown.write_text(render_markdown(evidence), encoding="utf-8")
        return 0
    except (ContractError, OSError, json.JSONDecodeError) as error:
        print(f"command_comparison_v2: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
