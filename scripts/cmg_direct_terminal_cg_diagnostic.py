#!/usr/bin/env python3
"""Run CG and tighter LSMR where requested-tolerance LSMR fell back.

The datasets and seeds match the direct-terminal holdout.  Both arms receive
an unrecorded warm-up and exactly three rotated recorded repetitions. This is
a focused development diagnostic, not qualification.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
from collections import Counter
from pathlib import Path
from typing import Sequence

import cmg_direct_terminal_calibration as direct
import cmg_hierarchy_guard_calibration as calibration


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    ROOT
    / "benchmarks/reports/development/cmg-direct-terminal-calibration-v1"
    / "holdout"
    / "cg_diagnostic"
)
CG = calibration.Arm("cg", "cg", "none")


@dataclass(frozen=True)
class TightLsmrArm:
    arm_id: str
    tolerance: float
    solver: str = "lsmr"
    preconditioner: str = "schwarz"

    def command(self, case: calibration.Case) -> str:
        controls = " ".join(f"x{index}" for index in range(1, case.controls + 1))
        lhs = f"y {controls}".strip()
        absorb = "worker firm" + (" year" if case.year_levels else "")
        return (
            f"fereg {lhs}, absorb({absorb}) dof(pairwise) solver(lsmr) "
            f"preconditioner(schwarz) threads({case.threads}) "
            f"tolerance({self.tolerance:.0e}) iterate({calibration.MAX_ITERATIONS})"
        )


LSMR10 = TightLsmrArm("lsmr_schwarz_1e10", 1e-10)


def failed_lsmr_case_ids(holdout: Path) -> set[str]:
    with (holdout / "median_summary.csv").open(
        newline="", encoding="utf-8"
    ) as handle:
        rows = list(csv.DictReader(handle))
    return {
        row["case_id"]
        for row in rows
        if row["arm_id"] == "lsmr_schwarz_1e8" and row["eligible"] != "1"
    }


def analyze(output: Path) -> None:
    holdout = output.parent
    with (holdout / "cells.csv").open(newline="", encoding="utf-8") as handle:
        holdout_cells = {row["case_id"]: row for row in csv.DictReader(handle)}
    with (output / "median_summary.csv").open(
        newline="", encoding="utf-8"
    ) as handle:
        summary = list(csv.DictReader(handle))
    by_case = {
        (row["case_id"], row["arm_id"]): row
        for row in summary
    }
    case_ids = sorted({case_id for case_id, _ in by_case})
    comparisons: list[dict[str, object]] = []
    winners: Counter[str] = Counter()
    for case_id in case_ids:
        base = holdout_cells[case_id]
        cg = by_case[(case_id, "cg")]
        lsmr10 = by_case[(case_id, "lsmr_schwarz_1e10")]
        eligible = {
            "cmg": float(base["cmg_seconds"]),
            "cg": float(cg["median_command_seconds"]),
        }
        if lsmr10["eligible"] == "1":
            eligible["lsmr10"] = float(lsmr10["median_command_seconds"])
        winner = min(eligible, key=eligible.get)
        winners[winner] += 1
        comparisons.append(
            {
                "case_id": case_id,
                "controls": base["controls"],
                "rhs": base["rhs"],
                "threads": base["threads"],
                "hierarchy_levels": base["levels"],
                "work_over_finest": base["work_over_finest"],
                "cmg_seconds": base["cmg_seconds"],
                "lsmr_1e8_fallback_seconds": base["lsmr_seconds"],
                "cg_eligible": cg["eligible"],
                "cg_seconds": cg["median_command_seconds"],
                "cg_iterations": cg["median_iterations"],
                "lsmr_1e10_eligible": lsmr10["eligible"],
                "lsmr_1e10_seconds": lsmr10["median_command_seconds"],
                "lsmr_1e10_iterations": lsmr10["median_iterations"],
                "lsmr_1e10_fallback": lsmr10["median_fallback"],
                "fastest_eligible": winner,
            }
        )
    direct.write_csv(output / "comparison.csv", comparisons)
    cg_cells = [row for row in summary if row["arm_id"] == "cg"]
    lsmr10_cells = [
        row for row in summary if row["arm_id"] == "lsmr_schwarz_1e10"
    ]
    report = [
        "# Direct-terminal LSMR fallback diagnostic",
        "",
        "This is development evidence, not qualification.",
        "",
        f"- {len(case_ids)} cells selected because requested-tolerance Schwarz-LSMR fell back deterministically in the fresh holdout.",
        f"- {len(summary) * calibration.REPETITIONS} recorded runs: CG and Schwarz-LSMR at 1e-10, exactly three repetitions per cell and arm.",
        f"- Explicit CG certified without fallback in {sum(row['eligible'] == '1' for row in cg_cells)}/{len(cg_cells)} cells.",
        f"- Schwarz-LSMR at 1e-10 certified without fallback in {sum(row['eligible'] == '1' for row in lsmr10_cells)}/{len(lsmr10_cells)} cells; it still fell back in {sum(row['eligible'] != '1' for row in lsmr10_cells)}.",
        f"- Fastest eligible arm counts within the subset: CMG={winners['cmg']}, CG={winners['cg']}, tighter LSMR={winners['lsmr10']}.",
        "- A separate unrecorded 1e-12 probe was stopped after one p3/17-RHS/one-thread call exceeded several minutes; over-tightening is not a bounded routing solution.",
        "",
        "Requested-tolerance LSMR stopped after only a few iterations in the probed failure, but its FE normal-equation diagnostic was about 0.0076. The certified CG continuation was scientifically valid but 7--69 times slower than a direct eligible comparator in this subset. These are deterministic certification/stop-criterion mismatches, so fallback cannot be treated as a normal direct-terminal route.",
        "",
    ]
    (output / "report.md").write_text("\n".join(report), encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=calibration.DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    args = parser.parse_args(argv)

    holdout = args.output.resolve().parent
    wanted = failed_lsmr_case_ids(holdout)
    plan = [
        case for case in direct.build_plan("holdout") if case.case_id in wanted
    ]
    if {case.case_id for case in plan} != wanted:
        missing = sorted(wanted - {case.case_id for case in plan})
        raise RuntimeError(f"holdout cases no longer reproduce: {missing}")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if not args.analyze_only:
        calibration.ARMS = (CG, LSMR10)
        calibration.run_focused(plan, args.stata, output, args.resume)
        raw = direct.read_csv(output / "focused_runs.csv")
        summary = calibration.summarize_focused(raw)
        direct.write_csv(output / "median_summary.csv", summary)
        print(f"cases={len(plan)} recorded_runs={len(raw)}")
    analyze(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
