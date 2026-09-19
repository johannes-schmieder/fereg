"""Pinned PyFixest public fit and optional FE extraction in one timed window."""
import gc
import hashlib
import importlib.metadata
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd
import pyfixest as pf

from common import read_json, write_json


def run(cfg):
    c, out = cfg["cell"], Path(cfg["output"])
    formula = "y ~ " + " + ".join(f"x{i}" for i in range(1,c["k"]+1)) + " | " + " + ".join(c["effects"])
    options = dict(vcov={"CRV1": " + ".join(c["clusters"])} if c["clusters"] else "iid" if c["model"]=="ols" else "hetero")
    if c["model"] == "ppml":
        options["ssc"] = pf.ssc(k_adj=False, G_adj=True, G_df="min")
    method = pf.feols if c["model"] == "ols" else pf.fepois

    def fit_model(data):
        fit = method(formula, data=data, **options)
        if c["save_fe"]:
            coefs = fit.fixef()
            contributions = []
            for name in c["effects"]:
                part = coefs[coefs.variable.astype(str).str.fullmatch(name)]
                if part.empty:
                    raise ValueError("unexpected fixed-effect extraction names")
                mapping = dict(zip(pd.to_numeric(part.level), part.coefficient))
                values = data[name].map(mapping).fillna(0).to_numpy()
                contributions.append(values)
            # Retain each observation-aligned channel through the end of timing.
            fit._light_saved_channels = contributions
        return fit

    warm = pd.read_stata(cfg["warm"], convert_categoricals=False)
    fit_model(warm)
    del warm
    gc.collect()
    data = pd.read_stata(cfg["data"], convert_categoricals=False)
    info = read_json(Path(cfg["data"]).with_suffix(".json"))
    for name, expected in info["column_sha256"].items():
        if hashlib.sha256(np.asarray(data[name], dtype="<f8").tobytes()).hexdigest() != expected:
            raise ValueError("import differs: " + name)
    (out/"START").write_text("START\n")
    while not (out/"START.CHECKED").exists():
        time.sleep(.001)
    started = time.perf_counter()
    fit, error = None, None
    try:
        fit = fit_model(data)
    except Exception as exc:
        error = type(exc).__name__ + ": " + str(exc)
    seconds = time.perf_counter()-started
    (out/"DONE").write_text("DONE\n")
    meta = dict(rc=0 if fit is not None else 1, seconds=seconds, error=error,
                pyfixest_version=importlib.metadata.version("pyfixest"), formula=formula)
    if fit is not None:
        b = fit.coef()
        correction = fit._N/(fit._N-1) if c["model"]=="ppml" and not c["clusters"] else 1
        V = fit._vcov*correction
        pd.DataFrame(dict(name=b.index, value=b.values, se=np.sqrt(np.diag(V)))).to_csv(out/"coefficients.csv",index=False)
        pd.DataFrame([(a,z,V[i,j]) for i,a in enumerate(b.index) for j,z in enumerate(b.index)], columns=["row","column","value"]).to_csv(out/"covariance.csv",index=False)
        retained = fit._data
        if "obs_id" not in retained or len(retained)!=fit._N:
            raise ValueError("cannot bind PyFixest retained sample")
        ids = retained.obs_id.to_numpy()
        positions = pd.Index(data.obs_id).get_indexer(ids)
        if np.any(positions < 0):
            raise ValueError("invalid retained observation IDs")
        xb = retained[list(b.index)].to_numpy() @ b.to_numpy()
        prediction = retained.y.to_numpy()-np.asarray(fit.resid()).ravel() if c["model"]=="ols" else np.exp(np.asarray(fit._Y_hat_link).ravel())
        saved = np.sum(fit._light_saved_channels,axis=0)[positions] if c["save_fe"] else np.full(len(ids),np.nan)
        np.asarray([ids,prediction,xb,saved],dtype="<f8").tofile(out/"state.bin")
        meta.update(N=int(fit._N), covariance_multiplier=correction)
        if c["model"]=="ols":
            residual = np.asarray(fit.resid()).ravel()
            meta.update(rss=float(residual@residual), converged=True)
        else:
            meta.update(deviance=float(fit.deviance),converged=bool(fit.convergence))
    write_json(out/"metadata.json",meta)
    print("LIGHT_WORKER_COMPLETE rc="+str(meta["rc"]),flush=True)


if __name__ == "__main__":
    run(read_json(sys.argv[1]))
