"""Independent bounded explicit-design checks, run separately from timings."""
import argparse
import json
from pathlib import Path
import sys

import pandas as pd

from audit import distance, read

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "benchmarks/systematic"))
from oracle import oracle


def check(root):
    plan, progress = read(root / "plan.json"), read(root / "progress.json")
    records = []
    for index, cell in enumerate(plan["cells"]):
        if any(cell.get(key) for key in ("weight", "vce", "absorb", "restriction", "fit_options")):
            raise ValueError("Explicit oracle does not implement this cell's options")
        expected = oracle(root / f"data/c{index}-full.dta", cell)
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
            checks = dict(coefficients=distance(b, expected["coefficients"], 1e-7),
                          covariance=distance(V, expected["covariance"], 1e-6))
            for key in (["rss"] if cell["model"] == "ols" else ["ll", "deviance"]):
                actual = result["metadata"].get(key, result["metadata"].get("loglik"))
                checks[key] = distance([actual], [expected[key]], 1e-8)
            variants[variant] = dict(checks=checks, passed=all(c["passed"] for c in checks.values()))
        records.append(dict(cell=cell, oracle=expected, variants=variants))
    return records


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    with args.output.open("x") as stream:
        json.dump(check(args.root), stream, indent=2, allow_nan=False)
        stream.write("\n")
