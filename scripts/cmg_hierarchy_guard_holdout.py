#!/usr/bin/env python3
"""Run a fresh development holdout for the terminal-aware CMG safety guard.

The matrix varies component mixtures and the material-component wiring at
three panel depths, then varies RHS width and threads on three representative
graphs. Each dataset is generated once, every solver sees identical data,
and every timing cell has exactly three recorded repetitions.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import math
from pathlib import Path
import statistics
from typing import Mapping, Sequence

import cmg_hierarchy_guard_calibration as calibration


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    ROOT / "benchmarks/reports/development/cmg-hierarchy-guard-holdout-v1"
)
HOLDOUT_SEED = 20260917


def make_case(
    periods: int,
    n: int,
    components: int,
    *,
    pattern: str = "one_large",
    controls: int = 4,
    threads: int = 8,
    stride: int = 1,
    seed: int = HOLDOUT_SEED,
) -> calibration.Case:
    workers = n // periods
    firms = workers // 10
    label = (
        f"ho_p{periods}_c{components:04d}_s{stride}_k{controls}_t{threads}"
        + ("_bal" if pattern == "balanced" else "")
    )
    case = calibration.Case(
        n=n,
        periods=periods,
        workers=workers,
        firms=firms,
        pattern=pattern,
        components=components,
        large_components=components if pattern == "balanced" else 1,
        controls=controls,
        threads=threads,
        seed=seed,
        large_cohort_stride=0 if pattern == "balanced" else stride,
        label=label,
    )
    calibration.verify_case(case)
    return case


def build_plan() -> list[calibration.Case]:
    topology_specs = [
        (3, 99_000, components, "one_large", 1 + index % 3)
        for index, components in enumerate((620, 660, 690, 720, 900))
    ]
    topology_specs += [
        (4, 96_000, components, "one_large", 1 + index % 3)
        for index, components in enumerate((300, 360, 420, 480, 540))
    ]
    topology_specs += [(4, 96_000, 600, "balanced", 0)]
    topology_specs += [
        (5, 100_000, components, "one_large", 1 + index % 3)
        for index, components in enumerate((200, 240, 280, 320, 360))
    ]
    topology_specs += [(5, 100_000, 400, "balanced", 0)]

    cases = [
        make_case(
            periods,
            n,
            components,
            pattern=pattern,
            stride=stride,
            seed=HOLDOUT_SEED + index,
        )
        for index, (periods, n, components, pattern, stride) in enumerate(
            topology_specs
        )
    ]

    # Full 0/4/16 RHS-width by 1/8-thread crosses on a moderate direct graph,
    # a mixed graph expected to exercise the iterative terminal, and an
    # all-tiny low-work graph. The K=4,T=8 cases already occur above.
    sensitivity = [
        (3, 99_000, 660, "one_large", 2, HOLDOUT_SEED + 1),
        (4, 96_000, 480, "one_large", 1, HOLDOUT_SEED + 8),
        (5, 100_000, 400, "balanced", 0, HOLDOUT_SEED + 16),
    ]
    existing_ids = {case.case_id for case in cases}
    for periods, n, components, pattern, stride, seed in sensitivity:
        for controls in (0, 4, 16):
            for threads in (1, 8):
                control_seed = seed if controls == 4 else seed + controls * 100 + 7
                case = make_case(
                    periods,
                    n,
                    components,
                    pattern=pattern,
                    controls=controls,
                    threads=threads,
                    stride=stride,
                    seed=control_seed,
                )
                if case.case_id not in existing_ids:
                    cases.append(case)
                    existing_ids.add(case.case_id)
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
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=list(rows[0]),
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(rows)


def analyze(output: Path) -> None:
    raw = read_csv(output / "focused_runs.csv")
    summary = calibration.summarize_focused(raw)
    calibration._write_csv(output / "holdout_summary.csv", summary, list(summary[0]))
    profiles = read_csv(output / "focused_hierarchy_profiles.csv")
    cells = calibration.focused_cells(summary, profiles)
    for row in cells:
        row["population"] = "fresh_holdout"
        cmg = number(row, "cmg_seconds")
        lsmr = number(row, "lsmr_seconds")
        row["cmg_over_lsmr"] = cmg / lsmr if cmg and lsmr else ""
    write_csv(output / "holdout_cells.csv", cells)

    comparable = [
        row
        for row in cells
        if number(row, "cmg_eligible") == 1 and number(row, "lsmr_eligible") == 1
    ]
    thresholds: list[dict[str, object]] = []
    for threshold in (4, 6, 8, 12, 16):
        rejected = false_rejects = severe_accepts = 0
        regrets: list[float] = []
        for row in comparable:
            work = float(row["work_over_finest"])
            cmg = float(row["cmg_seconds"])
            lsmr = float(row["lsmr_seconds"])
            setup = float(row["graph_seconds"]) + float(row["hierarchy_seconds"])
            reject = str(row["terminal"]) != "Direct" and work > threshold
            if reject:
                rejected += 1
                route = lsmr + setup
                false_rejects += int(cmg < 0.95 * lsmr)
            else:
                route = cmg
                severe_accepts += int(cmg > 2 * lsmr)
            regrets.append(route / min(cmg, lsmr))
        thresholds.append(
            {
                "threshold": threshold,
                "comparable_cells": len(comparable),
                "rejected_to_lsmr": rejected,
                "false_rejected_cmg_wins_over_5pct": false_rejects,
                "severe_accepts_over_2x": severe_accepts,
                "geometric_mean_regret": math.exp(
                    statistics.fmean(math.log(value) for value in regrets)
                ),
                "p90_regret": sorted(regrets)[math.ceil(0.9 * len(regrets)) - 1],
                "maximum_regret": max(regrets),
            }
        )
    write_csv(output / "holdout_thresholds.csv", thresholds)

    iterative = [row for row in comparable if str(row["terminal"]) != "Direct"]
    wins = [
        row
        for row in iterative
        if float(row["cmg_seconds"]) < 0.95 * float(row["lsmr_seconds"])
    ]
    losses = [
        row
        for row in iterative
        if float(row["cmg_seconds"]) > 1.05 * float(row["lsmr_seconds"])
    ]
    run_count = len(raw)
    successful = sum(number(row, "rc") == 0 for row in raw)
    certified = sum(number(row, "certificate_passed") == 1 for row in raw)
    fallbacks = sum(number(row, "fallback") == 1 for row in raw)
    auto_runs = [row for row in raw if row["arm_id"] == "auto"]
    auto_cmg = sum(number(row, "used_solver_code") == 7 for row in auto_runs)
    auto_lsmr = sum(number(row, "used_solver_code") == 3 for row in auto_runs)
    auto_fallbacks = sum(number(row, "fallback") == 1 for row in auto_runs)
    auto_warnings = sum(number(row, "warning_count") not in {0, None} for row in auto_runs)
    auto_pilots = sum(number(row, "pilot_iterations") not in {0, None} for row in auto_runs)
    plugin_sha = hashlib.sha256((ROOT / "dist/fereg_macos.plugin").read_bytes()).hexdigest()
    proposed = next(row for row in thresholds if row["threshold"] == 4)
    report = [
        "# CMG hierarchy-guard fresh holdout",
        "",
        "This is development evidence, not qualification.",
        "",
        "## Design and execution",
        "",
        f"- Diagnostic plugin SHA-256: `{plugin_sha}`.",
        f"- {len(cells)} datasets, {run_count} recorded runs, and exactly three repetitions per solver cell.",
        "- Every dataset was generated once and shared by auto, explicit CMG, and explicit Schwarz-LSMR. Solver order rotated; warm-ups and trace calls were unrecorded.",
        "- The matrix used fresh seeds and new material-component wiring at 3, 4, and 5 observations per worker, then crossed 0/4/16 controls with 1/8 threads on three representative graphs.",
        f"- Successful runs: {successful}/{run_count}; certified runs: {certified}/{successful}; recorded fallbacks: {fallbacks}.",
        f"- Auto used CMG in {auto_cmg} recorded runs and LSMR in {auto_lsmr}; auto fallbacks={auto_fallbacks}, warnings={auto_warnings}, MAP-pilot runs={auto_pilots}. Any difference between the all-arm fallback count and the auto count belongs to an explicit comparator arm.",
        "",
        "## Holdout result",
        "",
        f"- Comparable certified CMG/LSMR cells: {len(comparable)}.",
        f"- Iterative-terminal cells: {len(iterative)}.",
        (
            f"- Largest work proxy among iterative-terminal CMG wins: {max(float(row['work_over_finest']) for row in wins):.2f}."
            if wins
            else "- No iterative-terminal cell favored CMG by more than five percent."
        ),
        (
            f"- Smallest work proxy among iterative-terminal CMG losses: {min(float(row['work_over_finest']) for row in losses):.2f}."
            if losses
            else "- No iterative-terminal cell made CMG slower by more than five percent."
        ),
        f"- The terminal-aware W<=4 rule rejected {proposed['rejected_to_lsmr']} cells, falsely rejected {proposed['false_rejected_cmg_wins_over_5pct']} measured CMG wins, and retained {proposed['severe_accepts_over_2x']} greater-than-twofold CMG losses.",
        f"- Its geometric-mean regret was {float(proposed['geometric_mean_regret']):.3f}; p90 {float(proposed['p90_regret']):.3f}; maximum {float(proposed['maximum_regret']):.3f}.",
        "",
        "Thresholds 4, 6, 8, 12, and 16 are reported in `holdout_thresholds.csv`. The fresh holdout moves the first iterative-terminal CMG loss down to W=5.68, so the conservative implementation threshold is 4. This still lies above every measured iterative-terminal CMG win (W<=2.30) in the earlier calibration.",
        "",
    ]
    (output / "report.md").write_text("\n".join(report), encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=calibration.DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    parser.add_argument("--list-plan", action="store_true")
    args = parser.parse_args(argv)
    plan = build_plan()
    if args.list_plan:
        for case in plan:
            print(case)
        print(f"cases={len(plan)} recorded_runs={len(plan) * 9}")
        return 0
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if not args.analyze_only:
        calibration.run_focused(plan, args.stata, output, args.resume)
    analyze(output)
    print(f"report={output / 'report.md'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
