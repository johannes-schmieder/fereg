#!/usr/bin/env python3
"""Create vector PGFPlots figures from the frozen paper benchmark bundle."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path


EVIDENCE_REL = Path(
    "benchmarks/reports/captured/linux-x86_64/stata19/"
    "paper-akm-scaling-memory-v1"
)
SOURCE_COMMIT = "490e03a3e2458d8bc463c3f60389062e2f8fae42"
PLUGIN_SHA256 = "9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2"
REGHDFE_COMMIT = "29b2b203f534413369d675d0c0f674d61291f536"
ARMS = ("fereg", "reghdfe", "areg")
COLORS = {"fereg": "feregblue", "reghdfe": "reghdfeorange", "areg": "areggreen"}
MARKS = {"fereg": "*", "reghdfe": "square*", "areg": "triangle*"}
DASHES = {"fereg": "solid", "reghdfe": "dashed", "areg": "dash dot"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def verify(repo: Path) -> tuple[dict, list[dict[str, str]]]:
    bundle = repo / EVIDENCE_REL
    evidence = json.loads((bundle / "evidence.json").read_text(encoding="utf-8"))
    assert evidence["status"] == "success"
    assert evidence["mode"] == "production"
    assert evidence["benchmark_id"] == "paper-akm-scaling-memory-v1"
    assert evidence["source_commit"] == SOURCE_COMMIT
    assert evidence["plugin_sha256"] == PLUGIN_SHA256
    assert evidence["reghdfe_version"] == "6.14.1"
    assert evidence["reghdfe_commit"] == REGHDFE_COMMIT
    assert evidence["measurement_rows"] == 115
    assert evidence["summary_cells"] == 23
    assert evidence["preflight_rows"] == 11
    assert evidence["repetitions_per_cell"] == 5
    assert sha256(bundle / "runs.csv") == evidence["runs_csv_sha256"]
    assert sha256(bundle / "summary.csv") == evidence["summary_csv_sha256"]
    assert sha256(bundle / "science.csv") == evidence["science_csv_sha256"]
    rows = read_rows(bundle / "summary.csv")
    assert len(rows) == 23
    assert all(int(row["repetitions"]) == 5 for row in rows)
    cells = {(int(row["n"]), int(row["requested_threads"]), row["arm"]) for row in rows}
    expected: set[tuple[int, int, str]] = set()
    for n in (1_000_000, 2_000_000, 4_000_000, 8_000_000):
        expected.update((n, 4, arm) for arm in ARMS)
        expected.add((n, 16, "fereg"))
    for threads in (1, 2):
        expected.update((8_000_000, threads, arm) for arm in ARMS)
    expected.add((8_000_000, 8, "fereg"))
    assert cells == expected
    return evidence, rows


def num(value: str | float) -> str:
    return f"{float(value):.9g}"


def metric(row: dict[str, str], name: str) -> tuple[float, float, float]:
    mapping = {
        "seconds": ("median_seconds", "min_seconds", "max_seconds"),
        "peak": ("median_peak_rss_gib", "min_peak_rss_gib", "max_peak_rss_gib"),
        "ratio": ("median_peak_over_baseline", "min_peak_over_baseline", "max_peak_over_baseline"),
    }
    keys = mapping[name]
    return tuple(float(row[key]) for key in keys)  # type: ignore[return-value]


def coordinates(rows: list[dict[str, str]], xname: str, metric_name: str) -> tuple[str, str]:
    line: list[str] = []
    errors: list[str] = []
    for row in rows:
        x = int(row[xname])
        if xname == "n":
            x //= 1_000_000
        median, low, high = metric(row, metric_name)
        line.append(f"({x},{num(median)})")
        errors.append(
            f"({x},{num(median)}) += (0,{num(high - median)}) -= (0,{num(median - low)})"
        )
    return " ".join(line), " ".join(errors)


def plot_lines(
    rows: list[dict[str, str]],
    xname: str,
    metric_name: str,
    legend: bool,
) -> list[str]:
    lines: list[str] = []
    for arm in ARMS:
        selected = sorted((row for row in rows if row["arm"] == arm), key=lambda row: int(row[xname]))
        if not selected:
            continue
        coords, errors = coordinates(selected, xname, metric_name)
        style = f"color={COLORS[arm]},{DASHES[arm]},mark={MARKS[arm]}"
        lines.append(f"\\addplot+[thick,{style}] coordinates {{{coords}}};")
        if legend:
            label = "\\texttt{" + arm + "}"
            lines.append(f"\\addlegendentry{{{label}}}")
        lines.append(
            f"\\addplot+[only marks,mark=none,color={COLORS[arm]},opacity=.45,"
            "error bars/.cd,y dir=both,y explicit] coordinates {"
            f"{errors}" + "};"
        )
    return lines


def preamble() -> list[str]:
    return [
        r"\documentclass[tikz,border=3pt]{standalone}",
        r"\usepackage[T1]{fontenc}",
        r"\usepackage{lmodern}",
        r"\usepackage{pgfplots}",
        r"\usepgfplotslibrary{groupplots}",
        r"\pgfplotsset{compat=1.18}",
        r"\definecolor{feregblue}{HTML}{0072B2}",
        r"\definecolor{reghdfeorange}{HTML}{D55E00}",
        r"\definecolor{areggreen}{HTML}{009E73}",
        r"\begin{document}",
        r"\begin{tikzpicture}",
    ]


def axis_options(title: str, xlabel: str, ylabel: str, metric_name: str, legend: bool) -> str:
    options = [
        f"title={{{title}}}",
        f"xlabel={{{xlabel}}}",
        f"ylabel={{{ylabel}}}",
        "grid=major",
        "grid style={black!12}",
        "tick label style={font=\\footnotesize}",
        "label style={font=\\small}",
        "title style={font=\\small\\bfseries}",
        "scaled ticks=false",
        "axis line style={black!60}",
    ]
    if metric_name in ("seconds", "peak"):
        options.extend(("ymode=log", "log basis y=2"))
        if metric_name == "seconds":
            options.extend((
                "ytick={2,4,8,16,32,64,128,256,512}",
                "yticklabels={2,4,8,16,32,64,128,256,512}",
            ))
        else:
            options.extend((
                "ytick={0.5,1,2,3,4,5,8}",
                "yticklabels={0.5,1,2,3,4,5,8}",
            ))
    else:
        options.append("ymin=1")
    if legend:
        options.append(
            "legend style={font=\\footnotesize,draw=none,fill=white,fill opacity=.85,"
            "text opacity=1,at={(0.97,0.50)},anchor=east}"
        )
    return ",\n".join(options)


def write_size(rows: list[dict[str, str]], output: Path) -> None:
    selected = [row for row in rows if int(row["requested_threads"]) == 4]
    specs = [
        ("(a) Elapsed time", "seconds", "Seconds", True),
        ("(b) Peak memory", "peak", "Peak RSS (GiB)", False),
        ("(c) Relative memory", "ratio", "Peak / baseline RSS", False),
    ]
    lines = preamble()
    lines += [
        r"\begin{groupplot}[group style={group size=3 by 1,horizontal sep=1.05cm},",
        r"width=5.25cm,height=4.35cm,xtick={1,2,4,8},xmin=.85,xmax=9]",
    ]
    for title, metric_name, ylabel, legend in specs:
        lines.append("\\nextgroupplot[" + axis_options(
            title, "Observations (millions)", ylabel, metric_name, legend
        ) + "]")
        if metric_name == "ratio":
            lines.append(r"\addplot[black!45,thin,densely dotted,domain=.85:9] {1};")
        lines.extend(plot_lines(selected, "n", metric_name, legend))
    lines += [
        r"\end{groupplot}",
        r"\end{tikzpicture}",
        r"\end{document}",
    ]
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_cores(rows: list[dict[str, str]], output: Path) -> None:
    selected = [row for row in rows if int(row["n"]) == 8_000_000]
    specs = [
        ("(a) Elapsed time", "seconds", "Seconds", True),
        ("(b) Peak memory", "peak", "Peak RSS (GiB)", False),
        ("(c) Relative memory", "ratio", "Peak / baseline RSS", False),
    ]
    lines = preamble()
    lines += [
        r"\begin{groupplot}[group style={group size=3 by 1,horizontal sep=1.05cm},",
        r"width=5.25cm,height=4.35cm,xmode=log,log basis x=2,",
        r"xtick={1,2,4,8,16},xticklabels={1,2,4,8,16},xmin=.85,xmax=19]",
    ]
    for title, metric_name, ylabel, legend in specs:
        lines.append("\\nextgroupplot[" + axis_options(
            title, "Requested cores/threads", ylabel, metric_name, legend
        ) + "]")
        if metric_name == "ratio":
            lines.append(r"\addplot[black!45,thin,densely dotted,domain=.85:19] {1};")
        lines.extend(plot_lines(selected, "requested_threads", metric_name, legend))
    lines += [r"\end{groupplot}", r"\end{tikzpicture}", r"\end{document}"]
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo_root.resolve()
    _, rows = verify(repo)
    output = Path(__file__).resolve().parents[1] / "figures"
    output.mkdir(parents=True, exist_ok=True)
    write_size(rows, output / "akm_size_memory.tex")
    write_cores(rows, output / "akm_core_memory.tex")
    print("Verified frozen paper benchmark evidence and wrote two PGFPlots figures.")


if __name__ == "__main__":
    main()
