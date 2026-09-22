"""Numerical comparison and descriptive timings, without dropping failures."""
import argparse
import json
from pathlib import Path
import re
import statistics

import numpy as np
import pandas as pd

REPO = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads(path.read_text())


def matrix(path):
    table = pd.read_csv(path)
    return table.pivot(index="row", columns="column", values="value").apply(
        pd.to_numeric, errors="coerce").to_numpy()


def distance(actual, expected, rtol, atol=1e-10):
    actual, expected = np.asarray(actual), np.asarray(expected)
    assert actual.shape == expected.shape
    error = np.abs(actual - expected)
    allowed = atol + rtol * np.abs(expected)
    finite = np.isfinite(actual) & np.isfinite(expected)
    return dict(passed=bool(np.all(finite & (error <= allowed))),
        maximum_absolute_error=float(error.max()),
        maximum_tolerance_ratio=float((error / allowed).max()),
        failed_coordinates=int(np.count_nonzero(~finite | (error > allowed))))


def compare(a, b, model):
    ra, rb = read(a / "result.json"), read(b / "result.json")
    if ra["status"] != "pass" or rb["status"] != "pass":
        return dict(passed=False, statuses=[ra["status"], rb["status"]])
    ca, cb = [pd.read_csv(p / "coefficients.csv").set_index("name").sort_index() for p in [a, b]]
    va, vb = [pd.read_csv(p / "covariance.csv").set_index(["row", "column"]).sort_index() for p in [a, b]]
    assert ca.index.equals(cb.index) and va.index.equals(vb.index)
    checks = dict(sample=ra["sample_sha256"] == rb["sample_sha256"],
        N=ra["metadata"]["N"] == rb["metadata"]["N"],
        coefficients=distance(ca.value, cb.value, 1e-7),
        covariance=distance(va.value, vb.value, 1e-6))
    keys = [("rss", "rss")] if model == "ols" else [("ll", "loglik"), ("deviance", "deviance")]
    for left, right in keys:
        checks[left] = distance([ra["metadata"][left]], [rb["metadata"].get(right, rb["metadata"].get(left))], 1e-8)
    checks["passed"] = all(v if isinstance(v, bool) else v["passed"] for v in checks.values())
    return checks


def runtime(path, model):
    if model == "ols":
        return read(path / "result.json")["metadata"]
    ado = (REPO / "stata/_fereg_estimate_ppml.ado").read_text()
    names = re.search(r"matrix rownames `runtime_shadow' = (.*?)matrix colnames `runtime_shadow'", ado, re.S)[1]
    names = names.replace("///", "").split()
    values = matrix(path / "feppml_runtime.csv")[:, 0]
    assert len(names) == len(values)
    return {name: float(value) if np.isfinite(value) else None for name, value in zip(names, values)}


def audit(root):
    plan, progress = read(root / "plan.json"), read(root / "progress.json")
    records = []
    for index, cell in enumerate(plan["cells"]):
        attempts = [r for r in progress if r["case"] == index]
        timings, comparisons, runtimes = {}, [], {}
        for variant in plan["variants"]:
            rows = [r for r in attempts if r["variant"] == variant]
            clean = [r["seconds"] for r in rows if r["status"] == "pass" and not r["overlaps"] and not r["monitor_errors"]]
            timings[variant] = dict(attempts=len(rows), clean_successes=len(clean),
                minimum=min(clean) if clean else None, median=statistics.median(clean) if clean else None,
                maximum=max(clean) if clean else None)
            first = next((r for r in rows if r["status"] == "pass"), None)
            if first and variant != "fixest":
                runtimes[variant] = runtime(root / first["result"], cell["model"])
        baseline = next(v for v in plan["variants"] if v != "fixest")
        for repetition in range(plan["repeats"]):
            paths = {r["variant"]: root / r["result"] for r in attempts if r["repetition"] == repetition}
            for variant, path in paths.items():
                if variant != baseline and baseline in paths:
                    comparisons.append(dict(repetition=repetition, variant=variant,
                        checks=compare(paths[baseline], path, cell["model"])))
        records.append(dict(cell=cell, timings=timings, comparisons=comparisons, runtime=runtimes))
    return dict(schema="next-subsystem-audit-v1", scope="Pairwise exports only; independent oracles are separate", records=records)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    with args.output.open("x") as stream:
        json.dump(audit(args.root), stream, indent=2, allow_nan=False)
        stream.write("\n")
