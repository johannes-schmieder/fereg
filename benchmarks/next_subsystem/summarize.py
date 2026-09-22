"""Pair distributions, exact-export checks and measured native memory changes."""
import argparse
import json
from pathlib import Path
import statistics

import pandas as pd

from audit import audit, compare, exact_exports, read, runtime


def summarize(root, baseline, candidate):
    plan, progress = read(root / "plan.json"), read(root / "progress.json")
    cells = []
    for index, cell in enumerate(plan["cells"]):
        pairs, repeats = [], []
        for repetition in range(plan["repeats"]):
            attempts = {v: next(r for r in progress if r["case"] == index and r["repetition"] == repetition and r["variant"] == v) for v in [baseline, candidate]}
            paths = {v: root / r["result"] for v, r in attempts.items()}
            clean = all(r["status"] == "pass" and not r["overlaps"] and not r["monitor_errors"] for r in attempts.values())
            peaks = {}
            if clean:
                for v, path in paths.items():
                    if cell["model"] == "ols":
                        table = pd.read_csv(path / "fereg_runtime.csv")
                        peaks[v] = float(table[(table.row == 13) & (table.column == 2)].value.iloc[0])
                    else:
                        peaks[v] = runtime(path, "ppml")["allocation_peak"]
            pairs.append(dict(repetition=repetition, clean=clean,
                ratio=attempts[candidate]["seconds"] / attempts[baseline]["seconds"] if clean else None,
                checks=compare(paths[baseline], paths[candidate], cell["model"]),
                exact=exact_exports(paths[baseline], paths[candidate]) if clean else None,
                tracked_peak_bytes=peaks))
            if repetition:
                for variant, path in paths.items():
                    first = next(r for r in progress if r["case"] == index and r["repetition"] == 0 and r["variant"] == variant)
                    repeats.append(dict(variant=variant, repetition=repetition,
                        checks=compare(root / first["result"], path, cell["model"])))
        ratios = [p["ratio"] for p in pairs if p["clean"]]
        cells.append(dict(cell=cell, pairs=pairs, repeats=repeats,
            ratios=dict(minimum=min(ratios), median=statistics.median(ratios), maximum=max(ratios)) if ratios else None))
    return dict(scope="Local development; clean timings require separate scientific adjudication", cells=cells, audit=audit(root))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    args = parser.parse_args()
    with args.output.open("x") as stream:
        json.dump(summarize(args.root, args.baseline, args.candidate), stream, indent=2, allow_nan=False)
        stream.write("\n")
