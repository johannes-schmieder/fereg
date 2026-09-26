"""Enumerate, pack and audit the approved benchmark campaign."""
import argparse
import math
from collections import defaultdict
from pathlib import Path

from common import HERE, cell_id, cells, read_json, write_json, manifest_for, sha256


def light_plan(manifest, mode, results):
    from light import groups, smoke_groups, configure_group, historical_costs, pack
    root = results.parent
    history = root/'input/historical-runs.csv'
    if sha256(history) != manifest['historical_cost_sha256']:
        raise ValueError('Historical cost receipt changed')
    costs = historical_costs(history)
    if mode == 'production':
        from light_control import verify_acceptance
        verify_acceptance(root, 'smoke')
        verify_acceptance(root, 'pilot')
        selected = groups(manifest)
        from provenance import verify
        identity = verify(root)
        pending = []
        for g in selected:
            c, r = g['cell'], g['repetition']
            marker = results/cell_id(c)/f'{"cold" if r==0 else "warm"}-{r}'/'GROUP.COMPLETE.json'
            if marker.exists():
                if read_json(marker).get('measurement_fingerprint') != identity:
                    raise ValueError('Completed group belongs to another runtime')
                continue
            pending.append(g)
        selected = pending
        # Pilot elapsed times refine forecasts, while historical slow outcomes
        # remain available when the pilot omits that comparator or configuration.
        for path in results.glob('**/result.json'):
            d = read_json(path)
            if d.get('measurement_fingerprint') != identity:raise ValueError('Stale pilot result')
            if d['status']=='pass':
                c=d['cell'];key=(c['model'],c['design'],c['n'],c['k'],c['threads'],d['arm'])
                costs[key]=[d['seconds']]
    elif mode == 'smoke':
        selected = smoke_groups(manifest)
    else:
        selected=[]
        for g in groups(manifest):
            c=g['cell']
            if g['repetition'] != 1:continue
            if c.get('slopes') or (c['section']=='main' and c['k']==10 and c['threads']==28 and
                                  ((c['design']=='strong_d6' and c['n']==3932160) or
                                   (c['design']=='two_fe_dense' and c['n']==983040))):
                selected.append(g)
    prepared=[configure_group(g,manifest,costs) for g in selected]
    if mode == 'production':
        # Whole-group elapsed time includes generation, import, initialization,
        # private comparator setup and serialization, not just command timers.
        measured = {}
        for path in results.glob('**/GROUP.COMPLETE.json'):
            record = read_json(path)
            if record.get('elapsed_seconds', 0) > 0:
                measured[cell_id(record['cell'])] = record['elapsed_seconds']
        for g in prepared:
            if cell_id(g['cell']) in measured:
                g['predicted_seconds'] = max(g['predicted_seconds'], measured[cell_id(g['cell'])]*1.2)

    wall=manifest['scc']['wall_seconds'] if mode=='production' else (3600 if mode=='smoke' else 43200)
    if mode=='smoke':
        for g in prepared:
            g['limits']={a:dict(startup=120,command=60,extraction=60) for a in g['limits']}
        bundles=[dict(groups=prepared,predicted_seconds=sum(g['predicted_seconds'] for g in prepared))]
    else:
        bundles=pack(prepared,manifest['scc']['target_bundle_seconds'] if mode=='production' else 10800,wall)
    return dict(schema='systematic-light-plan-v2',mode=mode,bundles=bundles,wall_seconds=wall,
                results_directory='smoke-results' if mode=='smoke' else 'results',
                planned_groups=len(prepared),mem_per_core_gib=3,
                predicted_node_hours=sum(g['predicted_seconds'] for g in prepared)/3600,
                manifest_sha256=sha256(root/'input/manifest.json'))


def build_plan(manifest, mode, results=None):
    from light import enabled
    if enabled(manifest):return light_plan(manifest,mode,results)
    all_cells = cells(manifest)
    if mode == "smoke":
        selected = []
        for model in manifest["arms"]:
            for design in manifest["designs"]:
                cell = next(c for c in all_cells if c["model"] == model and c["design"] == design and c["section"] == "main")
                selected.append(dict(cell, n=1920 if design == "three_fe_sparse" else 7680, threads=4))
        groups = [{"cell": c, "repetition": 1} for c in selected]
        return {"schema": "systematic-plan-v1", "mode": mode,
                "bundles": [{"groups": groups[i:i+4]} for i in range(0, len(groups), 4)]}
    if mode == "pilot":
        selected = []
        for model in manifest["arms"]:
            for design, n, k, t, section in (
                ("one_fe", 61440, 10, 4, "main"),
                ("two_fe_sparse", 983040, 50, 28, "width"),
                ("strong_d6", 3932160, 10, 28, "main"),
                ("weak_d3", 61440, 10, 1, "main"),
                ("three_fe_sparse", 983040, 10, 4, "main"),
            ):
                base = next(c for c in all_cells if c["model"] == model and c["design"] == design)
                selected.append(dict(base, n=n, k=k, threads=t, section=section))
        return {"schema": "systematic-plan-v1", "mode": mode,
                "bundles": [{"groups": [{"cell": c, "repetition": 1} for c in selected[i:i+2]]} for i in range(0, len(selected), 2)]}
    timing = defaultdict(list)
    peak = []
    measured_jobs = set()
    if results:
        frozen_path = results.parent / "env/frozen.json"
        identity = read_json(frozen_path)["fingerprint"] if frozen_path.exists() else None
        if not identity:
            raise ValueError("production packing requires frozen pilot receipts")
        for file in results.glob("**/result.json"):
            r = read_json(file)
            if r["status"] == "pass" and r.get("measurement_fingerprint") == identity:
                c = r["cell"]
                timing[c["model"], c["design"], r["arm"]].append((c["n"], c["k"], r["seconds"]))
                peak.append(r["peak_rss_gib"])
                measured_jobs.add(str(r["job_id"]))
    if not timing:
        raise ValueError("production packing requires measured pilot records")
    def estimate(c):
        total = 0
        for arm in manifest["arms"][c["model"]]:
            records = timing.get((c["model"], c["design"], arm))
            if not records:
                records = [v for (model, _, a), values in timing.items() if model == c["model"] and a == arm for v in values]
            if not records:
                raise ValueError(f"pilot has no usable runtime for {c['model']}/{arm}")
            estimates = [seconds * max(.1, c["n"] / n) ** 1.3 * max(1, c["k"] / k) for n, k, seconds in records]
            total += min(2400, max(estimates) * 1.5) + 60
        return total
    buckets = defaultdict(list)
    for c in all_cells:
        for rep in range(7):
            group = {"cell": c, "repetition": rep}
            output = results / cell_id(c) / f"{'cold' if rep == 0 else 'warm'}-{rep}" if results else None
            if output and (output / "GROUP.COMPLETE.json").exists():
                continue
            buckets[c["model"], c["design"], rep].append(group)
    bundles = []
    for key, groups in sorted(buckets.items()):
        # Put the complete core ladder together before width and size slices.
        groups.sort(key=lambda g: (g["cell"]["n"] != manifest["scaling_n"], g["cell"]["section"] != "main", g["cell"]["k"], g["cell"]["n"], g["cell"]["threads"]))
        pending, cost = [], 0
        for group in groups:
            predicted = estimate(group["cell"])
            if pending and cost + predicted > manifest["scc"]["target_bundle_seconds"]:
                bundles.append({"groups": pending, "predicted_seconds": cost})
                pending, cost = [], 0
            pending.append(group)
            cost += predicted
        if pending:
            bundles.append({"groups": pending, "predicted_seconds": cost})
    if results:
        for file in (results.parent / "accounting").glob("*.json"):
            receipt = read_json(file)
            if receipt["job_id"] not in measured_jobs:
                continue
            for record in receipt["records"]:
                value = record["maxvmem"]
                if value[-1] in "KMGT":
                    peak.append(float(value[:-1]) * {"K": 2**-20, "M": 2**-10, "G": 1, "T": 2**10}[value[-1]])
                else:
                    peak.append(float(value) / 2**30)
    needed_per_core = max(3, max(peak, default=0) * 1.25 / 28)
    memory = next((m for m in (3, 4, 6, 8, 12, 16, 18, 28) if m >= needed_per_core), None)
    if memory is None:
        raise ValueError("pilot exceeds allowed memory envelope")
    return {"schema": "systematic-plan-v1", "mode": mode, "mem_per_core_gib": memory,
            "bundles": bundles, "planned_groups": sum(len(b["groups"]) for b in bundles)}


def audit(root, manifest):
    from light import enabled, repetitions, skips, active_arms
    missing, failures, comparisons = [], [], defaultdict(int)
    integrity_errors = []
    frozen = read_json(root / "env/frozen.json") if (root / "env/frozen.json").exists() else {}
    accounting = {}
    for file in (root / "accounting").glob("*.json"):
        receipt = read_json(file)
        for task in receipt["records"]:
            accounting[receipt["job_id"], task["task_id"]] = task
    count = 0
    for c in cells(manifest):
        for rep in repetitions(c, manifest):
            path = root / "results" / cell_id(c) / f"{'cold' if rep == 0 else 'warm'}-{rep}" / "GROUP.COMPLETE.json"
            if not path.exists():
                missing.append({"cell": c, "repetition": rep})
                continue
            record = read_json(path)
            count += 1
            if record.get("measurement_fingerprint") != frozen.get("fingerprint") or not frozen.get("fingerprint"):
                integrity_errors.append({"cell": cell_id(c), "repetition": rep, "reason": "missing or inconsistent frozen runtime"})
            if record["cell"] != c or record["repetition"] != rep:
                integrity_errors.append({"cell": cell_id(c), "repetition": rep, "reason": "group identity mismatch"})
            for arm, status in record["arm_status"].items():
                arm_path = path.parent / arm / "result.json"
                if not arm_path.exists():
                    integrity_errors.append({"cell": cell_id(c), "repetition": rep, "reason": f"missing {arm} receipt"})
                    continue
                arm_record = read_json(arm_path)
                task_id = arm_record.get("task_id")
                task_id = int(task_id) if str(task_id).isdigit() else 1
                scheduler = accounting.get((str(arm_record.get("job_id")), task_id))
                if not scheduler or scheduler["failed"] != 0 or scheduler["exit_status"] != 0 or scheduler["slots"] != 28:
                    integrity_errors.append({"cell": cell_id(c), "repetition": rep, "reason": f"unaccepted scheduler accounting for {arm}"})
                if arm_record.get("measurement_fingerprint") != record.get("measurement_fingerprint") or arm_record["status"] != status:
                    integrity_errors.append({"cell": cell_id(c), "repetition": rep, "reason": f"inconsistent {arm} receipt"})
                if status != "pass":
                    failures.append({"cell": cell_id(c), "repetition": rep, "arm": arm, "status": status})
            for comparison in record["comparisons"]:
                comparisons[comparison["status"]] += 1
            expected = active_arms(dict(cell=c,repetition=rep),manifest) if enabled(manifest) else manifest['arms'][c['model']]
            if enabled(manifest) and record.get('skips') != skips(c,rep,manifest):
                integrity_errors.append({'cell':cell_id(c),'repetition':rep,'reason':'missing or altered exclusion receipt'})
            if set(record["arm_status"]) != set(expected):
                integrity_errors.append({"cell": cell_id(c), "repetition": rep, "reason": "unexpected arm set"})
    return {"complete_groups": count, "expected_groups": sum(len(repetitions(c,manifest)) for c in cells(manifest)),
            "missing": missing, "failures": failures, "comparison_status": dict(comparisons),
            "integrity_errors": integrity_errors,
            "status": "complete" if not missing and not integrity_errors else "incomplete"}


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("mode", choices=["smoke", "pilot", "production", "audit"])
    p.add_argument("run_root", type=Path)
    p.add_argument("output", type=Path)
    a = p.parse_args()
    manifest = manifest_for(a.run_root)
    value = audit(a.run_root, manifest) if a.mode == "audit" else build_plan(manifest, a.mode, a.run_root / "results")
    write_json(a.output, value)
    print({k: v for k, v in value.items() if k not in ("bundles", "missing", "failures")})
