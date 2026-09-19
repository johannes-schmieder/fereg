"""Fresh-process command checks for the isolated Stata accessor experiment."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(root, previous, part):
    root, previous = root.resolve(), previous.resolve()
    profile = load("one_fe_profile", REPO / "benchmarks/one_fe_profile/run.py")
    folder = profile.stage(root) if not (root / "controller").exists() else root / "controller"
    sys.path.insert(0, str(folder))
    measure = load("input_measure", folder / "measure.py").measure
    (root / "work/refs").mkdir(parents=True, exist_ok=True)
    manifest = json.loads((REPO / "benchmarks/systematic/manifest.json").read_text())
    manifest["measurement"].update(command_timeout_seconds=120, rss_interval_seconds=.05)
    identity = json.loads((root / "build-identities.json").read_text())
    checks = []
    if part == "smoke":
        cells = [(360, 10, 4, False, 1, ("baseline", "cached", "fast"))]
    elif part == "primary":
        cells = [(983040, 50, 16, False, r, ("baseline", "cached", "fast")) for r in range(1, 6)]
    else:
        cells = [(983040, k, t, shuffled, r, ("baseline", "fast"))
                 for k, t, shuffled in [(10, 16, False), (50, 1, False), (50, 16, True)] for r in range(1, 4)]
    for n, k, threads, shuffled, rep, variants in cells:
        tag = f"n{n}-k{k}-gdefault-shuffle{int(shuffled)}"
        data = previous / "data" / (tag + ".dta")
        warm = previous / "data" / f"n7680-k{k}-gdefault-shuffle0.dta"
        cell = dict(model="ols", design="one_fe", n=n, k=k, threads=threads, effects=["g1"], section="width")
        rotate = (rep - 1) % len(variants)
        variants = variants[rotate:] + variants[:rotate]
        outputs = {}
        for variant in variants:
            output = root / "runs" / f"{part}-{tag}-t{threads}-r{rep}-{variant}"
            result = measure(root, cell, "fereg", "warm", data, warm, output, manifest,
                             identity=identity[variant], source=root / variant)
            print(output.name, result["status"], result["seconds"], flush=True)
            assert result["status"] == "pass", output
            outputs[variant] = output
            if part == "smoke":
                checks.append(dict(run=output.name, oracle=profile.oracle(data, output, k)))
        for variant in variants:
            checks.append(dict(run=outputs[variant].name, exact=profile.compare(outputs["baseline"], outputs[variant])))
    (root / (part + ".json")).write_text(json.dumps(dict(status="pass", checks=checks), indent=2) + "\n")
    print("INPUT_COMMAND_COMPLETE", part, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("previous", type=Path)
    p.add_argument("--part", choices=("smoke", "primary", "holdout"), required=True)
    a = p.parse_args()
    main(a.root, a.previous, a.part)
