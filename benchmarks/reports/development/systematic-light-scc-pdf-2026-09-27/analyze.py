"""Replay the illustrated report's additional comparisons without running fits."""
import csv
import json
from pathlib import Path
from statistics import median

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT.parent / "systematic-light-scc-final-2026-09-27" / "generated"


def read(name):
    with (SOURCE / name).open() as stream:
        return list(csv.DictReader(stream))


summary = read("summary.csv")
hashes = {row["path"]: row["sha256"] for row in read("captured-sha256.csv")}
capture = json.loads((ROOT / "captured-phases.json").read_text())
for row in capture["rows"]:
    assert hashes[row["path"]] == row["sha256"], row["path"]

key = lambda row: tuple(row[name] for name in ("model", "design", "n", "k", "threads", "section"))
default, strict = [
    {key(row): row for row in summary if row["model"] == "ppml"
     and row["comparator"] == arm and row["complete"] == "True"}
    for arm in ("fixest", "fixest_strict")
]
matched = sorted(default.keys() & strict.keys())
result = {
    "source": capture["source"],
    "captured_metadata_hashes_verified": len(capture["rows"]),
    "matched_ppml": {
        "configurations": len(matched),
        "median_native_default_paired_ratio": median(float(default[k]["ratio_median"]) for k in matched),
        "median_native_strict_paired_ratio": median(float(strict[k]["ratio_median"]) for k in matched),
        "median_strict_default_command_medians_ratio": median(
            float(strict[k]["comparator_median"]) / float(default[k]["comparator_median"])
            for k in matched),
    },
    "native_phase_summaries": [],
}
for design, n, threads in [("three_fe_sparse", 983040, 8), ("three_fe_sparse", 983040, 28),
                           ("strong_d6", 3932160, 4), ("strong_d6", 3932160, 28),
                           ("strong_d3", 3932160, 28), ("weak_d3", 3932160, 28)]:
    rows = [row for row in capture["rows"] if row["cell"]["model"] == "ppml"
            and row["cell"]["design"] == design and row["cell"]["n"] == n
            and row["cell"]["threads"] == threads and row["cell"]["section"] == "main"
            and row["phase"] == "warm" and row["status"] == "pass"]
    assert len(rows) == 4
    fields = ("fe_projection", "finite_solve", "strict_final_solve", "inference")
    result["native_phase_summaries"].append({
        "design": design, "n": n, "threads": threads,
        "median_command_seconds": median(row["seconds"] for row in rows),
        "median_phase_seconds": {field: median(row["metadata"]["feppml_time_" + field]
                                                for row in rows) for field in fields},
        "inner_iterations": [row["metadata"]["inner_iterations"] for row in rows],
        "outer_iterations": [row["metadata"]["ic"] for row in rows],
        "reported_final_linear_routes": [row["metadata"]["feppml_linear_route"] for row in rows],
        "input_hashes": [row["input_sha256"] for row in rows],
    })
(ROOT / "analysis.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
print(f"Verified {len(capture['rows'])} captured metadata hashes; {len(matched)} matched PPML configurations.")
