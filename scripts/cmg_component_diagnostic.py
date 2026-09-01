#!/usr/bin/env python3
"""Development diagnostic for CMG sensitivity to disconnected FE graphs.

The benchmark holds observations, FE levels, unique edges, cardinality ratio,
and both degree distributions fixed.  It changes only the connected-component
partition: equal components or one large component plus identical tiny ones.
Each generated dataset is warmed outside timing and then reused for exactly
three recorded repetitions with rotated solver order.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
from typing import Iterable, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_STATA = Path("/Applications/Stata18/StataMP.app/Contents/MacOS/stata-mp")
DEFAULT_OUTPUT = ROOT / "benchmarks/reports/development/cmg-component-diagnostic-v1"
REPETITIONS = 3
MAX_ITERATIONS = 16_000
N = 100_000
WORKERS = 50_000
FIRMS = 10_000
PERIODS = 2
SEED = 20260901
BALANCED_COMPONENT_COUNTS = (1, 10, 100, 1_000)
MATRIX_MIXED_COMPONENT_COUNTS = (10, 100, 1_000)
SWEEP_MIXED_COMPONENT_COUNTS = (200, 400, 600, 800, 1_200, 1_600, 2_000, 3_000, 4_000, 5_000)
MATRIX_CONTROLS = (0, 4, 16)
INCLUDE_THREAD_SUBSET = True


@dataclass(frozen=True)
class Arm:
    arm_id: str
    solver: str
    preconditioner: str

    def command(self, case: "Case") -> str:
        controls = " ".join(f"x{index}" for index in range(1, case.controls + 1))
        lhs = f"y {controls}".strip()
        options = [
            "absorb(worker firm)",
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
class Structure:
    pattern: str
    components: int
    largest_workers: int
    largest_firms: int
    tiny_components: int
    tiny_workers: int
    tiny_firms: int

    @property
    def largest_vertices(self) -> int:
        return self.largest_workers + self.largest_firms

    @property
    def largest_share(self) -> float:
        return self.largest_vertices / (WORKERS + FIRMS)

    @property
    def tag(self) -> str:
        return f"{self.pattern}_c{self.components:04d}"


@dataclass(frozen=True)
class Case:
    structure: Structure
    controls: int
    threads: int

    @property
    def case_id(self) -> str:
        return f"comp_{self.structure.tag}_k{self.controls}_t{self.threads}"

    @property
    def rhs(self) -> int:
        return self.controls + 1


def build_structures() -> list[Structure]:
    structures: list[Structure] = []
    for components in BALANCED_COMPONENT_COUNTS:
        structures.append(
            Structure(
                pattern="balanced",
                components=components,
                largest_workers=WORKERS // components,
                largest_firms=FIRMS // components,
                tiny_components=0,
                tiny_workers=0,
                tiny_firms=0,
            )
        )
    for components in MATRIX_MIXED_COMPONENT_COUNTS:
        structures.append(one_large_structure(components))
    return structures


def one_large_structure(components: int) -> Structure:
    tiny_components = components - 1
    ratio = WORKERS // FIRMS
    tiny_firms = PERIODS
    tiny_workers = ratio * tiny_firms
    return Structure(
        pattern="one_large",
        components=components,
        largest_workers=WORKERS - tiny_workers * tiny_components,
        largest_firms=FIRMS - tiny_firms * tiny_components,
        tiny_components=tiny_components,
        tiny_workers=tiny_workers,
        tiny_firms=tiny_firms,
    )


def build_plan() -> list[Case]:
    cases: list[Case] = []
    for structure in build_structures():
        for controls in MATRIX_CONTROLS:
            cases.append(Case(structure, controls, 8))
        if INCLUDE_THREAD_SUBSET:
            cases.append(Case(structure, 4, 1))
    # A single-RHS sweep localizes the mixed-component hierarchy cliff without
    # multiplying the broader RHS/thread sensitivity matrix.
    for components in SWEEP_MIXED_COMPONENT_COUNTS:
        cases.append(Case(one_large_structure(components), 0, 8))
    return cases


def verify_structure(structure: Structure) -> None:
    if N != WORKERS * PERIODS or WORKERS % FIRMS:
        raise RuntimeError("fixed panel dimensions are inconsistent")
    ratio = WORKERS // FIRMS
    if structure.pattern == "balanced":
        if WORKERS % structure.components or FIRMS % structure.components:
            raise RuntimeError(f"{structure.tag}: levels do not divide equally")
        expected_workers = structure.components * structure.largest_workers
        expected_firms = structure.components * structure.largest_firms
    else:
        expected_workers = (
            structure.largest_workers
            + structure.tiny_components * structure.tiny_workers
        )
        expected_firms = (
            structure.largest_firms
            + structure.tiny_components * structure.tiny_firms
        )
    if (expected_workers, expected_firms) != (WORKERS, FIRMS):
        raise RuntimeError(f"{structure.tag}: component allocation does not exhaust levels")
    if structure.largest_firms < PERIODS:
        raise RuntimeError(f"{structure.tag}: component has too few firms for unique edges")
    if structure.largest_workers != ratio * structure.largest_firms:
        raise RuntimeError(f"{structure.tag}: largest component changed cardinality ratio")
    if structure.tiny_components and structure.tiny_workers != ratio * structure.tiny_firms:
        raise RuntimeError(f"{structure.tag}: tiny components changed cardinality ratio")


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
    structure = case.structure
    lines = [
        f"set seed {SEED}",
        f"set obs {N}",
        "generate long obs_id = _n",
        f"generate long worker = ceil(_n / {PERIODS})",
        "bysort worker: generate byte period = _n - 1",
    ]
    if structure.pattern == "balanced":
        workers_per_component = WORKERS // structure.components
        firms_per_component = FIRMS // structure.components
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
        small_workers = structure.tiny_components * structure.tiny_workers
        small_firms = structure.tiny_components * structure.tiny_firms
        lines.extend(
            [
                "generate long firm = .",
                (
                    f"replace firm = floor((worker - 1) / {structure.tiny_workers}) * "
                    f"{structure.tiny_firms} + "
                    f"mod(mod(worker - 1, {structure.tiny_firms}) + period, "
                    f"{structure.tiny_firms}) + 1 if worker <= {small_workers}"
                ),
                (
                    f"replace firm = {small_firms} + "
                    f"mod(mod(worker - {small_workers} - 1, {structure.largest_firms}) + period, "
                    f"{structure.largest_firms}) + 1 if worker > {small_workers}"
                ),
            ]
        )
    lines.extend(
        [
            f"assert _N == {N}",
            f"assert worker >= 1 & worker <= {WORKERS}",
            f"assert firm >= 1 & firm <= {FIRMS}",
            "bysort worker (period): assert firm[1] != firm[_N]",
            "sort worker firm obs_id",
            "by worker firm: generate byte __edge_first = (_n == 1)",
            "quietly count if __edge_first",
            "scalar __unique_edges = r(N)",
            f"assert __unique_edges == {N}",
            "bysort worker: egen int __worker_degree = total(__edge_first)",
            f"assert __worker_degree == {PERIODS}",
            "bysort firm: egen int __firm_degree = total(__edge_first)",
            f"assert __firm_degree == {N // FIRMS}",
            "drop __edge_first __worker_degree __firm_degree",
            "capture drop __worker0 __component0 __local_worker0",
            "sort obs_id",
        ]
    )
    # Generate the maximum RHS width for every case.  Changing controls then
    # changes only the fit command, not the random stream or stored dataset.
    lines.append("generate double __xb = 0")
    for index in range(1, 17):
        beta = 0.30 / index if index % 2 else -0.20 / index
        lines.extend(
            [
                f"generate double x{index} = rnormal()",
                f"replace __xb = __xb + ({beta:.17g}) * x{index}",
            ]
        )
    lines.extend(
        [
            "generate double y = 1 + __xb + sin(worker*.017) + cos(firm*.013) + rnormal(0,.5)",
            "sort obs_id",
        ]
    )
    return lines


POST_VARIABLES = [
    "case_id", "pattern", "target_components", "constructed_largest_workers",
    "constructed_largest_firms", "constructed_largest_vertices",
    "constructed_largest_share", "tiny_components", "tiny_workers", "tiny_firms",
    "seed", "n", "workers", "firms", "periods", "unique_edges",
    "worker_degree", "firm_degree", "controls", "rhs", "threads", "repetition",
    "sequence", "arm_id", "requested_solver", "requested_preconditioner",
    "requested_tolerance", "rc", "command_seconds", "requested_solver_code",
    "used_solver_code", "iterations", "stop_reason", "certificate_passed",
    "absolute_infinity_norm", "relative_infinity_norm", "relative_two_norm",
    "normal_equation_residual", "solver_setup_seconds", "solver_solve_seconds",
    "fallback", "warning_count", "requested_preconditioner_code",
    "used_preconditioner_code", "pilot_iterations", "pilot_contraction",
    "native_seconds", "effective_threads", "reported_tolerance", "retained_workers",
    "retained_firms", "actual_components", "rss", "b1", "b2", "b3", "b4",
]


def _postfile_line(path: Path) -> str:
    return (
        "postfile __runs str48 case_id str12 pattern int target_components "
        "long constructed_largest_workers long constructed_largest_firms "
        "long constructed_largest_vertices double constructed_largest_share "
        "int tiny_components int tiny_workers int tiny_firms long seed long n long workers "
        "long firms byte periods long unique_edges byte worker_degree byte firm_degree "
        "byte controls byte rhs byte threads byte repetition byte sequence str24 arm_id "
        "str8 requested_solver str12 requested_preconditioner double requested_tolerance "
        "int rc double command_seconds requested_solver_code used_solver_code iterations "
        "stop_reason certificate_passed absolute_infinity_norm relative_infinity_norm "
        "relative_two_norm normal_equation_residual solver_setup_seconds solver_solve_seconds "
        "fallback warning_count requested_preconditioner_code used_preconditioner_code "
        "pilot_iterations pilot_contraction native_seconds effective_threads reported_tolerance "
        "retained_workers retained_firms actual_components rss b1 b2 b3 b4 "
        f'using "{path.as_posix()}", replace'
    )


def _run_block(case: Case, arm: Arm, repetition: int, sequence: int) -> list[str]:
    structure = case.structure
    metadata = [
        f'("{case.case_id}")', f'("{structure.pattern}")', f"({structure.components})",
        f"({structure.largest_workers})", f"({structure.largest_firms})",
        f"({structure.largest_vertices})", f"({structure.largest_share:.17g})",
        f"({structure.tiny_components})", f"({structure.tiny_workers})",
        f"({structure.tiny_firms})", f"({SEED})", f"({N})", f"({WORKERS})",
        f"({FIRMS})", f"({PERIODS})", "(__unique_edges)", f"({PERIODS})",
        f"({N // FIRMS})",
        f"({case.controls})", f"({case.rhs})", f"({case.threads})",
        f"({repetition})", f"({sequence})", f'("{arm.arm_id}")',
        f'("{arm.solver}")', f'("{arm.preconditioner}")', "(1e-8)",
    ]
    diagnostics = [
        "(__rc)", "(__seconds)", "(__req)", "(__used)", "(__iterations)",
        "(__stop)", "(__certified)", "(__absinf)", "(__relinf)", "(__rel2)",
        "(__normal)", "(__setup)", "(__solve)", "(__fallback)", "(__warnings)",
        "(__preq)", "(__pused)", "(__pilot_iters)", "(__pilot_contract)",
        "(__native)", "(__effective)", "(__reported_tol)", "(__level1)",
        "(__level2)", "(__components)", "(__rss)", "(__b1)", "(__b2)",
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
            "foreach __name in req used iterations stop certified absinf relinf rel2 normal "
            "setup solve fallback warnings preq pused pilot_iters pilot_contract native effective "
            "reported_tol level1 level2 components rss b1 b2 b3 b4 {"
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
        "    scalar __absinf = __solver[1,5]",
        "    scalar __relinf = __solver[1,6]",
        "    scalar __rel2 = __solver[1,7]",
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
        "    scalar __reported_tol = __runtime[23,2]",
        "    scalar __level1 = __fe[1,4]",
        "    scalar __level2 = __fe[2,4]",
        "    scalar __components = __dof[2,2]",
        "    scalar __rss = e(rss)",
        "    capture scalar __b1 = _b[x1]",
        "    capture scalar __b2 = _b[x2]",
        "    capture scalar __b3 = _b[x3]",
        "    capture scalar __b4 = _b[x4]",
        "}",
        "post __runs " + " ".join(metadata + diagnostics),
    ]


def _case_do(case: Case, output_dta: Path) -> str:
    lines = _preamble() + _dataset_lines(case)
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
            f'display as result "CMG_COMPONENT_CASE|case={case.case_id}|rows=" _N "|rc=0"',
            "exit 0",
        ]
    )
    return "\n".join(lines) + "\n"


def _run_stata(stata: Path, do_file: Path, cwd: Path) -> str:
    environment = dict(os.environ)
    environment.pop("FEREG_SOLVER_TRACE", None)
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
    if completed.returncode != 0 or "|rc=0" not in output:
        raise RuntimeError(
            f"Stata failed for {do_file.name} with process code {completed.returncode}\n"
            + output[-8_000:]
        )
    return output


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        raise RuntimeError(f"empty case output: {path}")
    return rows


def _write_csv(path: Path, rows: Iterable[dict[str, object]], fields: Sequence[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def _number(row: dict[str, str], field: str) -> float | None:
    value = row.get(field, "")
    if value in {"", "."}:
        return None
    try:
        result = float(value)
    except ValueError:
        return None
    return result if math.isfinite(result) else None


def summarize(raw: list[dict[str, str]]) -> list[dict[str, object]]:
    groups: dict[tuple[str, str], list[dict[str, str]]] = {}
    for row in raw:
        groups.setdefault((row["case_id"], row["arm_id"]), []).append(row)
    metadata = POST_VARIABLES[:21] + POST_VARIABLES[24:27]
    median_fields = [
        "command_seconds", "requested_solver_code", "used_solver_code", "iterations",
        "stop_reason", "certificate_passed", "absolute_infinity_norm",
        "relative_infinity_norm", "relative_two_norm", "normal_equation_residual",
        "solver_setup_seconds", "solver_solve_seconds", "fallback", "warning_count",
        "requested_preconditioner_code", "used_preconditioner_code", "pilot_iterations",
        "pilot_contraction", "native_seconds", "effective_threads", "reported_tolerance",
        "retained_workers", "retained_firms", "actual_components", "rss", "b1", "b2",
        "b3", "b4",
    ]
    result: list[dict[str, object]] = []
    for (case_id, arm_id), rows in sorted(groups.items()):
        if len(rows) != REPETITIONS:
            raise RuntimeError(
                f"{case_id}/{arm_id}: expected {REPETITIONS} rows, found {len(rows)}"
            )
        first = rows[0]
        cell: dict[str, object] = {field: first[field] for field in metadata}
        cell["arm_id"] = arm_id
        rcs = [_number(row, "rc") for row in rows]
        certs = [_number(row, "certificate_passed") for row in rows]
        fallbacks = {_number(row, "fallback") for row in rows}
        cell["recorded_repetitions"] = len(rows)
        cell["successful_repetitions"] = sum(value == 0 for value in rcs)
        cell["certified_repetitions"] = sum(value == 1 for value in certs)
        cell["eligible"] = int(
            cell["successful_repetitions"] == REPETITIONS
            and cell["certified_repetitions"] == REPETITIONS
            and fallbacks == {0.0}
        )
        cell["fallback_deterministic"] = int(len(fallbacks) == 1)
        for field in median_fields:
            values = [_number(row, field) for row in rows]
            finite = [value for value in values if value is not None]
            cell[f"median_{field}"] = statistics.median(finite) if finite else ""
        result.append(cell)
    return result


def make_component_table(summary: list[dict[str, object]]) -> list[dict[str, object]]:
    by_case: dict[str, dict[str, dict[str, object]]] = {}
    for row in summary:
        by_case.setdefault(str(row["case_id"]), {})[str(row["arm_id"])] = row
    result: list[dict[str, object]] = []
    for case_id, arms in sorted(
        by_case.items(),
        key=lambda item: (
            str(item[1]["auto"]["pattern"]),
            int(item[1]["auto"]["target_components"]),
            int(item[1]["auto"]["threads"]),
            int(item[1]["auto"]["controls"]),
        ),
    ):
        auto = arms["auto"]
        output: dict[str, object] = {
            field: auto[field]
            for field in (
                "case_id", "pattern", "target_components", "constructed_largest_workers",
                "constructed_largest_firms", "constructed_largest_vertices",
                "constructed_largest_share", "tiny_components", "tiny_workers", "tiny_firms",
                "n", "workers", "firms", "unique_edges", "worker_degree", "firm_degree",
                "controls", "rhs", "threads",
            )
        }
        output["actual_components"] = auto["median_actual_components"]
        for arm_id in ("auto", "lsmr_schwarz_1e8", "cmg"):
            arm = arms[arm_id]
            prefix = "lsmr" if arm_id == "lsmr_schwarz_1e8" else arm_id
            for target, source in (
                ("eligible", "eligible"),
                ("seconds", "median_command_seconds"),
                ("native_seconds", "median_native_seconds"),
                ("setup_seconds", "median_solver_setup_seconds"),
                ("solve_seconds", "median_solver_solve_seconds"),
                ("iterations", "median_iterations"),
                ("used_solver", "median_used_solver_code"),
                ("fallback", "median_fallback"),
            ):
                output[f"{prefix}_{target}"] = arm[source]
        auto_seconds = float(output["auto_seconds"])
        lsmr_seconds = float(output["lsmr_seconds"])
        cmg_seconds = float(output["cmg_seconds"])
        output["cmg_over_auto"] = cmg_seconds / auto_seconds
        output["cmg_over_lsmr"] = cmg_seconds / lsmr_seconds
        output["cmg_solve_per_iteration"] = (
            float(output["cmg_solve_seconds"]) / float(output["cmg_iterations"])
            if float(output["cmg_iterations"]) > 0
            else ""
        )
        result.append(output)
    return result


def analyze(raw: list[dict[str, str]], output: Path) -> None:
    if len(raw) % (len(ARMS) * REPETITIONS):
        raise RuntimeError(f"raw row count {len(raw)} is not a complete cell matrix")
    _write_csv(output / "cmg_component_runs.csv", raw, POST_VARIABLES)
    summary = summarize(raw)
    _write_csv(output / "cmg_component_summary.csv", summary, list(summary[0]))
    table = make_component_table(summary)
    _write_csv(output / "cmg_component_table.csv", table, list(table[0]))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--case", action="append")
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--analyze-only", action="store_true")
    parser.add_argument("--list-plan", action="store_true")
    args = parser.parse_args(argv)

    plan = build_plan()
    for structure in {case.structure for case in plan}:
        verify_structure(structure)
    if args.case:
        wanted = set(args.case)
        plan = [case for case in plan if case.case_id in wanted]
    if args.smoke:
        plan = [
            next(case for case in plan if case.case_id == "comp_balanced_c0001_k4_t8"),
            next(case for case in plan if case.case_id == "comp_one_large_c1000_k4_t8"),
        ]
    if args.list_plan:
        for case in plan:
            print(case)
        return 0

    output = args.output.resolve()
    work = output / "work"
    work.mkdir(parents=True, exist_ok=True)
    if args.analyze_only:
        raw_path = output / "cmg_component_runs.csv"
        analyze(_read_csv(raw_path), output)
        print(f"raw={raw_path}")
        print(f"summary={output / 'cmg_component_summary.csv'}")
        print(f"table={output / 'cmg_component_table.csv'}")
        return 0
    if not args.stata.is_file():
        raise RuntimeError(f"Stata executable not found: {args.stata}")
    if not (ROOT / "dist/fereg_macos.plugin").is_file():
        raise RuntimeError("dist/fereg_macos.plugin is missing; run scripts/build_macos.sh first")

    raw: list[dict[str, str]] = []
    for index, case in enumerate(plan, start=1):
        print(f"[{index}/{len(plan)}] {case.case_id}", flush=True)
        do_file = work / f"{case.case_id}.do"
        dta_file = work / f"{case.case_id}_runs.dta"
        case_csv = dta_file.with_suffix(".csv")
        if args.resume and case_csv.is_file():
            cached = _read_csv(case_csv)
            if len(cached) == len(ARMS) * REPETITIONS:
                raw.extend(cached)
                print(f"  reused {len(cached)} complete rows", flush=True)
                continue
        do_file.write_text(_case_do(case, dta_file), encoding="utf-8")
        _run_stata(args.stata, do_file, work)
        raw.extend(_read_csv(case_csv))
    analyze(raw, output)
    print(f"raw={output / 'cmg_component_runs.csv'}")
    print(f"summary={output / 'cmg_component_summary.csv'}")
    print(f"table={output / 'cmg_component_table.csv'}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"CMG component diagnostic failed: {error}", file=sys.stderr)
        raise
