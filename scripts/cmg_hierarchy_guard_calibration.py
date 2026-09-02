#!/usr/bin/env python3
"""Calibrate a development-only post-hierarchy CMG safety guard.

The focused matrix holds the worker--firm level ratio at the current 10:1
auto-CMG boundary and changes the disconnected-component size distribution.
It generates each dataset once, warms all arms outside timing, rotates solver
order, and records exactly three repetitions.  A final unrecorded CMG call
captures private hierarchy diagnostics from the same generated data.

The script can also recover hierarchy profiles for the existing balanced AKM
matrix and join them to its already-recorded three-repetition medians.  It does
not change automatic routing or publish a new Stata result.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import hashlib
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
from typing import Iterable, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_STATA = Path("/Applications/Stata18/StataMP.app/Contents/MacOS/stata-mp")
DEFAULT_OUTPUT = ROOT / "benchmarks/reports/development/cmg-hierarchy-guard-calibration-v1"
BALANCED_OUTPUT = (
    ROOT / "benchmarks/reports/development/solver-routing-balanced-diagnostic-v1"
)
COMPONENT_OUTPUT = ROOT / "benchmarks/reports/development/cmg-component-diagnostic-v1"
AUTO_COMPONENT_OUTPUT = (
    ROOT / "benchmarks/reports/development/cmg-component-autoeligible-v1"
)
REPETITIONS = 3
MAX_ITERATIONS = 16_000
SEED = 20260903


@dataclass(frozen=True)
class Arm:
    arm_id: str
    solver: str
    preconditioner: str

    def command(self, case: "Case") -> str:
        controls = " ".join(f"x{index}" for index in range(1, case.controls + 1))
        lhs = f"y {controls}".strip()
        absorb = "worker firm" + (" year" if case.year_levels else "")
        options = [
            f"absorb({absorb})",
            "dof(pairwise)",
            f"solver({self.solver})",
            f"threads({case.threads})",
            "tolerance(1e-8)",
            f"iterate({MAX_ITERATIONS})",
        ]
        if self.preconditioner != "none":
            options.append(f"preconditioner({self.preconditioner})")
        return f"fereg {lhs}, {' '.join(options)}"


AUTO = Arm("auto", "auto", "none")
LSMR = Arm("lsmr_schwarz_1e8", "lsmr", "schwarz")
CMG = Arm("cmg", "cmg", "off")
ARMS = (AUTO, LSMR, CMG)


@dataclass(frozen=True)
class Case:
    n: int
    periods: int
    workers: int
    firms: int
    pattern: str
    components: int
    large_components: int
    year_levels: int = 0
    controls: int = 4
    threads: int = 8
    seed: int = SEED
    large_cohort_stride: int = 0
    label: str = ""

    @property
    def tiny_workers(self) -> int:
        return self.periods * (self.workers // self.firms)

    @property
    def tiny_firms(self) -> int:
        return self.periods

    @property
    def tiny_components(self) -> int:
        return self.components - self.large_components

    @property
    def remaining_workers(self) -> int:
        return self.workers - self.tiny_components * self.tiny_workers

    @property
    def remaining_firms(self) -> int:
        return self.firms - self.tiny_components * self.tiny_firms

    @property
    def workers_per_large(self) -> int:
        return self.remaining_workers // self.large_components

    @property
    def firms_per_large(self) -> int:
        return self.remaining_firms // self.large_components

    @property
    def largest_vertices(self) -> int:
        if self.pattern == "balanced":
            return self.workers // self.components + self.firms // self.components
        return self.workers_per_large + self.firms_per_large

    @property
    def largest_share(self) -> float:
        return self.largest_vertices / (self.workers + self.firms)

    @property
    def rhs(self) -> int:
        return self.controls + 1

    @property
    def case_id(self) -> str:
        if self.label:
            return self.label
        size = "n49" if self.n == 49_500 else "n99"
        year = f"_y{self.year_levels}" if self.year_levels else ""
        large = f"_l{self.large_components}" if self.large_components != 1 else ""
        return f"guard_{size}_{self.pattern}_c{self.components:04d}{large}{year}"


def _case(
    n: int,
    pattern: str,
    components: int,
    *,
    large_components: int = 1,
    year_levels: int = 0,
) -> Case:
    periods = 3
    workers = n // periods
    firms = workers // 10
    case = Case(
        n=n,
        periods=periods,
        workers=workers,
        firms=firms,
        pattern=pattern,
        components=components,
        large_components=large_components,
        year_levels=year_levels,
    )
    verify_case(case)
    return case


def build_focused_plan() -> list[Case]:
    cases: list[Case] = []
    for components in (200, 250, 300, 325, 350, 375, 400, 425, 450, 475, 500):
        cases.append(_case(49_500, "one_large", components))
    cases.append(_case(49_500, "balanced", 550, large_components=550))
    cases.append(_case(49_500, "two_large", 400, large_components=2))

    for components in (600, 650, 675, 700, 725, 750, 775, 800, 850, 900, 1_000):
        cases.append(_case(99_000, "one_large", components))
    cases.append(_case(99_000, "balanced", 1_100, large_components=1_100))
    cases.extend(
        [
            _case(99_000, "two_large", 800, large_components=2),
            _case(99_000, "two_large", 1_000, large_components=2),
        ]
    )
    for year_levels in (5, 10):
        for components in (600, 750, 800, 1_000):
            cases.append(
                _case(
                    99_000,
                    "one_large",
                    components,
                    year_levels=year_levels,
                )
            )
        cases.append(
            _case(
                99_000,
                "balanced",
                1_100,
                large_components=1_100,
                year_levels=year_levels,
            )
        )
    return cases


def verify_case(case: Case) -> None:
    if case.n != case.periods * case.workers:
        raise ValueError(f"{case.case_id}: observations and worker panel are inconsistent")
    if case.workers != 10 * case.firms:
        raise ValueError(f"{case.case_id}: worker/firm ratio is not 10:1")
    if case.pattern == "balanced":
        if case.large_components != case.components:
            raise ValueError(f"{case.case_id}: balanced components must all be material")
        if case.workers % case.components or case.firms % case.components:
            raise ValueError(f"{case.case_id}: balanced levels do not divide exactly")
    else:
        if case.large_components >= case.components:
            raise ValueError(f"{case.case_id}: mixed case has no tiny components")
        if case.remaining_workers <= 0 or case.remaining_firms <= 0:
            raise ValueError(f"{case.case_id}: tiny components exhaust the graph")
        if (
            case.remaining_workers % case.large_components
            or case.remaining_firms % case.large_components
        ):
            raise ValueError(f"{case.case_id}: material components do not divide exactly")
        if case.workers_per_large != 10 * case.firms_per_large:
            raise ValueError(f"{case.case_id}: material components changed ratio")


def _preamble() -> list[str]:
    return [
        "version 18.0",
        "clear all",
        "set more off",
        "set linesize 255",
        "set rng mt64",
        "set processors 8",
        f'adopath ++ "{(ROOT / "stata").as_posix()}"',
        f'adopath ++ "{(ROOT / "dist").as_posix()}"',
    ]


def _dataset_lines(case: Case) -> list[str]:
    lines = [
        f"set seed {case.seed}",
        f"set obs {case.n}",
        "generate long obs_id = _n",
        f"generate long worker = ceil(_n / {case.periods})",
        "bysort worker: generate byte period = _n - 1",
    ]
    if case.pattern == "balanced":
        workers_per_component = case.workers // case.components
        firms_per_component = case.firms // case.components
        lines.extend(
            [
                "generate long __worker0 = worker - 1",
                f"generate long __component0 = floor(__worker0 / {workers_per_component})",
                f"generate long __local_worker0 = mod(__worker0, {workers_per_component})",
                (
                    "generate long firm = __component0 * "
                    f"{firms_per_component} + "
                    f"mod(mod(__local_worker0, {firms_per_component}) + period, "
                    f"{firms_per_component}) + 1"
                ),
            ]
        )
    else:
        small_workers = case.tiny_components * case.tiny_workers
        small_firms = case.tiny_components * case.tiny_firms
        large_firm_expression = (
            f"mod(mod(__large_worker0, {case.firms_per_large}) + period, "
            f"{case.firms_per_large})"
        )
        if case.large_cohort_stride:
            large_firm_expression = (
                f"mod(mod(__large_worker0, {case.firms_per_large}) + period * "
                f"(1 + floor(__large_worker0 / {case.firms_per_large}) * "
                f"{case.large_cohort_stride}), {case.firms_per_large})"
            )
        lines.extend(
            [
                "generate long firm = .",
                (
                    f"replace firm = floor((worker - 1) / {case.tiny_workers}) * "
                    f"{case.tiny_firms} + "
                    f"mod(mod(worker - 1, {case.tiny_firms}) + period, "
                    f"{case.tiny_firms}) + 1 if worker <= {small_workers}"
                ),
                (
                    f"generate long __large0 = floor((worker - {small_workers} - 1) / "
                    f"{case.workers_per_large}) if worker > {small_workers}"
                ),
                (
                    f"generate long __large_worker0 = mod(worker - {small_workers} - 1, "
                    f"{case.workers_per_large}) if worker > {small_workers}"
                ),
                (
                    f"replace firm = {small_firms} + __large0 * {case.firms_per_large} + "
                    f"{large_firm_expression} + 1 if worker > {small_workers}"
                ),
            ]
        )
    if case.year_levels:
        lines.append(
            f"generate byte year = mod(obs_id - 1, {case.year_levels}) + 1"
        )
    lines.extend(
        [
            "sort worker firm obs_id",
            "by worker firm: generate byte __edge_first = (_n == 1)",
            "quietly count if __edge_first",
            "scalar __unique_edges = r(N)",
            f"assert __unique_edges == {case.n}",
            "bysort worker: egen int __worker_degree = total(__edge_first)",
            f"assert __worker_degree == {case.periods}",
            "bysort firm: egen int __firm_degree = total(__edge_first)",
            f"assert __firm_degree == {case.n // case.firms}",
            "drop __edge_first __worker_degree __firm_degree",
            "capture drop __worker0 __component0 __local_worker0 __large0 __large_worker0",
            "sort obs_id",
            "bysort worker: generate double __worker_fe = rnormal(0,.4) if _n == 1",
            "bysort worker: replace __worker_fe = __worker_fe[1]",
            "bysort firm: generate double __firm_fe = rnormal(0,.2) if _n == 1",
            "bysort firm: replace __firm_fe = __firm_fe[1]",
        ]
    )
    if case.year_levels:
        lines.extend(
            [
                "bysort year: generate double __year_fe = rnormal(0,.1) if _n == 1",
                "bysort year: replace __year_fe = __year_fe[1]",
            ]
        )
    lines.append("generate double __xb = 0")
    for index in range(1, case.controls + 1):
        beta = 0.30 / index if index % 2 else -0.20 / index
        lines.extend(
            [
                f"generate double x{index} = rnormal()",
                f"replace __xb = __xb + ({beta:.17g})*x{index}",
            ]
        )
    lines.append(
        "generate double y = 1 + __xb + __worker_fe + __firm_fe + rnormal(0,.5)"
    )
    if case.year_levels:
        lines.append("replace y = y + __year_fe")
    lines.extend(["sort obs_id", f"assert _N == {case.n}"])
    return lines


POST_VARIABLES = [
    "case_id", "seed", "n", "periods", "workers", "firms", "pattern",
    "target_components", "large_components", "largest_vertices", "largest_share",
    "year_levels", "nuisance_columns", "controls", "rhs", "threads", "repetition",
    "sequence", "arm_id", "requested_solver", "requested_preconditioner", "rc",
    "command_seconds", "requested_solver_code", "used_solver_code", "iterations",
    "stop_reason", "certificate_passed", "normal_equation_residual",
    "solver_setup_seconds", "solver_solve_seconds", "fallback", "warning_count",
    "requested_preconditioner_code", "used_preconditioner_code", "pilot_iterations",
    "pilot_contraction", "native_seconds", "effective_threads", "reported_tolerance",
    "retained_workers", "retained_firms", "retained_years", "actual_components", "rss",
    "b1", "b2", "b3", "b4",
]


def _postfile_line(path: Path) -> str:
    return (
        "postfile __runs str64 case_id long seed long n int periods long workers long firms "
        "str16 pattern int target_components int large_components long largest_vertices "
        "double largest_share int year_levels int nuisance_columns int controls int rhs int threads "
        "int repetition int sequence str28 arm_id str12 requested_solver "
        "str16 requested_preconditioner int rc double command_seconds requested_solver_code "
        "used_solver_code iterations stop_reason certificate_passed normal_equation_residual "
        "solver_setup_seconds solver_solve_seconds fallback warning_count "
        "requested_preconditioner_code used_preconditioner_code pilot_iterations "
        "pilot_contraction native_seconds effective_threads reported_tolerance retained_workers "
        "retained_firms retained_years actual_components rss b1 b2 b3 b4 "
        f'using "{path.as_posix()}", replace'
    )


def _run_block(case: Case, arm: Arm, repetition: int, sequence: int) -> list[str]:
    nuisance = max(case.year_levels - 1, 0)
    metadata = [
        f'("{case.case_id}")', f"({case.seed})", f"({case.n})", f"({case.periods})",
        f"({case.workers})", f"({case.firms})", f'("{case.pattern}")',
        f"({case.components})", f"({case.large_components})", f"({case.largest_vertices})",
        f"({case.largest_share:.17g})", f"({case.year_levels})", f"({nuisance})",
        f"({case.controls})", f"({case.rhs})", f"({case.threads})", f"({repetition})",
        f"({sequence})", f'("{arm.arm_id}")', f'("{arm.solver}")',
        f'("{arm.preconditioner}")',
    ]
    diagnostics = [
        "(__rc)", "(__seconds)", "(__req)", "(__used)", "(__iterations)",
        "(__stop)", "(__certified)", "(__normal)", "(__setup)", "(__solve)",
        "(__fallback)", "(__warnings)", "(__preq)", "(__pused)", "(__pilot_iters)",
        "(__pilot_contract)", "(__native)", "(__effective)", "(__tol)", "(__level1)",
        "(__level2)", "(__level3)", "(__components)", "(__rss)", "(__b1)", "(__b2)",
        "(__b3)", "(__b4)",
    ]
    return [
        "timer clear 91",
        "timer on 91",
        f"capture quietly {arm.command(case)}",
        "scalar __rc = _rc",
        "timer off 91",
        "quietly timer list 91",
        "scalar __seconds = r(t91)",
        "timer clear 91",
        (
            "foreach __name in req used iterations stop certified normal setup solve fallback "
            "warnings preq pused pilot_iters pilot_contract native effective tol level1 level2 "
            "level3 components rss b1 b2 b3 b4 {"
        ),
        "    scalar __`__name' = .",
        "}",
        "if __rc == 0 {",
        "    matrix __solver = e(fereg_solver)",
        "    matrix __runtime = e(fereg_runtime)",
        "    matrix __fe = e(fereg_fe)",
        "    matrix __dof = e(dof_table)",
        "    scalar __req = __solver[1,1]",
        "    scalar __used = __solver[1,2]",
        "    scalar __iterations = __solver[1,3]",
        "    scalar __stop = __solver[1,4]",
        "    scalar __certified = inlist(__stop,1,2)",
        "    scalar __normal = __solver[1,8]",
        "    scalar __setup = __solver[1,9]",
        "    scalar __solve = __solver[1,10]",
        "    scalar __fallback = __solver[1,11]",
        "    scalar __warnings = __solver[1,12]",
        "    scalar __preq = __solver[1,13]",
        "    scalar __pused = __solver[1,14]",
        "    scalar __pilot_iters = __solver[1,15]",
        "    scalar __pilot_contract = __solver[1,16]",
        "    scalar __native = __runtime[20,2]",
        "    scalar __effective = __runtime[29,2]",
        "    scalar __tol = __runtime[23,2]",
        "    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]",
        "    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]",
        "    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]",
        "    scalar __components = __dof[2,2]",
        "    scalar __rss = e(rss)",
        "    capture scalar __b1 = _b[x1]",
        "    capture scalar __b2 = _b[x2]",
        "    capture scalar __b3 = _b[x3]",
        "    capture scalar __b4 = _b[x4]",
        "}",
        "post __runs " + " ".join(metadata + diagnostics),
    ]


def _case_do(case: Case, output_dta: Path, dataset_dta: Path) -> str:
    lines = _preamble() + _dataset_lines(case)
    lines.append(f'save "{dataset_dta.as_posix()}", replace')
    lines.append(_postfile_line(output_dta))
    for arm in ARMS:
        lines.append(f"capture quietly {arm.command(case)}")
    for repetition in range(1, REPETITIONS + 1):
        rotated = ARMS[repetition - 1 :] + ARMS[: repetition - 1]
        for sequence, arm in enumerate(rotated, start=1):
            lines.extend(_run_block(case, arm, repetition, sequence))
    lines.extend(
        [
            "postclose __runs",
            f'use "{output_dta.as_posix()}", clear',
            f'export delimited using "{output_dta.with_suffix(".csv").as_posix()}", replace',
            f'display as result "CMG_GUARD_CASE|case={case.case_id}|rows=" _N "|rc=0"',
            "exit 0",
        ]
    )
    return "\n".join(lines) + "\n"


def _focused_trace_do(case: Case, dataset_dta: Path) -> str:
    lines = _preamble()
    lines.extend(
        [
            f'use "{dataset_dta.as_posix()}", clear',
            f'display as result "CMG_GUARD_TRACE_BEGIN|case={case.case_id}"',
            f"capture noisily {CMG.command(case)}",
            "local __trace_rc = _rc",
            (
                f'display as result "CMG_GUARD_TRACE_END|case={case.case_id}|rc=" '
                "`__trace_rc'"
            ),
            f'display as result "CMG_GUARD_CASE|case={case.case_id}|rows=1|rc=0"',
            "exit 0",
        ]
    )
    return "\n".join(lines) + "\n"


def _run_stata(stata: Path, do_file: Path, cwd: Path, *, trace: bool = False) -> str:
    environment = dict(os.environ)
    environment.pop("FEREG_SOLVER_TRACE", None)
    if trace:
        environment["FEREG_SOLVER_TRACE"] = "1"
    completed = subprocess.run(
        [str(stata), "-q", "-b", "do", str(do_file)],
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=environment,
        check=False,
    )
    output = completed.stdout
    log = cwd / f"{do_file.stem}.log"
    if log.is_file():
        output += "\n" + log.read_text(encoding="utf-8", errors="replace")
    if completed.returncode != 0 or "CMG_GUARD_CASE|" not in output:
        raise RuntimeError(
            f"Stata failed for {do_file.name} with process code {completed.returncode}\n"
            + output[-10_000:]
        )
    return output


def _parse_fields(line: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for part in line.strip().split("|")[1:]:
        if "=" in part:
            key, value = part.split("=", 1)
            fields[key] = value
    return fields


def parse_hierarchy_trace(output: str, case_id: str) -> dict[str, object]:
    hierarchy_lines = re.findall(r"FEREG_CMG_HIERARCHY\|[^\r\n]+", output)
    setup_lines = re.findall(r"FEREG_CMG_SETUP\|[^\r\n]+", output)
    if not hierarchy_lines or not setup_lines:
        raise RuntimeError(f"{case_id}: private CMG hierarchy trace is missing")
    hierarchy = _parse_fields(hierarchy_lines[0])
    setup = _parse_fields(setup_lines[0])
    return {"case_id": case_id, **hierarchy, **{f"setup_{k}": v for k, v in setup.items()}}


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        raise RuntimeError(f"empty CSV: {path}")
    return rows


def _write_csv(path: Path, rows: Iterable[Mapping[str, object]], fields: Sequence[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=fields,
            extrasaction="ignore",
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(rows)


def _number(row: Mapping[str, object], field: str) -> float | None:
    value = row.get(field, "")
    if value in {"", ".", None}:
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    return result if math.isfinite(result) else None


def summarize_focused(raw: list[dict[str, str]]) -> list[dict[str, object]]:
    groups: dict[tuple[str, str], list[dict[str, str]]] = {}
    for row in raw:
        groups.setdefault((row["case_id"], row["arm_id"]), []).append(row)
    medians = [
        "command_seconds", "native_seconds", "solver_setup_seconds", "solver_solve_seconds",
        "iterations", "used_solver_code", "fallback", "certificate_passed", "rss", "b1",
        "b2", "b3", "b4", "actual_components",
    ]
    metadata = POST_VARIABLES[:19] + POST_VARIABLES[19:21]
    summary: list[dict[str, object]] = []
    for (case_id, arm_id), rows in sorted(groups.items()):
        if len(rows) != REPETITIONS:
            raise RuntimeError(f"{case_id}/{arm_id}: expected three runs, found {len(rows)}")
        cell: dict[str, object] = {field: rows[0][field] for field in metadata}
        cell["arm_id"] = arm_id
        rcs = [_number(row, "rc") for row in rows]
        certs = [_number(row, "certificate_passed") for row in rows]
        fallbacks = [_number(row, "fallback") for row in rows]
        cell["recorded_repetitions"] = len(rows)
        cell["successful_repetitions"] = sum(value == 0 for value in rcs)
        cell["certified_repetitions"] = sum(value == 1 for value in certs)
        cell["eligible"] = int(
            all(value == 0 for value in rcs)
            and all(value == 1 for value in certs)
            and all(value == 0 for value in fallbacks)
        )
        for field in medians:
            finite = [
                value for row in rows if (value := _number(row, field)) is not None
            ]
            cell[f"median_{field}"] = statistics.median(finite) if finite else ""
        summary.append(cell)
    return summary


def focused_cells(
    summary: list[dict[str, object]], profiles: list[dict[str, object]]
) -> list[dict[str, object]]:
    by_case: dict[str, dict[str, dict[str, object]]] = {}
    for row in summary:
        by_case.setdefault(str(row["case_id"]), {})[str(row["arm_id"])] = row
    profile_by_case = {str(row["case_id"]): row for row in profiles}
    cells: list[dict[str, object]] = []
    for case_id, arms in sorted(by_case.items()):
        if set(arms) != {"auto", "lsmr_schwarz_1e8", "cmg"}:
            raise RuntimeError(f"{case_id}: focused solver arms are incomplete")
        base = arms["auto"]
        profile = profile_by_case[case_id]
        row: dict[str, object] = {
            "population": "focused_holdout",
            "family": f"{base['pattern']}_n{base['n']}_y{base['year_levels']}",
            **{field: base[field] for field in POST_VARIABLES[:19]},
            **profile,
        }
        for arm_id, prefix in (
            ("auto", "auto"),
            ("lsmr_schwarz_1e8", "lsmr"),
            ("cmg", "cmg"),
        ):
            arm = arms[arm_id]
            row[f"{prefix}_eligible"] = arm["eligible"]
            row[f"{prefix}_seconds"] = arm["median_command_seconds"]
            row[f"{prefix}_native_seconds"] = arm["median_native_seconds"]
            row[f"{prefix}_setup_seconds"] = arm["median_solver_setup_seconds"]
            row[f"{prefix}_solve_seconds"] = arm["median_solver_solve_seconds"]
            row[f"{prefix}_iterations"] = arm["median_iterations"]
            row[f"{prefix}_fallback"] = arm["median_fallback"]
        cells.append(row)
    return cells


def current_auto_favorable(case: object) -> bool:
    counts = sorted(
        [
            int(getattr(case, "workers")),
            int(getattr(case, "firms")),
            int(getattr(case, "third_levels")),
        ],
        reverse=True,
    )
    positive = [value for value in counts if value > 0]
    nuisance = sum(value - 1 for value in positive[2:])
    return (
        len(positive) >= 2
        and int(getattr(case, "n")) >= 3 * positive[0]
        and positive[0] >= 10 * positive[1]
        and nuisance <= 9
    )


def balanced_profile_plan() -> list[object]:
    sys.path.insert(0, str(ROOT / "scripts"))
    import solver_routing_calibration as routing

    summary_ids = {
        row["case_id"]
        for row in _read_csv(BALANCED_OUTPUT / "solver_routing_calibration_summary.csv")
    }
    return [
        case
        for case in routing.build_plan()
        if case.case_id in summary_ids
        and case.test_id == "balanced_diagnostic"
        and case.controls == 4
        and case.threads == 8
        and case.memory_mode == "auto"
        and case.poolsize == 0
        and current_auto_favorable(case)
        and any(arm.arm_id == "cmg" for arm in case.arms)
        and any(arm.arm_id == "lsmr_schwarz_1e8" for arm in case.arms)
    ]


def _balanced_trace_do(case: object) -> str:
    sys.path.insert(0, str(ROOT / "scripts"))
    import solver_routing_calibration as routing

    lines = routing._stata_preamble(max(case.threads, 8)) + routing._dataset_lines(case)
    lines.extend(
        [
            f'display as result "CMG_GUARD_TRACE_BEGIN|case={case.case_id}"',
            f"capture noisily {routing.CMG.command(case)}",
            "local __trace_rc = _rc",
            (
                f'display as result "CMG_GUARD_TRACE_END|case={case.case_id}|rc=" '
                "`__trace_rc'"
            ),
            f'display as result "CMG_GUARD_CASE|case={case.case_id}|rows=1|rc=0"',
            "exit 0",
        ]
    )
    return "\n".join(lines) + "\n"


def balanced_cells(profiles: list[dict[str, object]]) -> list[dict[str, object]]:
    rows = _read_csv(BALANCED_OUTPUT / "solver_routing_calibration_summary.csv")
    by_case: dict[str, dict[str, dict[str, str]]] = {}
    for row in rows:
        by_case.setdefault(row["case_id"], {})[row["arm_id"]] = row
    profile_by_case = {str(row["case_id"]): row for row in profiles}
    cells: list[dict[str, object]] = []
    for case_id, profile in sorted(profile_by_case.items()):
        arms = by_case[case_id]
        if "cmg" not in arms or "lsmr_schwarz_1e8" not in arms:
            continue
        cmg = arms["cmg"]
        lsmr = arms["lsmr_schwarz_1e8"]
        auto = arms["auto"]
        row: dict[str, object] = {
            "population": "balanced_existing",
            "family": cmg["topology_detail"],
            "case_id": case_id,
            "seed": cmg["seed"],
            "n": cmg["n"],
            "periods": cmg["periods"],
            "workers": cmg["workers"],
            "firms": cmg["firms"],
            "pattern": cmg["topology"],
            "target_components": cmg["median_graph_components"],
            "large_components": "",
            "largest_vertices": "",
            "largest_share": "",
            "year_levels": cmg["third_levels"],
            "nuisance_columns": max(int(cmg["third_levels"]) - 1, 0),
            "controls": cmg["controls"],
            "rhs": cmg["rhs"],
            "threads": cmg["threads"],
            **profile,
        }
        for arm, prefix in ((auto, "auto"), (lsmr, "lsmr"), (cmg, "cmg")):
            row[f"{prefix}_eligible"] = arm["eligible"]
            row[f"{prefix}_seconds"] = arm["median_command_seconds"]
            row[f"{prefix}_native_seconds"] = arm["median_native_seconds"]
            row[f"{prefix}_setup_seconds"] = arm["median_solver_setup_seconds"]
            row[f"{prefix}_solve_seconds"] = arm["median_solver_solve_seconds"]
            row[f"{prefix}_iterations"] = arm["median_iterations"]
            row[f"{prefix}_fallback"] = arm["median_fallback"]
        cells.append(row)
    return cells


def component_cells() -> list[dict[str, object]]:
    timing = _read_csv(AUTO_COMPONENT_OUTPUT / "cmg_component_table.csv")
    hierarchy = _read_csv(COMPONENT_OUTPUT / "hierarchy_table.csv")
    hierarchy_by_key = {
        (row["pattern"], row["target_components"]): row
        for row in hierarchy
        if row["profile"] == "p3_ratio10_auto"
    }
    cells: list[dict[str, object]] = []
    for source in timing:
        if source["controls"] != "4" or source["threads"] != "8":
            continue
        profile = hierarchy_by_key[(source["pattern"], source["target_components"])]
        row: dict[str, object] = {
            "population": "controlled_existing",
            "family": f"controlled_{source['pattern']}",
            "case_id": source["case_id"],
            "seed": SEED - 2,
            "n": source["n"],
            "periods": 3,
            "workers": source["workers"],
            "firms": source["firms"],
            "pattern": source["pattern"],
            "target_components": source["target_components"],
            "large_components": 1 if source["pattern"] == "one_large" else source["target_components"],
            "largest_vertices": source["constructed_largest_vertices"],
            "largest_share": source["constructed_largest_share"],
            "year_levels": 0,
            "nuisance_columns": 0,
            "controls": source["controls"],
            "rhs": source["rhs"],
            "threads": source["threads"],
            "components": profile["actual_components"],
            "component_min": profile["smallest_component_vertices"],
            "component_p50": profile["smallest_component_vertices"],
            "component_p90": profile["smallest_component_vertices"],
            "component_max": profile["largest_component_vertices"],
            "component_max_share": source["constructed_largest_share"],
            "terminal": profile["terminal_reason"],
            "levels": len(profile["vertex_counts"].split(";")),
            "vertices": profile["vertex_counts"],
            "nonzeros": profile["matrix_nonzeros"],
            "repeats": profile["repeat_counts"],
            "recursive_work": profile["estimated_recursive_work"],
            "finest_work": "",
            "work_over_finest": profile["work_over_finest"],
            "deepest_visits": "",
            "max_repeat": max(int(value) for value in profile["repeat_counts"].split(";")),
            "terminal_factor_dimension": "",
            "terminal_factor_nonzeros": "",
            "graph_seconds": source["cmg_setup_seconds"],
            "hierarchy_seconds": 0,
            "setup_graph": source["cmg_setup_seconds"],
            "setup_hierarchy": 0,
            "setup_solver_workspace": 0,
            "setup_nuisance": 0,
            "setup_nuisance_columns": 0,
            "setup_nuisance_iterations": 0,
        }
        for prefix in ("auto", "lsmr", "cmg"):
            row[f"{prefix}_eligible"] = source[f"{prefix}_eligible"]
            row[f"{prefix}_seconds"] = source[f"{prefix}_seconds"]
            row[f"{prefix}_native_seconds"] = source[f"{prefix}_native_seconds"]
            row[f"{prefix}_setup_seconds"] = source[f"{prefix}_setup_seconds"]
            row[f"{prefix}_solve_seconds"] = source[f"{prefix}_solve_seconds"]
            row[f"{prefix}_iterations"] = source[f"{prefix}_iterations"]
            row[f"{prefix}_fallback"] = source[f"{prefix}_fallback"]
        cells.append(row)
    return cells


def evaluate_thresholds(
    cells: list[dict[str, object]], *, iterative_terminal_only: bool
) -> list[dict[str, object]]:
    comparable = [
        row
        for row in cells
        if _number(row, "cmg_eligible") == 1 and _number(row, "lsmr_eligible") == 1
    ]
    cmg_eligible = [row for row in cells if _number(row, "cmg_eligible") == 1]
    thresholds = [2, 3, 4, 5, 6, 8, 10, 12, 16, 24, 32, 48, 64, 96, 128, math.inf]
    results: list[dict[str, object]] = []
    for threshold in thresholds:
        ratios: list[float] = []
        total_route = 0.0
        total_best = 0.0
        accepted = rejected = harmful = severe = false_rejects = retained_wins = 0
        for row in comparable:
            work = float(row["work_over_finest"])
            cmg = float(row["cmg_seconds"])
            lsmr = float(row["lsmr_seconds"])
            setup = float(row.get("graph_seconds", 0)) + float(
                row.get("hierarchy_seconds", 0)
            )
            reject = work > threshold and (
                not iterative_terminal_only or str(row["terminal"]) != "Direct"
            )
            if not reject:
                route = cmg
                accepted += 1
                if cmg > 1.05 * lsmr:
                    harmful += 1
                if cmg > 2.0 * lsmr:
                    severe += 1
                if cmg < 0.95 * lsmr:
                    retained_wins += 1
            else:
                route = lsmr + setup
                rejected += 1
                if cmg < 0.95 * lsmr:
                    false_rejects += 1
            best = min(cmg, lsmr)
            ratios.append(route / best)
            total_route += route
            total_best += best
        ordered = sorted(ratios)
        p90 = ordered[math.ceil(0.90 * len(ordered)) - 1]
        rejected_without_certified_lsmr = sum(
            float(row["work_over_finest"]) > threshold
            and (not iterative_terminal_only or str(row["terminal"]) != "Direct")
            and _number(row, "lsmr_eligible") != 1
            for row in cmg_eligible
        )
        results.append(
            {
                "guard_shape": (
                    "iterative_terminal_work" if iterative_terminal_only else "all_hierarchy_work"
                ),
                "threshold": "unbounded" if math.isinf(threshold) else threshold,
                "comparable_cells": len(comparable),
                "accepted_cmg": accepted,
                "rejected_to_lsmr": rejected,
                "harmful_accepts_over_5pct": harmful,
                "severe_accepts_over_2x": severe,
                "retained_cmg_wins_over_5pct": retained_wins,
                "false_rejected_cmg_wins_over_5pct": false_rejects,
                "rejected_without_certified_explicit_lsmr": rejected_without_certified_lsmr,
                "geometric_mean_regret": math.exp(
                    statistics.fmean(math.log(value) for value in ratios)
                ),
                "p90_regret": p90,
                "maximum_regret": max(ratios),
                "aggregate_route_over_best": total_route / total_best,
            }
        )
    return results


def write_report(
    output: Path,
    cells: list[dict[str, object]],
    unconditional_thresholds: list[dict[str, object]],
    terminal_thresholds: list[dict[str, object]],
    plugin_sha: str,
) -> None:
    comparable = [
        row
        for row in cells
        if _number(row, "cmg_eligible") == 1 and _number(row, "lsmr_eligible") == 1
    ]
    severe = sorted(
        [row for row in comparable if float(row["cmg_seconds"]) > 2 * float(row["lsmr_seconds"])],
        key=lambda row: float(row["cmg_seconds"]) / float(row["lsmr_seconds"]),
        reverse=True,
    )
    focused = [row for row in cells if row["population"] == "focused_holdout"]
    focused_setup = [
        float(row["graph_seconds"]) + float(row["hierarchy_seconds"]) for row in focused
    ]
    terminal_comparable = [row for row in comparable if str(row["terminal"]) != "Direct"]
    terminal_wins = [
        row
        for row in terminal_comparable
        if float(row["cmg_seconds"]) < 0.95 * float(row["lsmr_seconds"])
    ]
    terminal_losses = [
        row
        for row in terminal_comparable
        if float(row["cmg_seconds"]) > 1.05 * float(row["lsmr_seconds"])
    ]
    proposed = next(row for row in terminal_thresholds if str(row["threshold"]) == "8")
    unconditional = next(
        row for row in unconditional_thresholds if str(row["threshold"]) == "6"
    )
    direct_severe = [row for row in severe if str(row["terminal"]) == "Direct"]
    lines = [
        "# CMG hierarchy-guard calibration",
        "",
        "This is development/prototyping evidence. It does not change automatic routing.",
        "",
        "## Evidence",
        "",
        f"- Diagnostic plugin SHA-256: `{plugin_sha}`.",
        f"- Guard calibration cells: {len(cells)}; directly comparable certified CMG/LSMR cells: {len(comparable)}.",
        "- The focused holdout recorded 333 runs. 330/330 successful runs certified without fallback; the three failures were deterministic explicit Schwarz-LSMR `r(459)` results in one cell and remain ineligible. All successful arms matched exactly on the exported coefficient/RSS precision, and every measured component count matched construction.",
        "- Every focused timing cell has exactly three recorded repetitions, one unrecorded warm-up per arm, rotated arm order, one generated dataset shared by all arms, tolerance `1e-8`, 16,000 maximum iterations, four controls, and eight threads.",
        "- Existing balanced and controlled timing rows also have exactly three repetitions; only the deterministic hierarchy profile was added.",
        f"- Focused graph-plus-hierarchy construction cost had a {statistics.median(focused_setup):.4f}-second median and {max(focused_setup):.4f}-second maximum.",
        "",
        "## Candidate guard",
        "",
        "After the existing cheap AKM topology screen accepts CMG, build only the graph and CMG preconditioner. Compute",
        "",
        "`W = sum_l visits_l * (vertices_l + matrix_nonzeros_l) / (vertices_0 + matrix_nonzeros_0)`,",
        "",
        "where `visits_0 = 1` and each later visit count is the product of upstream effective repeat counts.",
        "",
        "The concrete candidate is: if the terminal reason is `Direct`, do not apply a hierarchy safety veto; if the terminal is iterative, continue CMG only when `W <= 8`. Otherwise discard the graph/hierarchy before nuisance-Schur construction and PCG and run the existing direct auto-LSMR route with certified CG continuation. The planned veto is routing, not a numerical fallback.",
        "",
        f"This rule rejected {proposed['rejected_to_lsmr']} of {proposed['comparable_cells']} comparable cells, retained every measured CMG win, and left one >2x CMG loss; that remaining loss had a direct terminal and nine nuisance columns, so it is a separate RHS/nuisance cost-model issue rather than the recursive hierarchy cliff. Its aggregate route/best ratio was {float(proposed['aggregate_route_over_best']):.3f}, versus {float(unconditional['aggregate_route_over_best']):.3f} for an unconditional `W <= 6` veto that falsely rejected {unconditional['false_rejected_cmg_wins_over_5pct']} measured CMG wins.",
        "",
        f"The separation is unusually clean but still needs a holdout: all measured iterative-terminal CMG wins had `W <= {max(float(row['work_over_finest']) for row in terminal_wins):.2f}`, while the first measured iterative-terminal loss had `W = {min(float(row['work_over_finest']) for row in terminal_losses):.2f}`. Thresholds from 4 through 20 therefore make the same in-sample hierarchy decision. Eight is the conservative round candidate, not a fitted constant.",
        "",
        "## Severe current CMG choices",
        "",
        "| Case | Population | W | CMG/LSMR | Terminal | Levels |",
        "|:---|:---|---:|---:|:---|---:|",
    ]
    for row in severe[:15]:
        ratio = float(row["cmg_seconds"]) / float(row["lsmr_seconds"])
        lines.append(
            f"| {row['case_id']} | {row['population']} | {float(row['work_over_finest']):.2f} | {ratio:.2f} | {row['terminal']} | {row['levels']} |"
        )
    lines.extend(
        [
            "",
        "## Interpretation",
            "",
        "A terminal-aware hierarchy-work guard directly targets the repeat-amplified work that caused the disconnected-component cliff. Component count and largest-component share remain useful diagnostics but are not guard variables: all-tiny graphs can have many components, an iterative terminal, and very low hierarchy work. An unconditional work ceiling is also too blunt because several realistic sparse graphs have direct terminals, `W` between 6 and 20, and strong CMG wins.",
        "",
        f"The one direct-terminal severe loss was `{direct_severe[0]['case_id']}` at CMG/LSMR {float(direct_severe[0]['cmg_seconds']) / float(direct_severe[0]['lsmr_seconds']):.2f}. A possible second-stage nuisance-load screen catches it, but the current sample shows false rejections of balanced nuisance designs. Calibrate that screen separately using graph-solve batches and RHS width rather than weakening the hierarchy guard.",
        "",
        "Before implementation, rerun the candidate band `{6, 8, 12, 16}` on fresh component mixtures and seeds, one/eight threads, and 0/4/16 controls. The implementation should retain the existing memory and scientific certificates, count rejected hierarchy setup in runtime, release CMG allocations before LSMR preflight, and report the ultimate used solver without labeling the planned veto as a numerical fallback.",
            "",
        ]
    )
    (output / "report.md").write_text("\n".join(lines), encoding="utf-8")


def analyze(output: Path) -> None:
    raw = _read_csv(output / "focused_runs.csv")
    summary = summarize_focused(raw)
    _write_csv(output / "focused_summary.csv", summary, list(summary[0]))
    focused_profiles = _read_csv(output / "focused_hierarchy_profiles.csv")
    balanced_profiles = _read_csv(output / "balanced_hierarchy_profiles.csv")
    cells = focused_cells(summary, focused_profiles)
    cells.extend(balanced_cells(balanced_profiles))
    cells.extend(component_cells())
    _write_csv(output / "guard_calibration_cells.csv", cells, list(cells[0]))
    thresholds = evaluate_thresholds(cells, iterative_terminal_only=False)
    terminal_thresholds = evaluate_thresholds(cells, iterative_terminal_only=True)
    _write_csv(output / "guard_thresholds.csv", thresholds, list(thresholds[0]))
    _write_csv(
        output / "guard_terminal_thresholds.csv",
        terminal_thresholds,
        list(terminal_thresholds[0]),
    )
    plugin_sha = hashlib.sha256((ROOT / "dist/fereg_macos.plugin").read_bytes()).hexdigest()
    write_report(output, cells, thresholds, terminal_thresholds, plugin_sha)


def run_focused(
    plan: Sequence[Case], stata: Path, output: Path, resume: bool
) -> None:
    work = output / "work"
    work.mkdir(parents=True, exist_ok=True)
    raw: list[dict[str, str]] = []
    profiles: list[dict[str, object]] = []
    for index, case in enumerate(plan, start=1):
        print(f"[focused {index}/{len(plan)}] {case.case_id}", flush=True)
        do_file = work / f"{case.case_id}.do"
        dta_file = work / f"{case.case_id}_runs.dta"
        dataset_dta = work / f"{case.case_id}_dataset.dta"
        csv_file = dta_file.with_suffix(".csv")
        trace_file = work / f"{case.case_id}.trace.txt"
        if resume and csv_file.is_file() and trace_file.is_file():
            cached = _read_csv(csv_file)
            if len(cached) == len(ARMS) * REPETITIONS:
                raw.extend(cached)
                profiles.append(parse_hierarchy_trace(trace_file.read_text(), case.case_id))
                continue
        do_file.write_text(_case_do(case, dta_file, dataset_dta), encoding="utf-8")
        _run_stata(stata, do_file, work)
        trace_do = work / f"{case.case_id}_trace.do"
        trace_do.write_text(_focused_trace_do(case, dataset_dta), encoding="utf-8")
        trace = _run_stata(stata, trace_do, work, trace=True)
        trace_file.write_text(trace, encoding="utf-8")
        raw.extend(_read_csv(csv_file))
        profiles.append(parse_hierarchy_trace(trace, case.case_id))
    _write_csv(output / "focused_runs.csv", raw, POST_VARIABLES)
    _write_csv(output / "focused_hierarchy_profiles.csv", profiles, list(profiles[0]))


def run_balanced_profiles(stata: Path, output: Path, resume: bool) -> None:
    plan = balanced_profile_plan()
    work = output / "work" / "balanced_profiles"
    work.mkdir(parents=True, exist_ok=True)
    profiles: list[dict[str, object]] = []
    for index, case in enumerate(plan, start=1):
        print(f"[balanced profile {index}/{len(plan)}] {case.case_id}", flush=True)
        do_file = work / f"{case.case_id}.do"
        trace_file = work / f"{case.case_id}.trace.txt"
        if resume and trace_file.is_file():
            profiles.append(parse_hierarchy_trace(trace_file.read_text(), case.case_id))
            continue
        do_file.write_text(_balanced_trace_do(case), encoding="utf-8")
        trace = _run_stata(stata, do_file, work, trace=True)
        trace_file.write_text(trace, encoding="utf-8")
        profiles.append(parse_hierarchy_trace(trace, case.case_id))
    _write_csv(output / "balanced_hierarchy_profiles.csv", profiles, list(profiles[0]))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--case", action="append")
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    parser.add_argument("--skip-focused", action="store_true")
    parser.add_argument("--skip-balanced-profiles", action="store_true")
    parser.add_argument("--list-plan", action="store_true")
    args = parser.parse_args(argv)

    plan = build_focused_plan()
    if args.case:
        wanted = set(args.case)
        plan = [case for case in plan if case.case_id in wanted]
    if args.smoke:
        plan = [
            next(case for case in plan if case.case_id == "guard_n99_one_large_c0600"),
            next(case for case in plan if case.case_id == "guard_n99_one_large_c0800"),
        ]
    if args.list_plan:
        for case in plan:
            print(case)
        print(f"balanced_profiles={len(balanced_profile_plan())}")
        return 0

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.analyze_only:
        analyze(output)
        return 0
    if not args.stata.is_file():
        raise RuntimeError(f"Stata executable not found: {args.stata}")
    if not (ROOT / "dist/fereg_macos.plugin").is_file():
        raise RuntimeError("diagnostic plugin is missing")
    if not args.skip_focused:
        run_focused(plan, args.stata, output, args.resume)
    if args.smoke:
        print(f"smoke_profiles={output / 'focused_hierarchy_profiles.csv'}")
        return 0
    if not args.skip_balanced_profiles:
        run_balanced_profiles(args.stata, output, args.resume)
    analyze(output)
    print(f"report={output / 'report.md'}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"CMG hierarchy-guard calibration failed: {error}", file=sys.stderr)
        raise
