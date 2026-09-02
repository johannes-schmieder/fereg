#!/usr/bin/env python3
"""Replay the locked direct-terminal holdout after the production auto change.

This focused development check runs auto, explicit CMG, and explicit CG on the
same fresh-holdout datasets used to adjudicate the locked routing candidate.
Every dataset is generated once, order rotates, warm-ups are unrecorded, and
each solver cell has exactly three recorded repetitions.
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

import cmg_direct_terminal_conclusion as conclusion
import cmg_hierarchy_guard_calibration as calibration


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    ROOT
    / "benchmarks/reports/development/cmg-direct-terminal-conclusion-v1"
    / "postchange"
)
ARMS = (calibration.AUTO, conclusion.CMG, conclusion.CG)
USED_SOLVERS = {4: "cg", 7: "cmg"}


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


def analyze(output: Path, plan: Sequence[calibration.Case]) -> None:
    raw = read_csv(output / "focused_runs.csv")
    expected = len(plan) * len(ARMS) * calibration.REPETITIONS
    if len(raw) != expected:
        raise RuntimeError(f"expected {expected} recorded runs, found {len(raw)}")
    counts = Counter((row["case_id"], row["arm_id"]) for row in raw)
    if set(counts.values()) != {calibration.REPETITIONS}:
        raise RuntimeError("every solver cell must have exactly three repetitions")

    summary = calibration.summarize_focused(raw)
    write_csv(output / "median_summary.csv", summary)
    summaries = {(row["case_id"], row["arm_id"]): row for row in summary}
    profiles = {
        row["case_id"]: row
        for row in read_csv(output / "focused_hierarchy_profiles.csv")
    }

    cells: list[dict[str, object]] = []
    route_counts: Counter[str] = Counter()
    expected_counts: Counter[str] = Counter()
    winner_counts: Counter[str] = Counter()
    regrets: list[float] = []
    route_mismatches = 0
    fallback_cells = 0
    ineligible_cells = 0
    harmful_cg_selections = 0
    current_seconds = 0.0
    auto_seconds = 0.0
    for case in plan:
        feature = conclusion.build_feature_row(case, profiles[case.case_id])
        expected_route = conclusion.candidate_route(feature)
        expected_counts[expected_route] += 1
        by_arm = {
            arm.arm_id: summaries[(case.case_id, arm.arm_id)] for arm in ARMS
        }
        explicit = {
            arm: float(by_arm[arm]["median_command_seconds"])
            for arm in ("cmg", "cg")
            if number(by_arm[arm], "eligible") == 1
        }
        if set(explicit) != {"cmg", "cg"}:
            raise RuntimeError(f"explicit comparator failed: {case.case_id}")
        winner = min(explicit, key=explicit.get)
        winner_counts[winner] += 1
        auto = by_arm["auto"]
        auto_eligible = number(auto, "eligible") == 1
        if not auto_eligible:
            ineligible_cells += 1
        if number(auto, "median_fallback") != 0:
            fallback_cells += 1
        used_code = number(auto, "median_used_solver_code")
        used_route = USED_SOLVERS.get(int(used_code)) if used_code is not None else None
        if used_route is None:
            raise RuntimeError(
                f"unexpected auto solver code {used_code}: {case.case_id}"
            )
        route_counts[used_route] += 1
        if used_route != expected_route:
            route_mismatches += 1
        auto_runtime = float(auto["median_command_seconds"])
        regret = auto_runtime / explicit[winner] if auto_eligible else math.nan
        if math.isfinite(regret):
            regrets.append(regret)
        if used_route == "cg" and explicit["cmg"] < 0.95 * explicit["cg"]:
            harmful_cg_selections += 1
        current_seconds += explicit["cmg"]
        auto_seconds += auto_runtime
        feature.update(
            {
                "best_explicit": winner,
                "expected_route": expected_route,
                "auto_used_solver": used_route,
                "auto_eligible": int(auto_eligible),
                "auto_fallback": auto["median_fallback"],
                "auto_seconds": auto_runtime,
                "cmg_seconds": explicit["cmg"],
                "cg_seconds": explicit["cg"],
                "auto_regret": regret if math.isfinite(regret) else "",
            }
        )
        cells.append(feature)

    write_csv(output / "cells.csv", cells)
    report = [
        "# Direct-terminal auto guard: post-change replay",
        "",
        "This is focused development evidence, not qualification.",
        "",
        f"- Datasets: {len(plan)}; recorded runs: {len(raw)}; exactly three repetitions per solver cell.",
        f"- Explicit winner counts: {dict(sorted(winner_counts.items()))}.",
        f"- Locked expected routes: {dict(sorted(expected_counts.items()))}; observed auto routes: {dict(sorted(route_counts.items()))}.",
        f"- Route mismatches: {route_mismatches}; ineligible auto cells: {ineligible_cells}; auto fallback cells: {fallback_cells}.",
        f"- Harmful CG selections where explicit CMG was more than 5% faster: {harmful_cg_selections}.",
        f"- Auto regret: median {statistics.median(regrets):.3f}; maximum {max(regrets):.3f}; above 1.25: {sum(value > 1.25 for value in regrets)}; above 2.0: {sum(value > 2.0 for value in regrets)}.",
        f"- Aggregate current-CMG / observed-auto command time: {current_seconds / auto_seconds:.3f}x.",
        f"- Plugin SHA-256: `{hashlib.sha256((ROOT / 'dist/fereg_macos.plugin').read_bytes()).hexdigest()}`.",
        "",
    ]
    (output / "report.md").write_text("\n".join(report), encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=calibration.DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    parser.add_argument(
        "--case-id",
        action="append",
        default=[],
        help="run only the named locked holdout case; may be repeated",
    )
    args = parser.parse_args(argv)

    plan = conclusion.holdout_plan()
    if args.case_id:
        wanted = set(args.case_id)
        plan = [case for case in plan if case.case_id in wanted]
        actual = {case.case_id for case in plan}
        if actual != wanted:
            raise RuntimeError(f"unknown case ids: {sorted(wanted - actual)}")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if not args.analyze_only:
        if not args.stata.is_file():
            raise RuntimeError(f"Stata executable not found: {args.stata}")
        calibration.ARMS = ARMS
        calibration.run_focused(plan, args.stata, output, args.resume)
    analyze(output, plan)
    print(f"report={output / 'report.md'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
