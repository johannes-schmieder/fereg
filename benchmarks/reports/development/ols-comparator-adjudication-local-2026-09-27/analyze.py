"""Reconstruct independent bread and bind the untimed probe outputs."""
import hashlib
import json
from pathlib import Path
import sys
import numpy as np
import pandas as pd

root = Path(sys.argv[1])
records = json.loads((root / "results.json").read_text())
for record in records:
    directory = root / record["case"]["design"]
    data = pd.read_stata(directory / "input.dta", convert_categoricals=False)
    k = record["case"]["k"]
    X = data[[f"x{j}" for j in range(1, k+1)]].to_numpy()
    F = [np.eye(data[e].nunique())[pd.factorize(data[e], sort=True)[0]]
         for e in data if e.startswith("g")]
    D = np.column_stack([X, *F])
    u, singular, vh = np.linalg.svd(D, full_matrices=False)
    rank = int(np.sum(singular > max(D.shape) * np.finfo(float).eps * singular[0]))
    assert rank == record["oracle"]["rank"]
    scaled = vh[:rank, :k] / singular[:rank, None]
    bread = scaled.T @ scaled
    record["oracle"]["finite_bread"] = bread.tolist()
    for arm, values in record["arms"].items():
        V = np.loadtxt(directory / f"{arm}-V.csv", delimiter=",")
        meta = values["metadata"]
        actual_bread = V * (meta["N"]-meta["df_k"]) / meta["rss"]
        values["maximum_unscaled_bread_error"] = float(np.max(abs(actual_bread - bread)))
    record["output_sha256"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                               for p in directory.iterdir() if p.suffix in (".csv", ".bin", ".json")}
with Path(sys.argv[2]).open("x") as stream:
    json.dump(records, stream, indent=2, allow_nan=False)
    stream.write("\n")
