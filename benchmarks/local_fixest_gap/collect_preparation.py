"""Collect portable numeric evidence; exclude licensed logs and local paths."""
import argparse
import csv
import json
import statistics
import zipfile
from pathlib import Path

from common import read_json, sha256, write_json


def runtime(path):
    with path.open() as stream:
        rows = list(csv.DictReader(stream))
    values = {}
    for row in rows:
        if int(row["column"]) == 2:
            values[int(row["row"])] = float(row["value"])
    return values


def collect(root, output, *, pair_prefix="prep", science_marker="exact-science.json",
            exact_science=True, note=None):
    output.mkdir(parents=True, exist_ok=True)
    pairs, groups = [], {}
    accepted = set()
    for candidate in sorted((root / "runs").glob(f"{pair_prefix}-*-candidate")):
        if not (candidate / science_marker).exists():
            continue
        baseline = candidate.with_name(candidate.name.replace("-candidate", "-baseline"))
        record = {"pair": candidate.name.removesuffix("-candidate")}
        for arm, path in [("baseline", baseline), ("candidate", candidate)]:
            result = read_json(path / "result.json")
            assert result["status"] == "pass" and result["sentinel"]
            assert result["metadata"]["rc"] == 0
            native = runtime(path / "fereg_runtime.csv")
            record[arm] = {
                "command_seconds": result["seconds"],
                "input_seconds": native[19], "native_seconds": native[20],
                "outer_seconds": result["seconds"] - native[21],
                "native_peak_bytes": int(native[13]), "modeled_bytes": int(native[25]),
                "scratch_bytes": int(native[9]), "rss_gib": result["peak_rss_gib"],
                "plugin_sha256": result["measurement_fingerprint"]["plugin_sha256"],
            }
            accepted.add(path)
        record["case"] = result["cell"]["name"]
        record["group"] = "profile" if "-profile-" in record["pair"] else (
            "recheck" if "-recheck-" in record["pair"] else (
                "primary" if "-paired-" in record["pair"] else "control"))
        record["candidate_baseline_ratio"] = record["candidate"]["command_seconds"] / record["baseline"]["command_seconds"]
        record["exact_science"] = exact_science
        if not exact_science:
            record["science_comparison"] = read_json(candidate / science_marker)
        groups.setdefault((record["group"], record["case"]), []).append(record)
        pairs.append(record)
    summary = []
    for (group, case), records in sorted(groups.items()):
        row = {"group": group, "case": case, "pairs": len(records)}
        for arm in ("baseline", "candidate"):
            for field in records[0][arm]:
                if field != "plugin_sha256":
                    row[f"{arm}_{field}"] = statistics.median(record[arm][field] for record in records)
        row["median_paired_change_percent"] = 100 * (statistics.median(record["candidate_baseline_ratio"] for record in records) - 1)
        summary.append(row)
    write_json(output / "comparisons.json", {"pairs": pairs, "summary": summary,
        "note": note or "Primary medians retain the input-loading outlier; rechecks are separate. RSS uses process-tree sampling; modeled/native peaks use the result wire."})
    with (output / "summary.csv").open("w") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(summary[0]))
        writer.writeheader()
        writer.writerows(summary)
    with zipfile.ZipFile(output / "numeric-evidence.zip", "w", zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(accepted):
            for file in sorted(path.iterdir()):
                if file.suffix == ".csv" or file.name == science_marker or file.name in {
                    "result.json", "native-science.json", "exact-science.json", "baseline-comparison.json",
                    "import-hashes.json", "phases.json",
                }:
                    archive.write(file, f"runs/{path.name}/{file.name}")
        for path in sorted((root / "preparation-data").glob("*.json")):
            archive.write(path, f"inputs/{path.name}")
        for path in sorted((root / "weighted-data").glob("*.json")):
            archive.write(path, f"weighted-inputs/{path.name}")
    write_json(output / "archive.json", {"sha256": sha256(output / "numeric-evidence.zip"),
        "completed_pairs": len(pairs), "successful_measured_calls": 2 * len(pairs),
        "all_pairs_exact_science": exact_science,
        "contents": "Synthetic-input identities, complete numeric b/V/diagnostic exports, sample hashes, runtime and RSS receipts, saved-output values where requested, and phase traces. No raw licensed logs, binaries, request paths or private data."})
    for row in summary:
        print(row["group"], row["case"], row["baseline_command_seconds"], row["candidate_command_seconds"],
              round(row["median_paired_change_percent"], 1),
              row["baseline_native_peak_bytes"], row["candidate_native_peak_bytes"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    collect(args.root, args.output)
