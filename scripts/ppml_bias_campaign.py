#!/usr/bin/env python3
"""Freeze or execute disjoint jobs in the fresh-seed PPML confirmation plan.

Run under ci/supervise_stata.py locally. An SCC wrapper can select individual
job IDs without changing data, branch, correction, or FEVC seeds. Existing
attempt directories are never overwritten, including failed processes.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

SCENARIOS = ("fixed_240", "fixed_960", "fixed_3840", "groups_960",
             "groups_3840", "negative_960", "duration_960")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make_jobs():
    jobs = []
    # Finish a balanced first block before additional confirmation blocks.
    for first in range(10001, 10301, 20):
        for sid, scenario in enumerate(SCENARIOS):
            order = ("native", "legacy") if (sid + first) % 2 else ("legacy", "native")
            for engine in order:
                jobs.append(dict(engine=engine, scenario=scenario, first=first,
                                 last=first + 19, method="probe", probes=256))
    for scenario in SCENARIOS:
        jobs.append(dict(engine="native", scenario=scenario, first=10001,
                         last=10020, method="exact", probes=256))
        for first, last in ((10001, 10020), (10021, 10040), (10041, 10050)):
            jobs.append(dict(engine="native", scenario=scenario, first=first,
                             last=last, method="probe", probes=1024))
    for job in jobs:
        job["id"] = "{scenario}-{engine}-{method}-{probes}-{first}-{last}".format(**job)
    return jobs


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--plan", type=Path, required=True)
    p.add_argument("--work", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--plugin", type=Path, required=True)
    p.add_argument("--source-manifest", type=Path, required=True)
    p.add_argument("--stata", required=True)
    p.add_argument("--freeze", action="store_true")
    p.add_argument("--jobs", nargs="*", help="Explicit plan IDs; default all unstarted jobs")
    args = p.parse_args()
    root = Path(__file__).resolve().parents[1]
    identities = {
        "source_manifest_sha256": sha(args.source_manifest),
        "plugin_sha256": sha(args.plugin),
        "adapter_manifest_sha256": sha(args.work / "manifest.json"),
        "runner_sha256": sha(root / "scripts/ppml_bias_mc_run.py"),
    }
    if args.freeze:
        plan = dict(schema=1, identities=identities, jobs=make_jobs(),
                    interpretation="Development confirmation, not release qualification",
                    analysis=dict(
                        moments="All ten external moments; Split Branch worker-wage variance is undefined, with FEVC M/MS reported separately",
                        statistics=["bias", "MCSE of bias", "RMSE", "median error", "5/95 error quantiles", "maximum absolute error"],
                        populations=["all available", "paired available", "joint interior without legacy fallback or projection"],
                        failures="Retain every attempted draw, return code, stage, boundary, fallback and projected indicator; no rescue or silent reruns",
                        comparisons="Pair identical data/branches; verify dataset signatures and unchanged wage/FEVC outputs; native 256/1024/exact comparisons on the prespecified overlapping draws",
                        success_criterion="No universal zero-bias or RMSE-improvement requirement; report Monte Carlo uncertainty and approximation limitations"))
        args.plan.parent.mkdir(parents=True, exist_ok=True)
        with args.plan.open("x") as handle:
            json.dump(plan, handle, indent=2, sort_keys=True)
            handle.write("\n")
        print(f"Frozen {len(plan['jobs'])} jobs / 4690 attempts")
        return
    plan = json.loads(args.plan.read_text())
    if plan["identities"] != identities:
        raise SystemExit("Confirmation inputs differ from the frozen plan")
    known = {job["id"] for job in plan["jobs"]}
    if args.jobs is not None and set(args.jobs) - known:
        raise SystemExit("Unknown job ID")
    for job in plan["jobs"]:
        if args.jobs is not None and job["id"] not in args.jobs:
            continue
        out = args.out / job["id"]
        if out.exists():
            # A failed or partial attempt is evidence, never permission to retry.
            print(f"Already attempted: {job['id']}", flush=True)
            continue
        command = [sys.executable, str(root / "scripts/ppml_bias_mc_run.py"),
                   "--work", str(args.work), "--out", str(out),
                   "--stata", args.stata, "--plugin", str(args.plugin),
                   "--source-manifest", str(args.source_manifest)]
        for field in ("engine", "scenario", "first", "last", "method", "probes"):
            command.extend((f"--{field}", str(job[field])))
        print(f"Starting: {job['id']}", flush=True)
        subprocess.run(command, cwd=root, check=True)


if __name__ == "__main__":
    main()
