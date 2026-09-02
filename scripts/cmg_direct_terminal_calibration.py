#!/usr/bin/env python3
"""Calibrate an auto-only cost guard for direct-terminal CMG hierarchies.

This development benchmark crosses direct-terminal worker--firm graph shapes
with 0/4/16 controls and 1/8 threads.  Each dataset is generated once, all
solver arms see identical data, warm-ups are unrecorded, solver order rotates,
and every solver cell has exactly three recorded repetitions.  Calibration
and fresh holdout populations are deliberately separate.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import hashlib
import math
from pathlib import Path
import statistics
from typing import Mapping, Sequence

import cmg_hierarchy_guard_calibration as calibration


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    ROOT / "benchmarks/reports/development/cmg-direct-terminal-calibration-v1"
)
CALIBRATION_SEED = 20260941
HOLDOUT_SEED = 20261041

# Locked before running the fresh holdout.  The work units come directly from
# the already-constructed hierarchy; no component pass or timing probe is
# required by the candidate policy.
MAX_DEPTH_WORK_PER_OBSERVATION = 100.0
MAX_NUISANCE_WORK = 300_000.0
MAX_EFFECTIVE_RHS_BATCHES = 5.0
MIN_WIDE_FACTOR_RHS_BATCHES = 2.0
MAX_WIDE_FACTOR_WORK = 750_000.0


@dataclass(frozen=True)
class GraphSpec:
    graph_id: str
    periods: int
    n: int
    components: int
    pattern: str = "one_large"
    stride: int = 1
    year_levels: int = 0


def make_case(
    spec: GraphSpec,
    *,
    controls: int,
    threads: int,
    seed: int,
) -> calibration.Case:
    workers = spec.n // spec.periods
    firms = workers // 10
    case = calibration.Case(
        n=spec.n,
        periods=spec.periods,
        workers=workers,
        firms=firms,
        pattern=spec.pattern,
        components=spec.components,
        large_components=(
            spec.components if spec.pattern == "balanced" else 1
        ),
        year_levels=spec.year_levels,
        controls=controls,
        threads=threads,
        seed=seed,
        large_cohort_stride=(0 if spec.pattern == "balanced" else spec.stride),
        label=f"{spec.graph_id}_k{controls}_t{threads}",
    )
    calibration.verify_case(case)
    return case


def calibration_specs() -> list[GraphSpec]:
    return [
        GraphSpec("dtc_p3_c620_s1", 3, 99_000, 620, stride=1),
        GraphSpec("dtc_p3_c660_s2", 3, 99_000, 660, stride=2),
        GraphSpec("dtc_p3_c690_s3", 3, 99_000, 690, stride=3),
        GraphSpec("dtc_p3n49_c500_s0", 3, 49_500, 500, stride=0),
        GraphSpec("dtc_p4_c360_s2", 4, 96_000, 360, stride=2),
        GraphSpec("dtc_p4_c480_s1", 4, 96_000, 480, stride=1),
        GraphSpec("dtc_p5_c240_s2", 5, 100_000, 240, stride=2),
        GraphSpec("dtc_p5_c400_bal", 5, 100_000, 400, pattern="balanced"),
        # A 96k single-component version exceeded three minutes in the
        # unrecorded 17-RHS/one-thread warm-up.  Keep the connected stress
        # represented at 24k so the development matrix remains bounded.
        GraphSpec("dtc_p4n24_c001_bal", 4, 24_000, 1, pattern="balanced"),
        GraphSpec("dtc_p3_c620_s1_y5", 3, 99_000, 620, stride=1, year_levels=5),
        GraphSpec("dtc_p4_c480_s1_y10", 4, 96_000, 480, stride=1, year_levels=10),
        GraphSpec("dtc_p5_c240_s2_y5", 5, 100_000, 240, stride=2, year_levels=5),
        GraphSpec(
            "dtc_p5_c400_bal_y10",
            5,
            100_000,
            400,
            pattern="balanced",
            year_levels=10,
        ),
        GraphSpec(
            "dtc_p4n24_c001_bal_y5",
            4,
            24_000,
            1,
            pattern="balanced",
            year_levels=5,
        ),
    ]


def holdout_specs() -> list[GraphSpec]:
    return [
        GraphSpec("dth_p3_c640_s3", 3, 99_000, 640, stride=3),
        GraphSpec("dth_p3_c675_s1", 3, 99_000, 675, stride=1),
        GraphSpec("dth_p4_c450_s2", 4, 96_000, 450, stride=2),
        GraphSpec("dth_p5_c300_s3", 5, 100_000, 300, stride=3),
        GraphSpec("dth_p6_c200_s2", 6, 96_000, 200, stride=2),
        GraphSpec("dth_p8_c120_s1", 8, 96_000, 120, stride=1),
        GraphSpec("dth_p6_c200_bal", 6, 96_000, 200, pattern="balanced"),
        GraphSpec("dth_p4_c450_s2_y5", 4, 96_000, 450, stride=2, year_levels=5),
    ]


def build_plan(population: str) -> list[calibration.Case]:
    if population == "calibration":
        specs = calibration_specs()
        first_seed = CALIBRATION_SEED
    elif population == "holdout":
        specs = holdout_specs()
        first_seed = HOLDOUT_SEED
    else:
        raise ValueError(f"unknown population: {population}")
    cases: list[calibration.Case] = []
    for index, spec in enumerate(specs):
        # The graph is deterministic conditional on GraphSpec.  Holding the
        # seed fixed within a graph also keeps FE/noise draws comparable across
        # the RHS/thread sensitivity cells.
        seed = first_seed + index
        for controls in (0, 4, 16):
            for threads in (1, 8):
                cases.append(
                    make_case(
                        spec,
                        controls=controls,
                        threads=threads,
                        seed=seed,
                    )
                )
    return cases


def number(row: Mapping[str, object], field: str) -> float | None:
    value = row.get(field, "")
    if value in {"", ".", None}:
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    return result if math.isfinite(result) else None


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    if not rows:
        raise RuntimeError(f"refusing to write empty CSV: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=list(rows[0]),
            extrasaction="ignore",
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(rows)


def candidate_decision(row: Mapping[str, object]) -> tuple[bool, str]:
    n = int(row["n"])
    recursive_work = float(row["recursive_work"])
    levels = int(row["levels"])
    nuisance_columns = int(row["nuisance_columns"])
    rhs = int(row["rhs"])
    threads = int(row["threads"])
    terminal_factor_dimension = int(row["terminal_factor_dimension"] or 0)
    depth_work = levels * recursive_work / n
    nuisance_work = (
        recursive_work
        * nuisance_columns
        / min(nuisance_columns, threads)
        if nuisance_columns
        else 0.0
    )
    rhs_batches = rhs / min(rhs, threads)
    wide_factor_work = recursive_work * rhs_batches
    if depth_work > MAX_DEPTH_WORK_PER_OBSERVATION:
        return True, "depth_work"
    if nuisance_work > MAX_NUISANCE_WORK:
        return True, "nuisance_work"
    if rhs_batches >= MAX_EFFECTIVE_RHS_BATCHES:
        return True, "rhs_batches"
    if (
        rhs_batches >= MIN_WIDE_FACTOR_RHS_BATCHES
        and terminal_factor_dimension > 0
        and wide_factor_work > MAX_WIDE_FACTOR_WORK
    ):
        return True, "wide_factor_work"
    return False, "retain_cmg"


def evaluate_candidate(
    population: str, rows: list[dict[str, object]]
) -> dict[str, object]:
    direct = [
        row
        for row in rows
        if row["terminal"] == "Direct"
        and number(row, "cmg_eligible") == 1
    ]
    comparable = [row for row in direct if number(row, "lsmr_eligible") == 1]
    regrets: list[float] = []
    false_rejects = severe_accepts = harmful_accepts = rejected = 0
    rejected_without_certified_lsmr = 0
    reason_counts: dict[str, int] = {}
    for row in direct:
        reject, reason = candidate_decision(row)
        reason_counts[reason] = reason_counts.get(reason, 0) + 1
        row["candidate_reject"] = int(reject)
        row["candidate_reason"] = reason
        rejected += int(reject)
        if number(row, "lsmr_eligible") != 1:
            rejected_without_certified_lsmr += int(reject)
            row["candidate_route_seconds"] = ""
            row["candidate_regret"] = ""
            continue
        cmg = float(row["cmg_seconds"])
        lsmr = float(row["lsmr_seconds"])
        discarded_setup = float(row.get("graph_seconds") or 0) + float(
            row.get("hierarchy_seconds") or 0
        )
        route = lsmr + discarded_setup if reject else cmg
        regret = route / min(cmg, lsmr)
        regrets.append(regret)
        false_rejects += int(reject and cmg < 0.95 * lsmr)
        harmful_accepts += int(not reject and cmg > 1.05 * lsmr)
        severe_accepts += int(not reject and cmg > 2.0 * lsmr)
        row["candidate_route_seconds"] = route
        row["candidate_regret"] = regret
    ordered = sorted(regrets)
    return {
        "population": population,
        "direct_cmg_cells": len(direct),
        "comparable_direct_cells": len(comparable),
        "rejected_to_lsmr": rejected,
        "retained_cmg": len(direct) - rejected,
        "lsmr_ineligible_cells": len(direct) - len(comparable),
        "rejected_without_certified_lsmr": rejected_without_certified_lsmr,
        "false_rejected_cmg_wins_over_5pct": false_rejects,
        "harmful_accepts_over_5pct": harmful_accepts,
        "severe_accepts_over_2x": severe_accepts,
        "geometric_mean_regret": math.exp(
            statistics.fmean(math.log(value) for value in regrets)
        ),
        "p90_regret": ordered[math.ceil(0.90 * len(ordered)) - 1],
        "maximum_regret": max(regrets),
        **{f"reason_{reason}": count for reason, count in sorted(reason_counts.items())},
    }


def analyze_population(population: str, output: Path) -> list[dict[str, object]]:
    directory = output / population
    raw = read_csv(directory / "focused_runs.csv")
    summary = calibration.summarize_focused(raw)
    write_csv(directory / "median_summary.csv", summary)
    profiles = read_csv(directory / "focused_hierarchy_profiles.csv")
    cells = calibration.focused_cells(summary, profiles)
    for row in cells:
        row["population"] = population
        cmg = number(row, "cmg_seconds")
        lsmr = number(row, "lsmr_seconds")
        row["cmg_over_lsmr"] = cmg / lsmr if cmg and lsmr else ""
        row["rhs_per_thread"] = int(row["rhs"]) / int(row["threads"])
        finest = number(row, "finest_work")
        recursive = number(row, "recursive_work")
        factor_nonzeros = number(row, "terminal_factor_nonzeros")
        n = number(row, "n")
        row["recursive_work_per_observation"] = (
            recursive / n if recursive is not None and n else ""
        )
        row["factor_nonzeros_per_observation"] = (
            factor_nonzeros / n if factor_nonzeros is not None and n else ""
        )
        row["estimated_parallel_rhs_batches"] = math.ceil(
            int(row["rhs"]) / int(row["threads"])
        )
        row["parallel_recursive_work_per_observation"] = (
            recursive
            * int(row["estimated_parallel_rhs_batches"])
            / n
            if recursive is not None and n
            else ""
        )
        row["finest_work_per_observation"] = (
            finest / n if finest is not None and n else ""
        )
    evaluation = evaluate_candidate(population, cells)
    write_csv(directory / "cells.csv", cells)
    write_csv(directory / "candidate_evaluation.csv", [evaluation])

    eligible = [
        row
        for row in cells
        if number(row, "cmg_eligible") == 1
        and number(row, "lsmr_eligible") == 1
    ]
    direct = [row for row in eligible if row["terminal"] == "Direct"]
    run_count = len(raw)
    successful = sum(number(row, "rc") == 0 for row in raw)
    certified = sum(number(row, "certificate_passed") == 1 for row in raw)
    fallbacks = sum(number(row, "fallback") == 1 for row in raw)
    ratios = [float(row["cmg_over_lsmr"]) for row in direct]
    plugin_sha = hashlib.sha256((ROOT / "dist/fereg_macos.plugin").read_bytes()).hexdigest()
    report = [
        f"# Direct-terminal CMG {population}",
        "",
        "This is development evidence, not qualification.",
        "",
        f"- Diagnostic plugin SHA-256: `{plugin_sha}`.",
        f"- {len(cells)} datasets and {run_count} recorded runs; every solver cell has exactly three repetitions.",
        "- Each generated dataset was shared by auto, explicit CMG, and explicit Schwarz-LSMR; warm-ups and trace calls were unrecorded and solver order rotated.",
        f"- Successful runs: {successful}/{run_count}; certified runs: {certified}/{successful}; fallbacks: {fallbacks}.",
        f"- Certified comparable direct-terminal cells: {len(direct)}/{len(eligible)}.",
        f"- Locked candidate: rejected {evaluation['rejected_to_lsmr']}, including {evaluation['rejected_without_certified_lsmr']} cells without a certified requested-tolerance LSMR comparator; among comparable cells it false-rejected {evaluation['false_rejected_cmg_wins_over_5pct']} CMG wins above 5%, retained {evaluation['severe_accepts_over_2x']} greater-than-twofold CMG losses, and had maximum regret {float(evaluation['maximum_regret']):.3f}.",
    ]
    if ratios:
        report.extend(
            [
                f"- CMG/LSMR median ratio: min={min(ratios):.3f}, median={statistics.median(ratios):.3f}, max={max(ratios):.3f}.",
                "",
            ]
        )
    (directory / "report.md").write_text("\n".join(report), encoding="utf-8")
    if population == "calibration":
        prior = read_csv(
            ROOT
            / "benchmarks/reports/development/cmg-hierarchy-guard-calibration-v1"
            / "guard_calibration_cells.csv"
        )
        combined = [
            row
            for row in prior
            if row["terminal"] == "Direct"
            and number(row, "cmg_eligible") == 1
            and number(row, "lsmr_eligible") == 1
        ] + cells
        combined_evaluation = evaluate_candidate("prior_plus_calibration", combined)
        write_csv(output / "locked_candidate_evaluation.csv", [combined_evaluation])
        locked = [
            "# Locked direct-terminal CMG candidate",
            "",
            "Locked before running the fresh holdout. This is development evidence, not qualification.",
            "",
            f"- Reject when hierarchy levels times repeat-weighted work per observation exceeds {MAX_DEPTH_WORK_PER_OBSERVATION:g}.",
            f"- Reject when repeat-weighted work times effective nuisance batches exceeds {MAX_NUISANCE_WORK:g}.",
            f"- Reject at {MAX_EFFECTIVE_RHS_BATCHES:g} or more effective RHS batches.",
            f"- With a nonempty terminal factor, reject at {MIN_WIDE_FACTOR_RHS_BATCHES:g} or more RHS batches when repeat-weighted batch work exceeds {MAX_WIDE_FACTOR_WORK:g}.",
            "- A planned rejection pays the already-spent graph and hierarchy setup, then starts certified direct Schwarz-LSMR. It is not a fallback.",
            "",
            f"Prior plus new calibration cells: {combined_evaluation['comparable_direct_cells']}; rejected: {combined_evaluation['rejected_to_lsmr']}; false-rejected CMG wins above 5%: {combined_evaluation['false_rejected_cmg_wins_over_5pct']}; severe accepts above 2x: {combined_evaluation['severe_accepts_over_2x']}; maximum regret: {float(combined_evaluation['maximum_regret']):.3f}.",
            "",
        ]
        (output / "locked_candidate.md").write_text(
            "\n".join(locked), encoding="utf-8"
        )
    return cells


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=calibration.DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--population",
        choices=("calibration", "holdout"),
        required=True,
    )
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    parser.add_argument("--list-plan", action="store_true")
    args = parser.parse_args(argv)
    plan = build_plan(args.population)
    if args.list_plan:
        for case in plan:
            print(case)
        print(f"cases={len(plan)} recorded_runs={len(plan) * 9}")
        return 0

    output = args.output.resolve()
    directory = output / args.population
    directory.mkdir(parents=True, exist_ok=True)
    if not args.analyze_only:
        if not args.stata.is_file():
            raise RuntimeError(f"Stata executable not found: {args.stata}")
        if not (ROOT / "dist/fereg_macos.plugin").is_file():
            raise RuntimeError("diagnostic plugin is missing")
        calibration.run_focused(plan, args.stata, directory, args.resume)
    analyze_population(args.population, output)
    print(f"report={directory / 'report.md'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
