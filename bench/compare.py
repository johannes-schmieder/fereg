#!/usr/bin/env python3
"""Compact summaries; missing/failed runs stay visible and cannot pass accuracy."""
from __future__ import annotations
import argparse
import csv
from datetime import date
import io
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[1]


def coefficients(directory):
    with (Path(directory) / "coefficients.csv").open() as stream:
        return {row["name"]: float(row["value"]) for row in csv.DictReader(stream)}


def sample(directory):
    with (Path(directory) / "sample.csv").open() as stream:
        return tuple(int(row["obs_id"]) for row in csv.DictReader(stream))


def accuracy(record, reference, policy):
    if record["status"] != "ok" or reference is None or reference["status"] != "ok":
        return "unavailable", None
    try:
        actual, expected = coefficients(record["directory"]), coefficients(reference["directory"])
        if set(actual) != set(expected) or sample(record["directory"]) != sample(reference["directory"]):
            return "disagreement", None
        ratios = [abs(actual[key]-value)/(policy["coefficient_atol"]+policy["coefficient_rtol"]*abs(value))
                  for key, value in expected.items()]
        if any(not math.isfinite(actual[key]) or not math.isfinite(value) for key, value in expected.items()):
            return "disagreement", None
        maximum = max(ratios, default=0.0)
        return ("pass" if maximum <= 1 else "disagreement"), maximum
    except (OSError, ValueError, KeyError):
        return "unavailable", None


def median_metadata(records, name):
    values = []
    for record in records:
        try:
            value = float(record["metadata"][name])
        except (KeyError, ValueError):
            return ""
        if not math.isfinite(value) or value < 0:
            return ""
        values.append(value)
    return statistics.median(values) if values else ""


def summarize(path, destination=None):
    campaign = json.loads(Path(path).read_text())
    refs = {record["cell"]: record for record in campaign["records"] if record["reference"]}
    groups = {tuple(group): [] for group in campaign["expected_groups"]}
    for record in campaign["records"]:
        if not record["reference"]:
            groups.setdefault((record["cell"], record["threads"], record["arm"]), []).append(record)
    rows = []
    for (cell, threads, arm), records in groups.items():
        measured = [record for record in records if record["repetition"] != "warmup"]
        statuses = [record["status"] for record in records]
        complete = (len(measured)==campaign["protocol"]["repetitions"]
                    and {r["repetition"] for r in measured}==set(range(1,campaign["protocol"]["repetitions"]+1))
                    and sum(r["repetition"]=="warmup" for r in records)==1
                    and all(s=="ok" for s in statuses))
        status = "ok" if complete else ("timeout" if "timeout" in statuses else "error")
        checks = [accuracy(record, refs.get(cell), campaign["protocol"]) for record in measured]
        verdict = "pass" if complete and all(check=="pass" for check,_ in checks) else "unavailable"
        if any(check=="disagreement" for check,_ in checks):
            verdict = "disagreement"
        seconds = median_metadata(measured,"seconds") if complete else ""
        rows.append({"date": date.today().isoformat(), "source_sha": campaign["source_sha"],
                     "dirty": campaign["source_dirty"], "platform": campaign["platform"], "tier": campaign["tier"],
                     "partial": campaign["partial"], "comparison": campaign.get("comparison", "default"), "engine": campaign["engine"], "solver": campaign["solver"],
                     "cell": cell, "threads": threads, "arm": arm, "status": status, "accuracy": verdict,
                     "repetitions": len(measured), "seconds": seconds,
                     "input_seconds": median_metadata(measured,"input_seconds") if complete else "",
                     "native_seconds": median_metadata(measured,"native_seconds") if complete else "",
                     "total_seconds": median_metadata(measured,"total_seconds") if complete else "",
                     "max_scaled_error": max((v for _,v in checks if v is not None), default=""),
                     "ratio_fereg": ""})
    for row in rows:
        candidate = next((r for r in rows if r["cell"]==row["cell"] and r["threads"]==row["threads"] and r["arm"]=="fereg"), None)
        if candidate and row["status"]==candidate["status"]=="ok" and row["accuracy"]==candidate["accuracy"]=="pass" and row["seconds"]>0:
            row["ratio_fereg"] = candidate["seconds"]/row["seconds"]
    if not rows:
        raise ValueError("campaign contains no measured command groups")
    buffer = io.StringIO()
    writer = csv.DictWriter(buffer, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    if len(buffer.getvalue().encode())>100_000:
        raise ValueError("summary exceeds the 100 KB contract; split the campaign")
    if destination is None:
        suffix = "-dirty" if campaign["source_dirty"] else ""
        partial = "-partial" if campaign["partial"] else ""
        if campaign.get("comparison") == "solver":
            partial += "-solvers-" + campaign["solver"]
        destination = ROOT / "bench/history" / f"{date.today()}-{campaign['source_sha'][:8]}{suffix}-{campaign['platform']}-{campaign['tier']}{partial}.csv"
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        raise ValueError("summary already exists; do not overwrite previous evidence")
    destination.write_text(buffer.getvalue())
    print("cell threads arm seconds fereg/reference status accuracy")
    for row in rows:
        print(row["cell"], row["threads"], row["arm"], row["seconds"], row["ratio_fereg"], row["status"], row["accuracy"])
    for arm in sorted({row["arm"] for row in rows}-{ "fereg" }):
        all_pairs = [row for row in rows if row["arm"]==arm]
        passing = [row for row in all_pairs if row["accuracy"]=="pass" and isinstance(row["ratio_fereg"],float) and row["ratio_fereg"]>0]
        values = [row["ratio_fereg"] for row in passing]
        gm = math.exp(statistics.fmean(math.log(v) for v in values)) if values else None
        worst = max(passing, key=lambda row: row["ratio_fereg"], default=None)
        print(f"{arm}: geometric mean={gm}; passing pairs={len(passing)}/{len(all_pairs)}; "
              f"worst={worst['cell'] if worst else 'unavailable'}; timeouts={sum(r['status']=='timeout' for r in all_pairs)}")
    print(f"wall_seconds={campaign.get('wall_seconds')}; budget={campaign.get('wall_budget_seconds')}; summary={destination}")
    return rows


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("results", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    summarize(args.results, args.output)
