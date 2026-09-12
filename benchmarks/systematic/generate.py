"""Synthetic inputs shared literally by Stata, R and Python.

Paper memberships port generate_input.do from fevc_matlab_2026. No estimator
output is used to generate, select or modify the benchmark observations.
"""
from __future__ import annotations

import numpy as np
import pandas as pd

from common import sha256, write_json


def generate(cell, seed, path, manifest):
    n, k, design = cell["n"], cell["k"], cell["design"]
    spec = manifest["designs"][design]
    rng = np.random.Generator(np.random.PCG64(seed))
    i = np.arange(n, dtype=np.int64)
    groups = {}
    if spec["suite"] == "paper":
        degree = spec["degree"]
        workers = n // degree
        if n % (degree * 40):
            raise ValueError("paper rows must preserve whole graph dimensions")
        worker, period = i // degree, i % degree
        if design != "weak_d3":
            firms = workers // 40
            layer, base = worker // firms, worker % firms
            offset = np.zeros(n, dtype=np.int64)
            offset[period == 1] = (1 + layer % (firms // 4 - 1))[period == 1]
            offset[period == 2] = (int(np.ceil(firms / 3)) + 97 * layer % (firms // 4))[period == 2]
            offset[period == 3] = (int(np.ceil(2 * firms / 3)) + 193 * layer % (firms // 4))[period == 3]
            offset[period == 4] = firms - 1
            offset[period == 5] = int(np.ceil(2 * firms / 3)) - 1
            firm = (base + offset) % firms
        else:
            leaves, branches = workers // 5, min(40, workers // 128)
            if branches not in (20, 40):
                raise ValueError("weak graph requires the registered branch envelope")
            hubs = 1 + branches * 40
            leaf = worker % leaves
            branch = leaf % branches
            pattern = 5 * (leaf // branches) % 39
            panel = worker // leaves
            child = 1 + branch
            outer = 1 + branches + branch * 39 + (pattern + panel) % 39
            outer[(pattern < 7) & (panel == 4)] = 0
            firm = np.where(period == 0, child, np.where(period == 1, outer, hubs + leaf))
        groups = {"g1": worker + 1, "g2": firm + 1}
    else:
        periods = 5
        workers, firms = n // periods + (n % periods != 0), max(50, int(np.ceil(n / periods / 20)))
        worker, period = i // periods, i % periods
        assignments = np.empty((workers, periods), dtype=np.int64)
        assignments[:, 0] = (7919 * (np.arange(workers) + 1) - 1) % firms
        for t in range(1, periods):
            destinations = (104729 * (np.arange(workers) + 1) + 7919 * (t + 1) - 1) % firms
            assignments[:, t] = np.where(rng.random(workers) < spec["mobility"], destinations, assignments[:, t - 1])
        firm = assignments.reshape(-1)[:n]
        groups["g1"] = worker + 1
        if len(spec["effects"]) >= 2:
            groups["g2"] = firm + 1
        if len(spec["effects"]) == 3:
            counties = max(10, int(np.ceil(firms / 20)))
            groups["g3"] = (firm + 1 + 17 * (worker + 1) + 1049 * (period + 1) - 1) % counties + 1
    # Independent streams make the first controls invariant to requested width.
    data = {"obs_id": i + 1, **groups}
    xb = np.zeros(n)
    for j in range(1, k + 1):
        values = np.random.default_rng(np.random.SeedSequence([seed, j])).normal(size=n)
        data[f"x{j}"] = values
        xb += (0.30 / j if j % 2 else -0.20 / j) * values
    eta = 0.15 * np.sin(groups["g1"] / 7)
    if "g2" in groups:
        eta -= 0.12 * np.cos(groups["g2"] / 5)
    if "g3" in groups:
        eta += 0.08 * np.sin(groups["g3"] / 3)
    if cell["model"] == "ols":
        y = 1 + xb + eta + rng.normal(0, 0.5, n)
    else:
        eta += 0.06 * data["x1"] - 0.04 * data["x2"]
        y = rng.poisson(np.exp(eta)) if cell["section"] == "zeros" else 1 + (np.floor(100 * np.exp(eta) + (i + 1) % 17).astype(np.int64) % 19)
    data["y"] = y.astype(np.float64)
    frame = pd.DataFrame(data)
    levels = {name: int(frame[name].nunique()) for name in groups}
    minimum = {name: int(frame.groupby(name).size().min()) for name in groups}
    if cell["section"] != "zeros" and min(minimum.values()) < 2:
        raise ValueError("main generator introduced a singleton")
    path.parent.mkdir(parents=True, exist_ok=True)
    # DTA preserves IEEE doubles; all languages consume this same file.
    frame.to_stata(path, write_index=False, version=118, time_stamp=__import__("datetime").datetime(2000, 1, 1))
    reread = pd.read_stata(path, convert_categoricals=False)
    if not np.array_equal(frame.to_numpy(), reread.to_numpy()):
        raise ValueError("dataset writer failed exact numeric roundtrip")
    info = {"schema": "systematic-input-v1", "n": n, "k": k, "seed": seed,
            "design": design, "model": cell["model"], "section": cell["section"],
            "sha256": sha256(path), "levels": levels, "minimum_level_rows": minimum,
            "zeros": int((y == 0).sum()), "columns": list(frame.columns),
            "column_sha256": {c: __import__("hashlib").sha256(np.asarray(frame[c], dtype="<f8").tobytes()).hexdigest() for c in frame}}
    write_json(path.with_suffix(".json"), info)
    return info
