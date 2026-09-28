"""Bounded candidate/baseline/stock rotations using the systematic timing gate."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "systematic"))
from common import read_json, sha256, write_json
from generate import generate
from oracle import oracle
from runner import compare, measure, read_coefficients
from rotation_threads import native_threads_match, variant_cell


def oracle_checks(directory, independent, tolerance):
    receipt = read_json(directory / "result.json")
    if receipt["status"] != "pass" or independent.get("status") != "pass":
        return {"output": False}
    names = independent["coefficient_names"]
    coefficients = read_coefficients(directory)
    with (directory / "covariance.csv").open() as stream:
        covariance = {(r["row"], r["column"]): float(r["value"]) for r in csv.DictReader(stream)}
    objective = "rss" if receipt["cell"]["model"] == "ols" else "deviance"
    return {
        "sample": receipt.get("sample_sha256") == independent["sample_sha256"],
        "converged": receipt["metadata"].get("converged") == 1,
        "coefficients": bool(np.allclose([coefficients[n][0] for n in names], independent["coefficients"],
            rtol=tolerance["coefficient_rtol"], atol=tolerance["coefficient_atol"])),
        "covariance": bool(np.allclose([[covariance[a, b] for b in names] for a in names], independent["covariance"],
            rtol=tolerance["vce_rtol"], atol=tolerance["vce_atol"])),
        "objective": math.isclose(receipt["metadata"][objective], independent[objective],
            rel_tol=tolerance["objective_rtol"], abs_tol=tolerance["objective_atol"]),
    }


def run(args):
    if os.environ.get("NSLOTS") != "28" or len(os.sched_getaffinity(0)) != 28:
        raise ValueError("rotations require an isolated 28-core allocation")
    if "E5-2680 v4" not in Path("/proc/cpuinfo").read_text():
        raise ValueError("unexpected CPU class")
    args.output.mkdir(parents=True, exist_ok=False)
    plan, manifest = read_json(args.plan), read_json(args.manifest)
    for case in plan["cases"]:
        for variant in ("baseline", "candidate", "stock"):
            variant_cell(case, variant)
    # Verify both computational snapshots and stock reference before any fits.
    identities = {"plan": sha256(args.plan), "manifest": sha256(args.manifest), "sources": {}, "harness": {}}
    for name in ("baseline", "candidate"):
        root = args.roots / name
        expected = read_json(root / "source.json")
        for filename, digest in expected["files"].items():
            if sha256(root / "candidate" / filename) != digest:
                raise ValueError(f"{name} source changed: {filename}")
        identities["sources"][name] = {
            "source": sha256(root / "source.json"),
            "plugin": sha256(root / "candidate/dist/fereg_linux.plugin"),
        }
        if identities["sources"][name]["plugin"] != plan["plugins"][name]:
            raise ValueError(f"{name} plugin changed")
    frozen = read_json(args.reference / "env/frozen.json")
    for filename, digest in frozen["runtime"].items():
        if filename.startswith(("references/", "env/R/fixest/")):
            if sha256(args.reference / filename) != digest:
                raise ValueError(f"stock reference changed: {filename}")
    identities["reference"] = frozen["fingerprint"]
    for directory in (Path(__file__).parent, Path(__file__).resolve().parents[1] / "systematic"):
        for path in directory.iterdir():
            if path.suffix in (".py", ".R", ".do"):
                identities["harness"][str(path.relative_to(directory.parent))] = sha256(path)
    fingerprint = hashlib.sha256(json.dumps(identities, sort_keys=True).encode()).hexdigest()
    write_json(args.output / "identity.json", identities)
    write_json(args.output / "plan.json", plan)
    records = []
    for case in plan["cases"]:
        cell = case["cell"]
        for repetition, seed in enumerate(case["seeds"]):
            output = args.output / case["id"] / f"pair-{repetition + 1}"
            output.mkdir(parents=True)
            data = args.scratch / "input.dta"
            generate(cell, seed, data, manifest)
            expected_input = case.get("input_sha256", {}).get(str(seed))
            if expected_input is not None and sha256(data) != expected_input:
                raise ValueError("regenerated input differs from registered prior bytes")
            warm = args.scratch / "warm.dta"
            generate(dict(cell, n=manifest["measurement"]["warmup_rows"], section="main"), seed, warm, manifest)
            write_json(output / "input.json", read_json(data.with_suffix(".json")))
            independent = None
            if case.get("oracle"):
                try:
                    independent = oracle(data, cell)
                except Exception as error:
                    independent = {"status": "failure", "error": str(error)}
                write_json(output / "oracle.json", independent)
            order = plan["orders"][repetition % len(plan["orders"])]
            for variant in order:
                arm = ("fereg" if cell["model"] == "ols" else "feppml") if variant != "stock" else case["stock_arm"]
                if variant == "stock" and arm not in ("fixest", "fixest_strict"):
                    raise ValueError("unregistered stock command")
                measure(args.roots / variant, variant_cell(case, variant), arm, "warm", data, warm,
                        output / variant, manifest, fingerprint, limits=case["limits"][variant])
            comparisons = {name: compare(output / "candidate", output / name, manifest["science"])
                           for name in ("baseline", "stock")}
            # Native publication policy must agree even when the comparator
            # cannot expose or uses a different DoF/rank convention.
            a = read_json(output / "candidate/result.json")
            b = read_json(output / "baseline/result.json")
            if a["status"] == b["status"] == "pass":
                if "arm_threads" in case:
                    comparisons["baseline"]["checks"]["native_threads"] = all(
                        native_threads_match(receipt, variant_cell(case, name)["threads"])
                        for name, receipt in (("baseline", b), ("candidate", a)))
                comparisons["baseline"]["checks"]["native_rank_dof"] = all(
                    a["metadata"].get(k) == b["metadata"].get(k)
                    for k in ("rank", "df_a", "df_r", "df_resid"))
                if not all(comparisons["baseline"]["checks"].values()):
                    comparisons["baseline"]["status"] = "accuracy_failure"
            record = {"case": case["id"], "repetition": repetition + 1, "seed": seed,
                      "order": order, "comparisons": comparisons}
            if independent is not None:
                record["oracle_checks"] = {variant: oracle_checks(output / variant, independent, manifest["science"])
                                           for variant in ("baseline", "candidate")}
            write_json(output / "comparison.json", record)
            records.append(record)
            write_json(args.output / "progress.json", records)
            print(json.dumps(record), flush=True)
            data.unlink()
            data.with_suffix(".json").unlink()
            warm.unlink()
            warm.with_suffix(".json").unlink()
    write_json(args.output / "complete.json", {"complete": True, "pairs": len(records)})
    print("FEREG_ROTATIONS_COMPLETE", flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    for name in ("roots", "reference", "manifest", "plan", "output", "scratch"):
        p.add_argument("--" + name, type=Path, required=True)
    run(p.parse_args())
