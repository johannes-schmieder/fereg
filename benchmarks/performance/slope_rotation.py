"""Frozen-input OLS slope rotations, with scientific failures left unranked."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path

import numpy as np
import pandas as pd

from slope_probe import run_case


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def successful(result):
    return (result["process_rc"] == 0 and result["sentinel"]
            and not result["timed_out"] and not result["competing_work"])


def compare(candidate, other, tolerance):
    a, b = read(candidate / "receipt.json"), read(other / "receipt.json")
    if not successful(a) or not successful(b):
        return {"ranked": False, "checks": {"application": False}}
    am, bm = a["metadata"], b["metadata"]
    ar, br = (pd.read_csv(path / "rows.csv").sort_values("obs_id") for path in (candidate, other))
    sample = (ar.obs_id.is_unique and br.obs_id.is_unique
              and np.array_equal(ar.obs_id, br.obs_id)
              and np.array_equal(ar["sample"], br["sample"]))
    checks = {
        "application": True,
        "sample": bool(sample and am["N"] == bm["N"]),
        "converged": am["converged"] == bm["converged"] == 1,
        "dof": am["df_r"] == bm["df_r"],
        "rss": math.isclose(am["rss"], bm["rss"], rel_tol=tolerance["objective_rtol"],
                            abs_tol=tolerance["objective_atol"]),
    }
    for key, filename, prefix in (("coefficients", "b.csv", "coefficient"), ("covariance", "V.csv", "vce")):
        x, y = (np.loadtxt(path / filename, delimiter=",") for path in (candidate, other))
        checks[key] = bool(x.shape == y.shape and np.isfinite(x).all() and np.isfinite(y).all()
                           and np.allclose(x, y, rtol=tolerance[prefix + "_rtol"], atol=tolerance[prefix + "_atol"]))
    for field in ("fitted", "error"):
        checks[field] = bool(sample and np.isfinite(ar[field]).all() and np.isfinite(br[field]).all()
                             and np.allclose(ar[field], br[field], rtol=1e-7, atol=1e-7))
    ranked = all(checks.values())
    return {"ranked": ranked, "checks": checks,
            "ratio": am["seconds"] / bm["seconds"] if ranked else None,
            "candidate_seconds": am["seconds"], "other_seconds": bm["seconds"],
            "max_fitted_difference": float(np.max(np.abs(ar.fitted.to_numpy() - br.fitted.to_numpy()))) if sample else None}


def verify(args, plan):
    slots = 4 if args.smoke else 28
    if int(os.environ.get("NSLOTS", 0)) != slots or len(os.sched_getaffinity(0)) != slots:
        raise ValueError("unexpected SCC slot/affinity allocation")
    if not args.smoke and "E5-2680 v4" not in Path("/proc/cpuinfo").read_text():
        raise ValueError("unexpected timing CPU class")
    identities = {"plan": sha(args.plan), "source_files": {}, "plugins": {}, "inputs": {}}
    for name in ("baseline", "candidate"):
        root = getattr(args, name)
        for filename, digest in read(args.run / (name + "-source.json"))["files"].items():
            if sha(root / filename) != digest:
                raise ValueError(name + " source mismatch: " + filename)
        identities["source_files"][name] = sha(args.run / (name + "-source.json"))
        identities["plugins"][name] = sha(root / "dist/fereg_linux.plugin")
    if identities["plugins"]["baseline"] != plan["baseline_plugin_sha256"]:
        raise ValueError("baseline plugin mismatch")
    # Build smoke produces the candidate artifact; the full rotation consumes
    # that exact binary only after the application's validation receipts pass.
    if not args.smoke:
        acceptance = read(args.run / "smoke-probe/complete.json")
        if not acceptance["complete"]:
            raise ValueError("probe did not complete")
        for filename in ("summary.json", "slope-oracle.json"):
            result = read(args.run / "output" / filename)
            if result["status"] != "success" or result["tests_failed"] != 0:
                raise ValueError("upstream scientific fixture failed")
        built = read(args.run / "smoke-probe/identity.json")
        if identities["plugins"] != built["plugins"]:
            raise ValueError("artifacts differ from smoke")
    frozen = read(args.reference / "env/frozen.json")
    for filename, digest in frozen["runtime"].items():
        if filename.startswith(("references/", "env/R/fixest/")) and sha(args.reference / filename) != digest:
            raise ValueError("stock reference mismatch: " + filename)
    identities["reference"] = frozen["fingerprint"]
    for filename, digest in read(args.run / "input-files.json").items():
        if sha(args.run / filename) != digest:
            raise ValueError("literal input mismatch: " + filename)
        identities["inputs"][filename] = digest
    identities["harness"] = {p.name: sha(p) for p in (Path(__file__), Path(__file__).with_name("slope_probe.py"))}
    return identities


def main(args):
    plan = read(args.plan)
    identities = verify(args, plan)
    args.output.mkdir(exist_ok=False)
    write(args.output / "identity.json", identities)
    write(args.output / "plan.json", plan)
    records, comparisons = [], []
    cases = plan["cases"][:1] if args.smoke else plan["cases"]
    seeds = [plan["smoke_seed"]] if args.smoke else plan["seeds"]
    for repetition in range(1 if args.smoke else plan["rotations_per_seed"]):
        for seed in seeds:
            for cell in cases:
                data = args.run / ("pilot-inputs" if args.smoke else "inputs")
                if not args.smoke:
                    data = data / str(seed)
                data = data / cell["id"]
                arms = list(cell["arms"])
                if repetition % 2:
                    arms.reverse()
                directory = args.output / str(seed) / cell["id"] / str(repetition)
                for arm in arms:
                    root = args.baseline if arm == "baseline" else args.candidate
                    result = run_case(cell, data, directory / arm,
                                      "native" if arm in ("baseline", "candidate") else arm,
                                      root, args.reference / "env/R")
                    record = dict(seed=seed, case=cell["id"], repetition=repetition,
                                  arm=arm, input_sha256=sha(data / "input.dta"), result=result)
                    records.append(record)
                    write(args.output / "results.json", records)
                    print(seed, cell["id"], repetition, arm, result.get("metadata", result), flush=True)
                    if not successful(result):
                        raise RuntimeError("application failed; preserve this attempt without timing rank")
                checks = {arm: compare(directory / "candidate", directory / arm, plan["science"])
                          for arm in arms if arm != "candidate"}
                comparisons.append(dict(seed=seed, case=cell["id"], repetition=repetition, comparisons=checks))
                write(args.output / "comparisons.json", comparisons)
                if not checks["baseline"]["ranked"]:
                    raise RuntimeError("candidate differs from baseline science")
    write(args.output / "complete.json", {"complete": True, "fits": len(records), "pairs": len(comparisons)})
    print("FEREG_SLOPE_ROTATION_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("run", "plan", "baseline", "candidate", "reference", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--smoke", action="store_true")
    main(parser.parse_args())
