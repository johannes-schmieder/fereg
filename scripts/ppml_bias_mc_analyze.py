#!/usr/bin/env python3
"""Summarize every attempted confirmation draw without dropping failed fits."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
import pandas as pd


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_receipt(plan, job, receipt):
    for name in ("engine", "scenario", "first", "last", "method", "probes"):
        if receipt[name] != job[name]:
            raise ValueError(f"Receipt disagrees with frozen job: {name}")
    identities = plan["identities"]
    for name in ("plugin_sha256", "adapter_manifest_sha256"):
        if receipt[name] != identities[name]:
            raise ValueError(f"Confirmation identity changed: {name}")
    source = (json.dumps(receipt["source"], indent=2, sort_keys=True)+"\n").encode()
    if hashlib.sha256(source).hexdigest() != identities["source_manifest_sha256"]:
        raise ValueError("Confirmation source manifest changed")


def statistics(values):
    x = np.asarray(values, dtype=float)
    x = x[np.isfinite(x)]
    n = len(x)
    if not n:
        return {"n": 0}
    rmse = float(np.sqrt(np.mean(x*x)))
    return dict(n=n, bias=float(x.mean()),
                bias_mcse=float(x.std(ddof=1)/np.sqrt(n)) if n > 1 else None,
                rmse=rmse,
                rmse_mcse=float((x*x).std(ddof=1)/(2*rmse*np.sqrt(n))) if n > 1 and rmse else None,
                error_q05=float(np.quantile(x, .05)),
                error_median=float(np.median(x)),
                error_q95=float(np.quantile(x, .95)),
                maximum_absolute_error=float(np.max(np.abs(x))))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--plan", type=Path, required=True)
    p.add_argument("--runs", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    plan = json.loads(args.plan.read_text())
    attempts, moments, missing, incomplete, files = [], [], [], [], {}
    for job in plan["jobs"]:
        directory = args.runs / job["id"]
        if not directory.exists():
            missing.append(job["id"])
            continue
        receipt_path = directory / "receipt.json"
        if not receipt_path.exists():
            incomplete.append(job["id"])
            continue
        receipt = json.loads(receipt_path.read_text())
        validate_receipt(plan, job, receipt)
        files[job["id"]] = {"receipt_sha256": sha(receipt_path)}
        if not receipt.get("completed"):
            incomplete.append(job["id"])
            continue
        for kind in ("attempts", "moments"):
            path = directory / f"{kind}.csv"
            if sha(path) != receipt[f"{kind}_sha256"]:
                raise ValueError(f"Changed {path}")
            frame = pd.read_csv(path)
            if not frame.scenario.eq(job["scenario"]).all() or not frame.correction.eq(job["method"]).all():
                raise ValueError("CSV scenario or correction changed")
            if kind == "attempts" and list(frame.rep) != list(range(job["first"], job["last"]+1)):
                raise ValueError("CSV attempt inventory changed")
            if kind == "moments" and not frame.rep.between(job["first"], job["last"]).all():
                raise ValueError("Moment row outside the frozen draw range")
            frame["engine"] = job["engine"]
            frame["probes"] = 0 if job["method"] == "exact" else job["probes"]
            frame["job"] = job["id"]
            (attempts if kind == "attempts" else moments).append(frame)
            files[job["id"]][f"{kind}_sha256"] = sha(path)
    if not attempts:
        raise SystemExit("No completed attempts")
    calls = pd.concat(attempts, ignore_index=True)
    output = pd.concat(moments, ignore_index=True)
    identity = ["scenario", "engine", "probes", "rep"]
    if calls.duplicated(identity).any():
        raise ValueError("Duplicate confirmation attempts")
    errors = output.assign(error=output.estimate-output.truth)
    flag_names = [f"{part}_{flag}" for part in ("full", "a", "b")
                  for flag in ("fallback", "boundary", "projected")]
    calls["interior"] = (calls.rc == 0) & calls[flag_names].eq(0).all(axis=1)
    failures = []
    for key, frame in calls.groupby(["scenario", "engine", "probes"], sort=True):
        record = dict(zip(("scenario", "engine", "probes"), key))
        record.update(attempted=len(frame), failed=int((frame.rc != 0).sum()),
                      interior=int(frame.interior.sum()),
                      return_codes={str(int(k)): int(v) for k, v in frame.rc.value_counts().items()},
                      flags={name: dict(positive=int(frame[name].gt(0).sum()),
                                        observed=int(frame[name].notna().sum())) for name in flag_names},
                      fevc={population: dict(success=int(frame[f"kstatus_{population}"].eq(1).sum()),
                            observed=int(frame[f"kstatus_{population}"].notna().sum()),
                            return_codes={str(int(k)):int(v) for k,v in frame[f"fevc_rc_{population}"].value_counts().items()})
                            for population in ("m", "ms")})
        failures.append(record)
    columns = ["scenario", "engine", "probes", "population", "method", "moment"]
    summaries = []
    for key, frame in errors.groupby(columns, sort=True):
        summaries.append(dict(zip(columns, key), **statistics(frame.error)))

    paired = []
    checks = []
    # Report different numerical approximations separately from the production
    # legacy comparison. Finite samples are paired only when both fits exist.
    for scenario in sorted(calls.scenario.unique()):
        for comparator, probes in (("legacy", 256), ("native", 0), ("native", 1024)):
            left = calls[(calls.scenario == scenario) & (calls.engine == "native") & (calls.probes == 256)]
            right = calls[(calls.scenario == scenario) & (calls.engine == comparator) & (calls.probes == probes)]
            shared = left.merge(right, on="rep", suffixes=("_native", "_reference"))
            if shared.empty:
                continue
            matching = shared.dataset_signature_native == shared.dataset_signature_reference
            if not matching.all():
                raise ValueError(f"Unpaired DGP/branches: {scenario} {comparator}/{probes}")
            successful = shared[(shared.rc_native == 0) & (shared.rc_reference == 0)]
            interior = successful[successful.interior_native & successful.interior_reference]
            max_unchanged = 0.
            for population, allowed in (("paired_available", successful.rep), ("joint_interior", interior.rep)):
                a = errors[(errors.scenario == scenario) & (errors.engine == "native") & (errors.probes == 256) & errors.rep.isin(allowed)]
                b = errors[(errors.scenario == scenario) & (errors.engine == comparator) & (errors.probes == probes) & errors.rep.isin(allowed)]
                merged = a.merge(b, on=["rep", "population", "method", "moment"], suffixes=("_native", "_reference"))
                if not np.array_equal(merged.truth_native.to_numpy(), merged.truth_reference.to_numpy()):
                    raise ValueError("Paired moment truths differ")
                unchanged = merged[(merged.population.str.startswith("kss_")) |
                                   ((merged.population == "branch") & merged.moment.isin([1, 2, 5]))]
                if len(unchanged):
                    max_unchanged = max(max_unchanged, float((unchanged.estimate_native-unchanged.estimate_reference).abs().max()))
                for key, frame in merged.groupby(["population", "method", "moment"], sort=True):
                    record = dict(scenario=scenario, comparator=comparator, comparator_probes=probes,
                                  comparison_population=population,
                                  **dict(zip(("population", "method", "moment"), key)))
                    record.update(native=statistics(frame.error_native),
                                  reference=statistics(frame.error_reference),
                                  paired_difference=statistics(frame.estimate_native-frame.estimate_reference))
                    paired.append(record)
            checks.append(dict(scenario=scenario, comparator=comparator, probes=probes,
                               paired_attempts=len(shared), paired_successes=len(successful),
                               joint_interior=len(interior), maximum_wage_fevc_difference=max_unchanged))
            # The native adapter sorts while spreading FE values. Unchanged
            # wage/FEVC operations can therefore differ by summation roundoff.
            if max_unchanged > 1e-10:
                raise ValueError(f"Wage or FEVC changed: {scenario} {max_unchanged}")
    result = dict(schema=1, plan_sha256=sha(args.plan), script_sha256=sha(Path(__file__)),
                  wage_fevc_absolute_tolerance=1e-10, complete=not missing and not incomplete,
                  planned_attempts=sum(job["last"]-job["first"]+1 for job in plan["jobs"]), recorded_attempts=len(calls),
                  missing_jobs=missing, incomplete_jobs=incomplete, inputs=files,
                  failures=failures, all_available=summaries, paired=paired, pairing_checks=checks,
                  undefined="Split Branch moment 1 (worker-wage variance) is undefined; its plugin and FEVC M/MS estimates remain separately reported.")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("x") as handle:
        json.dump(result, handle, indent=2, sort_keys=True, allow_nan=False,
                  default=lambda value: value.item())
        handle.write("\n")
    print(json.dumps({key: result[key] for key in ("complete", "recorded_attempts", "planned_attempts")}, indent=2))


if __name__ == "__main__":
    main()
