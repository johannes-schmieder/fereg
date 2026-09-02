#!/usr/bin/env python3
"""Benchmark PyFixest on the datasets exported by manual_benchmarks.do.

Run manual_benchmarks.do with FEREG_FIXEST_EXPORT_DIR set, then run the R
companion and this script. Package import, dataset loading, and one warm-up fit
per cell are outside the recorded timings. Each requested thread count runs in
a fresh subprocess because PyFixest's native Rust demeaner initializes a
process-wide Rayon pool.
"""

from __future__ import annotations

import argparse
import importlib.metadata
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


CELL_COLUMNS = [
    "problem_type",
    "topology",
    "fe_dimensions",
    "base_n",
    "n_observations",
    "controls",
    "requested_threads",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare PyFixest with the manual Stata benchmark results."
    )
    parser.add_argument("benchmark_runs_csv", type=Path)
    parser.add_argument("dataset_dir", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("--worker-threads", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--worker-output", type=Path, help=argparse.SUPPRESS)
    return parser.parse_args()


def formula_for(problem_type: str) -> str:
    fixed_effects = {
        "two_fe_dense": "worker + firm",
        "two_fe_sparse": "worker + firm",
        "three_fe_sparse": "worker + firm + county",
    }
    try:
        absorbed = fixed_effects[problem_type]
    except KeyError as exc:
        raise ValueError(f"unsupported problem type: {problem_type}") from exc
    return f"y ~ x1 + x2 + x3 + x4 | {absorbed}"


def successful_fereg_cells(benchmark_path: Path):
    import pandas as pd

    benchmark_runs = pd.read_csv(benchmark_path)
    fereg_runs = benchmark_runs[
        (benchmark_runs["command"] == "fereg")
        & (benchmark_runs["status"] == "pass")
    ].copy()
    if fereg_runs.empty:
        raise RuntimeError("no successful fereg rows")
    cells = (
        fereg_runs[CELL_COLUMNS]
        .drop_duplicates()
        .sort_values(["problem_type", "n_observations", "requested_threads"])
        .reset_index(drop=True)
    )
    expected = {"two_fe_dense", "two_fe_sparse", "three_fe_sparse"}
    if set(cells["problem_type"]) != expected:
        raise RuntimeError("unexpected problem types in fereg benchmark rows")
    if len(cells) != 27 or not (cells["controls"] == 4).all():
        raise RuntimeError("expected 27 four-control fereg benchmark cells")
    return benchmark_runs, fereg_runs, cells


def run_worker(args: argparse.Namespace) -> None:
    import numpy as np
    import pandas as pd
    import pyfixest as pf

    if args.worker_output is None:
        raise RuntimeError("worker output path is required")
    _, fereg_runs, cells = successful_fereg_cells(args.benchmark_runs_csv)
    cells = cells[cells["requested_threads"] == args.worker_threads]
    if cells.empty:
        raise RuntimeError(f"no cells request {args.worker_threads} threads")

    dataset_cache: dict[str, pd.DataFrame] = {}
    rows: list[dict[str, object]] = []
    for _, cell in cells.iterrows():
        filename = (
            f"{cell['problem_type']}_base{int(cell['base_n'])}"
            f"_N{int(cell['n_observations'])}.csv"
        )
        if filename not in dataset_cache:
            data = pd.read_csv(args.dataset_dir / filename)
            if len(data) != int(cell["n_observations"]):
                raise RuntimeError(f"row-count mismatch in {filename}")
            dataset_cache[filename] = data
        data = dataset_cache[filename]
        formula = formula_for(str(cell["problem_type"]))
        repetitions = len(
            fereg_runs[
                (fereg_runs["problem_type"] == cell["problem_type"])
                & (fereg_runs["base_n"] == cell["base_n"])
                & (fereg_runs["n_observations"] == cell["n_observations"])
                & (
                    fereg_runs["requested_threads"]
                    == cell["requested_threads"]
                )
            ]
        )
        if repetitions != 5:
            raise RuntimeError(f"expected five repetitions for {filename}")

        # Keep all PyFixest estimator settings at their native defaults. The
        # environment created by the parent limits Rayon and common BLAS pools.
        warm_start = time.perf_counter()
        try:
            pf.feols(formula, data=data)
        except Exception as exc:
            warm_elapsed = time.perf_counter() - warm_start
            for repetition in range(1, repetitions + 1):
                rows.append(
                    {
                        **{column: cell[column] for column in CELL_COLUMNS},
                        "estimator": "pyfixest_native",
                        "repetition": repetition,
                        "status": "fail",
                        "elapsed_seconds": math.nan,
                        "warmup_elapsed_seconds": warm_elapsed,
                        "n_estimation": math.nan,
                        "rss": math.nan,
                        "demeaner": "rust_map",
                        "fixef_tolerance": 1e-6,
                        "fixef_maxiter": 10_000,
                        "rayon_threads": args.worker_threads,
                        "error": f"{type(exc).__name__}: {exc}",
                    }
                )
            print(
                "PYFIXEST_CELL|"
                f"{cell['problem_type']}|N={int(cell['n_observations'])}|"
                f"threads={args.worker_threads}|FAIL|{type(exc).__name__}: {exc}",
                flush=True,
            )
            continue
        for repetition in range(1, repetitions + 1):
            start = time.perf_counter()
            try:
                fit = pf.feols(formula, data=data)
                elapsed = time.perf_counter() - start
                residuals = np.asarray(fit._u_hat)
                rows.append(
                    {
                        **{column: cell[column] for column in CELL_COLUMNS},
                        "estimator": "pyfixest_native",
                        "repetition": repetition,
                        "status": "pass",
                        "elapsed_seconds": elapsed,
                        "warmup_elapsed_seconds": math.nan,
                        "n_estimation": int(fit._N),
                        "rss": float(residuals @ residuals),
                        "demeaner": "rust_map",
                        "fixef_tolerance": float(fit._fixef_tol),
                        "fixef_maxiter": int(fit._fixef_maxiter),
                        "rayon_threads": args.worker_threads,
                        "error": "",
                    }
                )
            except Exception as exc:
                elapsed = time.perf_counter() - start
                rows.append(
                    {
                        **{column: cell[column] for column in CELL_COLUMNS},
                        "estimator": "pyfixest_native",
                        "repetition": repetition,
                        "status": "fail",
                        "elapsed_seconds": elapsed,
                        "warmup_elapsed_seconds": math.nan,
                        "n_estimation": math.nan,
                        "rss": math.nan,
                        "demeaner": "rust_map",
                        "fixef_tolerance": 1e-6,
                        "fixef_maxiter": 10_000,
                        "rayon_threads": args.worker_threads,
                        "error": f"{type(exc).__name__}: {exc}",
                    }
                )
        print(
            "PYFIXEST_CELL|"
            f"{cell['problem_type']}|N={int(cell['n_observations'])}|"
            f"threads={args.worker_threads}|PASS",
            flush=True,
        )

    output = pd.DataFrame(rows)
    output.to_csv(args.worker_output, index=False)


def summarize_runs(data, estimator: str):
    import pandas as pd

    grouped = data.groupby(CELL_COLUMNS, sort=True, dropna=False)
    summary = grouped["elapsed_seconds"].agg(
        median_seconds="median",
        mean_seconds="mean",
        min_seconds="min",
        max_seconds="max",
        successful_runs="count",
    )
    summary = summary.reset_index()
    summary.insert(len(CELL_COLUMNS), "estimator", estimator)
    return summary


def main(args: argparse.Namespace) -> None:
    import pandas as pd

    benchmark_path = args.benchmark_runs_csv.resolve(strict=True)
    dataset_dir = args.dataset_dir.resolve(strict=True)
    output_dir = args.output_dir.resolve(strict=True)
    benchmark_runs, _, cells = successful_fereg_cells(benchmark_path)

    worker_results = []
    with tempfile.TemporaryDirectory(
        prefix="manual_benchmarks_pyfixest_", dir=output_dir
    ) as temporary_directory:
        for threads in sorted(cells["requested_threads"].unique()):
            partial_path = Path(temporary_directory) / f"threads_{threads}.csv"
            environment = os.environ.copy()
            thread_text = str(int(threads))
            environment.update(
                {
                    "RAYON_NUM_THREADS": thread_text,
                    "OPENBLAS_NUM_THREADS": thread_text,
                    "OMP_NUM_THREADS": thread_text,
                    "MKL_NUM_THREADS": thread_text,
                    "VECLIB_MAXIMUM_THREADS": thread_text,
                    "NUMEXPR_NUM_THREADS": thread_text,
                    "MPLCONFIGDIR": str(Path(temporary_directory) / "matplotlib"),
                }
            )
            subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    str(benchmark_path),
                    str(dataset_dir),
                    str(output_dir),
                    "--worker-threads",
                    thread_text,
                    "--worker-output",
                    str(partial_path),
                ],
                env=environment,
                check=True,
            )
            worker_results.append(pd.read_csv(partial_path))

    pyfixest_runs = pd.concat(worker_results, ignore_index=True).sort_values(
        [
            "problem_type",
            "n_observations",
            "requested_threads",
            "repetition",
        ]
    )
    if len(pyfixest_runs) != 135:
        raise RuntimeError("expected 135 PyFixest run rows")
    successful_pyfixest = pyfixest_runs[pyfixest_runs["status"] == "pass"]
    if not (
        successful_pyfixest["n_estimation"]
        == successful_pyfixest["n_observations"]
    ).all():
        raise RuntimeError("PyFixest estimation sample differs from exported data")
    pyfixest_runs.to_csv(
        output_dir / "manual_benchmarks_pyfixest_runs.csv", index=False
    )

    pyfixest_summary = summarize_runs(successful_pyfixest, "pyfixest_native")
    pyfixest_summary = cells.merge(
        pyfixest_summary, on=CELL_COLUMNS, how="left", validate="one_to_one"
    )
    pyfixest_summary["estimator"] = "pyfixest_native"
    pyfixest_summary["successful_runs"] = (
        pyfixest_summary["successful_runs"].fillna(0).astype(int)
    )
    pyfixest_summary["status"] = pyfixest_summary["successful_runs"].map(
        lambda count: "pass" if count == 5 else "fail"
    )
    pyfixest_summary.to_csv(
        output_dir / "manual_benchmarks_pyfixest_summary.csv", index=False
    )

    stata_estimators = []
    for command, estimator in (
        ("fereg", "fereg_auto"),
        ("reghdfe", "reghdfe_native"),
    ):
        selected = benchmark_runs[
            (benchmark_runs["command"] == command)
            & (benchmark_runs["status"] == "pass")
        ].copy()
        selected["estimator"] = estimator
        stata_summary = summarize_runs(selected, estimator)
        stata_summary["status"] = "pass"
        stata_estimators.append(stata_summary)

    fixest_path = output_dir / "manual_benchmarks_fixest_runs.csv"
    if not fixest_path.exists():
        raise RuntimeError(
            "manual_benchmarks_fixest_runs.csv is missing; run "
            "manual_benchmarks_fixest.R first"
        )
    fixest_runs = pd.read_csv(fixest_path)
    fixest_native = fixest_runs[fixest_runs["estimator"] == "fixest_native"]
    if len(fixest_native) != 135:
        raise RuntimeError("expected 135 native Fixest runs")
    fixest_summary = summarize_runs(fixest_native, "fixest_native")
    fixest_summary["status"] = "pass"

    combined = pd.concat(
        [*stata_estimators, fixest_summary, pyfixest_summary], ignore_index=True
    )
    fereg_medians = stata_estimators[0][
        CELL_COLUMNS + ["median_seconds"]
    ].rename(columns={"median_seconds": "fereg_median_seconds"})
    combined = combined.merge(fereg_medians, on=CELL_COLUMNS, how="left")
    combined["relative_to_fereg_auto"] = (
        combined["median_seconds"] / combined["fereg_median_seconds"]
    )
    combined = combined.drop(columns="fereg_median_seconds").sort_values(
        [
            "problem_type",
            "n_observations",
            "requested_threads",
            "estimator",
        ]
    )
    combined.to_csv(
        output_dir / "manual_benchmarks_four_estimator_summary.csv", index=False
    )

    print(f"PYTHON_VERSION={sys.version.split()[0]}")
    print(f"PYFIXEST_VERSION={importlib.metadata.version('pyfixest')}")
    print(
        f"CELLS={len(cells)} RUNS={len(pyfixest_runs)} "
        f"PASS={len(successful_pyfixest)} "
        f"FAIL={len(pyfixest_runs) - len(successful_pyfixest)}"
    )
    print("PYFIXEST_MANUAL_BENCHMARKS|PASS")


if __name__ == "__main__":
    parsed = parse_args()
    if parsed.worker_threads is not None:
        run_worker(parsed)
    else:
        main(parsed)
