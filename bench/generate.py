#!/usr/bin/env python3
"""Fixed-seed inputs ported from legacy_generators/{systematic,two_fe_dense}.

All estimators read exactly the same DTA; generation is outside command timings.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from datetime import datetime
from pathlib import Path
import numpy as np
import pandas as pd


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def groups(cell, rng):
    n, q = cell["rows"], cell["effects"]
    row = np.arange(n, dtype=np.int64)
    if cell["design"].startswith("akm"):
        periods = 5
        workers = (n + periods - 1) // periods
        firms = max(10, workers // 20)
        assignment = np.empty((workers, periods), dtype=np.int64)
        assignment[:, 0] = (7919 * (np.arange(workers) + 1) - 1) % firms
        mobility = 0.8 if cell["design"] == "akm-strong" else 0.01
        for t in range(1, periods):
            destination = (104729 * (np.arange(workers) + 1) + 7919 * (t + 1) - 1) % firms
            assignment[:, t] = np.where(rng.random(workers) < mobility, destination, assignment[:, t-1])
        result = {"g1": row // periods + 1, "g2": assignment.reshape(-1)[:n] + 1}
        if q == 3:
            result["g3"] = row % periods + 1
    else:
        # Dense panels/gravity share connected, balanced Cartesian memberships.
        levels = max(10, min(1000, int(np.sqrt(n / 4))))
        result = {"g1": row % levels + 1}
        if q >= 2:
            result["g2"] = (row // levels) % levels + 1
        if q == 3:
            result["g3"] = (row // (levels * levels)) % 4 + 1
    return result


def generate(cell, seed, path):
    if cell["design"] == "veneto":
        raise ValueError("Veneto must be supplied as an external prepared DTA; never generated or committed")
    n, k = cell["rows"], cell["regressors"]
    rng = np.random.Generator(np.random.PCG64(seed))
    data = {"obs_id": np.arange(1, n + 1, dtype=np.int64), **groups(cell, rng)}
    xb = np.zeros(n)
    for j in range(1, k + 1):
        values = np.random.default_rng(np.random.SeedSequence([seed, j])).normal(size=n)
        data[f"x{j}"] = values
        xb += (0.30 / j if j % 2 else -0.20 / j) * values
    eta = sum(0.15 / j * np.sin(data[f"g{j}"] / (5 + j)) for j in range(1, cell["effects"]+1))
    if cell.get("slopes"):
        data["slope"] = np.random.default_rng(np.random.SeedSequence([seed, 904])).normal(size=n)
        eta += 0.15 * np.sin(data["g1"] / 11) * data["slope"]
    if cell["model"] == "ols":
        data["y"] = 1.0 + xb + eta + rng.normal(0.0, 0.5, n)
    else:
        # Positive fractional outcomes avoid accidental separation in ordinary cells.
        mu = np.exp(0.4 + xb + eta)
        data["y"] = mu * rng.gamma(2.0, 0.5, n)
        if cell.get("separation"):
            data["y"] = rng.poisson(mu).astype(float)
            data["y"][data["g1"] <= 2] = 0.0
    if cell["weights"] == "analytic":
        data["weight"] = np.exp(np.random.default_rng(np.random.SeedSequence([seed, 905])).normal(0, 0.5, n))
    if cell["vce"] == "cluster":
        data["cluster"] = (data["g1"] - 1) % max(10, min(100, n // 100)) + 1
    frame = pd.DataFrame(data)
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    # Version 114 is numeric-lossless here and readable by base R's foreign package.
    frame.to_stata(path, write_index=False, version=114, time_stamp=datetime(2000, 1, 1))
    decoded = pd.read_stata(path, convert_categoricals=False)
    for name in frame:
        if not np.array_equal(frame[name].to_numpy(), decoded[name].to_numpy()):
            raise ValueError(f"DTA round trip changed {name}")
    metadata = {"cell": cell, "seed": seed, "sha256": sha256(path), "numpy": np.__version__, "pandas": pd.__version__,
                "rows": n, "zero_outcomes": int((frame.y == 0).sum())}
    path.with_suffix(".json").write_text(json.dumps(metadata, indent=2) + "\n")
    return metadata


def prepare_external(cell, source, destination):
    """Convert the pinned model input losslessly for base R; never change its model/sample."""
    source, destination = Path(source), Path(destination)
    source_hash = sha256(source)
    if source_hash != cell["input_sha256"]:
        raise ValueError("external model input differs from the declared source hash")
    frame = pd.read_stata(source, convert_categoricals=False)
    required = ["obs_id", "y", *[f"g{j}" for j in range(1, cell["effects"]+1)],
                *[f"x{j}" for j in range(1, cell["regressors"]+1)]]
    if len(frame) != cell["rows"] or not set(required) <= set(frame):
        raise ValueError("external input dimensions do not match the declared model")
    if not frame.obs_id.is_unique:
        raise ValueError("external observation IDs are not unique")
    destination.parent.mkdir(parents=True, exist_ok=True)
    frame.to_stata(destination, write_index=False, version=114, time_stamp=datetime(2000,1,1))
    decoded = pd.read_stata(destination, convert_categoricals=False)
    for name in frame:
        if not np.array_equal(frame[name].to_numpy(), decoded[name].to_numpy()):
            raise ValueError(f"external DTA conversion changed {name}")
    metadata = {"cell": cell, "source_sha256": source_hash, "sha256": sha256(destination),
                "rows": len(frame), "numpy": np.__version__, "pandas": pd.__version__}
    destination.with_suffix(".json").write_text(json.dumps(metadata,indent=2)+"\n")
    return metadata


if __name__ == "__main__":
    from run import load_suite, selected_cells
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier", choices=["smoke", "standard", "full"], default="smoke")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    suite = load_suite()
    for cell in selected_cells(suite, args.tier):
        if cell["design"] != "veneto":
            generate(cell, suite["seed"], args.output / (cell["id"] + ".dta"))
