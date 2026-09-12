"""Bounded SCC-only trace of the independent PPML validation solver."""
import os
import socket
from pathlib import Path

import numpy as np
import pandas as pd
from scipy import sparse
from scipy.sparse.linalg import lsmr

from common import HERE, read_json, write_json, sha256
from generate import generate
from oracle import dummy_matrix


def trace(data, effects, scaled):
    X = data[[f"x{j}" for j in range(1, 11)]].to_numpy()
    y = data.y.to_numpy()
    F, rank = dummy_matrix(data, effects)
    D = sparse.hstack([sparse.csr_matrix(X), F], format="csr")
    eta = np.full(len(y), np.log(y.mean()))
    history = []
    for iteration in range(100):
        mu = np.exp(eta)
        root = np.sqrt(mu)
        A = D.multiply(root[:, None]).tocsr()
        column_scale = 1 / np.sqrt(np.asarray(A.power(2).sum(axis=0)).ravel()) if scaled else np.ones(D.shape[1])
        rhs = (y - mu) / root
        result = lsmr(A.multiply(column_scale), rhs, atol=1e-13, btol=1e-13, maxiter=20000)
        direction = D @ (column_scale * result[0])
        before = float(np.sum(mu - y * eta))
        step = 1.0
        while step > 2**-30:
            trial = eta + step * direction
            objective = float(np.sum(np.exp(trial) - y * trial))
            if np.isfinite(objective) and objective <= before + 1e-11 * max(1, abs(before)):
                break
            step *= .5
        eta = trial
        fitted = np.exp(eta)
        score = np.asarray(D.T @ (y - fitted)).ravel()
        scale = np.maximum(1, np.asarray(abs(D).T @ (y + fitted)).ravel())
        row = {"iteration": iteration + 1, "step": step, "objective": objective,
               "max_eta_change": float(np.max(abs(step * direction))),
               "max_scaled_score": float(np.max(abs(score) / scale)),
               "score_column": int(np.argmax(abs(score) / scale)),
               "lsmr_stop": result[1], "lsmr_iterations": result[2],
               "lsmr_normar": result[4], "lsmr_condition": result[6]}
        history.append(row)
        if row["max_scaled_score"] < 1e-11:
            break
    return {"scaled": scaled, "X_dtype": str(X.dtype), "y_dtype": str(y.dtype),
            "D_dtype": str(D.dtype), "rank": rank, "history": history}


if __name__ == "__main__":
    assert os.environ.get("NSLOTS") == "28"
    assert "E5-2680 v4" in Path("/proc/cpuinfo").read_text()
    root = Path(os.environ["RUN_ROOT"])
    destination = root / "diagnostics" / f"oracle-{os.environ['JOB_ID']}"
    destination.mkdir(parents=True, exist_ok=False)
    manifest = read_json(HERE / "manifest.json")
    for design, seed in (("one_fe", 8675309), ("two_fe_sparse", 104729), ("strong_d2", 104729)):
        spec = manifest["designs"][design]
        cell = dict(model="ppml", design=design, n=7680, k=10, threads=4,
                    section="main", suite=spec["suite"], effects=spec["effects"])
        path = Path(os.environ["TMPDIR"]) / (design + ".dta")
        info = generate(cell, seed, path, manifest)
        frame = pd.read_stata(path, convert_categoricals=False)
        output = {"cell": cell, "seed": seed, "input": info, "job_id": os.environ["JOB_ID"],
                  "host": socket.gethostname(), "cpu_affinity": sorted(os.sched_getaffinity(0)),
                  "source_sha256": sha256(Path(__file__)),
                  "traces": [trace(frame, cell["effects"], scaled) for scaled in (False, True)]}
        write_json(destination / (design + ".json"), output)
        print(design, [(t["scaled"], t["y_dtype"], t["history"][-1]) for t in output["traces"]], flush=True)
    (destination / "COMPLETE").write_text("ORACLE_DIAGNOSTIC_COMPLETE\n")
    print("ORACLE_DIAGNOSTIC_COMPLETE", flush=True)
