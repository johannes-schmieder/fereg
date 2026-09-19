"""Canonical numerical inputs; graph generation reuses the frozen harness."""
import datetime
import hashlib
import importlib.util
from pathlib import Path

import numpy as np
import pandas as pd

from common import DESIGNS, REPO, SEED, sha256, write_json


def store(frame, path, c, provenance=None):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    assert len(frame) == c["n"] and np.isfinite(frame.to_numpy()).all()
    assert frame.obs_id.is_unique
    frame.to_stata(path, write_index=False, version=118,
                   time_stamp=datetime.datetime(2000, 1, 1))
    back = pd.read_stata(path, convert_categoricals=False)
    if not np.array_equal(frame.to_numpy(), back.to_numpy()):
        raise ValueError("DTA numeric roundtrip changed inputs")
    info = dict(schema="fereg-light-input-v1", n=len(frame), cell=c,
                sha256=sha256(path), columns=list(frame),
                column_sha256={name: hashlib.sha256(np.asarray(frame[name], dtype="<f8").tobytes()).hexdigest()
                               for name in frame}, provenance=provenance,
                minimum_level_rows={g: int(frame.groupby(g).size().min()) for g in c["effects"]})
    write_json(path.with_suffix(".json"), info)
    return info


def generate(c, path):
    path = Path(path)
    spec = importlib.util.spec_from_file_location("systematic_input_generator", REPO / "benchmarks/systematic/generate.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    temporary = path.with_name(path.stem + "-base.dta")
    # The outcome always uses K=10, even in width experiments.
    base = dict(c, k=10, section="main")
    module.generate(base, SEED, temporary, {"designs": DESIGNS})
    frame = pd.read_stata(temporary, convert_categoricals=False)
    for j in range(11, c["k"] + 1):
        frame[f"x{j}"] = np.random.default_rng(np.random.SeedSequence([SEED, j])).normal(size=len(frame))
    if c["k"] < 10:
        frame = frame.drop(columns=[f"x{j}" for j in range(c["k"] + 1, 11)])
    if c["slopes"]:
        frame["z"] = np.random.default_rng(np.random.SeedSequence([SEED, 999])).normal(size=len(frame))
        if (frame.groupby("g2").z.var() <= 0).any():
            raise ValueError("slope has no within-firm variation")
    ordered = ["obs_id", *c["effects"], *[f"x{j}" for j in range(1, c["k"] + 1)], "y"]
    if c["slopes"]:
        ordered.append("z")
    result = store(frame[ordered], path, c, {"generator_sha256": sha256(REPO / "benchmarks/systematic/generate.py"), "seed": SEED})
    temporary.unlink()
    temporary.with_suffix(".json").unlink()
    return result
