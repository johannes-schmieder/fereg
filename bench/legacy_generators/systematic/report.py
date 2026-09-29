"""Render individual vector panels and assemble them with LaTeX subcaption."""
from __future__ import annotations

import argparse
import csv
import math
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

from common import HERE, cell_id, cells, read_json, write_json
from campaign import audit

DESIGN_LABELS = {"one_fe": "One FE", "two_fe_dense": "Dense two FE", "two_fe_sparse": "Sparse two FE",
                 "three_fe_sparse": "Sparse three FE", "strong_d2": "Strong d=2", "strong_d3": "Strong d=3",
                 "strong_d6": "Strong d=6", "weak_d3": "Bottleneck d=3"}
COLORS = ["#1a476f", "#be4637", "#00876c", "#a0488e", "#d55e00", "#5575a8"]
MARKERS = ["o", "s", "D", "^", "v", "P"]
STYLES = ["-", "--", "-.", ":", (0, (6, 2)), (0, (2, 2))]


def collect(root, manifest):
    rows, comparisons = [], []
    errors = {(e["cell"], e["repetition"]) for e in audit(root, manifest)["integrity_errors"]}
    for cell in cells(manifest):
        for rep in range(7):
            phase = "cold" if rep == 0 else "warm"
            group = root / "results" / cell_id(cell) / f"{phase}-{rep}"
            marker = group / "GROUP.COMPLETE.json"
            if not marker.exists():
                continue
            g = read_json(marker)
            reference = "reghdfe" if cell["model"] == "ols" else "ppmlhdfe"
            candidate = "fereg" if cell["model"] == "ols" else "feppml"
            reference_pass = next((c["status"] == "pass" for c in g["comparisons"] if c["right"] == reference), False)
            science = {c["right"]: c["status"] for c in g["comparisons"]}
            science[candidate] = "pass" if reference_pass else "reference_unvalidated"
            oracle_pass = g.get("oracle_checks") is None or all(g["oracle_checks"].values())
            for arm in manifest["arms"][cell["model"]]:
                if not (group / arm / "result.json").exists():
                    raise ValueError(f"completed marker has no arm receipt: {group}/{arm}")
                record = read_json(group / arm / "result.json")
                eligible = record["status"] == "pass" and reference_pass and science.get(arm) == "pass" and oracle_pass and (cell_id(cell), rep) not in errors
                rows.append({**{k: v for k, v in cell.items() if k != "effects"}, "arm": arm,
                             "repetition": rep, "phase": phase, "seed": g["seed"],
                             "status": record["status"], "science": science.get(arm), "eligible": eligible,
                             "failure_phase": "" if record["status"] == "pass" else ("before target timing" if not record["rss_samples"] else "target invocation or extraction"),
                             "seconds": record["seconds"], "peak_rss_gib": record["peak_rss_gib"],
                             "timeout_bound": record["command_time_lower_bound"],
                             "N_retained": record["metadata"].get("N"), "host": record["host"],
                             "failed_checks": ";".join(k for c in g["comparisons"] if c["right"] == (reference if arm == candidate else arm) for k, value in c["checks"].items() if not value),
                             "job_id": record["job_id"], "input_sha256": record["input_sha256"]})
            for comparison in g["comparisons"]:
                a = next(r for r in rows[-len(manifest["arms"][cell["model"]]):] if r["arm"] == candidate)
                b = next(r for r in rows[-len(manifest["arms"][cell["model"]]):] if r["arm"] == comparison["right"])
                valid = a["eligible"] and b["eligible"]
                comparisons.append({**{k: v for k, v in cell.items() if k != "effects"},
                                    "comparator": comparison["right"], "repetition": rep, "phase": phase,
                                    "status": comparison["status"], "eligible": valid,
                                    "seconds": a["seconds"] / b["seconds"] if valid and b["seconds"] > 0 else None,
                                    "peak_rss_gib": a["peak_rss_gib"] / b["peak_rss_gib"] if valid and b["peak_rss_gib"] > 0 else None})
    return rows, comparisons


def quantiles(records, metric, repetitions=6):
    valid = [r[metric] for r in records if r["eligible"] and r[metric] is not None and math.isfinite(r[metric]) and r[metric] > 0]
    if len(valid) != repetitions or {r["repetition"] for r in records if r["eligible"]} != set(range(1, repetitions+1)):
        return None
    return np.quantile(valid, [.25, .5, .75], method="linear")


def csv_file(path, records):
    path.parent.mkdir(parents=True, exist_ok=True)
    if not records:
        path.write_text("")
        return
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(records[0]))
        writer.writeheader()
        writer.writerows(records)


def panel(path, rows, lines, line_key, axis, metric, title, manifest, ratio=False):
    fig, ax = plt.subplots(figsize=(4.4, 3.5), layout="constrained")
    xkey = "n" if axis in ("maximum", "fixed") else "threads"
    xs = manifest["sizes"] if xkey == "n" else manifest["threads"]
    has_points = False
    has_bounds = False
    for j, label in enumerate(lines):
        series = [r for r in rows if r[line_key] == label and r["phase"] == "warm"]
        medians, lower, upper = [], [], []
        for x in xs:
            q = quantiles([r for r in series if r[xkey] == x], metric)
            if q is not None:
                has_points = True
                lower.append(q[1] - q[0]); medians.append(q[1]); upper.append(q[2] - q[1])
            else:
                medians.append(np.nan); lower.append(np.nan); upper.append(np.nan)
                ax.plot(x, .025 + j * .025, marker="x", markersize=3.3, color=COLORS[j],
                        transform=ax.get_xaxis_transform(), clip_on=False)
                bounds = [r.get("timeout_bound") for r in series if r[xkey] == x and r.get("timeout_bound")]
                if bounds and metric == "seconds" and not ratio:
                    has_bounds = True
                    ax.plot(x, max(bounds), marker="^", fillstyle="none", color=COLORS[j], markersize=5)
        display_label = DESIGN_LABELS.get(label, label)
        if any(np.isfinite(medians)):
            ax.errorbar(xs, medians, yerr=[lower, upper], label=display_label,
                        color=COLORS[j], marker=MARKERS[j], linestyle=STYLES[j],
                        markersize=3.5, linewidth=1.1, elinewidth=.65, capsize=2)
        else:
            ax.plot([], [], label=display_label + " (unavailable)", color=COLORS[j], marker=MARKERS[j], linestyle=STYLES[j])
    if not has_points and not has_bounds:
        ax.set_ylim(.1, 10)
    ax.set_xscale("log"); ax.set_yscale("log")
    ax.set_xticks(xs)
    ax.set_xticklabels([f"{x/1e6:.2f}M" if x >= 500000 else f"{x/1000:.1f}k" for x in xs] if xkey == "n" else list(map(str, xs)), fontsize=8)
    ax.set_xlabel("Observations" if xkey == "n" else "Requested cores (caps apply)", fontsize=9)
    ax.set_ylabel(("Candidate/comparator time ratio" if metric == "seconds" else "Candidate/comparator RSS ratio") if ratio else
                  ("Command time (seconds)" if metric == "seconds" else "Peak process-tree RSS (GiB)"), fontsize=9)
    ax.tick_params(axis="y", labelsize=8)
    ax.grid(True, which="major", color="#dddddd", linewidth=.5, linestyle="--")
    ax.spines[["top", "right"]].set_visible(False)
    if ratio:
        ax.axhline(1, color="#888888", linewidth=.7, linestyle="--")
    if not has_points:
        ax.text(.5, .5, "No complete validated series", ha="center", va="center", transform=ax.transAxes, fontsize=9)
    ax.legend(loc="upper center", bbox_to_anchor=(.5, -.20), ncols=2, fontsize=7, frameon=False)
    fig.savefig(path)
    plt.close(fig)


def tex_escape(value):
    return str(value).replace("\\", r"\textbackslash{}").replace("_", r"\_").replace("%", r"\%").replace("&", r"\&").replace("#", r"\#")


def composite(caption, files, captions, note):
    text = [r"\begin{figure}[p]\centering", r"\caption{" + tex_escape(caption) + "}"]
    for i, (file, label) in enumerate(zip(files, captions)):
        text.extend([r"\begin{subfigure}[t]{0.485\linewidth}\centering",
                     r"\includegraphics[width=\linewidth]{panels/" + file + "}",
                     r"\caption{" + tex_escape(label) + "}", r"\end{subfigure}" + (r"\hfill" if i % 2 == 0 else r"\par\medskip")])
    text += [r"\begin{minipage}{0.98\linewidth}\footnotesize " + tex_escape(note) + r"\end{minipage}", r"\end{figure}\clearpage"]
    return "\n".join(text)


def findings(ratios, manifest):
    lines = [r"\section{Largest-sample comparisons}",
             r"This table uses ten controls, 3,932,160 observations and maximum available program capacity. Each design first contributes a median paired ratio across six validated repetitions. Entries show the median and range across the eligible designs; the coverage column is essential when a comparator does not pass every design. Speedup is comparator time divided by candidate time; values above one favor the candidate. RSS ratios below one favor the candidate.",
             r"\small\begin{longtable}{llrrr}\toprule Model & Comparator & Designs & Speedup: median [range] & RSS ratio: median [range]\\\midrule\endhead"]
    for model, arms in manifest["arms"].items():
        for arm in arms[1:]:
            selected = [r for r in ratios if r["model"] == model and r["comparator"] == arm and r["section"] == "main"
                        and r["n"] == max(manifest["sizes"]) and r["threads"] == 28 and r["phase"] == "warm"]
            times, memory = [], []
            for design in manifest["designs"]:
                records = [r for r in selected if r["design"] == design]
                tq, mq = quantiles(records, "seconds"), quantiles(records, "peak_rss_gib")
                if tq is not None and mq is not None:
                    times.append(1 / tq[1]); memory.append(mq[1])
            def interval(values):
                return "--" if not values else f"{np.median(values):.2f} [{min(values):.2f}, {max(values):.2f}]"
            lines.append(" & ".join(map(tex_escape, [model, arm, f"{len(times)}/8", interval(times), interval(memory)])) + r"\\")
    return lines + [r"\bottomrule\end{longtable}\normalsize"]


def render(root, output, allow_incomplete=False):
    from common import manifest_for
    from light import enabled
    manifest = manifest_for(root)
    if enabled(manifest):
        from light_report import render as render_light
        return render_light(root, output, allow_incomplete)
    completeness = audit(root, manifest)
    if completeness["status"] != "complete" and not allow_incomplete:
        raise ValueError("report cannot be finalized while registered groups are missing")
    output.mkdir(parents=True, exist_ok=True)
    panels = output / "panels"
    panels.mkdir(exist_ok=True)
    rows, ratios = collect(root, manifest)
    csv_file(output / "runs.csv", rows)
    csv_file(output / "paired-ratios.csv", ratios)
    write_json(output / "completeness.json", completeness)
    write_json(output / "manifest.json", manifest)
    write_json(output / "references.json", read_json(HERE / "references.json"))
    if (root / "env/frozen.json").exists():
        write_json(output / "provenance.json", read_json(root / "env/frozen.json"))
    summary = []
    grouped = defaultdict(list)
    for r in rows:
        if r["phase"] == "warm":
            grouped[(r["model"], r["design"], r["n"], r["k"], r["threads"], r["section"], r["arm"])].append(r)
    for key, records in grouped.items():
        record = dict(zip(("model", "design", "n", "k", "threads", "section", "arm"), key))
        record["validated_repetitions"] = sum(r["eligible"] for r in records)
        for metric in ("seconds", "peak_rss_gib"):
            q = quantiles(records, metric)
            for i, name in enumerate(("q1", "median", "q3")):
                record[metric + "_" + name] = float(q[i]) if q is not None else None
        summary.append(record)
    csv_file(output / "summary.csv", summary)
    tex = [r"\documentclass[11pt]{article}", r"\usepackage[margin=0.8in]{geometry}",
           r"\usepackage{graphicx,subcaption,booktabs,longtable,hyperref}",
           r"\hypersetup{colorlinks=true,urlcolor=blue,linkcolor=blue}",
           r"\begin{document}", r"\title{Systematic benchmarks: fereg and feppml}",
           r"\author{}\date{September 2026}\maketitle",
           r"\section{Coverage and interpretation}",
           tex_escape(f"Status: {completeness['status']}. Completed {completeness['complete_groups']} of {completeness['expected_groups']} registered comparison groups."),
           r"\par Candidate source: \texttt{" + manifest["candidate_commit"] + "}.",
           r"\par This report measures public estimator commands on synthetic data. It does not qualify a release. Default algorithms and stopping rules are retained. Every measurement reserves an entire 28-core Broadwell E5-2680v4 node. Stata-controlled work is limited by its verified license (normally four processors). Native fereg, fixest and pyfixest pools are capped by the requested budget. Greg and gglm expose no per-call native thread control.",
           r"\section{Measurement}",
           r"Time includes estimator setup, absorption, estimation and requested inference; startup, input generation/import, initialization, validation and serialization are excluded. Main points summarize six independently initialized, fresh-process repetitions. A separate cold invocation is retained in the data. Identical input files and balanced arm ordering support within-repetition ratios. Both axes are logarithmic. Capped bars show the interquartile range, not confidence intervals.",
           r"\par Memory is the absolute estimator-phase peak sum of process-tree RSS, sampled every 10 milliseconds and expressed in GiB ($2^{30}$ bytes), without baseline subtraction. Sampling may miss shorter-lived peaks. Data-loaded process memory and language runtime overhead are included.",
           r"\par OLS requests IID inference; PPML requests robust inference. R and Python PPML request unadjusted sandwich covariance and its extracted covariance is normalized by $N/(N-1)$ to match the pinned Stata contract. The normalization is recorded separately from native output. Solver tolerances are unchanged. Positive counts isolate finite-optimum PPML scaling; the zero-outcome appendix preserves native separation and sample handling.",
           r"\par A finite plotted point requires all six repetitions, successful application receipts and scientific comparisons. Candidate/reference science remains mandatory; gtools' narrower observable result contract remains diagnostic. Failures and incomplete comparisons are retained in the tables and CSV files, with no fabricated finite ratios.",
           r"\par An initialization failure prevents a warm target measurement. It is recorded as a failure before target timing and does not establish that the larger target fit would fail. The separate cold invocation omits initialization; it remains in runs.csv. No initialization failure is replaced by a made-up target runtime.",
           r"\par The observation ladder is 61,440, 245,760, 983,040, 1,966,080 and 3,932,160. Core scaling holds observations at 983,040 and uses 1, 2, 4, 8, 14 and 28 requested cores. The figure arrangement follows Figures 2 and 3 of fevc-paper: individual time and memory panels, observation and core axes, and separate paired-ratio figures assembled in LaTeX.",
           r"\par Econometric designs use five rows per worker and approximately twenty workers per firm, with 20\% mobility in the dense case and 2\% in the sparse case. The third effect is crossed with worker and firm. Paper designs preserve degree-two/three/six long-range memberships and the degree-three shallow hub-tree bottleneck construction. Fixed-effect counts grow with observations. Controls are independent Gaussian draws; the graph, controls and outcomes are generated before any estimator is invoked."]
    tex += findings(ratios, manifest) + [r"\clearpage"]
    note = "Points are medians; capped bars are interquartile ranges of six validated repetitions. All axes are logarithmic. Every job reserves 28 cores; Stata-controlled work is capped at its licensed capacity. Colored crosses at the foot mark unvalidated or incomplete points; open upward triangles mark command-time lower bounds after timeouts. Detailed checks are in runs.csv."
    for model in manifest["arms"]:
        for version in ("stata", "expanded"):
            arms = manifest["arms"][model] if version == "expanded" else [a for a in manifest["arms"][model] if a not in ("fixest", "pyfixest")]
            tex += [r"\section{" + tex_escape(f"{model.upper()}: {version} comparisons") + "}"]
            for design in manifest["designs"]:
                selected = [r for r in rows if r["model"] == model and r["design"] == design and r["section"] == "main" and r["arm"] in arms]
                names = {}
                for axis in ("maximum", "fixed", "cores"):
                    subset = [r for r in selected if (r["n"] == manifest["scaling_n"] if axis == "cores" else r["threads"] == (28 if axis == "maximum" else 4))]
                    for metric in ("seconds", "peak_rss_gib"):
                        name = f"absolute-{model}-{version}-{design}-{axis}-{metric}.pdf"
                        panel(panels / name, subset, arms, "arm", axis, metric, design, manifest)
                        names[axis, metric] = name
                for axis in ("maximum", "fixed"):
                    files = [names[axis, "seconds"], names["cores", "seconds"], names[axis, "peak_rss_gib"], names["cores", "peak_rss_gib"]]
                    tex.append(composite(f"{model.upper()} {DESIGN_LABELS[design]}: {axis} capacity", files,
                                         ["Time by observations", "Time by requested cores", "Memory by observations", "Memory by requested cores"],
                                         note + f" Observation panels use {'maximum available capacity' if axis == 'maximum' else 'a four-core budget'}; core panels use 983,040 observations."))
            for suite in ("panels", "paper"):
                designs = [d for d, s in manifest["designs"].items() if s["suite"] == suite]
                for comparator in arms[1:]:
                    selected = [r for r in ratios if r["model"] == model and r["suite"] == suite and r["section"] == "main" and r["comparator"] == comparator]
                    names = {}
                    for axis in ("maximum", "fixed", "cores"):
                        subset = [r for r in selected if (r["n"] == manifest["scaling_n"] if axis == "cores" else r["threads"] == (28 if axis == "maximum" else 4))]
                        for metric in ("seconds", "peak_rss_gib"):
                            name = f"ratio-{model}-{version}-{suite}-{comparator}-{axis}-{metric}.pdf"
                            panel(panels / name, subset, designs, "design", axis, metric, suite, manifest, ratio=True)
                            names[axis, metric] = name
                    for axis in ("maximum", "fixed"):
                        files = [names[axis, "seconds"], names["cores", "seconds"], names[axis, "peak_rss_gib"], names["cores", "peak_rss_gib"]]
                        tex.append(composite(f"{arms[0]}/{comparator}: {suite} designs, {axis} capacity", files,
                                             ["Time ratios by observations", "Time ratios by requested cores", "Memory ratios by observations", "Memory ratios by requested cores"],
                                             note + " Ratios are formed within paired repetitions before aggregation. Ratios below one favor the candidate; the horizontal line marks parity."))
    tex += [r"\appendix\section{Width and zero-outcome results}",
            r"The width checks use 4 and 50 controls; zero-outcome cases use ordinary Poisson outcomes. Both use 983,040 observations and four/maximum requested cores. Full repetition-level records are in the accompanying CSV files.",
            r"\scriptsize\begin{longtable}{lllrrrrr}\toprule Model & Design & Arm & K & Cores & Valid & Time (s) & RSS (GiB)\\\midrule\endhead"]
    for s in summary:
        if s["section"] == "main":
            continue
        t = "--" if s["seconds_median"] is None else f"{s['seconds_median']:.3g}"
        m = "--" if s["peak_rss_gib_median"] is None else f"{s['peak_rss_gib_median']:.3g}"
        tex.append(" & ".join(map(tex_escape, [s["model"] + (" zeros" if s["section"] == "zeros" else ""), DESIGN_LABELS[s["design"]], s["arm"], s["k"], s["threads"], s["validated_repetitions"], t, m])) + r"\\")
    tex += [r"\bottomrule\end{longtable}\normalsize", r"\section{Failures and missing evidence}",
            r"\small\begin{longtable}{llr}\toprule Arm & Status & Count\\\midrule\endhead"]
    statuses = Counter((r["arm"], r["status"] if r["status"] != "pass" else r["science"]) for r in rows)
    for (arm, status), count in sorted(statuses.items()):
        tex.append(tex_escape(arm) + " & " + tex_escape(status) + f" & {count}" + r"\\")
    tex += [r"\bottomrule\end{longtable}", r"\section{Reproducibility}",
            r"The accompanying manifest, source and dependency hashes, per-task scheduler receipts, run-level measurements and paired ratios define this report. Input data are deterministic and regenerated in node-local scratch. The panel PDFs are separate vector files included with subcaption; no composite figure is rasterized.",
            r"\par SCC modules are Stata/MP 19, R 4.5.2 and Python 3.12.4. Exact package dependencies, compiled comparator files and the candidate plugin are bound by the runtime inventory. The candidate uses Rust 1.85.1. Areg is part of the licensed Stata installation.",
            r"\small\begin{longtable}{lp{0.68\linewidth}}\toprule Reference & Version or source commit\\\midrule\endhead"]
    for name, reference in read_json(HERE / "references.json").items():
        tex.append(tex_escape(name) + r" & \texttt{" + tex_escape(reference["version_or_commit"]) + r"}\\")
    tex += [r"\bottomrule\end{longtable}", r"\end{document}"]
    (output / "report.tex").write_text("\n\n".join(tex) + "\n")
    for _ in range(2):
        subprocess.run(["pdflatex", "-interaction=nonstopmode", "-halt-on-error", "report.tex"], cwd=output,
                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("run_root", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--allow-incomplete", action="store_true")
    a = p.parse_args()
    render(a.run_root, a.output, a.allow_incomplete)
