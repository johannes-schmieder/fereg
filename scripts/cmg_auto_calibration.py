#!/usr/bin/env python3
"""Validate and analyze the frozen CMG automatic-routing calibration."""
from __future__ import annotations

import argparse
import csv
import json
from collections import defaultdict
from pathlib import Path
import statistics
from typing import Any, Iterable, Mapping


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/ols_cmg_auto_calibration_v1.json"


class ContractError(RuntimeError):
    """The CMG calibration input or acceptance contract failed closed."""


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to load CMG calibration manifest: {error}") from error
    if document.get("schema_version") != 1 or document.get("manifest_id") != "ols_cmg_auto_calibration_v1":
        raise ContractError("CMG calibration manifest identity is not frozen")
    rule = document.get("frozen_selection_rule")
    expected_rule = {
        "minimum_observations_per_high_level": 8,
        "minimum_high_to_second_level_ratio": 10,
        "maximum_nuisance_nonbase_columns": 0,
        "absorbed_slopes_allowed": False,
    }
    if rule != expected_rule:
        raise ContractError("CMG automatic selection rule differs from production v1")
    acceptance = document.get("acceptance", {})
    if acceptance.get("maximum_selected_case_slowdown_fraction") != 0.05:
        raise ContractError("CMG selected-case slowdown gate is not frozen at five percent")
    if not document.get("setup_cost_included") or not document.get("scientific_certificate_required"):
        raise ContractError("CMG calibration must include setup and require certification")
    return document


def selects_cmg(row: Mapping[str, Any]) -> bool:
    observations = int(row["observations"])
    high_levels = int(row["high_levels"])
    second_levels = int(row["second_levels"])
    nuisance = int(row["nuisance_columns"])
    slopes = str(row.get("absorbed_slopes", "0")).lower() in {"1", "true", "yes"}
    if min(observations, high_levels, second_levels) <= 0 or nuisance < 0:
        raise ContractError("CMG calibration topology dimensions are invalid")
    return (
        not slopes
        and nuisance == 0
        and observations >= 8 * high_levels
        and high_levels >= 10 * second_levels
    )


def analyze_rows(rows: Iterable[Mapping[str, Any]]) -> dict[str, Any]:
    grouped: dict[tuple[str, int], dict[str, list[float]]] = defaultdict(
        lambda: defaultdict(list)
    )
    topology: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        case_id = str(row["case_id"])
        threads = int(row["threads"])
        solver = str(row["solver"])
        seconds = float(row["seconds"])
        if solver not in {"cmg", "lsmr"} or threads <= 0 or seconds <= 0:
            raise ContractError("CMG calibration measurement is malformed")
        grouped[(case_id, threads)][solver].append(seconds)
        topology.setdefault(case_id, row)
    selected = []
    rejected = []
    total_cmg = 0.0
    total_lsmr = 0.0
    for (case_id, threads), arms in sorted(grouped.items()):
        if set(arms) != {"cmg", "lsmr"}:
            raise ContractError(f"case {case_id}/{threads} is missing a solver arm")
        cmg = statistics.median(arms["cmg"])
        lsmr = statistics.median(arms["lsmr"])
        record = {
            "case_id": case_id,
            "threads": threads,
            "cmg_seconds": cmg,
            "lsmr_seconds": lsmr,
            "cmg_to_lsmr": cmg / lsmr,
        }
        if selects_cmg(topology[case_id]):
            if cmg > lsmr * 1.05:
                raise ContractError(
                    f"selected CMG case {case_id}/{threads} is more than five percent slower"
                )
            selected.append(record)
            total_cmg += cmg
            total_lsmr += lsmr
        else:
            rejected.append(record)
    if not selected:
        raise ContractError("CMG calibration contains no selected topology")
    if total_cmg >= total_lsmr:
        raise ContractError("selected CMG aggregate is not faster than explicit LSMR")
    return {
        "selected": selected,
        "rejected": rejected,
        "selected_total_cmg_seconds": total_cmg,
        "selected_total_lsmr_seconds": total_lsmr,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--results", type=Path)
    arguments = parser.parse_args()
    load_manifest(arguments.manifest)
    if arguments.results is None:
        print(json.dumps({"manifest": str(arguments.manifest), "status": "valid"}, indent=2))
        return 0
    try:
        with arguments.results.open(newline="", encoding="utf-8") as source:
            report = analyze_rows(csv.DictReader(source))
    except OSError as error:
        raise ContractError(f"unable to read CMG calibration results: {error}") from error
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
