#!/usr/bin/env python3
"""Conclude direct-terminal CMG routing with reliable solver comparisons.

The calibration population reuses the generated designs selected by the locked
direct-terminal cost guard.  The fresh holdout uses new seeds and graph shapes.
Every dataset is generated once, all arms receive an unrecorded warm-up, solver
order rotates, and each solver cell has exactly three recorded repetitions.
This is focused development evidence, not qualification.
"""

from __future__ import annotations

import argparse
from collections import Counter
import csv
import hashlib
import math
from pathlib import Path
import statistics
from typing import Mapping, Sequence

import cmg_direct_terminal_calibration as direct
import cmg_hierarchy_guard_calibration as calibration


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    ROOT
    / "benchmarks/reports/development/cmg-direct-terminal-conclusion-v1"
)
PRIOR_OUTPUT = (
    ROOT
    / "benchmarks/reports/development/cmg-direct-terminal-calibration-v1"
)
HOLDOUT_SEED = 20261141


CMG = calibration.CMG
CG = calibration.Arm("cg", "cg", "none")
ARMS = (CMG, CG)


def calibration_case_ids() -> set[str]:
    with (PRIOR_OUTPUT / "calibration" / "cells.csv").open(
        newline="", encoding="utf-8"
    ) as handle:
        rows = list(csv.DictReader(handle))
    return {
        row["case_id"]
        for row in rows
        if row["terminal"] == "Direct" and row["candidate_reject"] == "1"
    }


def calibration_plan() -> list[calibration.Case]:
    wanted = calibration_case_ids()
    plan = [
        case
        for case in direct.build_plan("calibration")
        if case.case_id in wanted
    ]
    actual = {case.case_id for case in plan}
    if actual != wanted:
        raise RuntimeError(
            f"prior calibration cases no longer reproduce: {sorted(wanted - actual)}"
        )
    return plan


def holdout_specs() -> list[direct.GraphSpec]:
    return [
        direct.GraphSpec("dtf_p3_c650_s4", 3, 99_000, 650, stride=4),
        direct.GraphSpec("dtf_p3_c685_s2", 3, 99_000, 685, stride=2),
        direct.GraphSpec("dtf_p4_c420_s3", 4, 96_000, 420, stride=3),
        direct.GraphSpec("dtf_p5_c260_s1", 5, 100_000, 260, stride=1),
        direct.GraphSpec("dtf_p6_c180_s3", 6, 96_000, 180, stride=3),
        direct.GraphSpec("dtf_p7_c140_s2", 7, 98_000, 140, stride=2),
        direct.GraphSpec(
            "dtf_p5_c250_bal", 5, 100_000, 250, pattern="balanced"
        ),
        direct.GraphSpec(
            "dtf_p4_c420_s3_y5", 4, 96_000, 420, stride=3, year_levels=5
        ),
        direct.GraphSpec(
            "dtf_p5_c260_s1_y10", 5, 100_000, 260, stride=1, year_levels=10
        ),
    ]


def holdout_plan() -> list[calibration.Case]:
    cases: list[calibration.Case] = []
    for index, spec in enumerate(holdout_specs()):
        seed = HOLDOUT_SEED + index
        for controls in (0, 4, 16):
            for threads in (1, 8):
                cases.append(
                    direct.make_case(
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


def build_feature_row(
    case: calibration.Case,
    profile: Mapping[str, object],
) -> dict[str, object]:
    row: dict[str, object] = {
        "case_id": case.case_id,
        "seed": case.seed,
        "n": case.n,
        "periods": case.periods,
        "workers": case.workers,
        "firms": case.firms,
        "pattern": case.pattern,
        "target_components": case.components,
        "largest_share": case.largest_share,
        "year_levels": case.year_levels,
        "nuisance_columns": max(0, case.year_levels - 1),
        "controls": case.controls,
        "rhs": case.rhs,
        "threads": case.threads,
    }
    row.update(profile)
    reject, reason = direct.candidate_decision(row)
    recursive_work = float(row["recursive_work"])
    levels = int(row["levels"])
    rhs = case.rhs
    threads = case.threads
    nuisance = int(row["nuisance_columns"])
    row.update(
        {
            "cost_guard_reject": int(reject),
            "cost_guard_reason": reason,
            "depth_work": levels * recursive_work / case.n,
            "rhs_batches": rhs / min(rhs, threads),
            "nuisance_work": (
                recursive_work * nuisance / min(nuisance, threads)
                if nuisance
                else 0.0
            ),
        }
    )
    return row


def candidate_route(row: Mapping[str, object]) -> str:
    """Locked primary rule; see the report-root locked-candidate note."""
    if int(row["cost_guard_reject"]) == 0:
        return "cmg"
    depth_work = float(row["depth_work"])
    nuisance_columns = int(row["nuisance_columns"])
    threads = int(row["threads"])
    rhs = int(row["rhs"])
    one_effective_batch = threads == 1 or rhs <= threads
    if (depth_work > 200.0 or nuisance_columns >= 9) and one_effective_batch:
        return "cg"
    return "cmg"


def extended_candidate_route(row: Mapping[str, object]) -> str:
    """Secondary calibration candidate, locked but not preferred."""
    primary = candidate_route(row)
    if primary == "cg" or int(row["cost_guard_reject"]) == 0:
        return primary
    threads = int(row["threads"])
    rhs = int(row["rhs"])
    recursive_work = float(row["recursive_work"])
    terminal_factor = int(row["terminal_factor_dimension"] or 0)
    if (
        threads == 1
        and rhs >= 17
        and recursive_work > 500_000.0
        and (terminal_factor == 0 or terminal_factor >= 100)
    ):
        return "cg"
    return "cmg"


def analyze(population: str, output: Path, plan: Sequence[calibration.Case]) -> None:
    directory = output / population
    raw = read_csv(directory / "focused_runs.csv")
    expected = len(plan) * len(ARMS) * calibration.REPETITIONS
    if len(raw) != expected:
        raise RuntimeError(f"expected {expected} recorded runs, found {len(raw)}")
    counts = Counter((row["case_id"], row["arm_id"]) for row in raw)
    if set(counts.values()) != {calibration.REPETITIONS}:
        raise RuntimeError("every solver cell must have exactly three repetitions")

    summary = calibration.summarize_focused(raw)
    write_csv(directory / "median_summary.csv", summary)
    summaries = {(row["case_id"], row["arm_id"]): row for row in summary}
    profiles = {
        row["case_id"]: row
        for row in read_csv(directory / "focused_hierarchy_profiles.csv")
    }

    cells: list[dict[str, object]] = []
    winners: Counter[str] = Counter()
    routes: Counter[str] = Counter()
    fallbacks: Counter[str] = Counter()
    regrets: list[float] = []
    extended_regrets: list[float] = []
    extended_routes: Counter[str] = Counter()
    route_failures = 0
    extended_route_failures = 0
    for case in plan:
        feature = build_feature_row(case, profiles[case.case_id])
        eligible: dict[str, float] = {}
        for arm in ARMS:
            result = summaries[(case.case_id, arm.arm_id)]
            feature[f"{arm.arm_id}_eligible"] = result["eligible"]
            feature[f"{arm.arm_id}_seconds"] = result["median_command_seconds"]
            feature[f"{arm.arm_id}_iterations"] = result["median_iterations"]
            feature[f"{arm.arm_id}_fallback"] = result["median_fallback"]
            if number(result, "eligible") == 1:
                eligible[arm.arm_id] = float(result["median_command_seconds"])
            if float(result["median_fallback"]) > 0:
                fallbacks[arm.arm_id] += 1
        if "cmg" not in eligible or "cg" not in eligible:
            raise RuntimeError(f"required reliable comparator failed: {case.case_id}")
        winner = min(eligible, key=eligible.get)
        winners[winner] += 1
        route = candidate_route(feature)
        routes[route] += 1
        if route not in eligible:
            route_failures += 1
            regret: float | str = ""
        else:
            regret = eligible[route] / min(eligible.values())
            regrets.append(regret)
        extended_route = extended_candidate_route(feature)
        extended_routes[extended_route] += 1
        if extended_route not in eligible:
            extended_route_failures += 1
            extended_regret: float | str = ""
        else:
            extended_regret = eligible[extended_route] / min(eligible.values())
            extended_regrets.append(extended_regret)
        feature.update(
            {
                "best_eligible": winner,
                "candidate_route": route,
                "candidate_regret": regret,
                "extended_candidate_route": extended_route,
                "extended_candidate_regret": extended_regret,
            }
        )
        cells.append(feature)

    write_csv(directory / "cells.csv", cells)
    severe = sum(value > 2.0 for value in regrets)
    above_25 = sum(value > 1.25 for value in regrets)
    extended_severe = sum(value > 2.0 for value in extended_regrets)
    extended_above_25 = sum(value > 1.25 for value in extended_regrets)
    report = [
        f"# Direct-terminal conclusion: {population}",
        "",
        "This is focused development evidence, not qualification.",
        "",
        "Schwarz-LSMR at 1e-11 was removed after a normal five-RHS, one-thread calibration cell exceeded the bounded diagnostic runtime. The final comparison therefore contains only the two reliable explicit destinations, CMG and direct CG.",
        "",
        f"- Datasets: {len(plan)}; recorded runs: {len(raw)}; exactly three repetitions per solver cell.",
        f"- Winner counts: {dict(sorted(winners.items()))}.",
        f"- Fallback cells by arm: {dict(sorted(fallbacks.items())) or '{}'}.",
        f"- Provisional candidate route counts: {dict(sorted(routes.items()))}.",
        f"- Candidate ineligible destinations: {route_failures}; regrets above 1.25: {above_25}; regrets above 2.0: {severe}.",
        (
            f"- Candidate median regret: {statistics.median(regrets):.3f}; "
            f"maximum regret: {max(regrets):.3f}."
            if regrets
            else "- Candidate produced no eligible routed cells."
        ),
        f"- Secondary extended route counts: {dict(sorted(extended_routes.items()))}; ineligible destinations: {extended_route_failures}; regrets above 1.25: {extended_above_25}; regrets above 2.0: {extended_severe}; maximum regret: {max(extended_regrets):.3f}.",
        f"- Diagnostic plugin SHA-256: `{hashlib.sha256((ROOT / 'dist/fereg_macos.plugin').read_bytes()).hexdigest()}`.",
        "",
    ]
    (directory / "report.md").write_text("\n".join(report), encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=calibration.DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--population", choices=("calibration", "holdout"), required=True
    )
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    parser.add_argument("--list-plan", action="store_true")
    args = parser.parse_args(argv)

    plan = calibration_plan() if args.population == "calibration" else holdout_plan()
    if args.list_plan:
        for case in plan:
            print(case)
        print(
            f"cases={len(plan)} recorded_runs="
            f"{len(plan) * len(ARMS) * calibration.REPETITIONS}"
        )
        return 0

    output = args.output.resolve()
    directory = output / args.population
    directory.mkdir(parents=True, exist_ok=True)
    if not args.analyze_only:
        if not args.stata.is_file():
            raise RuntimeError(f"Stata executable not found: {args.stata}")
        calibration.ARMS = ARMS
        calibration.run_focused(plan, args.stata, directory, args.resume)
    analyze(args.population, output, plan)
    print(f"report={directory / 'report.md'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
