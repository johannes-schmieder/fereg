"""Replay frozen comparisons and bind captured evidence; never run estimators."""
import csv
import json
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path


root, output = map(Path, sys.argv[1:])
sys.path.insert(0, str(root / "controller"))
from common import cell_id, manifest_for, read_json, sha256, write_json
from light import groups
from provenance import verify
from runner import compare

assert not (output / "captured-audit.json").exists()
identity = verify(root)
manifest = manifest_for(root)
plan_path = root / "plans/production-adjudicated.json"
assert sha256(plan_path) == "80e2439f6d134f9c9e1afd2d701910d6b38886d148e3e32d9783463396f84702"
plan = read_json(plan_path)
accounting = read_json(root / "accounting/7747518.json")
assert accounting["status"] == "pass" and len(accounting["records"]) == 16
assert {r["task_id"] for r in accounting["records"]} == set(range(1, 17))
files = {plan_path, root / "accounting/7747518.json", root / "env/frozen.json",
         root / "env/PILOT.ADJUDICATED.json", root / "env/PILOT.REJECTED.json",
         root / "env/SMOKE.ACCEPTED.json", root / "input/manifest.json"}
assert not (root / "env/PILOT.ACCEPTED.json").exists()
for task, expected in enumerate(plan["bundles"], 1):
    path = root / f"bundle-results/production-adjudicated-{task}.json"
    bundle = read_json(path)
    assert bundle["status"] == "complete" and not bundle["pending"]
    assert bundle["completed"] == expected["groups"]
    assert bundle["plan_sha256"] == sha256(plan_path)
    log = root / f"logs/production.{task}.log"
    text = log.read_text()
    assert "SYSTEMATIC_BUNDLE_COMPLETE" in text
    assert "Traceback (most recent call last)" not in text
    files.update((path, log))

statuses, native, counts = Counter(), Counter(), Counter()
native_failures = []
for group in groups(manifest):
    cell, rep = group["cell"], group["repetition"]
    directory = root / "results" / cell_id(cell) / f"{'cold' if rep == 0 else 'warm'}-{rep}"
    record = read_json(directory / "GROUP.COMPLETE.json")
    source = read_json(directory / "input.json")
    candidate = manifest["arms"][cell["model"]][0]
    assert record["measurement_fingerprint"] == identity
    for comparison in record["comparisons"]:
        replay = compare(directory / candidate, directory / comparison["right"], manifest["science"])
        assert replay == comparison, (cell_id(cell), rep, comparison["right"])
        counts["replayed_comparisons"] += 1
    for arm, status in record["arm_status"].items():
        receipt = read_json(directory / arm / "result.json")
        assert receipt["input_sha256"] == source["sha256"]
        assert receipt["measurement_fingerprint"] == identity
        statuses[status] += 1
        if status == "pass":
            assert receipt["sentinel"] and receipt["rss_samples"] > 0
            assert (directory / arm / "DONE").exists()
        if arm == candidate:
            native[status] += 1
            if status != "pass":
                native_failures.append(dict(cell=cell_id(cell), repetition=rep, status=status,
                                            rc=receipt["metadata"].get("rc"),
                                            timeout_bound=receipt.get("command_time_lower_bound")))
    counts["groups"] += 1
    files.update(directory.glob("*.json"))
    for arm in record["arm_status"]:
        files.update(p for name in ("result.json", "metadata.json", "coefficients.csv", "covariance.csv")
                     if (p := directory / arm / name).exists())

with (output / "captured-sha256.csv").open("w", newline="") as handle:
    writer = csv.writer(handle)
    writer.writerow(("path", "bytes", "sha256"))
    for path in sorted(files):
        writer.writerow((str(path.relative_to(root)), path.stat().st_size, sha256(path)))
receipt = dict(schema="systematic-captured-audit-v1", status="pass",
               captured_at=datetime.now(timezone.utc).isoformat(), measurement_fingerprint=identity,
               counts=dict(counts), arm_status=dict(statuses), native_status=dict(native),
               native_failures=native_failures, terminal_bundles=16, pending_groups=0,
               inventory_sha256=sha256(output / "captured-sha256.csv"))
write_json(output / "captured-audit.json", receipt)
print(json.dumps({k: v for k, v in receipt.items() if k != "native_failures"}))
