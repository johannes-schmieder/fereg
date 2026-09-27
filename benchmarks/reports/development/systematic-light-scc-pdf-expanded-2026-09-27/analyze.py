"""Explain coverage using frozen receipts; no fits or eligibility changes."""
import csv
import hashlib
import json
from collections import Counter
from pathlib import Path
from statistics import median

import numpy as np

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT.parent / "systematic-light-scc-final-2026-09-27"


def read(name):
    with (SOURCE / "generated" / name).open() as stream:
        return list(csv.DictReader(stream))


summary, pairs, runs = map(read, ("summary.csv", "paired-ratios.csv", "runs.csv"))
capture = json.loads((ROOT / "diagnostic-extract.json").read_text())
hashes = {row["path"]: row["sha256"] for row in read("captured-sha256.csv")}
records = capture["failures"] + [a for row in capture["ols_failed_pairs"] for a in row["arms"].values()]
records += [a for row in capture["ols_examples"] for a in row["arms"].values()] + [capture["input_example"]]
for row in records:
    assert hashes[row["path"]] == row["sha256"], row["path"]

# Independently count components of the two-way incidence graph. Reproduce only
# the graph generator, then verify both group columns against captured input.
info = capture["input_example"]["input"]
n = info["n"]
workers = (n + 4) // 5
firms = max(50, int(np.ceil(n / 5 / 20)))
rng = np.random.Generator(np.random.PCG64(info["seed"]))
assignments = np.empty((workers, 5), dtype=np.int64)
assignments[:, 0] = (7919 * (np.arange(workers) + 1) - 1) % firms
for t in range(1, 5):
    destinations = (104729 * (np.arange(workers) + 1) + 7919 * (t + 1) - 1) % firms
    assignments[:, t] = np.where(rng.random(workers) < .02, destinations, assignments[:, t - 1])
g1, g2 = np.arange(n) // 5, assignments.reshape(-1)[:n]
for name, values in (("g1", g1), ("g2", g2)):
    assert hashlib.sha256(np.asarray(values + 1, dtype="<f8").tobytes()).hexdigest() == info["column_sha256"][name]
parent = list(range(workers + firms))


def find(x):
    while parent[x] != x:
        parent[x] = parent[parent[x]]
        x = parent[x]
    return x


for left, right in zip(g1, g2):
    a, b = find(int(left)), find(workers + int(right))
    if a != b:
        parent[a] = b
components = len({find(x) for x in range(len(parent))})
rank = workers + firms - components
assert rank == capture["ols_examples"][0]["arms"]["fereg"]["metadata"]["df_a"]

nr = []
for row in summary:
    if row["model"] != "ols" or row["comparator"] != "fixest" or row["complete"] == "True":
        continue
    matched = [p for p in pairs if p["phase"] == "warm" and all(
        p[k] == row[k] for k in ("model", "design", "n", "k", "threads", "section", "comparator"))]
    nr.append({**row, "pair_status_counts": dict(Counter(p["status"] for p in matched)),
               "failed_checks": sorted({c for p in matched for c in p["failed_checks"].split(";") if c})})
slopes = []
for model, design, section in sorted({(r["model"], r["design"], r["section"])
                                     for r in runs if r["section"].startswith("slopes_")}):
    native = "fereg" if model == "ols" else "feppml"
    selected = [r for r in runs if (r["model"], r["design"], r["section"], r["arm"])
                == (model, design, section, native)]
    assert len(selected) == 4 and all(r["status"] == "pass" for r in selected)
    times = [float(r["seconds"]) for r in selected]
    refs = [r for r in summary if (r["model"], r["design"], r["section"]) == (model, design, section)]
    slopes.append(dict(model=model, design=design, section=section, n=61440,
                       native_median_seconds=median(times), native_range_seconds=[min(times), max(times)],
                       reference_results=refs))

examples = []
for row in capture["ols_examples"][:2]:
    native, ref = row["arms"]["fereg"], row["arms"]["fixest"]
    df_native = native["metadata"]["df_r"]
    df_ref = ref["metadata"]["N"] - ref["metadata"]["df_k"]
    normalized = [float(b) * df_ref / df_native / float(a) - 1
                  for a, b in zip(native["covariance_diagonal"], ref["covariance_diagonal"])]
    examples.append(dict(cell=row["cell"], native_df_r=df_native, reference_df_r=df_ref,
                         diagnostic_max_relative_diagonal_difference_after_df_scaling=max(map(abs, normalized))))

out = dict(source=capture["source"], verified_record_count=len(records),
           comparators=json.loads((SOURCE / "analysis.json").read_text())["comparators"],
           ols_unranked=nr, ols_failed_warm_pairs=len(capture["ols_failed_pairs"]),
           failed_ols_pairs_also_passing_reghdfe=sum(r["reghdfe_comparison"]["status"] == "pass" for r in capture["ols_failed_pairs"]),
           slope_configurations=slopes,
           independent_two_fe_topology=dict(n=n, workers=workers, firms=firms, components=components,
                                            fe_rank=rank, finite_rank=10, residual_df=n-rank-10,
                                            group_column_hashes_match=True),
           covariance_diagnostics=examples,
           arm_status={m: {a: dict(Counter(r["status"] for r in runs if r["model"] == m and r["arm"] == a))
                           for a in sorted({r["arm"] for r in runs if r["model"] == m})} for m in ("ols", "ppml")})
(ROOT / "analysis.json").write_text(json.dumps(out, indent=2, sort_keys=True) + "\n")
print(json.dumps({k: out[k] for k in ("verified_record_count", "independent_two_fe_topology", "covariance_diagnostics")}))
