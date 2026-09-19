"""Bounded paired Stata measurements of one-FE pass prototypes."""
import argparse
import hashlib
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


def main(root, previous, part, variants, label=""):
    root, previous = root.resolve(), previous.resolve()
    profile = load("one_fe_profile", REPO / "benchmarks/one_fe_profile/run.py")
    folder = profile.stage(root) if not (root / "controller").exists() else root / "controller"
    sys.path.insert(0, str(folder))
    measure = load("pass_measure", folder / "measure.py").measure
    (root / "work/refs").mkdir(parents=True, exist_ok=True)
    manifest = json.loads((REPO / "benchmarks/systematic/manifest.json").read_text())
    manifest["measurement"].update(command_timeout_seconds=120, rss_interval_seconds=.05)
    if part == "smoke":
        cells = [(360, 10, 4, False, 1)]
    elif part == "screen":
        cells = [(983040, 50, 16, False, r) for r in range(1, 4)]
    elif part == "confirm":
        cells = [(983040, 50, 16, False, r) for r in range(1, 6)]
    elif part == "trace":
        cells = [(983040, 50, 16, False, 1)]
    elif part == "pipeline-controls":
        cells = [(983040, k, 16, shuffled, r)
                 for k, shuffled in [(10, False), (50, True)] for r in range(1, 4)]
    else:
        cells = [(983040, k, t, shuffled, r)
                 for k, t, shuffled in [(10, 16, False), (50, 1, False), (50, 16, True)] for r in range(1, 4)]
    checks = []
    for n, k, threads, shuffled, rep in cells:
        tag = f"n{n}-k{k}-gdefault-shuffle{int(shuffled)}"
        data = previous / "data" / (tag + ".dta")
        warm = previous / "data" / f"n7680-k{k}-gdefault-shuffle0.dta"
        cell = dict(model="ols", design="one_fe", n=n, k=k, threads=threads, effects=["g1"], section="width")
        rotate = (rep - 1) % len(variants)
        order = variants[rotate:] + variants[:rotate]
        outputs = {}
        for variant in order:
            output = root / "runs" / f"{label}{part}-{tag}-t{threads}-r{rep}-{variant}"
            if variant == "fixest":
                identity = dict(fixest_archive_sha256=hashlib.sha256((previous / "fixest.tar.gz").read_bytes()).hexdigest())
                arm, source = "fixest", None
            else:
                plugin = root / variant / "dist/fereg_macos.plugin"
                identity = dict(plugin_sha256=hashlib.sha256(plugin.read_bytes()).hexdigest(), base_commit="55c19cb679d675194c6baed74b09279e1f026b16", variant=variant)
                arm, source = "fereg", root / variant
            result = measure(root, cell, arm, "warm", data, warm, output, manifest,
                             identity=identity, source=source, trace=part == "trace")
            print(output.name, result["status"], result["seconds"], flush=True)
            assert result["status"] == "pass", output
            outputs[variant] = output
            if part == "smoke":
                checks.append(dict(run=output.name, oracle=profile.oracle(data, output, k)))
        for variant in order:
            checks.append(dict(run=outputs[variant].name, exact=profile.compare(outputs["baseline"], outputs[variant])))
    (root / (label + part + ".json")).write_text(json.dumps(dict(status="pass", checks=checks), indent=2) + "\n")
    print("ONE_FE_PASSES_COMPLETE", part, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("previous", type=Path)
    p.add_argument("--part", choices=("smoke", "screen", "confirm", "trace", "holdout", "pipeline-controls"), required=True)
    p.add_argument("--variants", nargs="+", default=["baseline", "direct", "overlap", "combined"])
    p.add_argument("--label", default="")
    a = p.parse_args()
    main(a.root, a.previous, a.part, a.variants, a.label)
