"""Pinned PyFixest worker: load and validate before timed public invocation."""
import gc
import hashlib
import importlib.metadata
import json
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd
import pyfixest as pf

from common import read_json, write_json


def run_model(config, progress):
    output = Path(config["output"])
    cell = config["cell"]
    formula = "y ~ " + " + ".join(f"x{i}" for i in range(1, cell["k"] + 1)) + " | " + " + ".join(cell["effects"])
    fit_function = pf.feols if cell["model"] == "ols" else pf.fepois
    kwargs = {"vcov": "iid" if cell["model"] == "ols" else "hetero"}
    if cell["model"] == "ppml":
        kwargs["ssc"] = pf.ssc(k_adj=False)
    progress["error_stage"] = "warmup"
    if config["phase"] == "warm":
        warm = pd.read_stata(config["warm"], convert_categoricals=False)
        fit_function(formula, data=warm, **kwargs)
        del warm
        gc.collect()
    progress["error_stage"] = "input_validation"
    data = pd.read_stata(config["data"], convert_categoricals=False)
    info = read_json(Path(config["data"]).with_suffix(".json"))
    for column, expected in info["column_sha256"].items():
        if hashlib.sha256(np.asarray(data[column], dtype="<f8").tobytes()).hexdigest() != expected:
            raise ValueError(f"import hash differs for {column}")
    (output / "READY").write_text("READY\n")
    time.sleep(.1)
    (output / "START").write_text("START\n")
    while not (output / "START.CHECKED").exists():
        time.sleep(.001)
    progress["error_stage"] = "estimation"
    start = time.perf_counter()
    fit = None
    error = None
    try:
        fit = fit_function(formula, data=data, **kwargs)
    except Exception as exc:
        error = f"{type(exc).__name__}: {exc}"
    elapsed = time.perf_counter() - start
    (output / "DONE").write_text("DONE\n")
    metadata = {"rc": 0 if fit is not None else 1, "seconds": elapsed,
                "python_version": sys.version.split()[0], "pyfixest_version": importlib.metadata.version("pyfixest"),
                "formula": formula, "vcov": kwargs["vcov"], "error": error}
    metadata["error_stage"] = "estimation" if error else None
    progress.update(metadata)
    if fit is not None:
        progress["error_stage"] = "extraction"
        b = fit.coef()
        se = fit.se()
        correction = fit._N / (fit._N - 1) if cell["model"] == "ppml" else 1.0
        metadata["covariance_normalization"] = "HC0 multiplied by N/(N-1)" if cell["model"] == "ppml" else "native IID"
        metadata["covariance_multiplier"] = correction
        se = se * np.sqrt(correction)
        pd.DataFrame({"name": b.index, "value": b.values, "se": se.values}).to_csv(output / "coefficients.csv", index=False)
        V = fit._vcov * correction
        pd.DataFrame([(name, other, float(V[i, j])) for i, name in enumerate(b.index)
                      for j, other in enumerate(b.index)], columns=["row", "column", "value"]).to_csv(output / "covariance.csv", index=False)
        metadata["N"] = int(fit._N)
        metadata["sample_kind"] = "count_only"
        for key in ("_deviance", "_loglik", "_converged", "_df_k", "_df_t"):
            value = getattr(fit, key, None)
            if isinstance(value, (float, int, np.integer, np.floating, bool)) and np.isfinite(value):
                metadata[key.lstrip("_")] = float(value)
        if cell["model"] == "ols":
            residual = np.asarray(fit.resid())
            metadata["rss"] = float(residual @ residual)
        else:
            metadata["deviance"] = float(fit.deviance)
            metadata["converged"] = bool(fit.convergence)
        # Stored observation indices are version dependent; never manufacture a sample.
        retained = getattr(fit, "_data", None)
        if retained is not None and "obs_id" in retained and len(retained) == fit._N:
            retained[["obs_id"]].to_csv(output / "sample.csv", index=False)
            metadata["sample_kind"] = "stored_observation_ids"
    write_json(output / "metadata.json", metadata)
    print("SYSTEMATIC_WORKER_COMPLETE rc=" + str(metadata["rc"]), flush=True)


def run(config):
    progress = {"rc": 1, "seconds": None, "error_stage": "initialization"}
    try:
        run_model(config, progress)
    except Exception as error:
        # Startup and extraction failures need the same durable receipt as a
        # failed timed command. They never become successful timing samples.
        progress.update(rc=1, error=f"{type(error).__name__}: {error}")
        write_json(Path(config["output"]) / "metadata.json", progress)
        print("SYSTEMATIC_WORKER_COMPLETE rc=1", flush=True)


if __name__ == "__main__":
    run(read_json(sys.argv[1]))
