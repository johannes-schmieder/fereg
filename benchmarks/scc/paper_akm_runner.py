#!/usr/bin/env python3
"""Run fresh-process Stata timings and external RSS sampling on one SCC host."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import platform
import random
import shutil
import socket
import statistics
import subprocess
import time
from collections import defaultdict
from pathlib import Path


SOURCE_COMMIT = "490e03a3e2458d8bc463c3f60389062e2f8fae42"
PLUGIN_SHA256 = "9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2"
REGHDFE_COMMIT = "29b2b203f534413369d675d0c0f674d61291f536"
REPS = 5
SIZES = (1_000_000, 2_000_000, 4_000_000, 8_000_000)
ARMS = ("fereg", "reghdfe", "areg")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_rss_kib(pid: int) -> int | None:
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1])
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None
    return None


def stata_executable() -> str:
    for name in ("stata-mp", "stata-se", "stata"):
        resolved = shutil.which(name)
        if resolved:
            return resolved
    raise RuntimeError("Stata executable not found after loading stata-mp/19")


def run_stata(stata: str, do_file: Path, args: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    cwd.mkdir(parents=True, exist_ok=True)
    command = [stata, "-b", "do", str(do_file), *args]
    completed = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    (cwd / "runner.stdout").write_text(completed.stdout, encoding="utf-8")
    (cwd / "runner.stderr").write_text(completed.stderr, encoding="utf-8")
    return completed


def generate_data(stata: str, source_tree: Path, data_dir: Path, n: int) -> Path:
    data_dir.mkdir(parents=True, exist_ok=True)
    output = data_dir / f"akm_n{n}.dta"
    if output.exists():
        return output
    work = data_dir / f"generate_n{n}"
    result = run_stata(
        stata,
        source_tree / "benchmarks/stata/paper_akm_generate.do",
        [str(output), str(n), str(20260830 + n // 1_000_000)],
        work,
    )
    if result.returncode != 0 or not output.exists():
        raise RuntimeError(f"dataset generation failed for N={n}: rc={result.returncode}")
    return output


def preflight_cells() -> list[tuple[int, int]]:
    cells = [(n, threads) for n in SIZES for threads in (4, 16)]
    cells.extend((8_000_000, threads) for threads in (1, 2, 8))
    return cells


def measurement_cells() -> list[tuple[int, int, str]]:
    cells: list[tuple[int, int, str]] = []
    for n in SIZES:
        cells.extend((n, 4, arm) for arm in ARMS)
        cells.append((n, 16, "fereg"))
    for threads in (1, 2):
        cells.extend((8_000_000, threads, arm) for arm in ARMS)
    cells.append((8_000_000, 8, "fereg"))
    assert len(cells) == 23
    return cells


def run_preflight(
    stata: str,
    source_tree: Path,
    source_root: Path,
    comparator_root: Path,
    run_root: Path,
    data_path: Path,
    n: int,
    threads: int,
) -> dict[str, str]:
    output = run_root / "preflights" / f"n{n}_t{threads}"
    result = run_stata(
        stata,
        source_tree / "benchmarks/stata/paper_akm_preflight.do",
        [str(source_root), str(output), str(comparator_root), str(data_path), str(n), str(threads)],
        output,
    )
    science = output / "science.csv"
    if result.returncode != 0 or not science.exists() or not (output / "SUCCESS.txt").exists():
        raise RuntimeError(f"scientific preflight failed for N={n}, threads={threads}: rc={result.returncode}")
    with science.open(newline="", encoding="utf-8") as handle:
        row = next(csv.DictReader(handle))
    if row["science_fail"] != "0":
        raise RuntimeError(f"scientific gate failed for N={n}, threads={threads}")
    return row


def run_measurement(
    stata: str,
    source_tree: Path,
    source_root: Path,
    comparator_root: Path,
    run_root: Path,
    data_path: Path,
    n: int,
    threads: int,
    arm: str,
    rep: int,
) -> dict[str, str | int | float]:
    output = run_root / "measurements" / f"n{n}_t{threads}_{arm}" / f"rep{rep}"
    output.mkdir(parents=True, exist_ok=True)
    command = [
        stata,
        "-b",
        "do",
        str(source_tree / "benchmarks/stata/paper_akm_measure.do"),
        str(source_root),
        str(output),
        str(comparator_root),
        str(data_path),
        arm,
        str(n),
        str(threads),
        str(rep),
    ]
    process = subprocess.Popen(command, cwd=output, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    baseline: list[int] = []
    command_rss: list[int] = []
    sample_started = time.monotonic()
    while process.poll() is None:
        rss = read_rss_kib(process.pid)
        if rss is not None:
            if (output / "START").exists() and not (output / "DONE").exists():
                command_rss.append(rss)
            elif (output / "READY").exists() and not (output / "START").exists():
                baseline.append(rss)
        time.sleep(0.01)
    stdout, stderr = process.communicate()
    (output / "runner.stdout").write_text(stdout, encoding="utf-8")
    (output / "runner.stderr").write_text(stderr, encoding="utf-8")
    result_path = output / "result.csv"
    if process.returncode != 0 or not result_path.exists():
        raise RuntimeError(
            f"measurement failed: N={n}, threads={threads}, arm={arm}, rep={rep}, rc={process.returncode}"
        )
    if len(baseline) < 20 or len(command_rss) < 1:
        raise RuntimeError(
            f"invalid RSS phases: N={n}, threads={threads}, arm={arm}, rep={rep}, "
            f"baseline={len(baseline)}, command={len(command_rss)}"
        )
    baseline_median = statistics.median(baseline)
    peak = max(command_rss)
    if baseline_median <= 0 or peak < baseline_median:
        raise RuntimeError(
            f"invalid RSS levels: N={n}, threads={threads}, arm={arm}, rep={rep}, "
            f"baseline={baseline_median}, peak={peak}"
        )
    with result_path.open(newline="", encoding="utf-8") as handle:
        row: dict[str, str | int | float] = dict(next(csv.DictReader(handle)))
    if row["rc"] != "0" or int(float(str(row["e_n"]))) != n:
        raise RuntimeError(f"invalid command receipt for {output}")
    if arm == "fereg":
        if int(float(str(row["fereg_requested_threads"]))) != threads:
            raise RuntimeError(f"fereg requested-thread receipt failed for {output}")
        if int(float(str(row["fereg_effective_threads"]))) != threads:
            raise RuntimeError(f"fereg effective-thread receipt failed for {output}")
    row.update(
        {
            "baseline_rss_kib": float(baseline_median),
            "peak_rss_kib": peak,
            "peak_over_baseline": peak / baseline_median,
            "baseline_samples": len(baseline),
            "command_samples": len(command_rss),
            "sampler_elapsed_seconds": time.monotonic() - sample_started,
        }
    )
    (output / "memory_summary.json").write_text(json.dumps({
        "baseline_rss_kib": baseline_median,
        "peak_rss_kib": peak,
        "peak_over_baseline": peak / baseline_median,
        "baseline_samples": len(baseline),
        "command_samples": len(command_rss),
        "sampling_interval_seconds": 0.01,
    }, indent=2) + "\n", encoding="utf-8")
    return row


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    if not rows:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = list(rows[0])
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def aggregate(run_root: Path, rows: list[dict[str, object]], science: list[dict[str, object]], mode: str) -> None:
    evidence_dir = run_root / "evidence"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    write_csv(evidence_dir / "runs.csv", rows)
    if science:
        write_csv(evidence_dir / "science.csv", science)

    summaries: list[dict[str, object]] = []
    grouped: dict[tuple[int, int, str], list[dict[str, object]]] = defaultdict(list)
    for row in rows:
        grouped[(int(row["n"]), int(row["requested_threads"]), str(row["arm"]))].append(row)
    for key in sorted(grouped):
        group = grouped[key]
        n, threads, arm = key
        summaries.append({
            "n": n,
            "requested_threads": threads,
            "arm": arm,
            "repetitions": len(group),
            "median_seconds": statistics.median(float(row["seconds"]) for row in group),
            "min_seconds": min(float(row["seconds"]) for row in group),
            "max_seconds": max(float(row["seconds"]) for row in group),
            "median_peak_rss_gib": statistics.median(float(row["peak_rss_kib"]) for row in group) / 2**20,
            "min_peak_rss_gib": min(float(row["peak_rss_kib"]) for row in group) / 2**20,
            "max_peak_rss_gib": max(float(row["peak_rss_kib"]) for row in group) / 2**20,
            "median_peak_over_baseline": statistics.median(float(row["peak_over_baseline"]) for row in group),
            "min_peak_over_baseline": min(float(row["peak_over_baseline"]) for row in group),
            "max_peak_over_baseline": max(float(row["peak_over_baseline"]) for row in group),
        })
    write_csv(evidence_dir / "summary.csv", summaries)

    expected = 1 if mode == "benchmark-canary" else 23 * REPS
    if len(rows) != expected:
        raise RuntimeError(f"expected {expected} measurement rows, found {len(rows)}")
    if mode == "production":
        if len(summaries) != 23 or any(int(row["repetitions"]) != REPS for row in summaries):
            raise RuntimeError("incomplete production cell matrix")
        if len(science) != 11 or any(str(row["science_fail"]) != "0" for row in science):
            raise RuntimeError("incomplete or failed scientific preflight matrix")

    evidence = {
        "status": "success",
        "mode": mode,
        "benchmark_id": "paper-akm-scaling-memory-v1",
        "source_commit": SOURCE_COMMIT,
        "plugin_sha256": PLUGIN_SHA256,
        "reghdfe_version": "6.14.1",
        "reghdfe_commit": REGHDFE_COMMIT,
        "measurement_rows": len(rows),
        "summary_cells": len(summaries),
        "preflight_rows": len(science),
        "repetitions_per_cell": REPS if mode == "production" else 1,
        "hostname": socket.gethostname(),
        "platform": platform.platform(),
        "processor": platform.processor(),
        "sge_job_id": os.environ.get("JOB_ID", ""),
        "sge_slots": os.environ.get("NSLOTS", ""),
        "runs_csv_sha256": sha256(evidence_dir / "runs.csv"),
        "summary_csv_sha256": sha256(evidence_dir / "summary.csv"),
        "science_csv_sha256": sha256(evidence_dir / "science.csv") if science else "",
    }
    (evidence_dir / "evidence.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    lines = [
        "# Paper AKM scaling and memory benchmark",
        "",
        f"Status: success ({mode})",
        f"Host: `{evidence['hostname']}`; SGE job: `{evidence['sge_job_id']}`; slots: `{evidence['sge_slots']}`.",
        "",
        "| N | threads | command | seconds | peak GiB | peak/baseline |",
        "|---:|---:|:---|---:|---:|---:|",
    ]
    for row in summaries:
        lines.append(
            f"| {int(row['n']):,} | {row['requested_threads']} | {row['arm']} | "
            f"{float(row['median_seconds']):.4f} | {float(row['median_peak_rss_gib']):.3f} | "
            f"{float(row['median_peak_over_baseline']):.3f} |"
        )
    (evidence_dir / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (evidence_dir / "SUCCESS.txt").write_text("status=success\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--comparator-root", type=Path, required=True)
    parser.add_argument(
        "--mode", choices=("benchmark-canary", "production"), required=True
    )
    args = parser.parse_args()
    run_root = args.run_root.resolve()
    source_root = args.source_root.resolve()
    comparator_root = args.comparator_root.resolve()
    source_tree = run_root / "source"
    stata = stata_executable()

    plugin = source_root / "dist/fereg_linux.plugin"
    if sha256(plugin) != PLUGIN_SHA256:
        raise RuntimeError("qualified Linux plugin hash mismatch")
    if not (comparator_root / "r/reghdfe.ado").exists():
        candidates = list(comparator_root.rglob("reghdfe.ado"))
        if not candidates:
            raise RuntimeError("pinned reghdfe comparator not found")

    sizes = (8_000_000,) if args.mode == "benchmark-canary" else SIZES
    data = {n: generate_data(stata, source_tree, run_root / "data", n) for n in sizes}
    science: list[dict[str, object]] = []
    if args.mode == "production":
        for n, threads in preflight_cells():
            science.append(run_preflight(
                stata, source_tree, source_root, comparator_root, run_root, data[n], n, threads
            ))

    if args.mode == "benchmark-canary":
        schedule = [(8_000_000, 4, "areg", 1)]
    else:
        schedule = [(*cell, rep) for rep in range(1, REPS + 1) for cell in measurement_cells()]
        random.Random(20260830).shuffle(schedule)
    rows: list[dict[str, object]] = []
    for index, (n, threads, arm, rep) in enumerate(schedule, start=1):
        print(f"MEASUREMENT {index}/{len(schedule)} N={n} threads={threads} arm={arm} rep={rep}", flush=True)
        rows.append(run_measurement(
            stata, source_tree, source_root, comparator_root, run_root, data[n], n, threads, arm, rep
        ))
    aggregate(run_root, rows, science, args.mode)
    print("PAPER_AKM_RUN_SUCCESS", flush=True)


if __name__ == "__main__":
    main()
