#!/usr/bin/env python3
"""Development-only calibration of fereg's automatic FE-solver routing.

The runner deliberately does not compare broad estimators or mutate the
production routing policy.  It generates each synthetic dataset once inside a
Stata process, warms eligible fereg arms outside timing, rotates arm order over
exactly three recorded repetitions, and records every success or failure.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass, replace
import hashlib
import itertools
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
from typing import Iterable, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_STATA = Path("/Applications/Stata18/StataMP.app/Contents/MacOS/stata-mp")
DEFAULT_OUTPUT = ROOT / "benchmarks/reports/development/solver-routing-calibration-v1"
REPETITIONS = 3
MAX_ITERATIONS = 16_000
SOLVER_LABELS = {
    1: "auto",
    2: "MAP",
    3: "LSMR",
    4: "CG",
    5: "Irons-Tuck",
    6: "SQUAREM",
    7: "CMG",
}


@dataclass(frozen=True)
class Arm:
    arm_id: str
    solver: str
    preconditioner: str
    tolerance: float

    def command(self, case: "Case", residual: str | None = None) -> str:
        controls = " ".join(f"x{index}" for index in range(1, case.controls + 1))
        lhs = f"y {controls}".strip()
        options = [
            f"absorb({case.absorb_order})",
            "dof(pairwise)",
            f"solver({self.solver})",
            f"threads({case.threads})",
            f"tolerance({self.tolerance:.0e})",
            f"iterate({MAX_ITERATIONS})",
        ]
        if case.memory_mode != "auto":
            options.append(f"memory({case.memory_mode})")
        if case.poolsize > 0:
            options.append(f"poolsize({case.poolsize})")
        if self.preconditioner != "none":
            options.append(f"preconditioner({self.preconditioner})")
        if residual is not None:
            options.append(f"residuals({residual})")
        return f"fereg {lhs}, {' '.join(options)}"


AUTO = Arm("auto", "auto", "none", 1e-8)
MAP = Arm("map", "map", "none", 1e-8)
CG = Arm("cg", "cg", "none", 1e-8)
LSMR8 = Arm("lsmr_schwarz_1e8", "lsmr", "schwarz", 1e-8)
LSMR10 = Arm("lsmr_schwarz_1e10", "lsmr", "schwarz", 1e-10)
CMG = Arm("cmg", "cmg", "off", 1e-8)
LSMR_DIAGONAL = Arm("lsmr_diagonal_1e8", "lsmr", "diagonal", 1e-8)
LSMR_OFF = Arm("lsmr_off_1e8", "lsmr", "off", 1e-8)


@dataclass(frozen=True)
class Case:
    test_id: str
    case_id: str
    topology: str
    topology_detail: str
    seed: int
    n: int
    periods: int
    workers: int
    firms: int
    third_name: str
    third_levels: int
    mobility: float
    controls: int
    threads: int
    memory_mode: str
    poolsize: int
    absorb_order: str
    arms: tuple[Arm, ...]

    @property
    def fe_dimensions(self) -> int:
        return 2 + int(self.third_levels > 0)

    @property
    def canonical_level_counts(self) -> tuple[int, int, int]:
        return self.workers, self.firms, self.third_levels


def _seed(label: str) -> int:
    digest = hashlib.sha256(label.encode("utf-8")).digest()
    return 1_000_000 + int.from_bytes(digest[:4], "big") % 1_000_000_000


def _nuisance_columns(level_counts: Sequence[int]) -> int:
    positive = sorted((value for value in level_counts if value > 0), reverse=True)
    return sum(value - 1 for value in positive[2:])


def _principal_arms(cmg_eligible: bool, diagnostics: bool = False) -> tuple[Arm, ...]:
    arms: list[Arm] = [AUTO, MAP, CG, LSMR8, LSMR10]
    if cmg_eligible:
        arms.append(CMG)
    if diagnostics:
        arms.extend([LSMR_DIAGONAL, LSMR_OFF])
    return tuple(arms)


def _panel_case(
    *,
    test_id: str,
    case_id: str,
    topology: str,
    topology_detail: str,
    n: int,
    periods: int = 5,
    mobility: float = 0.02,
    controls: int = 4,
    threads: int = 8,
    memory_mode: str = "auto",
    poolsize: int = 0,
    firms: int | None = None,
    third_name: str = "",
    third_levels: int = 0,
    absorb_order: str | None = None,
    arms: tuple[Arm, ...] | None = None,
    seed: int | None = None,
) -> Case:
    if n % periods:
        raise ValueError(f"{case_id}: N must be divisible by periods")
    workers = n // periods
    firms = firms if firms is not None else max(50, math.ceil(workers / 20))
    canonical = ["worker", "firm"] + ([third_name] if third_levels else [])
    absorb_order = absorb_order or " ".join(canonical)
    nuisance = _nuisance_columns((workers, firms, third_levels))
    cmg_eligible = nuisance <= 64
    if arms is None:
        arms = _principal_arms(cmg_eligible)
    return Case(
        test_id=test_id,
        case_id=case_id,
        topology=topology,
        topology_detail=topology_detail,
        seed=seed if seed is not None else _seed(case_id),
        n=n,
        periods=periods,
        workers=workers,
        firms=firms,
        third_name=third_name,
        third_levels=third_levels,
        mobility=mobility,
        controls=controls,
        threads=threads,
        memory_mode=memory_mode,
        poolsize=poolsize,
        absorb_order=absorb_order,
        arms=arms,
    )


def build_plan() -> list[Case]:
    cases: list[Case] = []

    # Test 1: exact three_fe_sparse generator and the 10k--30k cliff grid.
    for n in (10_000, 12_500, 15_000, 17_500, 20_000, 22_500, 25_000, 27_500, 30_000):
        workers = n // 5
        firms = max(50, math.ceil(workers / 20))
        counties = max(10, math.ceil(firms / 20))
        diagnostic = n in {15_000, 25_000, 30_000}
        cases.append(
            _panel_case(
                test_id="cliff",
                case_id=f"cliff_n{n}",
                topology="worker_firm_county_sparse",
                topology_detail="periods5_mobility002_manual_generator",
                n=n,
                mobility=0.02,
                third_name="county",
                third_levels=counties,
                arms=_principal_arms(True, diagnostics=diagnostic),
                # This is the exact original three_fe_sparse 25k seed
                # (20260831 + 1000*problem_index + size_index).  Holding it
                # fixed across the dense size grid isolates N around the
                # reported cliff while reproducing the known 25k cell.
                seed=20263836,
            )
        )

    # Test 2: common topology families at the requested four sizes.
    topology_specs = [
        ("wf_dense", "worker_firm_dense", 0.20, "", lambda w, f: 0),
        ("wf_sparse", "worker_firm_sparse", 0.02, "", lambda w, f: 0),
        ("wf_very_sparse", "worker_firm_very_sparse", 0.005, "", lambda w, f: 0),
        ("wfy5", "worker_firm_year5", 0.02, "year", lambda w, f: 5),
        ("wfy20", "worker_firm_year20", 0.02, "year", lambda w, f: 20),
        ("wfc20", "worker_firm_county20", 0.02, "county", lambda w, f: 20),
        ("wf_balanced3", "worker_firm_balanced_third", 0.02, "region", lambda w, f: max(80, w // 4)),
    ]
    for short, topology, mobility, third_name, third_rule in topology_specs:
        for n in (10_000, 25_000, 50_000, 100_000):
            workers = n // 5
            firms = max(50, math.ceil(workers / 20))
            third_levels = third_rule(workers, firms)
            cases.append(
                _panel_case(
                    test_id="topology",
                    case_id=f"topology_{short}_n{n}",
                    topology=topology,
                    topology_detail=f"periods5_mobility{mobility:g}",
                    n=n,
                    mobility=mobility,
                    third_name=third_name,
                    third_levels=third_levels,
                )
            )

    # Test 3: all six absorb-order permutations through the 64-column limit.
    for year_levels in (5, 10, 20, 40, 65):
        permutation_seed = _seed(f"cmg_year{year_levels}_shared")
        for order in itertools.permutations(("worker", "firm", "year")):
            order_tag = "_".join(word[0] for word in order)
            cases.append(
                _panel_case(
                    test_id="cmg_order",
                    case_id=f"cmg_year{year_levels}_{order_tag}",
                    topology="worker_firm_year_cmg",
                    topology_detail=f"year_levels{year_levels}",
                    n=50_000,
                    mobility=0.02,
                    third_name="year",
                    third_levels=year_levels,
                    absorb_order=" ".join(order),
                    arms=(AUTO, CG, LSMR8, CMG),
                    seed=permutation_seed,
                )
            )
    cases.append(
        _panel_case(
            test_id="cmg_order",
            case_id="cmg_year66_y_w_f",
            topology="worker_firm_year_cmg",
            topology_detail="year_levels66_beyond_limit",
            n=50_000,
            mobility=0.02,
            third_name="year",
            third_levels=66,
            absorb_order="year worker firm",
            arms=(AUTO, CG, LSMR8, CMG),
        )
    )

    # Test 4: one-axis boundary sweeps plus three interaction traps, for both
    # dense and sparse mobility.  This is intentionally not a full Cartesian
    # product: it isolates each hard guard while remaining a quick calibration.
    boundary_shapes = set()
    boundary_shapes.update((periods, 10, 0) for periods in (4, 8, 16))
    boundary_shapes.update((8, ratio, 0) for ratio in (2, 5, 10, 20))
    boundary_shapes.update((8, 10, levels) for levels in (5, 10, 20, 40, 65))
    boundary_shapes.update({(4, 5, 10), (8, 5, 40), (16, 20, 65)})
    for mobility, mobility_tag in ((0.20, "dense"), (0.02, "sparse")):
        for periods, ratio, nuisance_levels in sorted(boundary_shapes):
            workers = 4_000
            firms = workers // ratio
            n = workers * periods
            third_name = "year" if nuisance_levels else ""
            canonical = ["worker", "firm"] + ([third_name] if nuisance_levels else [])
            absorb_order = (
                f"{third_name} worker firm" if nuisance_levels else " ".join(canonical)
            )
            cases.append(
                _panel_case(
                    test_id="cmg_boundary",
                    case_id=(
                        f"boundary_{mobility_tag}_p{periods}_r{ratio}_n{max(nuisance_levels - 1, 0)}"
                    ),
                    topology="cmg_eligibility_boundary",
                    topology_detail=(
                        f"obs_per_largest{periods}_ratio{ratio}_nuisance{max(nuisance_levels - 1, 0)}_{mobility_tag}"
                    ),
                    n=n,
                    periods=periods,
                    mobility=mobility,
                    firms=firms,
                    third_name=third_name,
                    third_levels=nuisance_levels,
                    absorb_order=absorb_order,
                    arms=(AUTO, CG, LSMR8, CMG),
                )
            )

    # Test 5: RHS and thread sensitivity on three representative designs.
    sensitivity_specs = [
        ("wf_dense", "worker_firm_dense", 0.20, "", 0),
        ("wf_sparse", "worker_firm_sparse", 0.02, "", 0),
        ("wfy5", "worker_firm_year5", 0.02, "year", 5),
    ]
    for short, topology, mobility, third_name, third_levels in sensitivity_specs:
        for controls in (0, 4, 16):
            for threads in (1, 8):
                cases.append(
                    _panel_case(
                        test_id="rhs_threads",
                        case_id=f"sensitivity_{short}_k{controls}_t{threads}",
                        topology=topology,
                        topology_detail=f"rhs{controls + 1}_threads{threads}",
                        n=50_000,
                        mobility=mobility,
                        controls=controls,
                        threads=threads,
                        third_name=third_name,
                        third_levels=third_levels,
                        arms=(AUTO, MAP, CG, LSMR8, CMG),
                    )
                )

    # Focused out-of-sample AKM validation.  Each shape has three independent
    # dataset seeds; every solver cell still has exactly three timed repeats.
    # These cases concentrate on the proposed observations-per-worker,
    # worker/firm-ratio, and 4--11 nuisance-column decision boundaries.
    validation_shapes: list[dict[str, object]] = []

    for n in (15_000, 30_000, 60_000):
        validation_shapes.append(
            dict(
                tag=f"simple_n{n}", n=n, periods=3, ratio=20, mobility=0.15,
                third_name="", third_levels=0, controls=4, threads=8,
            )
        )
    for periods, ratio, mobility, tag in (
        (3, 5, 0.15, "p3r5_m15"),
        (4, 5, 0.02, "p4r5_m02"),
        (5, 5, 0.02, "p5r5_m02"),
        (5, 20, 0.005, "p5r20_m005"),
        (5, 20, 0.02, "p5r20_m02"),
        (5, 20, 0.20, "p5r20_m20"),
        (8, 2, 0.02, "p8r2_m02"),
    ):
        workers = 10_000
        validation_shapes.append(
            dict(
                tag=tag, n=workers * periods, periods=periods, ratio=ratio,
                mobility=mobility, third_name="", third_levels=0, controls=4, threads=8,
            )
        )
    for levels in (5, 6, 8, 10, 12, 20):
        validation_shapes.append(
            dict(
                tag=f"year{levels}", n=50_000, periods=5, ratio=20, mobility=0.02,
                third_name="year", third_levels=levels, controls=4, threads=8,
            )
        )
    for levels in (5, 8, 10, 12):
        validation_shapes.append(
            dict(
                tag=f"county{levels}", n=50_000, periods=5, ratio=20, mobility=0.02,
                third_name="county", third_levels=levels, controls=4, threads=8,
            )
        )
    for levels in (5, 10):
        for controls in (0, 16):
            validation_shapes.append(
                dict(
                    tag=f"year{levels}_k{controls}", n=50_000, periods=5, ratio=20,
                    mobility=0.02, third_name="year", third_levels=levels,
                    controls=controls, threads=8,
                )
            )
        validation_shapes.append(
            dict(
                tag=f"year{levels}_t1", n=50_000, periods=5, ratio=20, mobility=0.02,
                third_name="year", third_levels=levels, controls=4, threads=1,
            )
        )
        for n in (25_000, 100_000):
            validation_shapes.append(
                dict(
                    tag=f"year{levels}_n{n}", n=n, periods=5, ratio=20, mobility=0.02,
                    third_name="year", third_levels=levels, controls=4, threads=8,
                )
            )

    for shape in validation_shapes:
        tag = str(shape["tag"])
        n = int(shape["n"])
        periods = int(shape["periods"])
        workers = n // periods
        firms = max(2, workers // int(shape["ratio"]))
        for seed_index in range(1, 4):
            cases.append(
                _panel_case(
                    test_id="akm_validation",
                    case_id=f"val_{tag}_s{seed_index}",
                    topology="akm_out_of_sample",
                    topology_detail=(
                        f"{tag}_seed{seed_index}_ratio{shape['ratio']}"
                    ),
                    n=n,
                    periods=periods,
                    mobility=float(shape["mobility"]),
                    controls=int(shape["controls"]),
                    threads=int(shape["threads"]),
                    firms=firms,
                    third_name=str(shape["third_name"]),
                    third_levels=int(shape["third_levels"]),
                    arms=(AUTO, CG, LSMR8, CMG),
                    seed=_seed(f"akm_validation_{tag}_seed{seed_index}"),
                )
            )

    # Remaining balanced-topology diagnostic.  The dense core crosses panel
    # depth and worker/firm cardinality while concentrating mobility tails at
    # the known 5:1 conflict.  Every core/nuisance shape has three independent
    # data seeds; RHS/thread/size sensitivity is a smaller one-seed overlay.
    balanced_periods = (3, 4, 5, 6, 8, 12)
    balanced_ratios = (2.0, 3.0, 5.0, 7.5, 10.0)
    balanced_shapes = {
        (periods, ratio, mobility)
        for periods in balanced_periods
        for ratio in balanced_ratios
        for mobility in (0.02, 0.20)
    }
    balanced_shapes.update(
        (periods, 5.0, mobility)
        for periods in balanced_periods
        for mobility in (0.005, 0.10)
    )
    for periods, ratio, mobility in sorted(balanced_shapes):
        workers = 8_000
        firms = max(2, round(workers / ratio))
        ratio_tag = str(ratio).replace(".", "")
        mobility_tag = f"{mobility:.3f}".replace(".", "").rstrip("0")
        for seed_index in range(1, 4):
            diagnostic = (
                seed_index == 1
                and mobility == 0.02
                and periods in (3, 5, 8, 12)
                and ratio in (2.0, 5.0, 10.0)
            )
            arms = (AUTO, LSMR8, LSMR10, CMG)
            if diagnostic:
                arms = (AUTO, MAP, CG, LSMR8, LSMR10, CMG)
            tag = f"p{periods}_r{ratio_tag}_m{mobility_tag}_s{seed_index}"
            cases.append(
                _panel_case(
                    test_id="balanced_diagnostic",
                    case_id=f"bal_{tag}",
                    topology="balanced_worker_firm",
                    topology_detail=(
                        f"periods{periods}_ratio{ratio:g}_mobility{mobility:g}_seed{seed_index}"
                    ),
                    n=workers * periods,
                    periods=periods,
                    mobility=mobility,
                    firms=firms,
                    arms=arms,
                    seed=_seed(f"balanced_{tag}"),
                )
            )

    for periods in (4, 5, 8):
        for ratio in (5.0, 10.0):
            for mobility in (0.02, 0.20):
                for nuisance_columns in (4, 9, 10, 19):
                    workers = 8_000
                    firms = max(2, round(workers / ratio))
                    ratio_tag = str(ratio).replace(".", "")
                    mobility_tag = f"{mobility:.3f}".replace(".", "").rstrip("0")
                    for seed_index in range(1, 4):
                        tag = (
                            f"p{periods}_r{ratio_tag}_m{mobility_tag}_q{nuisance_columns}_s{seed_index}"
                        )
                        cases.append(
                            _panel_case(
                                test_id="balanced_diagnostic",
                                case_id=f"baln_{tag}",
                                topology="balanced_worker_firm_nuisance",
                                topology_detail=(
                                    f"periods{periods}_ratio{ratio:g}_mobility{mobility:g}_nuisance{nuisance_columns}_seed{seed_index}"
                                ),
                                n=workers * periods,
                                periods=periods,
                                mobility=mobility,
                                firms=firms,
                                third_name="year",
                                third_levels=nuisance_columns + 1,
                                arms=(AUTO, LSMR8, LSMR10, CMG),
                                seed=_seed(f"balanced_nuisance_{tag}"),
                            )
                        )

    for periods, ratio in ((5, 5.0), (8, 2.0), (8, 5.0)):
        ratio_tag = str(ratio).replace(".", "")
        sensitivity_cells = [
            (target_n, controls, 8)
            for target_n in (25_000, 50_000, 100_000)
            for controls in (0, 4, 16)
        ]
        sensitivity_cells.extend((50_000, controls, 1) for controls in (0, 4, 16))
        for target_n, controls, threads in sensitivity_cells:
            workers = max(2, round(target_n / periods))
            n = workers * periods
            firms = max(2, round(workers / ratio))
            tag = f"p{periods}_r{ratio_tag}_n{n}_k{controls}_t{threads}"
            matched_seed = _seed(f"balanced_sensitivity_p{periods}_r{ratio_tag}_n{n}")
            cases.append(
                _panel_case(
                    test_id="balanced_diagnostic",
                    case_id=f"bals_{tag}",
                    topology="balanced_worker_firm_sensitivity",
                    topology_detail=(
                        f"periods{periods}_ratio{ratio:g}_mobility0.02_n{n}_controls{controls}_threads{threads}"
                    ),
                    n=n,
                    periods=periods,
                    mobility=0.02,
                    controls=controls,
                    threads=threads,
                    firms=firms,
                    arms=(AUTO, LSMR8, LSMR10, CMG),
                    seed=matched_seed,
                )
            )

        # Repeat the matched 50k, 4/16-control cells with a deliberately
        # narrow low-memory pool.  This forces two or five RHS blocks and
        # exposes any solver setup paid again across blocks without turning
        # the main matrix into a memory-mode benchmark.
        for controls in (4, 16):
            for threads in (1, 8):
                target_n = 50_000
                workers = max(2, round(target_n / periods))
                n = workers * periods
                firms = max(2, round(workers / ratio))
                matched_tag = f"p{periods}_r{ratio_tag}_n{n}_k{controls}_t{threads}"
                low_tag = f"{matched_tag}_low_p4"
                matched_seed = _seed(f"balanced_sensitivity_p{periods}_r{ratio_tag}_n{n}")
                cases.append(
                    _panel_case(
                        test_id="balanced_diagnostic",
                        case_id=f"bals_{low_tag}",
                        topology="balanced_worker_firm_sensitivity",
                        topology_detail=(
                            f"periods{periods}_ratio{ratio:g}_mobility0.02_n{n}_controls{controls}_threads{threads}_memorylow_pool4"
                        ),
                        n=n,
                        periods=periods,
                        mobility=0.02,
                        controls=controls,
                        threads=threads,
                        memory_mode="low",
                        poolsize=4,
                        firms=firms,
                        arms=(AUTO, LSMR8, LSMR10, CMG),
                        seed=matched_seed,
                    )
                )
    return cases


def _stata_preamble(threads: int) -> list[str]:
    return [
        "version 18.0",
        "clear all",
        "set more off",
        "set linesize 255",
        "set rng mt64",
        f"set processors {threads}",
        f'adopath ++ "{(ROOT / "stata").as_posix()}"',
        f'adopath ++ "{(ROOT / "dist").as_posix()}"',
    ]


def _dataset_lines(case: Case) -> list[str]:
    lines = [
        f"set seed {case.seed}",
        f"set obs {case.n}",
        "generate long obs_id = _n",
        f"generate long worker = ceil(_n / {case.periods})",
        "bysort worker: generate int period = _n",
        (
            "bysort worker (period): generate long firm = "
            f"mod(7919*worker - 1, {case.firms}) + 1 if _n == 1"
        ),
        (
            "bysort worker (period): replace firm = "
            f"cond(runiform() < {case.mobility:.17g}, "
            f"mod(104729*worker + 7919*period - 1, {case.firms}) + 1, "
            "firm[_n-1]) if _n > 1"
        ),
    ]
    if case.third_levels:
        if case.third_name == "county":
            lines.append(
                "generate long county = "
                f"mod(firm + 17*worker + 1049*period - 1, {case.third_levels}) + 1"
            )
        else:
            lines.append(
                f"generate long {case.third_name} = "
                f"mod(obs_id - 1, {case.third_levels}) + 1"
            )
    lines.extend(
        [
            "sort worker firm obs_id",
            "by worker firm: generate byte __edge_first = (_n == 1)",
            "quietly count if __edge_first",
            "scalar __graph_unique_edges = r(N)",
            "scalar __graph_duplicate_fraction = 1 - __graph_unique_edges / _N",
            f"scalar __graph_edges_per_vertex = __graph_unique_edges / ({case.workers} + {case.firms})",
            "bysort worker: egen int __worker_degree = total(__edge_first)",
            "by worker: generate byte __worker_first = (_n == 1)",
            "quietly summarize __worker_degree if __worker_first, detail",
            "scalar __graph_worker_degree_mean = r(mean)",
            "scalar __graph_worker_degree_p50 = r(p50)",
            "scalar __graph_worker_degree_p90 = r(p90)",
            "scalar __graph_worker_degree_max = r(max)",
            "bysort firm: egen int __firm_degree = total(__edge_first)",
            "by firm: generate byte __firm_first = (_n == 1)",
            "quietly summarize __firm_degree if __firm_first, detail",
            "scalar __graph_firm_degree_mean = r(mean)",
            "scalar __graph_firm_degree_p50 = r(p50)",
            "scalar __graph_firm_degree_p90 = r(p90)",
            "scalar __graph_firm_degree_max = r(max)",
            "drop __edge_first __worker_degree __worker_first __firm_degree __firm_first",
            "sort obs_id",
        ]
    )
    lines.extend(
        [
            "bysort worker: generate double __worker_fe = rnormal(0,.4) if _n == 1",
            "bysort worker: replace __worker_fe = __worker_fe[1]",
            "bysort firm: generate double __firm_fe = rnormal(0,.2) if _n == 1",
            "bysort firm: replace __firm_fe = __firm_fe[1]",
        ]
    )
    if case.third_levels:
        lines.extend(
            [
                f"bysort {case.third_name}: generate double __third_fe = rnormal(0,.1) if _n == 1",
                f"bysort {case.third_name}: replace __third_fe = __third_fe[1]",
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
    outcome = "generate double y = 1 + __xb + __worker_fe + __firm_fe + rnormal(0,.5)"
    lines.append(outcome)
    if case.third_levels:
        lines.append("replace y = y + __third_fe")
    lines.append("sort obs_id")
    lines.append(f"assert _N == {case.n}")
    return lines


POST_VARIABLES = [
    "case_id", "test_id", "topology", "topology_detail", "seed", "n", "periods",
    "workers", "firms", "third_levels", "fe_dimensions", "mobility", "controls",
    "rhs", "threads", "memory_mode", "poolsize", "absorb_order", "unique_edges", "duplicate_edge_fraction",
    "edges_per_vertex", "worker_degree_mean", "worker_degree_p50", "worker_degree_p90",
    "worker_degree_max", "firm_degree_mean", "firm_degree_p50", "firm_degree_p90",
    "firm_degree_max", "repetition", "sequence", "arm_id",
    "requested_solver", "requested_preconditioner", "requested_tolerance", "rc",
    "command_seconds", "requested_solver_code", "used_solver_code", "iterations",
    "stop_reason", "certificate_passed", "absolute_infinity_norm",
    "relative_infinity_norm", "relative_two_norm", "normal_equation_residual",
    "solver_setup_seconds", "solver_solve_seconds", "fallback", "warning_count",
    "requested_preconditioner_code", "used_preconditioner_code", "pilot_iterations",
    "pilot_contraction", "residual_used_solver_code", "residual_iterations",
    "residual_fallback", "residual_pilot_iterations", "residual_pilot_contraction",
    "native_seconds", "precommit_seconds", "effective_threads", "reported_tolerance",
    "retained_level_1", "retained_level_2", "retained_level_3", "rss", "b1", "b2",
    "b3", "b4", "graph_components", "warning",
]


def _postfile_line(output: Path) -> str:
    return (
        "postfile __runs str48 case_id str20 test_id str40 topology str64 topology_detail "
        "long seed long n int periods long workers long firms long third_levels byte fe_dimensions "
        "double mobility int controls int rhs int threads str8 memory_mode int poolsize str32 absorb_order "
        "double unique_edges duplicate_edge_fraction edges_per_vertex worker_degree_mean "
        "worker_degree_p50 worker_degree_p90 worker_degree_max firm_degree_mean firm_degree_p50 "
        "firm_degree_p90 firm_degree_max int repetition int sequence "
        "str28 arm_id str12 requested_solver str16 requested_preconditioner double requested_tolerance "
        "int rc double command_seconds requested_solver_code used_solver_code iterations stop_reason "
        "certificate_passed absolute_infinity_norm relative_infinity_norm relative_two_norm "
        "normal_equation_residual solver_setup_seconds solver_solve_seconds fallback warning_count "
        "requested_preconditioner_code used_preconditioner_code pilot_iterations pilot_contraction "
        "residual_used_solver_code residual_iterations residual_fallback residual_pilot_iterations "
        "residual_pilot_contraction native_seconds precommit_seconds effective_threads reported_tolerance "
        "retained_level_1 retained_level_2 retained_level_3 rss b1 b2 b3 b4 graph_components "
        "str48 warning "
        f'using "{output.as_posix()}", replace'
    )


def _run_block(
    case: Case,
    arm: Arm,
    repetition: int,
    sequence: int,
    *,
    noisy: bool = False,
) -> list[str]:
    command = arm.command(case)
    warning = (
        'cond(__fallback == 1 & __req == 1 & __used == 4 & __pilot_contract >= .008, '
        '"auto_lsmr_failed_then_cg", '
        'cond(__fallback == 1 & __req == 7 & __used == 4, "cmg_failed_then_cg", '
        'cond(__warnings > 0, "solver_warning", "")))'
    )
    metadata = [
        f'("{case.case_id}")', f'("{case.test_id}")', f'("{case.topology}")',
        f'("{case.topology_detail}")', f'({case.seed})', f'({case.n})',
        f'({case.periods})', f'({case.workers})', f'({case.firms})',
        f'({case.third_levels})', f'({case.fe_dimensions})', f'({case.mobility:.17g})',
        f'({case.controls})', f'({case.controls + 1})', f'({case.threads})',
        f'("{case.memory_mode}")', f'({case.poolsize})',
        f'("{case.absorb_order}")', "(__graph_unique_edges)",
        "(__graph_duplicate_fraction)", "(__graph_edges_per_vertex)",
        "(__graph_worker_degree_mean)", "(__graph_worker_degree_p50)",
        "(__graph_worker_degree_p90)", "(__graph_worker_degree_max)",
        "(__graph_firm_degree_mean)", "(__graph_firm_degree_p50)",
        "(__graph_firm_degree_p90)", "(__graph_firm_degree_max)",
        f'({repetition})', f'({sequence})',
        f'("{arm.arm_id}")', f'("{arm.solver}")', f'("{arm.preconditioner}")',
        f'({arm.tolerance:.17g})',
    ]
    diagnostics = [
        "(__rc)", "(__seconds)", "(__req)", "(__used)", "(__iterations)",
        "(__stop)", "(__certified)", "(__absinf)", "(__relinf)", "(__rel2)",
        "(__normal)", "(__setup)", "(__solve)", "(__fallback)", "(__warnings)",
        "(__preq)", "(__pused)", "(__pilot_iters)", "(__pilot_contract)",
        "(__res_used)", "(__res_iters)", "(__res_fallback)", "(__res_pilot_iters)",
        "(__res_pilot_contract)", "(__native)", "(__precommit)", "(__effective)",
        "(__reported_tol)", "(__level1)", "(__level2)", "(__level3)", "(__rss)",
        "(__b1)", "(__b2)", "(__b3)", "(__b4)", "(__graph_components)",
        "(`\"`__warning'\"')",
    ]
    return [
        "timer clear 91",
        "timer on 91",
        f"capture {'noisily' if noisy else 'quietly'} {command}",
        "scalar __rc = _rc",
        "timer off 91",
        "quietly timer list 91",
        "scalar __seconds = r(t91)",
        "timer clear 91",
        (
            "foreach __name in req used iterations stop certified absinf relinf rel2 normal setup solve "
            "fallback warnings preq pused pilot_iters pilot_contract res_used res_iters res_fallback "
            "res_pilot_iters res_pilot_contract native precommit effective reported_tol level1 level2 "
            "level3 rss b1 b2 b3 b4 graph_components {"
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
        "    scalar __res_used = __solver[2,2]",
        "    scalar __res_iters = __solver[2,3]",
        "    scalar __res_fallback = __solver[2,11]",
        "    scalar __res_pilot_iters = __solver[2,15]",
        "    scalar __res_pilot_contract = __solver[2,16]",
        "    scalar __native = __runtime[20,2]",
        "    scalar __precommit = __runtime[21,2]",
        "    scalar __effective = __runtime[29,2]",
        "    scalar __reported_tol = __runtime[23,2]",
        "    if rowsof(__fe) >= 1 scalar __level1 = __fe[1,4]",
        "    if rowsof(__fe) >= 2 scalar __level2 = __fe[2,4]",
        "    if rowsof(__fe) >= 3 scalar __level3 = __fe[3,4]",
        "    if rowsof(__dof) >= 2 scalar __graph_components = __dof[2,2]",
        "    scalar __rss = e(rss)",
        "    capture scalar __b1 = _b[x1]",
        "    capture scalar __b2 = _b[x2]",
        "    capture scalar __b3 = _b[x3]",
        "    capture scalar __b4 = _b[x4]",
        "}",
        f"local __warning = {warning}",
        "post __runs " + " ".join(metadata + diagnostics),
    ]


def _case_do(case: Case, output_dta: Path, *, noisy: bool = False) -> str:
    lines = _stata_preamble(max(case.threads, 8)) + _dataset_lines(case)
    lines.append(_postfile_line(output_dta))
    # One unrecorded warm-up per arm on the already-generated data.
    for arm in case.arms:
        lines.append(f"capture quietly {arm.command(case)}")
    for repetition in range(1, REPETITIONS + 1):
        rotated = case.arms[repetition - 1 :] + case.arms[: repetition - 1]
        for sequence, arm in enumerate(rotated, start=1):
            lines.extend(_run_block(case, arm, repetition, sequence, noisy=noisy))
    lines.extend(
        [
            "postclose __runs",
            f'use "{output_dta.as_posix()}", clear',
            f'export delimited using "{output_dta.with_suffix(".csv").as_posix()}", replace',
            f'display as result "CALIBRATION_CASE|case={case.case_id}|rows=" _N "|rc=0"',
            "exit 0",
        ]
    )
    return "\n".join(lines) + "\n"


def _run_stata(stata: Path, do_file: Path, cwd: Path, trace: bool = False) -> str:
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
    if completed.returncode != 0:
        raise RuntimeError(
            f"Stata failed for {do_file.name} with process code {completed.returncode}\n{output[-6000:]}"
        )
    return output


def _read_case_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if len(rows) == 0:
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
    summaries: list[dict[str, object]] = []
    median_fields = [
        "command_seconds", "native_seconds", "solver_setup_seconds", "solver_solve_seconds",
        "iterations", "pilot_iterations", "pilot_contraction", "fallback", "warning_count",
        "used_solver_code", "used_preconditioner_code", "stop_reason", "certificate_passed",
        "absolute_infinity_norm", "relative_infinity_norm", "relative_two_norm",
        "normal_equation_residual", "effective_threads", "reported_tolerance", "rss", "b1",
        "b2", "b3", "b4", "graph_components",
    ]
    for (case_id, arm_id), rows in sorted(groups.items()):
        if len(rows) != REPETITIONS:
            raise RuntimeError(f"{case_id}/{arm_id}: expected {REPETITIONS} recorded runs, found {len(rows)}")
        first = rows[0]
        result: dict[str, object] = {
            field: first[field]
            for field in (
                "case_id", "test_id", "topology", "topology_detail", "seed", "n", "periods",
                "workers", "firms", "third_levels", "fe_dimensions", "mobility", "controls", "rhs",
                "threads", "memory_mode", "poolsize", "absorb_order", "unique_edges", "duplicate_edge_fraction",
                "edges_per_vertex", "worker_degree_mean", "worker_degree_p50",
                "worker_degree_p90", "worker_degree_max", "firm_degree_mean",
                "firm_degree_p50", "firm_degree_p90", "firm_degree_max", "arm_id",
                "requested_solver", "requested_preconditioner", "requested_tolerance",
            )
        }
        rcs = [_number(row, "rc") for row in rows]
        certified = [_number(row, "certificate_passed") for row in rows]
        result["recorded_repetitions"] = len(rows)
        result["successful_repetitions"] = sum(value == 0 for value in rcs)
        result["certified_repetitions"] = sum(value == 1 for value in certified)
        fallback_values = {_number(row, "fallback") for row in rows}
        result["certified_run"] = int(
            result["successful_repetitions"] == REPETITIONS
            and result["certified_repetitions"] == REPETITIONS
        )
        # A requested solver that reaches a different solver through a safety
        # fallback remains a correctness success, but is not an eligible
        # winner for calibration of the requested route.
        result["eligible"] = int(
            result["certified_run"] == 1 and fallback_values == {0.0}
        )
        for field in median_fields:
            values = [_number(row, field) for row in rows]
            finite = [value for value in values if value is not None]
            result[f"median_{field}"] = statistics.median(finite) if finite else ""
        result["fallback_deterministic"] = int(len(fallback_values) == 1)
        used_values = {_number(row, "used_solver_code") for row in rows}
        result["route_deterministic"] = int(len(used_values) == 1)
        summaries.append(result)
    return summaries


def add_decisions(summary: list[dict[str, object]]) -> list[dict[str, object]]:
    by_case: dict[str, list[dict[str, object]]] = {}
    for row in summary:
        by_case.setdefault(str(row["case_id"]), []).append(row)
    decisions: list[dict[str, object]] = []
    for case_id, rows in sorted(by_case.items()):
        auto = next((row for row in rows if row["arm_id"] == "auto"), None)
        explicit = [
            row for row in rows
            if row["arm_id"] != "auto" and int(row["eligible"]) == 1
        ]
        best = min(explicit, key=lambda row: float(row["median_command_seconds"])) if explicit else None
        first = rows[0]
        auto_time = (
            float(auto["median_command_seconds"])
            if auto and int(auto["certified_run"]) == 1
            else None
        )
        best_time = float(best["median_command_seconds"]) if best else None
        decisions.append(
            {
                "case_id": case_id,
                "test_id": first["test_id"],
                "topology": first["topology"],
                "topology_detail": first["topology_detail"],
                "n": first["n"],
                "controls": first["controls"],
                "rhs": first["rhs"],
                "threads": first["threads"],
                "memory_mode": first["memory_mode"],
                "poolsize": first["poolsize"],
                "absorb_order": first["absorb_order"],
                "best_certified_explicit_solver": best["arm_id"] if best else "none",
                "best_explicit_seconds": best_time if best_time is not None else "",
                "auto_used_solver_code": auto["median_used_solver_code"] if auto else "",
                "auto_selected_solver": (
                    SOLVER_LABELS.get(int(float(auto["median_used_solver_code"])), "unknown")
                    if auto and auto["median_used_solver_code"] != ""
                    else ""
                ),
                "auto_seconds": auto_time if auto_time is not None else "",
                "auto_fallback": auto["median_fallback"] if auto else "",
                "auto_pilot_contraction": auto["median_pilot_contraction"] if auto else "",
                "auto_regret_percent": (
                    100.0 * (auto_time / best_time - 1.0)
                    if auto_time is not None and best_time is not None and best_time > 0
                    else ""
                ),
            }
        )
    return decisions


def _summary_index(summary: list[dict[str, object]]) -> dict[tuple[str, str], dict[str, object]]:
    return {(str(row["case_id"]), str(row["arm_id"])): row for row in summary}


def make_cliff_table(
    summary: list[dict[str, object]], decisions: list[dict[str, object]]
) -> list[dict[str, object]]:
    indexed = _summary_index(summary)
    arms = ("auto", "map", "cg", "lsmr_schwarz_1e8", "lsmr_schwarz_1e10", "cmg")
    rows: list[dict[str, object]] = []
    for decision in sorted(
        (row for row in decisions if row["test_id"] == "cliff"),
        key=lambda row: int(row["n"]),
    ):
        case_id = str(decision["case_id"])
        output: dict[str, object] = {
            "case_id": case_id,
            "n": decision["n"],
            "auto_selected_solver": decision["auto_selected_solver"],
            "auto_fallback": decision["auto_fallback"],
            "auto_pilot_contraction": decision["auto_pilot_contraction"],
            "best_certified_explicit_solver": decision["best_certified_explicit_solver"],
            "auto_regret_percent": decision["auto_regret_percent"],
        }
        for arm in arms:
            cell = indexed.get((case_id, arm))
            output[f"{arm}_median_seconds"] = cell["median_command_seconds"] if cell else ""
            output[f"{arm}_eligible"] = cell["eligible"] if cell else ""
        rows.append(output)
    return rows


def make_cmg_order_tables(
    summary: list[dict[str, object]],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    selected = [row for row in summary if row["test_id"] == "cmg_order"]
    by_case: dict[str, dict[str, dict[str, object]]] = {}
    for row in selected:
        by_case.setdefault(str(row["case_id"]), {})[str(row["arm_id"])] = row

    detail: list[dict[str, object]] = []
    for case_id, arms in sorted(by_case.items(), key=lambda item: (int(item[1]["auto"]["third_levels"]), item[0])):
        auto = arms["auto"]
        output: dict[str, object] = {
            "case_id": case_id,
            "seed": auto["seed"],
            "year_levels": auto["third_levels"],
            "nuisance_columns": int(auto["third_levels"]) - 1,
            "absorb_order": auto["absorb_order"],
            "auto_selected_solver": SOLVER_LABELS.get(int(float(auto["median_used_solver_code"])), "unknown"),
            "auto_fallback": auto["median_fallback"],
            "auto_median_seconds": auto["median_command_seconds"],
        }
        for arm in ("cg", "lsmr_schwarz_1e8", "cmg"):
            cell = arms.get(arm)
            output[f"{arm}_eligible"] = cell["eligible"] if cell else ""
            output[f"{arm}_median_seconds"] = cell["median_command_seconds"] if cell else ""
        detail.append(output)

    invariance: list[dict[str, object]] = []
    for year_levels in (5, 10, 20, 40, 65, 66):
        for arm_id in ("auto", "cg", "lsmr_schwarz_1e8", "cmg"):
            cells = [
                row for row in selected
                if int(row["third_levels"]) == year_levels and row["arm_id"] == arm_id
            ]
            eligible = [row for row in cells if int(row["eligible"]) == 1]
            if not cells:
                continue
            canonical = next(
                (row for row in eligible if row["absorb_order"] == "worker firm year"),
                eligible[0] if eligible else None,
            )
            times = [float(row["median_command_seconds"]) for row in eligible]
            max_b_difference = ""
            max_rss_relative_difference = ""
            if canonical is not None:
                max_b_difference = max(
                    max(
                        abs(float(row[f"median_b{index}"]) - float(canonical[f"median_b{index}"]))
                        for index in range(1, 5)
                    )
                    for row in eligible
                )
                max_rss_relative_difference = max(
                    abs(float(row["median_rss"]) - float(canonical["median_rss"]))
                    / float(canonical["median_rss"])
                    for row in eligible
                )
            invariance.append(
                {
                    "year_levels": year_levels,
                    "nuisance_columns": year_levels - 1,
                    "arm_id": arm_id,
                    "successful_orders": len(eligible),
                    "requested_orders": len(cells),
                    "min_median_seconds": min(times) if times else "",
                    "max_median_seconds": max(times) if times else "",
                    "max_over_min_runtime": max(times) / min(times) if times else "",
                    "max_abs_coefficient_difference": max_b_difference,
                    "max_relative_rss_difference": max_rss_relative_difference,
                }
            )
    return detail, invariance


def analyze(raw: list[dict[str, str]], output: Path) -> None:
    raw_path = output / "solver_routing_calibration_runs.csv"
    _write_csv(raw_path, raw, POST_VARIABLES)
    summary = summarize(raw)
    _write_csv(output / "solver_routing_calibration_summary.csv", summary, list(summary[0]))
    decisions = add_decisions(summary)
    _write_csv(output / "routing_decision_table.csv", decisions, list(decisions[0]))
    topology_sizes = [row for row in decisions if row["test_id"] == "topology"]
    if topology_sizes:
        _write_csv(output / "topology_size_table.csv", topology_sizes, list(topology_sizes[0]))

    cliff = make_cliff_table(summary, decisions)
    if cliff:
        _write_csv(output / "cliff_table.csv", cliff, list(cliff[0]))
    cmg_detail, cmg_invariance = make_cmg_order_tables(summary)
    if cmg_detail:
        _write_csv(output / "cmg_order_table.csv", cmg_detail, list(cmg_detail[0]))
        _write_csv(output / "cmg_order_invariance.csv", cmg_invariance, list(cmg_invariance[0]))
    boundary = [row for row in decisions if row["test_id"] == "cmg_boundary"]
    if boundary:
        _write_csv(output / "cmg_boundary_table.csv", boundary, list(boundary[0]))
    balanced = [row for row in summary if row["test_id"] == "balanced_diagnostic"]
    if balanced:
        phase_rows: list[dict[str, object]] = []
        for row in balanced:
            iterations = float(row["median_iterations"] or 0.0)
            solve = float(row["median_solver_solve_seconds"] or 0.0)
            command = float(row["median_command_seconds"] or 0.0)
            setup = float(row["median_solver_setup_seconds"] or 0.0)
            phase_rows.append(
                {
                    "case_id": row["case_id"],
                    "topology": row["topology"],
                    "seed": row["seed"],
                    "n": row["n"],
                    "periods": row["periods"],
                    "workers": row["workers"],
                    "firms": row["firms"],
                    "third_levels": row["third_levels"],
                    "mobility": row["mobility"],
                    "controls": row["controls"],
                    "threads": row["threads"],
                    "memory_mode": row["memory_mode"],
                    "poolsize": row["poolsize"],
                    "unique_edges": row["unique_edges"],
                    "duplicate_edge_fraction": row["duplicate_edge_fraction"],
                    "edges_per_vertex": row["edges_per_vertex"],
                    "worker_degree_mean": row["worker_degree_mean"],
                    "worker_degree_p90": row["worker_degree_p90"],
                    "firm_degree_mean": row["firm_degree_mean"],
                    "firm_degree_p90": row["firm_degree_p90"],
                    "graph_components": row["median_graph_components"],
                    "arm_id": row["arm_id"],
                    "eligible": row["eligible"],
                    "median_command_seconds": command,
                    "median_native_seconds": row["median_native_seconds"],
                    "median_setup_seconds": setup,
                    "median_solve_seconds": solve,
                    "median_iterations": iterations,
                    "solve_seconds_per_iteration": solve / iterations if iterations > 0 else "",
                    "setup_share_of_command": setup / command if command > 0 else "",
                    "median_fallback": row["median_fallback"],
                    "median_used_solver_code": row["median_used_solver_code"],
                    "median_used_preconditioner_code": row["median_used_preconditioner_code"],
                }
            )
        _write_csv(output / "balanced_phase_table.csv", phase_rows, list(phase_rows[0]))
    trace_path = output / "fallback_trace.csv"
    if trace_path.is_file():
        trace_rows = _read_case_csv(trace_path)
        by_case: dict[str, list[dict[str, str]]] = {}
        for row in trace_rows:
            by_case.setdefault(row["case_id"], []).append(row)
        phases: list[dict[str, object]] = []
        for case_id, rows in sorted(by_case.items()):
            pilot = next((row for row in rows if row.get("phase") == "pilot"), {})
            lsmr = next((row for row in rows if row.get("phase") == "lsmr"), {})
            cg = next((row for row in rows if row.get("phase") == "cg"), {})
            pilot_setup = float(pilot.get("setup") or 0.0)
            pilot_solve = float(pilot.get("solve") or 0.0)
            cg_reported_setup = float(cg.get("setup") or 0.0)
            cg_reported_solve = float(cg.get("solve") or 0.0)
            phases.append(
                {
                    "case_id": case_id,
                    "n": pilot.get("n", ""),
                    "pilot_contraction": pilot.get("contraction", ""),
                    "pilot_setup_seconds": pilot_setup,
                    "pilot_solve_seconds": pilot_solve,
                    "lsmr_outcome": lsmr.get("outcome", ""),
                    "lsmr_iterations": lsmr.get("iterations", ""),
                    "lsmr_tolerance": lsmr.get("tolerance", ""),
                    "lsmr_relative_infinity_norm": lsmr.get("relinf", ""),
                    "lsmr_elapsed_seconds": lsmr.get("elapsed", ""),
                    "fallback_cg_present": int(bool(cg)),
                    "fallback_cg_iterations": cg.get("iterations", ""),
                    "fallback_cg_setup_seconds_excluding_pilot": max(
                        0.0, cg_reported_setup - pilot_setup
                    ),
                    "fallback_cg_solve_seconds_excluding_pilot": max(
                        0.0, cg_reported_solve - pilot_solve
                    ),
                    "fallback_cg_relative_infinity_norm": cg.get("relinf", ""),
                }
            )
        if phases:
            _write_csv(output / "fallback_phase_table.csv", phases, list(phases[0]))
    make_cliff_plot(summary, output / "cliff_plot.png")


def _trace_do(case: Case) -> str:
    lines = _stata_preamble(max(case.threads, 8)) + _dataset_lines(case)
    lines.extend(
        [
            f'display as result "TRACE_BEGIN|case={case.case_id}"',
            f"capture noisily {AUTO.command(case)}",
            "local __rc = _rc",
            f'display as result "TRACE_END|case={case.case_id}|rc=" `__rc\'',
            "exit 0",
        ]
    )
    return "\n".join(lines) + "\n"


def parse_trace(case: Case, output: str) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    sequence = 0
    for line in output.splitlines():
        marker = "FEREG_SOLVER_TRACE|"
        if marker not in line:
            continue
        payload = line[line.index(marker) :].strip()
        values: dict[str, object] = {
            "case_id": case.case_id,
            "n": case.n,
            "trace_sequence": sequence,
        }
        for token in payload.split("|")[1:]:
            if "=" not in token:
                continue
            key, value = token.split("=", 1)
            values[key] = value
        if values.get("phase") == "pilot":
            sequence += 1
            values["trace_sequence"] = sequence
        else:
            values["trace_sequence"] = max(sequence, 1)
        rows.append(values)
    return rows


def make_cliff_plot(summary: list[dict[str, object]], output: Path) -> None:
    try:
        import matplotlib.pyplot as plt
    except ImportError:
        return
    rows = [row for row in summary if row["test_id"] == "cliff" and int(row["eligible"]) == 1]
    labels = {
        "auto": "auto",
        "map": "MAP",
        "cg": "CG",
        "lsmr_schwarz_1e8": "Schwarz-LSMR 1e-8",
        "lsmr_schwarz_1e10": "Schwarz-LSMR 1e-10",
        "cmg": "CMG",
    }
    fig, axis = plt.subplots(figsize=(8.2, 4.8))
    for arm_id, label in labels.items():
        selected = sorted(
            (row for row in rows if row["arm_id"] == arm_id),
            key=lambda row: int(row["n"]),
        )
        if not selected:
            continue
        axis.plot(
            [int(row["n"]) for row in selected],
            [float(row["median_command_seconds"]) for row in selected],
            marker="o",
            linewidth=1.5,
            label=label,
        )
    auto_fallback = [
        row for row in rows if row["arm_id"] == "auto" and float(row["median_fallback"]) == 1
    ]
    if auto_fallback:
        axis.scatter(
            [int(row["n"]) for row in auto_fallback],
            [float(row["median_command_seconds"]) for row in auto_fallback],
            marker="x",
            s=70,
            color="red",
            label="auto fallback",
            zorder=5,
        )
    axis.set_xlabel("Observations")
    axis.set_ylabel("Median command time (seconds)")
    axis.set_title("three_fe_sparse: 10k–30k solver cliff")
    axis.grid(alpha=0.25)
    axis.legend(fontsize=8, ncol=2)
    fig.tight_layout()
    fig.savefig(output, dpi=180)
    plt.close(fig)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--section", action="append", choices=[
        "cliff", "topology", "cmg_order", "cmg_boundary", "rhs_threads",
        "akm_validation", "balanced_diagnostic",
    ])
    parser.add_argument("--case", action="append", help="Run only the named case id")
    parser.add_argument("--topology", action="append", help="Run only the named topology")
    parser.add_argument(
        "--arm",
        action="append",
        choices=[
            "auto",
            "map",
            "cg",
            "lsmr_schwarz_1e8",
            "lsmr_schwarz_1e10",
            "cmg",
            "lsmr_diagonal_1e8",
            "lsmr_off_1e8",
        ],
        help="Run only the named arm; may be repeated",
    )
    parser.add_argument("--smoke", action="store_true", help="Run one case from each selected section")
    parser.add_argument("--skip-trace", action="store_true")
    parser.add_argument(
        "--noisy",
        action="store_true",
        help="Print captured fereg errors for focused diagnostic reruns",
    )
    parser.add_argument("--trace-only", action="store_true", help="Run unrecorded auto traces only")
    parser.add_argument("--resume", action="store_true", help="Reuse complete per-case CSV files")
    parser.add_argument(
        "--analyze-only",
        action="store_true",
        help="Regenerate summaries from the existing raw CSV without running Stata",
    )
    parser.add_argument(
        "--replacement-raw",
        type=Path,
        help="With --analyze-only, replace matching case ids from this raw CSV",
    )
    parser.add_argument("--list-plan", action="store_true")
    args = parser.parse_args(argv)

    if not args.analyze_only and not args.stata.is_file():
        raise RuntimeError(f"Stata executable not found: {args.stata}")
    if not args.analyze_only and not (ROOT / "dist/fereg_macos.plugin").is_file():
        raise RuntimeError("dist/fereg_macos.plugin is missing; run scripts/build_macos.sh first")

    plan = build_plan()
    if args.section:
        wanted = set(args.section)
        plan = [case for case in plan if case.test_id in wanted]
    if args.case:
        wanted_cases = set(args.case)
        plan = [case for case in plan if case.case_id in wanted_cases]
    if args.topology:
        wanted_topologies = set(args.topology)
        plan = [case for case in plan if case.topology in wanted_topologies]
    if args.arm:
        wanted_arms = set(args.arm)
        plan = [
            replace(case, arms=tuple(arm for arm in case.arms if arm.arm_id in wanted_arms))
            for case in plan
        ]
        plan = [case for case in plan if case.arms]
    if args.smoke:
        one_per_section: dict[str, Case] = {}
        for case in plan:
            one_per_section.setdefault(case.test_id, case)
        plan = list(one_per_section.values())
    if args.list_plan:
        for case in plan:
            print(case)
        return 0

    output = args.output.resolve()
    work = output / "work"
    work.mkdir(parents=True, exist_ok=True)
    if args.analyze_only:
        raw_path = output / "solver_routing_calibration_runs.csv"
        raw = _read_case_csv(raw_path)
        if args.replacement_raw is not None:
            replacement = _read_case_csv(args.replacement_raw.resolve())
            replacement_cases = {row["case_id"] for row in replacement}
            raw = [row for row in raw if row["case_id"] not in replacement_cases] + replacement
        analyze(raw, output)
        print(f"raw={raw_path}")
        print(f"summary={output / 'solver_routing_calibration_summary.csv'}")
        print(f"decisions={output / 'routing_decision_table.csv'}")
        return 0
    if args.trace_only:
        trace_rows: list[dict[str, object]] = []
        for case in plan:
            do_file = work / f"trace_{case.case_id}.do"
            do_file.write_text(_trace_do(case), encoding="utf-8")
            trace_rows.extend(parse_trace(case, _run_stata(args.stata, do_file, work, trace=True)))
        if trace_rows:
            trace_fields = sorted({key for row in trace_rows for key in row})
            _write_csv(output / "fallback_trace.csv", trace_rows, trace_fields)
        return 0

    raw: list[dict[str, str]] = []
    for index, case in enumerate(plan, start=1):
        print(f"[{index}/{len(plan)}] {case.case_id}", flush=True)
        do_file = work / f"{case.case_id}.do"
        dta_file = work / f"{case.case_id}_runs.dta"
        case_csv = dta_file.with_suffix(".csv")
        if args.resume and case_csv.is_file():
            cached = _read_case_csv(case_csv)
            expected = len(case.arms) * REPETITIONS
            if len(cached) == expected and all(row.get("case_id") == case.case_id for row in cached):
                raw.extend(cached)
                print(f"  reused {expected} complete rows", flush=True)
                continue
        do_file.write_text(_case_do(case, dta_file, noisy=args.noisy), encoding="utf-8")
        _run_stata(args.stata, do_file, work)
        raw.extend(_read_case_csv(case_csv))

    raw_path = output / "solver_routing_calibration_runs.csv"
    analyze(raw, output)

    trace_rows: list[dict[str, object]] = []
    if not args.skip_trace:
        for case in (case for case in plan if case.test_id == "cliff"):
            do_file = work / f"trace_{case.case_id}.do"
            do_file.write_text(_trace_do(case), encoding="utf-8")
            trace_rows.extend(parse_trace(case, _run_stata(args.stata, do_file, work, trace=True)))
        if trace_rows:
            trace_fields = sorted({key for row in trace_rows for key in row})
            _write_csv(output / "fallback_trace.csv", trace_rows, trace_fields)

    print(f"raw={raw_path}")
    print(f"summary={output / 'solver_routing_calibration_summary.csv'}")
    print(f"decisions={output / 'routing_decision_table.csv'}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"solver routing calibration failed: {error}", file=sys.stderr)
        raise
