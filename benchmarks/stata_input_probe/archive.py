"""Preserve numeric receipts and identities without raw licensed logs or local paths."""
import argparse
import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import statistics

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def main(root, output):
    output.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location("profile_run", REPO / "benchmarks/one_fe_profile/run.py")
    profile = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(profile)
    rows = []
    science = []
    folders = sorted((root / "runs").glob("*/result.json"))
    assert len(folders) == 36
    for path in folders:
        result = json.loads(path.read_text())
        name = path.parent.name
        variant = name.split("-")[-1]
        assert result["status"] == "pass" and result["process_returncode"] == 0 and result["sentinel"]
        baseline = path.parent.with_name(name.rsplit("-", 1)[0] + "-baseline")
        exact = profile.compare(baseline, path.parent)
        with (path.parent / "fereg_runtime.csv").open() as f:
            rt = {int(x["row"]): float(x["value"]) for x in csv.DictReader(f) if x["column"] == "2"}
        case = name.rsplit("-r", 1)[0]
        row = dict(run=name, case=case, variant=variant, repetition=int(name.rsplit("-r", 1)[1].split("-")[0]),
                   seconds=result["seconds"], input=rt[19], native=rt[20], remaining=result["seconds"]-rt[21],
                   preflight=rt[18], native_allocation_peak_bytes=rt[13], input_sha256=result["input_sha256"],
                   sample_sha256=result["sample_sha256"], artifact_sha256=result["measurement_fingerprint"]["plugin"])
        rows.append(row)
        science.append(dict(run=name, exact=exact, output_sha256={p.name: sha(p) for p in path.parent.glob("*.csv")
                                                               if p.name != "fereg_runtime.csv"}))
    summary = []
    for case in sorted({r["case"] for r in rows}):
        values = [r for r in rows if r["case"] == case]
        variants = {}
        baselines = {r["repetition"]: r for r in values if r["variant"] == "baseline"}
        for variant in sorted({r["variant"] for r in values}):
            current = [r for r in values if r["variant"] == variant]
            variants[variant] = {field: statistics.median(r[field] for r in current)
                                 for field in ("seconds", "input", "native", "remaining", "native_allocation_peak_bytes")}
            variants[variant].update(n=len(current), minimum=min(r["seconds"] for r in current), maximum=max(r["seconds"] for r in current),
                                     paired_command_ratios=[r["seconds"]/baselines[r["repetition"]]["seconds"] for r in current],
                                     paired_input_ratios=[r["input"]/baselines[r["repetition"]]["input"] for r in current])
        summary.append(dict(case=case, variants=variants))
    kernels = []
    for k in (10, 50):
        source = root / f"kernel-k{k}.tsv"
        with source.open() as f:
            raw = list(csv.DictReader(f, delimiter="\t"))
        assert len(raw) == 55 and len({r["hash"] for r in raw}) == 1
        for mode in range(5):
            samples = [float(r["seconds"]) for r in raw if int(r["mode"]) == mode and int(r["repeat"]) > 0]
            assert len(samples) == 10
            kernels.append(dict(k=k, mode=mode, seconds=statistics.median(samples), minimum=min(samples), maximum=max(samples), samples=samples))
        shutil.copyfile(source, output / source.name)
    for variant in ("cached", "fast"):
        assert (root / variant / "guard-ubsan.log").read_text().strip() == "INPUT_GUARD_PASS"
        shutil.copyfile(root / (variant + ".patch"), output / (variant + ".patch"))
    write(output / "measurements.json", rows)
    write(output / "summary.json", summary)
    write(output / "kernel-summary.json", kernels)
    write(output / "science.json", dict(accepted_command_calls=36, checks=science, independent_oracles=json.loads((root / "smoke.json").read_text()),
                                       licensed_fixtures=json.loads((root / "fixtures-fast.json").read_text()),
                                       guard_ubsan=dict(cached="pass", fast="pass"),
                                       accessor_semantics=[(root / name).read_text() for name in ("semantic.tsv", "semantic-reordered.tsv")]))
    identities = json.loads((root / "build-identities.json").read_text())
    identities["probe_plugin_sha256"] = sha(root / "probe.plugin")
    identities["probe_source_sha256"] = sha(root / "probe.c")
    identities["controller"] = {p.name: sha(p) for p in (root / "controller").iterdir() if p.is_file()}
    identities["harness"] = {p.name: sha(p) for p in (REPO / "benchmarks/stata_input_probe").iterdir() if p.is_file()}
    identities["limits"] = ["Apple Silicon Stata/MP 19 only; arm64 diagnostic relinks, no cross-platform claim.",
                            "ASan runtime deadlocked before main; terminated, no ASan pass claimed.",
                            "Kernel v2 overlapped the stalled sanitizer process and is excluded; final kernel run was isolated.",
                            "First command launch was blocked by sandbox process enumeration before any fit; permitted retry succeeded."]
    write(output / "identities.json", identities)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("output", type=Path)
    a = p.parse_args()
    main(a.root, a.output)
