"""Summarize the frozen four-repeat report without changing pair eligibility."""
import csv
import json
import statistics
from collections import Counter, defaultdict
from pathlib import Path

root = Path(__file__).resolve().parent


def read(name):
    with (root / "generated" / name).open() as handle:
        return list(csv.DictReader(handle))


summary, pairs, runs = map(read, ("summary.csv", "paired-ratios.csv", "runs.csv"))
keys = ("model", "design", "n", "k", "threads", "section", "comparator")
by_pair = defaultdict(list)
for row in pairs:
    if row["phase"] == "warm":
        by_pair[tuple(row[k] for k in keys)].append(row)

comparators = []
for model, arm in sorted({(r["model"], r["comparator"]) for r in summary}):
    selected = [r for r in summary if r["model"] == model and r["comparator"] == arm]
    complete = [r for r in selected if r["complete"] == "True"]
    values = [float(r["ratio_median"]) for r in complete]
    rss = []
    for row in complete:
        repeated = by_pair[tuple(row[k] for k in keys)]
        assert len(repeated) == 4 and all(r["eligible"] == "True" for r in repeated)
        rss.append(statistics.median(float(r["rss_ratio"]) for r in repeated))
    comparators.append(dict(
        model=model, comparator=arm, registered_configurations=len(selected),
        complete_configurations=len(complete),
        median_ratio=statistics.median(values) if values else None,
        configuration_ratio_quartiles=statistics.quantiles(values, n=4, method="inclusive") if len(values) > 1 else None,
        configuration_ratio_range=[min(values), max(values)] if values else None,
        median_ten_percent_wins=sum(v <= .9 for v in values),
        median_wins=sum(v < 1 for v in values), median_losses=sum(v > 1 for v in values),
        every_repeat_ten_percent_wins=sum(float(r["ratio_max"]) <= .9 for r in complete),
        every_repeat_losses=sum(float(r["ratio_min"]) > 1 for r in complete),
        repetition_losses=sum(int(r["losses"]) for r in complete),
        median_rss_ratio=statistics.median(rss) if rss else None,
        rss_ratio_range=[min(rss), max(rss)] if rss else None))

failures = Counter((r["model"], r["design"], r["n"], r["k"], r["threads"],
                    r["section"], r["status"], r["timeout_bound"])
                   for r in runs if r["arm"] in ("fereg", "feppml") and r["status"] != "pass")
accounting = json.loads((root / "accounting/7747518.json").read_text())
wall = [r["wall_seconds"] for r in accounting["records"]]
result = dict(
    comparators=comparators,
    arm_status={arm: dict(Counter(r["status"] for r in runs if r["arm"] == arm))
                for arm in sorted({r["arm"] for r in runs})},
    native_failures=[dict(zip(("model", "design", "n", "k", "threads", "section", "status", "timeout_seconds"), key),
                          repetitions=count) for key, count in sorted(failures.items())],
    production_node_hours=sum(wall) / 3600, task_hours_range=[min(wall) / 3600, max(wall) / 3600],
    scheduler_maxvmem_values=sorted({r["maxvmem"] for r in accounting["records"]}))
(root / "analysis.json").write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n")
