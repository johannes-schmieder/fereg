"""Collect exclusive command costs and explicitly nested diagnostic phases."""
import argparse
import csv
import json
from pathlib import Path
import re
import statistics


def collect(root, output):
    rows = []
    for path in sorted((root / "runs").glob("*/result.json")):
        if any(label in path.parent.name for label in ("-deep-", "-sample-", "-outer-", "-overhead-", "-publication-")):
            continue
        r = json.loads(path.read_text())
        if r["cell"]["n"] == 360:
            continue
        row = dict(run=path.parent.name, arm=r["arm"], n=r["cell"]["n"], k=r["cell"]["k"],
                   threads=r["cell"]["threads"], input_sha256=r["input_sha256"],
                   traced="-trace-" in path.parent.name, seconds=r["seconds"],
                   status=r["status"], rss_gib=r["peak_rss_gib"], rss_samples=r["rss_samples"],
                   sample_sha256=r.get("sample_sha256"), phases={}, nested={})
        row["repetition"] = int(re.search(r"-r(\d+)-", path.parent.name)[1])
        if r["status"] != "pass":
            rows.append(row)
            continue
        if r["arm"] == "fereg":
            with (path.parent / "fereg_runtime.csv").open() as stream:
                runtime = {int(x["row"]): float(x["value"]) for x in csv.DictReader(stream) if x["column"] == "2"}
            row["phases"] = dict(preflight=runtime[18], input=runtime[19], native=runtime[20],
                                 outer_and_publication=row["seconds"]-runtime[21])
            row["native_allocation_peak_bytes"] = runtime[13]
            row["native_effective_threads"] = runtime[29]
            trace = (path.parent / "stderr.txt").read_text()
            for line in trace.splitlines():
                match = re.match(r"FEREG_PHASE\|([^|]+)\|(.*)", line)
                if match:
                    row["nested"][match[1]] = {k: float(v) for k, v in
                        (field.split("=", 1) for field in match[2].split("|"))}
        elif (path.parent / "profile.json").exists():
            profile = json.loads((path.parent / "profile.json").read_text())
            row["phases"] = profile["phases"]
            row["phases"]["unattributed"] = row["seconds"]-sum(profile["phases"].values())
            row["nested"] = {k: v for k, v in profile.items() if k != "phases"}
        rows.append(row)
    groups = {}
    for row in rows:
        key = (row["n"], row["k"], row["threads"], row["input_sha256"])
        groups.setdefault(key, []).append(row)
    summaries = []
    for key, values in groups.items():
        summary = dict(n=key[0], k=key[1], threads=key[2], input_sha256=key[3], arms={})
        for arm in ("fereg", "fixest"):
            current = [r for r in values if r["arm"] == arm and r["status"] == "pass"]
            stock = [r for r in current if not r["traced"]]
            traced = [r for r in current if r["traced"]]
            info = dict(stock_n=len(stock), traced_n=len(traced))
            if stock:
                times = [r["seconds"] for r in stock]
                info.update(seconds=statistics.median(times), minimum=min(times), maximum=max(times))
            for name, subset in (("stock_phases", stock), ("trace_phases", traced)):
                if subset:
                    info[name] = {p: statistics.median(r["phases"][p] for r in subset)
                                  for p in subset[0]["phases"]}
            if traced:
                info["traced_seconds"] = statistics.median(r["seconds"] for r in traced)
                info["nested"] = {label: {p: statistics.median(r["nested"][label][p] for r in traced)
                                          for p in phases}
                                  for label, phases in traced[0]["nested"].items()}
            summary["arms"][arm] = info
        if all("seconds" in summary["arms"][arm] for arm in ("fereg", "fixest")):
            paired = {arm: {r["repetition"]: r["seconds"] for r in values
                           if r["arm"] == arm and not r["traced"] and r["status"] == "pass"}
                      for arm in ("fereg", "fixest")}
            repeats = paired["fereg"].keys() & paired["fixest"].keys()
            summary["paired_ratios"] = [paired["fereg"][r] / paired["fixest"][r] for r in sorted(repeats)]
            summary["fereg_over_fixest"] = statistics.median(summary["paired_ratios"])
        summaries.append(summary)
    output.mkdir(parents=True, exist_ok=True)
    (output / "measurements.json").write_text(json.dumps(rows, indent=2) + "\n")
    (output / "summary.json").write_text(json.dumps(summaries, indent=2) + "\n")
    for s in summaries:
        print(s["n"], s["k"], s["threads"],
              *(f"{a}={s['arms'][a].get('seconds', 0):.3f}" for a in ("fereg", "fixest")),
              f"ratio={s.get('fereg_over_fixest', 0):.2f}")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("output", type=Path)
    args = p.parse_args()
    collect(args.root, args.output)
