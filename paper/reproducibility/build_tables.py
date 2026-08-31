#!/usr/bin/env python3
"""Create manuscript tables from frozen evidence without running benchmarks."""

from __future__ import annotations

import argparse
import json
import math
import re
from pathlib import Path


OLS_REL = Path(
    "benchmarks/reports/captured/linux-x86_64/stata19/adr13-qualified/"
    "fma-repair-5"
)
VCE_REL = Path(
    "benchmarks/reports/captured/linux-x86_64/stata19/adr14-vce-qualified/"
    "contiguous-repair-8"
)

OLS_SOURCE = "947f79373306760a635494935cc84c0f5ebfcda8"
OLS_PLUGIN = "94017d23d97c1a0890cd9a1a180dad4ce3482b5b29cbe6efdbece5016b3e7099"
VCE_SOURCE = "490e03a3e2458d8bc463c3f60389062e2f8fae42"
VCE_ARCHIVE = "4f4e44cc539419ae67043cfca824c92aed7c6b1969c1a37825939e2397d02aab"
VCE_PLUGIN = "9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2"


def read_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def markdown_rows(path: Path, columns: int) -> list[list[str]]:
    rows: list[list[str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != columns:
            continue
        if all(re.fullmatch(r":?-+:?", cell) for cell in cells):
            continue
        rows.append(cells)
    return rows


def geometric_mean(values: list[float]) -> float:
    return math.exp(sum(math.log(value) for value in values) / len(values))


def verify_ols(bundle: Path) -> tuple[list[list[str]], dict]:
    evidence = read_json(bundle / "evidence.json")
    assert evidence["source_commit"] == OLS_SOURCE
    assert evidence["linux_plugin_sha256"] == OLS_PLUGIN
    assert evidence["expected_scopes"] == 5
    assert evidence["timing_rows"] == 180
    assert len(evidence["preflights"]) == 5
    assert all(row["science_fail"] == "0" for row in evidence["preflights"])
    assert evidence["arms"] == ["fereg", "greg", "reghdfe"]
    comparisons = [
        row for row in evidence["comparisons"]
        if "fereg_over_reghdfe" in row
    ]
    assert len(comparisons) == 10
    for mode, target in (("fit", 0.3038665454023387),
                         ("save_fe", 0.34205543115516485)):
        ratios = [
            float(row["fereg_over_reghdfe"])
            for row in comparisons if row["mode"] == mode
        ]
        assert len(ratios) == 5
        assert abs(geometric_mean(ratios) - target) < 1e-12
    return comparisons, evidence


def verify_vce(bundle: Path) -> tuple[list[list[str]], dict]:
    evidence = read_json(bundle / "evidence.json")
    assert evidence["status"] == "success"
    assert evidence["source_commit"] == VCE_SOURCE
    assert evidence["archive_sha256"] == VCE_ARCHIVE
    assert evidence["plugin_sha256"] == VCE_PLUGIN
    assert evidence["scopes"] == 8
    assert evidence["preflights"] == 8
    assert evidence["timing_rows"] == 72
    assert evidence["arms"] == ["fereg", "greg", "reghdfe"]

    rows = markdown_rows(bundle / "report.md", 10)
    header = ["N", "VCE", "Threads", "fereg", "Greg", "reghdfe",
              "f/Greg", "f/reghdfe", "a8fbad2", "new/a8"]
    assert rows and rows[0] == header
    rows = rows[1:9]
    assert len(rows) == 8
    assert abs(geometric_mean([float(row[7]) for row in rows]) - 0.258) < 5e-4
    return rows, evidence


def tex_escape(value: str) -> str:
    return value.replace("_", r"\_")


def write_ols(rows: list[dict], output: Path) -> None:
    design = {"akm_standard": "Worker--firm", "one_fe": "One FE"}
    mode = {"fit": "Fit", "save_fe": "Fit + save FE"}
    lines = [
        r"\begin{table}[tbp]",
        r"\centering",
        r"\caption{Existing OLS benchmark evidence: \texttt{fereg} versus \texttt{reghdfe}}",
        r"\label{tab:bench-ols}",
        r"\small",
        r"\begin{threeparttable}",
        r"\begin{tabular}{llrrrrr}",
        r"\toprule",
        r"Design & Mode & $K$ & Threads & \texttt{fereg} & \texttt{reghdfe} & Ratio \\",
        r"\midrule",
    ]
    for row in rows:
        lines.append(
            f"{design[row['design']]} & {mode[row['mode']]} & "
            f"{row['controls']} & {row['requested_threads']} & "
            f"{row['fereg_seconds']:.3f} & {row['reghdfe_seconds']:.3f} & "
            f"{row['fereg_over_reghdfe']:.3f} \\\\"  # noqa: W605
        )
    lines += [
        r"\midrule",
        r"\multicolumn{4}{l}{Geometric mean, fit cells} & \multicolumn{2}{r}{ } & 0.304 \\",
        r"\multicolumn{4}{l}{Geometric mean, fit + save FE cells} & \multicolumn{2}{r}{ } & 0.342 \\",
        r"\bottomrule",
        r"\end{tabular}",
        r"\begin{tablenotes}[flushleft]\footnotesize",
        r"\item Notes: Seconds are within-shard warm medians. All designs have $N=1{,}000{,}000$. "
        r"Worker--firm models have two absorbed effects and 30 finite controls; one-FE models have "
        r"10 or 30 controls. Ratio is \texttt{fereg}/\texttt{reghdfe}; values below one favor "
        r"\texttt{fereg}. Threads are the requested native \texttt{fereg} threads. The 16-thread "
        r"shards ran Stata/IC with four Stata processors. Source and scientific gates are described "
        r"in Section~\ref{sec:benchmarks}.",
        r"\end{tablenotes}",
        r"\end{threeparttable}",
        r"\end{table}",
        "",
    ]
    output.write_text("\n".join(lines), encoding="utf-8")


def write_vce(rows: list[list[str]], output: Path) -> None:
    lines = [
        r"\begin{table}[tbp]",
        r"\centering",
        r"\caption{Existing robust and clustered benchmark evidence}",
        r"\label{tab:bench-vce}",
        r"\small",
        r"\begin{threeparttable}",
        r"\begin{tabular}{llrrrr}",
        r"\toprule",
        r"$N$ & VCE & Threads & \texttt{fereg} & \texttt{reghdfe} & Ratio \\",
        r"\midrule",
    ]
    for row in rows:
        vce = "Cluster (one way)" if row[1] == "cluster1" else "HC1 robust"
        lines.append(
            f"{tex_escape(row[0])} & {vce} & {row[2]} & {row[3]} & "
            f"{row[5]} & {row[7]} \\\\"  # noqa: W605
        )
    lines += [
        r"\midrule",
        r"\multicolumn{5}{l}{Geometric mean across eight cells} & 0.258 \\",
        r"\bottomrule",
        r"\end{tabular}",
        r"\begin{tablenotes}[flushleft]\footnotesize",
        r"\item Notes: Seconds are within-shard warm medians for worker--firm models with 30 finite "
        r"controls. Ratio is \texttt{fereg}/\texttt{reghdfe}. Every shard ran on a 32-processor "
        r"SCC node. Stata/IC used one processor in the one-thread cells and four processors in the "
        r"16-thread cells; the native \texttt{fereg} request was validated separately. See "
        r"Section~\ref{sec:benchmarks} for scope and qualification cautions.",
        r"\end{tablenotes}",
        r"\end{threeparttable}",
        r"\end{table}",
        "",
    ]
    output.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo_root.resolve()
    paper = Path(__file__).resolve().parents[1]
    output = paper / "tables"
    output.mkdir(parents=True, exist_ok=True)

    ols_rows, _ = verify_ols(repo / OLS_REL)
    vce_rows, _ = verify_vce(repo / VCE_REL)
    write_ols(ols_rows, output / "benchmark_ols.tex")
    write_vce(vce_rows, output / "benchmark_vce.tex")
    print("Verified frozen evidence and wrote two LaTeX tables.")


if __name__ == "__main__":
    main()
