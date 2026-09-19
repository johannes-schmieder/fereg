"""Archive path-free numerical receipts for one completed optimization step."""
import argparse
import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(root, output, label, baseline, candidate):
    root, output = root.resolve(), output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location("profile_run", REPO / "benchmarks/one_fe_profile/run.py")
    profile = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(profile)
    rows, science = [], []
    for path in sorted((root / "runs").glob(label + "-*/result.json")):
        r = json.loads(path.read_text())
        name = path.parent.name
        variant = name.rsplit("-", 1)[1]
        assert r["status"] == "pass" and r["process_returncode"] == 0 and r["sentinel"]
        reference = path.parent.with_name(name.rsplit("-", 1)[0] + "-" + baseline)
        comparison = profile.compare(reference, path.parent)
        row = dict(run=name, case=name.rsplit("-r", 1)[0], variant=variant,
                   repetition=int(name.rsplit("-r", 1)[1].split("-")[0]),
                   seconds=r["seconds"], input_sha256=r["input_sha256"], sample_sha256=r["sample_sha256"])
        if r["arm"] == "fereg":
            with (path.parent / "fereg_runtime.csv").open() as f:
                runtime = {int(v["row"]): float(v["value"]) for v in csv.DictReader(f) if v["column"] == "2"}
            row.update(input=runtime[19], native=runtime[20], remaining=r["seconds"]-runtime[21],
                       native_allocation_peak_bytes=runtime[13], plugin=r["measurement_fingerprint"]["plugin"])
        rows.append(row)
        science.append(dict(run=name, comparison=comparison, output_sha256={p.name: sha(p) for p in path.parent.glob("*.csv")}))
    assert rows
    summary = []
    for case in sorted({r["case"] for r in rows}):
        values = [r for r in rows if r["case"] == case]
        paired = {r["repetition"]: r for r in values if r["variant"] == baseline}
        variants = {}
        for variant in sorted({r["variant"] for r in values}):
            current = [r for r in values if r["variant"] == variant]
            variants[variant] = {field: statistics.median(r[field] for r in current)
                                 for field in ("seconds", "input", "native", "remaining", "native_allocation_peak_bytes")
                                 if field in current[0]}
            variants[variant].update(n=len(current), paired_ratios=[r["seconds"]/paired[r["repetition"]]["seconds"] for r in current])
        summary.append(dict(case=case, variants=variants))
    identities = {name: json.loads((root / ("identity-" + name + ".json")).read_text()) for name in (baseline, candidate)}
    identities["controller"] = {p.name: sha(p) for p in (root / "controller").iterdir() if p.is_file()}
    identities["harness"] = {p.name: sha(p) for p in Path(__file__).parent.glob("*.py")}
    fixtures = json.loads((root / ("fixtures-" + candidate + ".json")).read_text())
    for name, value in [("measurements", rows), ("summary", summary), ("identities", identities),
                        ("science", dict(accepted_calls=len(rows), checks=science, licensed_fixtures=fixtures))]:
        (output / (name + ".json")).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    print("ARCHIVE_PASS", label, len(rows))


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("label")
    p.add_argument("baseline")
    p.add_argument("candidate")
    a = p.parse_args()
    main(a.root, a.output, a.label, a.baseline, a.candidate)
