"""Summarize every registered OLS slope pair, rejecting incomplete panels."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics


def read(path):
    return json.loads(path.read_text())


def distribution(values):
    return {"minimum": min(values), "median": statistics.median(values), "maximum": max(values)}


def summarize(root):
    plan, results, comparisons, complete = (read(root / name) for name in
        ("plan.json", "results.json", "comparisons.json", "complete.json"))
    expected_pairs = {(c["id"], seed, rep) for c in plan["cases"] for seed in plan["seeds"]
                      for rep in range(plan["rotations_per_seed"])}
    expected_fits = {(c["id"], seed, rep, arm) for c in plan["cases"] for seed in plan["seeds"]
                     for rep in range(plan["rotations_per_seed"]) for arm in c["arms"]}
    observed_fits = {(r["case"], r["seed"], r["repetition"], r["arm"]) for r in results}
    observed_pairs = {(r["case"], r["seed"], r["repetition"]) for r in comparisons}
    if (not complete["complete"] or complete["fits"] != len(expected_fits)
            or complete["pairs"] != len(expected_pairs) or observed_fits != expected_fits
            or observed_pairs != expected_pairs or len(results) != len(expected_fits)
            or len(comparisons) != len(expected_pairs)):
        raise ValueError("incomplete or duplicated registered slope panel")
    rows = []
    for case in plan["cases"]:
        pairs = [r for r in comparisons if r["case"] == case["id"]]
        row = dict(case=case, pairs=len(pairs), arms={}, comparisons={})
        for arm in case["arms"]:
            records = [r["result"] for r in results if r["case"] == case["id"] and r["arm"] == arm]
            success = [r for r in records if r["process_rc"] == 0 and r["sentinel"] and not r["timed_out"]]
            row["arms"][arm] = {"successful": len(success), "registered": len(records)}
            if success:
                row["arms"][arm].update(command_seconds=distribution([r["metadata"]["seconds"] for r in success]),
                                         process_seconds=distribution([r["process_seconds"] for r in success]))
                peaks = [r["process_peak_rss_bytes"] for r in success if r["process_peak_rss_bytes"] is not None]
                row["arms"][arm]["process_peak_rss_bytes"] = distribution(peaks) if len(peaks) == len(records) else None
            if arm == "candidate":
                continue
            checks = [p["comparisons"][arm] for p in pairs]
            eligible = [c for c in checks if c["ranked"] and all(c["checks"].values()) and c["ratio"] is not None]
            row["comparisons"][arm] = {"eligible": len(eligible), "registered": len(checks),
                "ratios": distribution([c["ratio"] for c in eligible]) if len(eligible) == len(checks) else None,
                "failed_checks": sorted({k for c in checks for k, v in c["checks"].items() if not v})}
        rows.append(row)
    return {"fits": len(results), "pairs": len(comparisons), "cases": rows,
            "source_receipts": {name: hashlib.sha256((root / name).read_bytes()).hexdigest()
                                for name in ("identity.json", "plan.json", "results.json", "comparisons.json", "complete.json")}}


def render(summary):
    text = ["# Fresh OLS slope confirmation", "", f"{summary['fits']} fits, {summary['pairs']} paired repetitions.", "",
            "Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].",
            "NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.", "",
            "| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |",
            "|---|---:|---:|---:|---:|---:|---:|"]
    def ratio(row, arm):
        entry = row["comparisons"].get(arm)
        if entry is None:
            return "not rerun"
        d = entry["ratios"]
        return f"{d['median']:.3f} [{d['minimum']:.3f}, {d['maximum']:.3f}]" if d else f"NR ({entry['eligible']}/{entry['registered']} eligible)"
    for row in summary["cases"]:
        arms = row["arms"]
        times = [f"{arms[a]['command_seconds']['median'] * 1000:.1f}" for a in ("baseline", "candidate")]
        memory = [f"{arms[a]['process_peak_rss_bytes']['median'] / 2**20:.1f}" if arms[a]["process_peak_rss_bytes"] else "missing"
                  for a in ("baseline", "candidate")]
        text.append(f"| {row['case']['id']} | {' | '.join(times)} | {ratio(row, 'baseline')} | {ratio(row, 'default')} | {ratio(row, 'strict')} | {' / '.join(memory)} |")
    text += ["", "Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.",
             "Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.", ""]
    for row in summary["cases"]:
        for arm, comparison in row["comparisons"].items():
            if comparison["failed_checks"]:
                text.append(f"- {row['case']['id']} / {arm}: " + ", ".join(comparison["failed_checks"]) + ".")
    return "\n".join(text) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    summary = summarize(args.root)
    args.output.mkdir(exist_ok=False)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2, allow_nan=False) + "\n")
    (args.output / "README.md").write_text(render(summary))
