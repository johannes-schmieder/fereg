"""Independent bounded explicit-design checks, run separately from timings."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile

import pandas as pd

from audit import distance, read

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "benchmarks/systematic"))
from oracle import oracle


def oracle_sample(frame, cell):
    if cell["model"] == "ppml" and not (frame.y > 0).all():
        raise ValueError("Nonpositive PPML outcomes require a separate separation oracle")
    candidate = len(frame)
    rounds = []
    while True:
        keep = pd.Series(True, index=frame.index)
        for effect in cell["effects"]:
            keep &= frame.groupby(effect)[effect].transform("size") > 1
        dropped = int((~keep).sum())
        if not dropped:
            break
        rounds.append(dropped)
        frame = frame.loc[keep].copy()
    if frame.empty:
        raise ValueError("Independent singleton pruning removed the entire sample")
    digest = hashlib.sha256()
    for observation in frame.obs_id:
        digest.update(int(observation).to_bytes(8, "little"))
    return frame, dict(candidate=candidate, retained=len(frame),
        singleton_drops_by_round=rounds, sha256=digest.hexdigest())


def check(root):
    plan, progress = read(root / "plan.json"), read(root / "progress.json")
    records = []
    for index, cell in enumerate(plan["cells"]):
        if any(cell.get(key) for key in ("weight", "vce", "absorb", "restriction", "fit_options")):
            raise ValueError("Explicit oracle does not implement this cell's options")
        frame = pd.read_stata(root / f"data/c{index}-full.dta", convert_categoricals=False)
        frame, sample = oracle_sample(frame, cell)
        with tempfile.TemporaryDirectory(prefix="explicit-oracle-", dir=root) as directory:
            path = Path(directory) / "retained.dta"
            frame.to_stata(path, write_index=False, version=118)
            expected = oracle(path, cell)
        variants = {}
        names = expected["coefficient_names"]
        for variant in plan["variants"]:
            attempt = next(r for r in progress if r["case"] == index and r["variant"] == variant)
            path = root / attempt["result"]
            result = read(path / "result.json")
            if result["status"] != "pass":
                variants[variant] = dict(status=result["status"], passed=False)
                continue
            b = pd.read_csv(path / "coefficients.csv").set_index("name").loc[names].value
            V = pd.read_csv(path / "covariance.csv").pivot(index="row", columns="column", values="value").loc[names, names]
            checks = dict(sample=result["sample_sha256"] == sample["sha256"],
                          N=result["metadata"]["N"] == sample["retained"],
                          coefficients=distance(b, expected["coefficients"], 1e-7),
                          covariance=distance(V, expected["covariance"], 1e-6))
            for key in (["rss"] if cell["model"] == "ols" else ["ll", "deviance"]):
                actual = result["metadata"].get(key, result["metadata"].get("loglik"))
                checks[key] = distance([actual], [expected[key]], 1e-8)
            variants[variant] = dict(checks=checks,
                passed=all(c if isinstance(c, bool) else c["passed"] for c in checks.values()))
        records.append(dict(cell=cell, independent_sample=sample, oracle=expected, variants=variants))
    return records


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    with args.output.open("x") as stream:
        json.dump(check(args.root), stream, indent=2, allow_nan=False)
        stream.write("\n")
