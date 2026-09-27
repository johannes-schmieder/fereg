"""Run from repository root with STATA_BIN, R_STOCK and an unused OUTPUT_DIR.

NumPy/Pandas provide the independent explicit design; no SciPy or estimator
library is used by the oracle. R_STOCK must be an isolated pinned stock install.
"""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys

import numpy as np
import pandas as pd

repo = Path.cwd()
report = Path(__file__).resolve().parent
registration = json.loads((report / "registration.json").read_text())
output = Path(os.environ["OUTPUT_DIR"]).resolve()
output.mkdir(exist_ok=False)
sys.path.insert(0, str(repo / "benchmarks/systematic"))
from generate import generate

manifest = json.loads((repo / "benchmarks/systematic/manifest-light-v2.json").read_text())
tolerance = manifest["science"]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(repo / "dist/fereg_macos.plugin") == registration["native_plugin_sha256"]
environment = {k: v for k, v in os.environ.items() if not k.startswith("FEREG_")}
environment.update(OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS="1", VECLIB_MAXIMUM_THREADS="1")
records = []

for case in registration["cases"]:
    directory = output / case["design"]
    directory.mkdir()
    cell = {k: v for k, v in case.items() if k != "seed"}
    cell.update(model="ols", section="main", threads=1,
                effects=manifest["designs"][case["design"]]["effects"])
    info = generate(cell, case["seed"], directory / "input.dta", manifest)
    frame = pd.read_stata(directory / "input.dta", convert_categoricals=False)
    binary = b"".join(np.asarray(frame[c], dtype="<f8").tobytes() for c in frame)
    (directory / "input.bin").write_bytes(binary)
    (directory / "columns.txt").write_text("\n".join(frame.columns) + "\n")
    subprocess.run(["Rscript", str(report / "reference.R"), os.environ["R_STOCK"],
                    str(directory / "input.bin"), str(directory), ",".join(cell["effects"])],
                   env=environment, check=True, timeout=120, stdout=(directory / "reference.log").open("w"),
                   stderr=subprocess.STDOUT)
    assert (directory / "r-input.bin").read_bytes() == binary
    k = cell["k"]
    controls = [f"x{j}" for j in range(1, k + 1)]
    script = f'''version 19.0
clear all
set more off
set processors 1
sysdir set PLUS "{directory}"
sysdir set PERSONAL "{directory}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{repo}/stata"
adopath ++ "{repo}/dist"
use "{directory}/input.dta", clear
mata:
fh=fopen("{directory}/stata-input.bin","w")
bc=bufio()
names=tokens("{' '.join(frame.columns)}")
for (j=1;j<=cols(names);j++) fbufput(bc,fh,"%8z",st_data(.,names[j]))
fclose(fh)
end
fereg y {' '.join(controls)}, absorb({' '.join(cell['effects'])}) threads(1)
assert e(converged)==1
generate byte sample=e(sample)
assert sample==1
mata:
b=st_matrix("e(b)")[1,1..{k}]
V=st_matrix("e(V)")[1..{k},1..{k}]
fh=fopen("{directory}/native-b.csv","w")
fput(fh,"name,coefficient,se")
for (j=1;j<={k};j++) fput(fh,"x"+strofreal(j)+","+strofreal(b[j],"%24.17g")+","+strofreal(sqrt(V[j,j]),"%24.17g"))
fclose(fh)
fh=fopen("{directory}/native-V.csv","w")
for (j=1;j<={k};j++) fput(fh,invtokens(strofreal(V[j,.],"%24.17g"),","))
fclose(fh)
fh=fopen("{directory}/native-meta.csv","w")
fput(fh,"N,rss,df_k,fit_nparams,converged")
m=(st_numscalar("e(N)"),st_numscalar("e(rss)"),st_numscalar("e(N)")-st_numscalar("e(df_r)"),st_numscalar("e(rank)")+st_numscalar("e(df_a)"),st_numscalar("e(converged)"))
fput(fh,invtokens(strofreal(m,"%24.17g"),","))
fclose(fh)
end
keep obs_id
export delimited using "{directory}/native-sample.csv", replace
display "OLS_NATIVE_ADJUDICATION_COMPLETE"
exit 0
'''
    (directory / "fit.do").write_text(script)
    subprocess.run([os.environ["STATA_BIN"], "-b", "do", str(directory / "fit.do")],
                   cwd=directory, env=environment, check=True, timeout=120)
    assert "OLS_NATIVE_ADJUDICATION_COMPLETE" in (directory / "fit.log").read_text()
    assert (directory / "stata-input.bin").read_bytes() == binary
    X = frame[controls].to_numpy()
    dummies = [np.eye(frame[e].nunique())[pd.factorize(frame[e], sort=True)[0]] for e in cell["effects"]]
    D = np.column_stack([X, *dummies])
    u, singular, vh = np.linalg.svd(D, full_matrices=False)
    rank = int(np.sum(singular > max(D.shape) * np.finfo(float).eps * singular[0]))
    y = frame.y.to_numpy()
    coefficient = vh[:rank].T @ ((u[:, :rank].T @ y) / singular[:rank])
    residual = y - D @ coefficient
    rss = float(residual @ residual)
    scaled = vh[:rank, :k] / singular[:rank, None]
    V = scaled.T @ scaled * rss / (len(y) - rank)
    score = np.max(np.abs(D.T @ residual) / np.linalg.norm(D, axis=0)) / np.linalg.norm(residual)
    assert score < 1e-10
    result = {"case": case, "input_sha256": info["sha256"], "exact_input_replay": True,
              "oracle": {"rank": rank, "rss": rss, "maximum_relative_score": float(score),
                         "smallest_kept_singular_value": float(singular[rank-1]),
                         "largest_discarded_singular_value": float(singular[rank]) if rank < len(singular) else None},
              "arms": {}}
    for arm in ["native", *registration["reference_arms"]]:
        b = pd.read_csv(directory / f"{arm}-b.csv")
        actual_V = np.loadtxt(directory / f"{arm}-V.csv", delimiter=",")
        meta = pd.read_csv(directory / f"{arm}-meta.csv").iloc[0].to_dict()
        sample = pd.read_csv(directory / f"{arm}-sample.csv").obs_id.to_numpy()
        close = lambda a, b, name: bool(np.allclose(a, b, rtol=tolerance[f"{name}_rtol"], atol=tolerance[f"{name}_atol"]))
        checks = {"sample_exact": bool(np.array_equal(sample, frame.obs_id.to_numpy())),
                  "converged": bool(meta["converged"]), "coefficient_names": list(b.name) == controls,
                  "coefficients": close(b.coefficient, coefficient[:k], "coefficient"),
                  "covariance": close(actual_V, V, "vce"), "rss": close(meta["rss"], rss, "objective"),
                  "explicit_design_dof": meta["df_k"] == rank}
        result["arms"][arm] = {"metadata": meta, "checks": checks,
            "maximum_coefficient_error": float(np.max(abs(b.coefficient - coefficient[:k]))),
            "maximum_covariance_error": float(np.max(abs(actual_V - V)))}
    records.append(result)
    (output / "results.json").write_text(json.dumps(records, indent=2, allow_nan=False) + "\n")
    print(case["design"], {arm: value["checks"] for arm, value in result["arms"].items()}, flush=True)
